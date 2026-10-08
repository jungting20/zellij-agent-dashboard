mod actions;
mod command;
mod hooks;
mod host;
mod panes;
mod process;
mod storage;
mod terminal;
mod zellij;

use dashboard_core::{AgentEvent, ApplyResult, Liveness, SCHEMA_VERSION};
use std::{
    env,
    io::{self, Read},
    path::{Path, PathBuf},
};
use storage::{now_ms, LockedStore};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let is_hook = args.iter().any(|s| s == "hook");
    let runner = command::SystemCommandRunner;
    let terminal = zellij::ZellijCli::new("zellij", &runner);
    let dependencies = host::HostDependencies {
        terminal: &terminal,
        runner: &runner,
    };
    if let Err(error) = run(args, &dependencies) {
        eprintln!("agent-dashboard: {error}");
        // Observability hooks must never change the agent's own outcome.
        if !is_hook {
            std::process::exit(1);
        }
    }
}

fn read_input() -> Result<String, String> {
    let mut bytes = Vec::new();
    io::stdin()
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("hook payload exceeds 1 MiB".into());
    }
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

fn json(value: &impl serde::Serialize) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string(value).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn default_dir() -> Result<PathBuf, String> {
    if let Some(dir) = env::var_os("XDG_STATE_HOME") {
        return Ok(PathBuf::from(dir).join("zellij-agent-dashboard"));
    }
    Ok(PathBuf::from(env::var_os("HOME").ok_or("HOME is unset")?)
        .join(".local/state/zellij-agent-dashboard"))
}

fn run(mut args: Vec<String>, deps: &host::HostDependencies) -> Result<(), String> {
    let mut dir = default_dir()?;
    if args.first().map(String::as_str) == Some("--state-dir") {
        if args.len() < 3 {
            return Err("--state-dir requires an absolute path and command".into());
        }
        dir = PathBuf::from(args.remove(1));
        args.remove(0);
    }
    let command = args.first().map(String::as_str).unwrap_or("help");
    match command {
        "requests" => {
            let locked = LockedStore::open(&dir)?;
            let mut records: Vec<_> = locked.store.requests.values().cloned().collect();
            records.sort_by_key(|r| std::cmp::Reverse(r.at_ms));
            records.truncate(50);
            json(&records)
        }
        "catalog" => json(&actions::catalog(&dir, deps)?),
        "action" => {
            let request = serde_json::from_str(args.get(1).ok_or("action requires JSON request")?)
                .map_err(|e| e.to_string())?;
            json(&actions::action(&dir, request, deps)?)
        }
        "result" => json(&actions::result(
            &dir,
            args.get(1).ok_or("result requires request ID")?,
        )?),
        "edited" => json(&actions::edited(
            &dir,
            args.get(1).ok_or("edited requires request ID")?,
            deps,
        )?),
        "help" | "--help" => {
            println!("dashboard-host [--state-dir /path] <scan|snapshot|resolve ID|pin ID true/false|preview ID|hook --tool claude|hook-config>");
            Ok(())
        }
        "hook-config" => json(&hooks::claude_config(
            &env::current_exe().map_err(|e| e.to_string())?,
            &dir,
        )),
        "hook" => {
            if args.get(1).map(String::as_str) != Some("--tool")
                || args.get(2).map(String::as_str) != Some("claude")
            {
                return Err("only the verified Claude hook adapter is currently supported".into());
            }
            hook(&dir, deps)
        }
        "scan" => {
            let mut locked = LockedStore::open(&dir)?;
            let at = now_ms();
            // All collector replicas share the same serialization and scan budget.
            if at.saturating_sub(locked.store.last_scan_ms) >= 1800 {
                let inventory = process::inventory(deps.runner)?;
                locked.store.reconcile(&inventory.found, at);
                actions::link_launches(&mut locked.store, &inventory);
                panes::refresh_metadata(&mut locked.store, deps);
                locked.save()?;
            }
            json(&locked.store.snapshot(at))
        }
        "snapshot" => {
            let locked = LockedStore::open(&dir)?;
            json(&locked.store.snapshot(now_ms()))
        }
        "pin" => {
            let id = args.get(1).ok_or("pin requires an agent ID")?;
            let pinned = args
                .get(2)
                .ok_or("pin requires true or false")?
                .parse::<bool>()
                .map_err(|_| "pin requires true or false")?;
            let mut locked = LockedStore::open(&dir)?;
            let inventory = process::inventory(deps.runner)?;
            locked.store.reconcile(&inventory.found, now_ms());
            locked.store.set_pinned(id, pinned)?;
            locked.save()?;
            json(&locked.store.agents[id])
        }
        "preview" => {
            let id = args.get(1).ok_or("preview requires an agent ID")?;
            let agent = {
                let locked = LockedStore::open(&dir)?;
                locked
                    .store
                    .agents
                    .get(id)
                    .cloned()
                    .ok_or("agent no longer exists")?
            };
            json(&panes::preview(&agent, deps)?)
        }
        "resolve" => {
            let id = args.get(1).ok_or("resolve requires an agent ID")?;
            let mut locked = LockedStore::open(&dir)?;
            let inventory = process::inventory(deps.runner)?;
            locked.store.reconcile(&inventory.found, now_ms());
            locked.save()?;
            let agent = locked
                .store
                .agents
                .get(id)
                .ok_or("agent no longer exists")?;
            if agent.liveness != Liveness::Live {
                return Err("agent process changed or exited; refresh the list".into());
            }
            json(agent)
        }
        "ingest" => {
            let event: AgentEvent =
                serde_json::from_str(&read_input()?).map_err(|e| e.to_string())?;
            let mut locked = LockedStore::open(&dir)?;
            let inventory = process::inventory(deps.runner)?;
            locked.store.reconcile(&inventory.found, now_ms());
            let applied = locked.store.apply(&event)? == ApplyResult::Applied;
            locked.save()?;
            json(&serde_json::json!({"applied":applied, "revision":locked.store.revision}))
        }
        _ => Err(format!("unknown command {command}")),
    }
}

