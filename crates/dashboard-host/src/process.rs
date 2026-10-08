use crate::command::{CommandRunner, CommandSpec};
use dashboard_core::{FoundProcess, Identity};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Debug)]
pub struct Process {
    pub pid: u32,
    pub ppid: u32,
    pub started: String,
    pub command: String,
}

pub struct Inventory {
    pub processes: Vec<Process>,
    pub found: Vec<FoundProcess>,
    pub epochs: BTreeMap<String, String>,
}

pub fn env_value(command: &str, key: &str) -> Option<String> {
    let needle = format!(" {key}=");
    let start = command.find(&needle)? + needle.len();
    let rest = &command[start..];
    let end = rest
        .char_indices()
        .find_map(|(index, c)| {
            if c != ' ' {
                return None;
            }
            let next = &rest[index + 1..];
            let equal = next.find('=')?;
            let name = &next[..equal];
            (!name.is_empty()
                && name.chars().enumerate().all(|(i, c)| {
                    c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())
                }))
            .then_some(index)
        })
        .unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

fn parse_line(line: &str) -> Result<Process, String> {
    let mut fields = line.split_whitespace();
    let pid = fields
        .next()
        .ok_or("missing PID")?
        .parse()
        .map_err(|_| "invalid PID")?;
    let ppid = fields
        .next()
        .ok_or("missing PPID")?
        .parse()
        .map_err(|_| "invalid PPID")?;
    let mut started = Vec::new();
    for _ in 0..5 {
        started.push(fields.next().ok_or("incomplete process timestamp")?);
    }
    // Preserve spaces in both argv and environment values.
    let command_start = fields.next().ok_or("missing process command")?;
    let offset = command_start.as_ptr() as usize - line.as_ptr() as usize;
    Ok(Process {
        pid,
        ppid,
        started: started.join(" "),
        command: line[offset..].into(),
    })
}

fn codex_app_server<'a>(mut args: impl Iterator<Item = &'a str>) -> bool {
    while let Some(arg) = args.next() {
        match arg {
            "--" => return false,
            "-c" | "--config" | "--enable" | "--disable" | "-m" | "--model" | "-p"
            | "--profile" | "-C" | "--cd" | "-a" | "--ask-for-approval" | "-s" | "--sandbox"
            | "-i" | "--image" => {
                args.next();
            }
            flag if flag.starts_with('-') => {}
            command => return command == "app-server",
        }
    }
    false
}

fn tool(command: &str) -> Option<&'static str> {
    let mut args = command.split_whitespace();
    let first = args.next()?;
    let name = Path::new(first).file_name()?.to_str()?;
    match name {
        "claude" => Some("claude"),
        "codex" => (!codex_app_server(args)).then_some("codex"),
        "agent" => Some("cursor"),
        "agy" => Some("gemini"),
        "hermes" => Some("hermes"),
        "pi" => Some("pi"),
        "gemini" => Some("gemini"),
        "node" => {
            let script = args.next()?;
            if script.contains("/@anthropic-ai/claude-code/") {
                Some("claude")
            } else if script.contains("/@openai/codex/") {
                (!codex_app_server(args)).then_some("codex")
            } else if script.contains("/pi-coding-agent/") {
                Some("pi")
            } else if script.contains("/@google/gemini-cli/") {
                Some("gemini")
            } else {
                None
            }
        }
        _ if first.contains("/claude/versions/") => Some("claude"),
        _ => None,
    }
}

