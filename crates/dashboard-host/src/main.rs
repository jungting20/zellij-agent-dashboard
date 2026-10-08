mod actions;
mod collector;
mod command;
mod hooks;
mod host;
mod legacy_json;
mod panes;
mod process;
mod repository;
mod screen;
mod sqlite_repository;
mod terminal;
mod zellij;

use dashboard_core::{AgentEvent, ApplyResult};
use repository::now_ms;
use std::{
    env,
    io::{self, Read},
    path::PathBuf,
};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let is_hook = args.iter().any(|s| s == "hook");
    let runner = command::SystemCommandRunner;
    let terminal = zellij::ZellijCli::new("zellij", &runner);
    if let Err(error) = run(args, &terminal, &runner) {
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

fn run(
    mut args: Vec<String>,
    terminal: &dyn terminal::TerminalHost,
    runner: &dyn command::CommandRunner,
) -> Result<(), String> {
    let mut dir = default_dir()?;
    if args.first().map(String::as_str) == Some("--state-dir") {
        if args.len() < 3 {
            return Err("--state-dir requires an absolute path and command".into());
        }
        dir = PathBuf::from(args.remove(1));
        args.remove(0);
    }
    let repository = repository::at(&dir);
    let deps = &host::HostDependencies {
        terminal,
        runner,
        repository: repository.as_ref(),
    };
    let command = args.first().map(String::as_str).unwrap_or("help");
    match command {
        "requests" => json(&deps.repository.recent_requests(50)?),
        "catalog" => json(&actions::catalog(deps)?),
        "action" => {
            let request = serde_json::from_str(args.get(1).ok_or("action requires JSON request")?)
                .map_err(|e| e.to_string())?;
            json(&actions::action(&dir, request, deps)?)
        }
        "result" => json(&actions::result(
            deps.repository,
            args.get(1).ok_or("result requires request ID")?,
        )?),
        "edited" => json(&actions::edited(
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
            hook(deps)
        }
        "scan" => json(&collector::scan(deps)?),
        "snapshot" => json(&deps.repository.snapshot(now_ms())?),
        "pin" => {
            let id = args.get(1).ok_or("pin requires an agent ID")?;
            let pinned = args
                .get(2)
                .ok_or("pin requires true or false")?
                .parse::<bool>()
                .map_err(|_| "pin requires true or false")?;
            let inventory_at = now_ms();
            let inventory = process::inventory(deps.runner)?;
            let mut locked = deps.repository.begin()?;
            locked.store.reconcile(&inventory.found, inventory_at);
            locked.store.set_pinned(id, pinned)?;
            locked.commit()?;
            json(&locked.store.agents[id])
        }
        "preview" => {
            let id = args.get(1).ok_or("preview requires an agent ID")?;
            let agent = deps.repository.agent(id)?.ok_or("agent no longer exists")?;
            json(&panes::preview(&agent, deps)?)
        }
        "resolve" => json(&actions::resolve(
            args.get(1).ok_or("resolve requires an agent ID")?,
            deps,
        )?),
        "ingest" => {
            let event: AgentEvent =
                serde_json::from_str(&read_input()?).map_err(|e| e.to_string())?;
            let inventory_at = now_ms();
            let inventory = process::inventory(deps.runner)?;
            let mut locked = deps.repository.begin()?;
            locked.store.reconcile(&inventory.found, inventory_at);
            let applied = locked.store.apply(&event)? == ApplyResult::Applied;
            locked.commit()?;
            json(&serde_json::json!({"applied":applied, "revision":locked.store.revision}))
        }
        _ => Err(format!("unknown command {command}")),
    }
}

fn hook(deps: &host::HostDependencies) -> Result<(), String> {
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
    if input.kind().is_none() {
        return Ok(());
    }
    let event_id = uuid::Uuid::new_v4().to_string();
    {
        let inventory_at = now_ms();
        let inventory = process::inventory(deps.runner)?;
        let mut locked = deps.repository.begin()?;
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
        locked.store.reconcile(&inventory.found, inventory_at);
        let sequence = locked
            .store
            .agents
            .get(&identity.agent_id)
            .ok_or("hook process changed during collection")?
            .sequence
            + 1;
        let signal = input
            .signal(identity, sequence, observed_at, event_id.clone())
            .unwrap();
        locked.store.apply_signal(&signal)?;
        locked.commit()?;
    }
    // Notification is an optimization. State was already committed, so pipe
    // failure or a missing UI cannot lose the event or block an agent forever.
    let _ = deps
        .terminal
        .notify_changed(&terminal::SessionId(session), &event_id);
    Ok(())
}
