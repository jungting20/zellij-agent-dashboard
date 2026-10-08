//! Finite collection, with a shared expiring claim and no terminal I/O under lock.
use crate::{
    actions,
    host::{pane_id, HostDependencies},
    panes, process,
    repository::now_ms,
    screen::Detector,
    terminal::SessionId,
};
use dashboard_core::{Liveness, ScanLease, Snapshot, StateSignal, StatusObservation, StatusSource};
use std::time::{Duration, Instant};

pub fn scan(deps: &HostDependencies) -> Result<Snapshot, String> {
    let token = uuid::Uuid::new_v4().to_string();
    {
        let mut locked = deps.repository.begin()?;
        let at = now_ms();
        if at.saturating_sub(locked.store.last_scan_ms) < 1800
            || locked
                .store
                .scan_lease
                .as_ref()
                .is_some_and(|lease| lease.expires_at_ms > at)
        {
            return Ok(locked.store.snapshot(at));
        }
        // A crashed helper releases its claim by expiry, including after reload.
        locked.store.scan_lease = Some(ScanLease {
            token: token.clone(),
            expires_at_ms: at + 10_000,
        });
        locked.commit()?;
    }
    let result = collect(deps, &token);
    // Release on success or failure; never release a newer helper's claim.
    let mut locked = deps.repository.begin()?;
    if owns_claim(&locked, &token) {
        locked.store.scan_lease = None;
        locked.commit()?;
    }
    result?;
    Ok(locked.store.snapshot(now_ms()))
}

fn owns_claim(locked: &crate::repository::UnitOfWork, token: &str) -> bool {
    locked
        .store
        .scan_lease
        .as_ref()
        .is_some_and(|lease| lease.token == token && lease.expires_at_ms > now_ms())
}

