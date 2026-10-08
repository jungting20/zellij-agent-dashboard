use crate::{
    host::{pane_id, HostDependencies},
    process,
    repository::now_ms,
    terminal::{SessionId, TerminalPane},
};
use dashboard_core::{
    Agent, Identity, Liveness, PaneInfo, PaneOutput, PanePresence, StatusSource, Store,
};
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

fn apply_metadata(store: &mut Store, session: &str, panes: &[TerminalPane], observed_at_ms: u64) {
    let targets: Vec<_> = store
        .agents
        .values()
        .filter(|a| a.liveness == Liveness::Live && a.identity.session_name == session)
        .map(|a| (a.identity.clone(), a.pane.clone()))
        .collect();
    for (identity, mut info) in targets {
        let pane = panes.iter().find(|p| p.id == pane_id(identity.pane_id));
        info.observed_at_ms = observed_at_ms;
        info.presence = if pane.is_some() {
            PanePresence::Present
        } else {
            PanePresence::Missing
        };
        if let Some(pane) = pane {
            info.tab_id = pane.tab_id;
            info.tab_name.clone_from(&pane.tab_name);
            info.title.clone_from(&pane.title);
        }
        if store.observe_pane(&identity, info) {
            let agent = store.agents.get_mut(&identity.agent_id).unwrap();
            if agent.status_source != StatusSource::Hook {
                if let Some(cwd) = pane.and_then(|p| p.cwd.as_ref()).filter(|s| !s.is_empty()) {
                    agent.cwd.clone_from(cwd);
                }
            }
        }
    }
}

/// Action-time validation: failure never authorizes focus or input to a pane.
pub fn require_present(identity: &Identity, deps: &HostDependencies) -> Result<PaneInfo, String> {
    let observed_at_ms = now_ms();
    let panes = deps.terminal.list_panes(
        &SessionId(identity.session_name.clone()),
        true,
        Duration::from_millis(500),
    )?;
    let pane = panes
        .iter()
        .find(|p| p.id == pane_id(identity.pane_id))
        .ok_or("agent pane no longer exists; refresh the list")?;
    Ok(PaneInfo {
        presence: PanePresence::Present,
        observed_at_ms,
        tab_id: pane.tab_id,
        tab_name: pane.tab_name.clone(),
        title: pane.title.clone(),
    })
}

pub fn refresh_metadata(store: &mut Store, deps: &HostDependencies) -> BTreeSet<String> {
    let mut fresh = BTreeSet::new();
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
        let observed_at_ms = now_ms();
        let result = deps.terminal.list_panes(
            &SessionId(session.clone()),
            true,
            remaining.min(Duration::from_millis(250)),
        );
        // Missing/older session servers must not invalidate a successful ps scan.
        if let Ok(panes) = result {
            fresh.extend(
                store
                    .agents
                    .values()
                    .filter(|a| {
                        a.liveness == Liveness::Live
                            && a.identity.session_name == session
                            && panes.iter().any(|p| p.id == pane_id(a.identity.pane_id))
                    })
                    .map(|a| a.identity.agent_id.clone()),
            );
            apply_metadata(store, &session, &panes, observed_at_ms);
        }
    }
    fresh
}

pub fn preview(agent: &Agent, deps: &HostDependencies) -> Result<PaneOutput, String> {
    if agent.ended {
        return Err("agent session ended".into());
    }
    let present = |inventory: process::Inventory| {
        inventory.found.iter().any(|p| p.identity == agent.identity)
    };
    if !present(process::inventory(deps.runner)?) {
        return Err("agent process changed or exited".into());
    }
    require_present(&agent.identity, deps)?;
    let text = deps.terminal.screen(
        &SessionId(agent.identity.session_name.clone()),
        &pane_id(agent.identity.pane_id),
    )?;
    if !present(process::inventory(deps.runner)?) {
        return Err("agent changed while reading output".into());
    }
    Ok(PaneOutput {
        agent_id: agent.identity.agent_id.clone(),
        text,
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
        let mut panes = vec![TerminalPane {
            id: pane_id(3),
            tab_id: Some(2),
            tab_name: "Development".into(),
            title: String::new(),
            cwd: Some("/actual".into()),
        }];
        apply_metadata(&mut store, "한글 세션", &panes, 1001);
        assert_eq!(store.agents["a"].pane.tab_id, Some(2));
        assert_eq!(store.agents["a"].cwd, "/actual");
        store.agents.get_mut("a").unwrap().last_report_ms = Some(1000);
        store.agents.get_mut("a").unwrap().status_source = StatusSource::Screen;
        panes[0].cwd = Some("/screen-project".into());
        apply_metadata(&mut store, "한글 세션", &panes, 1001);
        assert_eq!(store.agents["a"].cwd, "/screen-project");
        store.agents.get_mut("a").unwrap().status_source = StatusSource::Hook;
        store.agents.get_mut("a").unwrap().cwd = "/hook".into();
        apply_metadata(&mut store, "한글 세션", &panes, 1001);
        assert_eq!(store.agents["a"].cwd, "/hook");
        assert_eq!(store.agents["a"].pane.tab_id, Some(2));
    }
}
