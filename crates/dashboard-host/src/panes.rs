use crate::{command, process};
use dashboard_core::{Agent, Liveness, PaneInfo, PaneOutput, Store};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    process::Command,
    time::{Duration, Instant},
};

#[derive(Deserialize)]
struct Pane {
    id: u32,
    is_plugin: bool,
    #[serde(default)]
    tab_id: Option<u32>,
    #[serde(default)]
    tab_name: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    pane_cwd: Option<String>,
}

fn apply_metadata(store: &mut Store, session: &str, bytes: &[u8]) -> Result<(), String> {
    let panes: Vec<Pane> = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    for agent in store
        .agents
        .values_mut()
        .filter(|a| a.liveness == Liveness::Live && a.identity.session_name == session)
    {
        if let Some(pane) = panes
            .iter()
            .find(|p| !p.is_plugin && p.id == agent.identity.pane_id)
        {
            agent.pane = PaneInfo {
                tab_id: pane.tab_id,
                tab_name: pane.tab_name.clone(),
                title: pane.title.clone(),
            };
            if agent.last_report_ms.is_none() {
                if let Some(cwd) = pane.pane_cwd.as_ref().filter(|s| !s.is_empty()) {
                    agent.cwd.clone_from(cwd);
                }
            }
        }
    }
    Ok(())
}

pub fn refresh_metadata(store: &mut Store) {
    let sessions: BTreeSet<_> = store
        .agents
        .values()
        .filter(|a| a.liveness == Liveness::Live)
        .map(|a| a.identity.session_name.clone())
        .collect();
    let deadline = Instant::now() + Duration::from_millis(700);
    for session in sessions {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining < Duration::from_millis(20) {
            break;
        }
        let result = command::output(
            Command::new("zellij").args([
                "--session",
                &session,
                "action",
                "list-panes",
                "--all",
                "--json",
            ]),
            remaining.min(Duration::from_millis(250)),
            1024 * 1024,
        );
        // Missing/older session servers must not invalidate a successful ps scan.
        if let Ok(bytes) = result {
            let _ = apply_metadata(store, &session, &bytes);
        }
    }
}

pub fn preview(agent: &Agent) -> Result<PaneOutput, String> {
    if agent.ended {
        return Err("agent session ended".into());
    }
    let present = |inventory: process::Inventory| {
        inventory.found.iter().any(|p| p.identity == agent.identity)
    };
    if !present(process::inventory()?) {
        return Err("agent process changed or exited".into());
    }
    let pane = format!("terminal_{}", agent.identity.pane_id);
    let bytes = command::output(
        Command::new("zellij").args([
            "--session",
            &agent.identity.session_name,
            "action",
            "dump-screen",
            "--pane-id",
            &pane,
        ]),
        Duration::from_millis(500),
        64 * 1024,
    )?;
    if !present(process::inventory()?) {
        return Err("agent changed while reading output".into());
    }
    Ok(PaneOutput {
        agent_id: agent.identity.agent_id.clone(),
        text: String::from_utf8_lossy(&bytes).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashboard_core::{FoundProcess, Identity};

    #[test]
    fn tab_metadata_uses_terminal_identity_and_preserves_hook_cwd() {
        let mut store = Store::default();
        store.reconcile(
            &[FoundProcess {
                identity: Identity {
                    agent_id: "a".into(),
                    session_name: "한글 세션".into(),
                    session_epoch: "epoch".into(),
                    pane_id: 3,
                    incarnation_id: "run".into(),
                    pid: 42,
                    process_started: "now".into(),
                },
                tool: "claude".into(),
                cwd: "/inherited".into(),
            }],
            1000,
        );
        let bytes = br#"[{"id":3,"is_plugin":true,"tab_id":9,"tab_name":"wrong"},{"id":3,"is_plugin":false,"tab_id":2,"tab_name":"Development","pane_cwd":"/actual"}]"#;
        apply_metadata(&mut store, "한글 세션", bytes).unwrap();
        assert_eq!(store.agents["a"].pane.tab_id, Some(2));
        assert_eq!(store.agents["a"].cwd, "/actual");
        store.agents.get_mut("a").unwrap().last_report_ms = Some(1000);
        store.agents.get_mut("a").unwrap().cwd = "/hook".into();
        apply_metadata(&mut store, "한글 세션", bytes).unwrap();
        assert_eq!(store.agents["a"].cwd, "/hook");
        assert!(apply_metadata(&mut store, "한글 세션", b"invalid").is_err());
        assert_eq!(store.agents["a"].pane.tab_id, Some(2));
    }
}