fn collect(deps: &HostDependencies, token: &str) -> Result<(), String> {
    let detector = Detector::embedded()?;
    let inventory_at = now_ms();
    let inventory = process::inventory(deps.runner)?;
    let mut collected = {
        let mut locked = deps.repository.begin()?;
        if !owns_claim(&locked, token) || inventory_at < locked.store.last_scan_ms {
            return Ok(());
        }
        locked.store.reconcile(&inventory.found, inventory_at);
        actions::link_launches(&mut locked.store, &inventory);
        locked.commit()?;
        locked.store.clone()
    };
    // Metadata reads are bounded and do not block hooks on the storage lock.
    let fresh_metadata = panes::refresh_metadata(&mut collected, deps);
    let mut targets: Vec<_> = collected
        .agents
        .values()
        .filter(|a| {
            a.liveness == Liveness::Live
                && !a.ended
                && a.status_source != StatusSource::Hook
                && detector.supports(&a.tool)
                && now_ms().saturating_sub(a.discovered_at_ms) >= 3000
        })
        .cloned()
        .collect();
    targets.sort_by_key(|a| (a.last_screen_attempt_ms, a.identity.agent_id.clone()));
    let deadline = Instant::now() + Duration::from_millis(1200);
    let mut signals = Vec::new();
    let mut attempts = Vec::new();
    for agent in targets {
        if Instant::now() >= deadline {
            break;
        }
        // Capture start time rejects a slower sample that finishes after a newer
        // sample; identity is verified again after all terminal reads.
        let at = now_ms();
        let signal = match deps.terminal.screen(
            &SessionId(agent.identity.session_name.clone()),
            &pane_id(agent.identity.pane_id),
        ) {
            Ok(text) if !text.trim().is_empty() => detector
                .signal(
                    &agent,
                    &text,
                    if fresh_metadata.contains(&agent.identity.agent_id) {
                        &agent.pane.title
                    } else {
                        ""
                    },
                    at,
                )
                .unwrap(),
            result => StateSignal::Screen(StatusObservation {
                identity: agent.identity.clone(),
                tool: agent.tool.clone(),
                observation_id: uuid::Uuid::new_v4().to_string(),
                observed_at_ms: at,
                status: None,
                visible_idle: false,
                rule_id: if result.is_err() {
                    "screen_read_failed"
                } else {
                    "screen_empty"
                }
                .into(),
            }),
        };
        attempts.push((agent.identity, at));
        signals.push(signal);
    }
    let verified_at = now_ms();
    let verified = process::inventory(deps.runner)?;
    let mut locked = deps.repository.begin()?;
    if !owns_claim(&locked, token) || verified_at < locked.store.last_scan_ms {
        return Ok(());
    }
    locked.store.reconcile(&verified.found, verified_at);
    for agent in collected.agents.values() {
        if verified.found.iter().any(|p| p.identity == agent.identity) {
            if let Some(current) = locked.store.agents.get_mut(&agent.identity.agent_id) {
                current.pane = agent.pane.clone();
                if current.status_source != StatusSource::Hook {
                    current.cwd = agent.cwd.clone();
                }
            }
        }
    }
    for (identity, at) in attempts {
        if let Some(agent) = locked.store.agents.get_mut(&identity.agent_id) {
            if agent.identity == identity {
                agent.last_screen_attempt_ms = at;
            }
        }
    }
    for signal in signals {
        locked.store.apply_signal(&signal)?;
    }
    locked.commit()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        command::{CommandError, CommandOutput, CommandRunner, CommandSpec},
        terminal::{NewPane, PaneId, TerminalHost, TerminalPane},
    };
    use dashboard_core::{AgentEvent, EventKind, Identity, Status, EVENT_SCHEMA_VERSION};
    use std::{
        cell::Cell,
        path::{Path, PathBuf},
    };

    struct Fake {
        dir: PathBuf,
        repository: Box<dyn crate::repository::Repository>,
        changed: Cell<bool>,
        replace: bool,
        hook: bool,
        fail: bool,
        reads: Cell<u32>,
    }
    impl Fake {
        fn inventory(&self) -> String {
            let started = if self.changed.get() {
                "10:02:00"
            } else {
                "10:01:00"
            };
            format!("10 1 Mon Oct 5 10:00:00 2026 zellij --server /tmp/dev\n20 10 Mon Oct 5 {started} 2026 /bin/codex ZELLIJ_SESSION_NAME=dev ZELLIJ_PANE_ID=7 PWD=/tmp\n{} 1 Mon Oct 5 10:00:00 2026 /bin/test\n", std::process::id())
        }
        fn seed(&self) -> Identity {
            let inventory = process::parse_inventory(&self.inventory()).unwrap();
            let mut locked = crate::repository::at(&self.dir).begin().unwrap();
            locked.store.reconcile(&inventory.found, now_ms() - 5000);
            locked.commit().unwrap();
            inventory.found[0].identity.clone()
        }
        fn deps(&self) -> HostDependencies<'_> {
            HostDependencies {
                repository: self.repository.as_ref(),
                terminal: self,
                runner: self,
            }
        }
    }
    impl CommandRunner for Fake {
        fn run(&self, spec: &CommandSpec) -> Result<CommandOutput, CommandError> {
            assert_eq!(spec.program, "ps");
            Ok(CommandOutput {
                success: true,
                code: Some(0),
                stdout: self.inventory().into_bytes(),
                stderr: vec![],
            })
        }
    }
    impl TerminalHost for Fake {
        fn list_panes(
            &self,
            _: &SessionId,
            _: bool,
            _: Duration,
        ) -> Result<Vec<TerminalPane>, String> {
            // This would time out if the collector held the state lock during I/O.
            let _locked = crate::repository::at(&self.dir).begin().unwrap();
            Ok(vec![TerminalPane {
                id: pane_id(7),
                tab_id: Some(1),
                tab_name: "dev".into(),
                title: "plain name".into(),
                cwd: None,
            }])
        }
        fn screen(&self, _: &SessionId, _: &PaneId) -> Result<String, String> {
            self.reads.set(self.reads.get() + 1);
            // Another collector sees the active claim and performs no reads.
            scan(&self.deps()).unwrap();
            if self.hook {
                let mut locked = crate::repository::at(&self.dir).begin().unwrap();
                let agent = locked.store.agents.values().next().unwrap().clone();
                locked
                    .store
                    .apply(&AgentEvent {
                        schema_version: EVENT_SCHEMA_VERSION,
                        identity: agent.identity,
                        tool: agent.tool,
                        event_id: "hook-during-screen".into(),
                        sequence: 1,
                        kind: EventKind::TurnFinished,
                        observed_at_ms: now_ms(),
                        cwd: String::new(),
                        summary: String::new(),
                        detail: String::new(),
                    })
                    .unwrap();
                locked.commit().unwrap();
            }
            if self.replace {
                self.changed.set(true);
            }
            if self.fail {
                Err("screen unavailable".into())
            } else {
                Ok("• Thinking (3s • esc to interrupt)\n› ".into())
            }
        }
        fn write_text(&self, _: &SessionId, _: &PaneId, _: &str) -> Result<(), String> {
            panic!("unexpected write")
        }
        fn write_bytes(&self, _: &SessionId, _: &PaneId, _: &[u8]) -> Result<(), String> {
            panic!("unexpected write")
        }
        fn close_pane(&self, _: &SessionId, _: &PaneId) -> Result<(), String> {
            panic!("unexpected close")
        }
        fn new_pane(&self, _: &SessionId, _: &NewPane) -> Result<PaneId, String> {
            panic!("unexpected launch")
        }
        fn notify_changed(&self, _: &SessionId, _: &str) -> Result<(), String> {
            panic!("unexpected notification")
        }
    }
    fn fake(dir: &Path) -> Fake {
        Fake {
            dir: dir.into(),
            repository: crate::repository::at(dir),
            changed: Cell::new(false),
            replace: false,
            hook: false,
            fail: false,
            reads: Cell::new(0),
        }
    }

    #[test]
    fn expiring_claim_recovers_and_duplicate_collectors_do_not_read_screens() {
        let dir = tempfile::tempdir().unwrap();
        let fake = fake(dir.path());
        let identity = fake.seed();
        {
            let mut locked = crate::repository::at(dir.path()).begin().unwrap();
            locked.store.scan_lease = Some(ScanLease {
                token: "crashed".into(),
                expires_at_ms: now_ms() - 1,
            });
            locked.commit().unwrap();
        }
        let snapshot = scan(&fake.deps()).unwrap();
        assert_eq!(
            snapshot
                .agents
                .iter()
                .find(|a| a.identity == identity)
                .unwrap()
                .status,
            Status::Working
        );
        scan(&fake.deps()).unwrap();
        assert_eq!(fake.reads.get(), 1);
        assert!(crate::repository::at(dir.path())
            .begin()
            .unwrap()
            .store
            .scan_lease
            .is_none());
    }

    #[test]
    fn hook_arriving_during_unlocked_screen_read_wins() {
        let dir = tempfile::tempdir().unwrap();
        let mut fake = fake(dir.path());
        fake.hook = true;
        let identity = fake.seed();
        scan(&fake.deps()).unwrap();
        let locked = crate::repository::at(dir.path()).begin().unwrap();
        let agent = &locked.store.agents[&identity.agent_id];
        assert_eq!(agent.status_source, StatusSource::Hook);
        assert_eq!(agent.status, Status::Done);
        assert_eq!(agent.last_screen_report_ms, None);
    }

    #[test]
    fn process_replacement_during_screen_read_discards_old_sample() {
        let dir = tempfile::tempdir().unwrap();
        let mut fake = fake(dir.path());
        fake.replace = true;
        let identity = fake.seed();
        let snapshot = scan(&fake.deps()).unwrap();
        assert_eq!(
            snapshot
                .agents
                .iter()
                .find(|a| a.identity == identity)
                .unwrap()
                .liveness,
            Liveness::Gone
        );
        let replacement = snapshot
            .agents
            .iter()
            .find(|a| a.liveness == Liveness::Live)
            .unwrap();
        assert_eq!(replacement.status_source, StatusSource::Unknown);
        assert_eq!(replacement.status, Status::Found);
    }

    #[test]
    fn failed_screen_read_preserves_work_and_cancels_idle_confirmations() {
        let dir = tempfile::tempdir().unwrap();
        let mut fake = fake(dir.path());
        fake.fail = true;
        let identity = fake.seed();
        {
            let mut locked = crate::repository::at(dir.path()).begin().unwrap();
            let agent = locked.store.agents.get_mut(&identity.agent_id).unwrap();
            agent.status = Status::Working;
            agent.idle_confirmations = 2;
            locked.commit().unwrap();
        }
        scan(&fake.deps()).unwrap();
        let locked = crate::repository::at(dir.path()).begin().unwrap();
        let agent = &locked.store.agents[&identity.agent_id];
        assert_eq!(agent.status, Status::Working);
        assert_eq!(agent.idle_confirmations, 0);
        assert_eq!(agent.last_report_ms, None);
    }
}