pub fn parse_inventory(text: &str) -> Result<Inventory, String> {
    let processes: Vec<_> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(parse_line)
        .collect::<Result<_, _>>()?;
    if processes.is_empty() {
        return Err("empty process inventory; previous state retained".into());
    }
    let mut servers = BTreeMap::new();
    for process in &processes {
        let mut args = process.command.split_whitespace();
        let executable = args.next().unwrap_or("");
        if Path::new(executable).file_name().and_then(|s| s.to_str()) != Some("zellij")
            || args.next() != Some("--server")
        {
            continue;
        }
        // Server socket names can contain spaces. Environment values follow argv.
        let socket_and_env = args.collect::<Vec<_>>().join(" ");
        let socket = socket_and_env
            .split(" ZELLIJ=")
            .next()
            .unwrap_or(&socket_and_env);
        let socket = socket.split(" PATH=").next().unwrap_or(socket);
        let socket_end = socket
            .char_indices()
            .find_map(|(i, c)| {
                if c != ' ' {
                    return None;
                }
                let tail = &socket[i + 1..];
                let name = tail.split('=').next()?;
                (tail.contains('=') && name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric()))
                    .then_some(i)
            })
            .unwrap_or(socket.len());
        let session = Path::new(socket[..socket_end].trim())
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if !session.is_empty() {
            servers.insert(
                session.to_owned(),
                format!("{}:{}", process.pid, process.started),
            );
        }
    }
    let mut found = Vec::new();
    for process in &processes {
        let Some(tool) = tool(&process.command) else {
            continue;
        };
        let Some(session) = env_value(&process.command, "ZELLIJ_SESSION_NAME") else {
            continue;
        };
        let Some(pane) =
            env_value(&process.command, "ZELLIJ_PANE_ID").and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        let Some(epoch) = servers.get(&session) else {
            continue;
        };
        let incarnation = format!("{}:{}", process.pid, process.started);
        // JSON tuple encoding avoids ambiguous IDs even for spaces and punctuation.
        let agent_id = serde_json::to_string(&(&session, epoch, pane, &incarnation)).unwrap();
        found.push(FoundProcess {
            identity: Identity {
                agent_id,
                session_name: session,
                session_epoch: epoch.clone(),
                pane_id: pane,
                incarnation_id: incarnation,
                pid: process.pid,
                process_started: process.started.clone(),
            },
            tool: tool.into(),
            cwd: env_value(&process.command, "PWD").unwrap_or_default(),
        });
    }
    // Node launchers and native children may both have the same pane environment.
    // Keep the deepest agent process; it is the one hooks execute beneath.
    let candidates = found.clone();
    found.retain(|candidate| {
        !candidates.iter().any(|other| {
            other.identity.slot() == candidate.identity.slot()
                && other.identity.pid != candidate.identity.pid
                && is_ancestor(&processes, candidate.identity.pid, other.identity.pid)
        })
    });
    Ok(Inventory {
        processes,
        found,
        epochs: servers,
    })
}

pub fn is_ancestor(processes: &[Process], ancestor: u32, mut child: u32) -> bool {
    for _ in 0..64 {
        if child == ancestor {
            return true;
        }
        let Some(process) = processes.iter().find(|p| p.pid == child) else {
            return false;
        };
        if process.ppid == child || process.ppid == 0 {
            return false;
        }
        child = process.ppid;
    }
    false
}