fn hook(dir: &Path, deps: &host::HostDependencies) -> Result<(), String> {
    let Ok(session) = env::var("ZELLIJ_SESSION_NAME") else {
        return Ok(());
    };
    let Some(pane) = env::var("ZELLIJ_PANE_ID")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
    else {
        return Ok(());
    };
    let observed_at = now_ms();
    let input: hooks::ClaudeHook =
        serde_json::from_str(&read_input()?).map_err(|e| format!("invalid hook JSON: {e}"))?;
    let Some(kind) = input.kind() else {
        return Ok(());
    };
    let event_id = uuid::Uuid::new_v4().to_string();
    {
        let mut locked = LockedStore::open(dir)?;
        let inventory = process::inventory(deps.runner)?;
        let found = inventory
            .found
            .iter()
            .find(|p| {
                p.tool == "claude"
                    && p.identity.session_name == session
                    && p.identity.pane_id == pane
                    && process::is_ancestor(
                        &inventory.processes,
                        p.identity.pid,
                        std::process::id(),
                    )
            })
            .ok_or_else(|| {
                let candidates: Vec<_> = inventory.found.iter().filter(|p| p.tool == "claude")
                    .map(|p| (&p.identity.session_name, p.identity.pane_id, p.identity.pid)).collect();
                let mut chain = Vec::new();
                let mut pid = std::process::id();
                for _ in 0..16 {
                    let Some(p) = inventory.processes.iter().find(|p| p.pid == pid) else { break };
                    chain.push((p.pid, p.ppid));
                    if p.ppid == 0 || p.ppid == pid { break; }
                    pid = p.ppid;
                }
                format!("hook has no live Claude ancestor: session={session:?} pane={pane} candidates={candidates:?} ancestry={chain:?}")
            })?;
        let identity = found.identity.clone();
        locked.store.reconcile(&inventory.found, now_ms());
        let sequence = locked.store.agents[&identity.agent_id].sequence + 1;
        let event = AgentEvent {
            schema_version: SCHEMA_VERSION,
            event_id: event_id.clone(),
            identity,
            tool: "claude".into(),
            kind,
            sequence,
            observed_at_ms: observed_at,
            cwd: input.cwd.clone(),
            summary: hooks::limit(&input.prompt, 4096),
            detail: input.detail(),
        };
        locked.store.apply(&event)?;
        locked.save()?;
    }
    // Notification is an optimization. State was already committed, so pipe
    // failure or a missing UI cannot lose the event or block an agent forever.
    let _ = deps
        .terminal
        .notify_changed(&terminal::SessionId(session), &event_id);
    Ok(())
}