pub fn inventory(runner: &dyn CommandRunner) -> Result<Inventory, String> {
    let mut spec = CommandSpec::new("ps", None, 64 * 1024 * 1024).args([
        "axeww",
        "-o",
        "pid=,ppid=,lstart=,command=",
    ]);
    // Keep timestamps English while preserving Korean argv/environment bytes.
    spec.env = vec![
        ("LC_ALL".into(), None),
        ("LC_TIME".into(), Some("C".into())),
        ("LC_CTYPE".into(), Some("en_US.UTF-8".into())),
    ];
    let output = runner
        .run(&spec)
        .map_err(|e| format!("process inventory: {e}"))?;
    if !output.success {
        return Err("process inventory failed; previous state retained".into());
    }
    let parsed = parse_inventory(&String::from_utf8_lossy(&output.stdout))?;
    if !parsed.processes.iter().any(|p| p.pid == std::process::id()) {
        return Err("incomplete process inventory; previous state retained".into());
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_session_names_with_spaces_and_ignores_unrelated_commands() {
        let text = "10 1 Mon Oct 5 10:00:00 2026 /usr/bin/zellij --server /tmp/my session PATH=/bin PWD=/tmp\n20 10 Mon Oct 5 10:01:00 2026 /bin/codex ZELLIJ_SESSION_NAME=my session ZELLIJ_PANE_ID=7 PWD=/a path X=1\n21 20 Mon Oct 5 10:01:01 2026 /bin/sh -c codex ZELLIJ_SESSION_NAME=my session ZELLIJ_PANE_ID=8\n";
        let inventory = parse_inventory(text).unwrap();
        assert_eq!(inventory.found.len(), 1);
        assert_eq!(inventory.found[0].identity.session_name, "my session");
        assert_eq!(inventory.found[0].cwd, "/a path");
    }

    #[test]
    fn detects_claude_native_version_and_deepest_codex_child() {
        let text = "10 1 Mon Oct 5 10:00:00 2026 zellij --server /tmp/dev\n20 10 Mon Oct 5 10:01:00 2026 node /x/@openai/codex/bin/codex.js ZELLIJ_SESSION_NAME=dev ZELLIJ_PANE_ID=7\n21 20 Mon Oct 5 10:01:01 2026 /bin/codex ZELLIJ_SESSION_NAME=dev ZELLIJ_PANE_ID=7\n22 10 Mon Oct 5 10:02:00 2026 /Users/me/.local/share/claude/versions/2.1.70 ZELLIJ_SESSION_NAME=dev ZELLIJ_PANE_ID=8\n";
        let inventory = parse_inventory(text).unwrap();
        assert_eq!(inventory.found.len(), 2);
        assert_eq!(inventory.found[0].identity.pid, 21);
        assert_eq!(inventory.found[1].tool, "claude");
    }

    #[test]
    fn a_restarted_server_produces_a_new_identity() {
        let a = "10 1 Mon Oct 5 10:00:00 2026 zellij --server /tmp/dev\n20 10 Mon Oct 5 10:01:00 2026 codex ZELLIJ_SESSION_NAME=dev ZELLIJ_PANE_ID=7\n";
        let first = parse_inventory(a).unwrap();
        let second = parse_inventory(&a.replace("10 1", "11 1")).unwrap();
        assert_ne!(
            first.found[0].identity.agent_id,
            second.found[0].identity.agent_id
        );
        assert!(parse_inventory("truncated").is_err());
        assert!(parse_inventory("").is_err());
    }

    #[test]
    fn excludes_codex_app_servers_with_inherited_terminal_environment() {
        let text = "10 1 Mon Oct 5 10:00:00 2026 zellij --server /tmp/dev\n20 10 Mon Oct 5 10:01:00 2026 codex ZELLIJ_SESSION_NAME=dev ZELLIJ_PANE_ID=7\n21 1 Mon Oct 5 10:01:01 2026 codex app-server daemon ZELLIJ_SESSION_NAME=dev ZELLIJ_PANE_ID=8\n22 1 Mon Oct 5 10:01:02 2026 node /x/@openai/codex/bin/codex.js app-server --listen unix:// ZELLIJ_SESSION_NAME=dev ZELLIJ_PANE_ID=9\n23 1 Mon Oct 5 10:01:03 2026 /x/.codex/packages/app-server-daemon/releases/test/bin/codex --config x=1 app-server ZELLIJ_SESSION_NAME=dev ZELLIJ_PANE_ID=10\n";
        let inventory = parse_inventory(text).unwrap();
        assert_eq!(inventory.found.len(), 1);
        assert_eq!(inventory.found[0].identity.pid, 20);
        assert_eq!(tool("codex --config x=1 app-server --listen unix://"), None);
        assert_eq!(
            tool("/x/app-server-daemon/bin/codex -- interactive app-server prompt"),
            Some("codex")
        );
    }
}
