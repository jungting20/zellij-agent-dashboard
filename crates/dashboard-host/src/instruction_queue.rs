//! One finite dispatch per successful collection. Claims survive host/plugin reloads.
use crate::{actions, host::HostDependencies, repository::now_ms};

pub fn dispatch(deps: &HostDependencies) -> Result<(), String> {
    let claimed = {
        let mut locked = deps.repository.begin_runtime()?;
        locked.store.expire_queue_deliveries(now_ms());
        let claimed = locked.store.claim_queued_input(now_ms());
        locked.commit()?;
        claimed
    };
    let Some((target, instruction)) = claimed else {
        return Ok(());
    };
    let result = (|| {
        actions::checked(&target, deps)?;
        deps.repository
            .runtime()?
            .validate_queued_delivery(&target, &instruction.id, now_ms())?;
        actions::send(&target, &instruction.text, deps)
    })();
    let mut locked = deps.repository.begin_runtime()?;
    locked
        .store
        .finish_queued_delivery(&target, &instruction.id, result.err())?;
    locked.commit()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        command::{CommandError, CommandOutput, CommandRunner, CommandSpec},
        host::pane_id,
        process, repository,
        terminal::{NewPane, PaneId, SessionId, TerminalHost, TerminalPane},
    };
    use dashboard_core::{
        DeliveryState, Identity, PaneInfo, PanePresence, StateSignal, Status, StatusObservation,
    };
    use std::{
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc, Mutex,
        },
        time::Duration,
    };

    #[derive(Default)]
    struct Fake {
        writes: AtomicUsize,
        pasted: AtomicUsize,
        replace_on_paste: bool,
        replaced: AtomicBool,
        texts: Mutex<Vec<String>>,
    }
    impl Fake {
        fn inventory(&self) -> String {
            let start = if self.replaced.load(Ordering::SeqCst) {
                "10:02:00"
            } else {
                "10:01:00"
            };
            format!("10 1 Mon Oct 5 10:00:00 2026 zellij --server /tmp/dev\n20 10 Mon Oct 5 {start} 2026 /bin/codex ZELLIJ_SESSION_NAME=dev ZELLIJ_PANE_ID=7 PWD=/tmp\n{} 1 Mon Oct 5 10:00:00 2026 /bin/test\n",std::process::id())
        }
        fn seed(&self, repository: &dyn repository::Repository) -> Identity {
            let found = process::parse_inventory(&self.inventory()).unwrap().found;
            let target = found[0].identity.clone();
            let at = now_ms();
            let mut tx = repository.begin_runtime().unwrap();
            tx.store.reconcile(&found, at - 5000);
            tx.store.observe_pane(
                &target,
                PaneInfo {
                    presence: PanePresence::Present,
                    observed_at_ms: at - 3000,
                    ..PaneInfo::default()
                },
            );
            for (timestamp, status) in [(at - 2000, Status::Working), (at - 500, Status::Idle)] {
                tx.store
                    .apply_signal(&StateSignal::Screen(StatusObservation {
                        identity: target.clone(),
                        tool: "codex".into(),
                        observation_id: format!("screen-{timestamp}"),
                        observed_at_ms: timestamp,
                        status: Some(status),
                        visible_idle: true,
                        rule_id: "test".into(),
                    }))
                    .unwrap();
                if status == Status::Working {
                    tx.store
                        .queue_input(&target, "one", "first\nsecond line", at - 1500)
                        .unwrap();
                    tx.store
                        .queue_input(&target, "two", "next instruction", at - 1400)
                        .unwrap();
                }
            }
            tx.commit().unwrap();
            target
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
            Ok(vec![TerminalPane {
                id: pane_id(7),
                tab_id: Some(1),
                tab_name: "dev".into(),
                title: String::new(),
                cwd: Some("/tmp".into()),
            }])
        }
        fn screen(&self, _: &SessionId, _: &PaneId) -> Result<String, String> {
            panic!("unexpected screen")
        }
        fn write_text(&self, _: &SessionId, _: &PaneId, text: &str) -> Result<(), String> {
            self.pasted.fetch_add(1, Ordering::SeqCst);
            self.texts.lock().unwrap().push(text.into());
            if self.replace_on_paste {
                self.replaced.store(true, Ordering::SeqCst);
            }
            Ok(())
        }
        fn write_bytes(&self, _: &SessionId, _: &PaneId, bytes: &[u8]) -> Result<(), String> {
            assert_eq!(bytes, [13]);
            self.writes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn close_pane(&self, _: &SessionId, _: &PaneId) -> Result<(), String> {
            panic!("unexpected close")
        }
        fn new_pane(&self, _: &SessionId, _: &NewPane) -> Result<PaneId, String> {
            panic!("unexpected creation")
        }
        fn notify_changed(&self, _: &SessionId, _: &str) -> Result<(), String> {
            Ok(())
        }
    }
    #[test]
    fn input_actions_enqueue_idempotently_and_idle_without_queue_sends_immediately() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Fake::default();
        let repo = repository::at(dir.path());
        let target = fake.seed(repo.as_ref());
        let deps = HostDependencies {
            repository: repo.as_ref(),
            runner: &fake,
            terminal: &fake,
        };
        let request = dashboard_core::ActionRequest {
            request_id: "manual-input".into(),
            action: dashboard_core::Action::Input {
                target: target.clone(),
                text: "third instruction".into(),
            },
        };
        let result = actions::action(dir.path(), request.clone(), &deps).unwrap();
        assert!(result.message.contains("예약 추가"));
        actions::action(dir.path(), request, &deps).unwrap();
        assert_eq!(
            repo.runtime().unwrap().agents[&target.agent_id]
                .instruction_queue
                .pending
                .len(),
            3
        );
        assert_eq!(fake.writes.load(Ordering::SeqCst), 0);
        let mut tx = repo.begin_runtime().unwrap();
        for id in ["one", "two", "manual-input"] {
            tx.store.cancel_queued_input(&target, id).unwrap();
        }
        tx.commit().unwrap();
        let result = actions::action(
            dir.path(),
            dashboard_core::ActionRequest {
                request_id: "idle-input".into(),
                action: dashboard_core::Action::Input {
                    target,
                    text: "immediate".into(),
                },
            },
            &deps,
        )
        .unwrap();
        assert!(result.message.contains("입력 전송"));
        assert_eq!(fake.writes.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn concurrent_collectors_deliver_once_and_preserve_multiline_fifo() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Arc::new(Fake::default());
        let repo = repository::at(dir.path());
        let target = fake.seed(repo.as_ref());
        std::thread::scope(|scope| {
            for _ in 0..2 {
                let fake = fake.clone();
                let path = dir.path();
                scope.spawn(move || {
                    let repo = repository::at(path);
                    let deps = HostDependencies {
                        repository: repo.as_ref(),
                        runner: fake.as_ref(),
                        terminal: fake.as_ref(),
                    };
                    dispatch(&deps).unwrap();
                });
            }
        });
        assert_eq!(fake.writes.load(Ordering::SeqCst), 1);
        assert_eq!(
            *fake.texts.lock().unwrap(),
            ["\x1b[200~first\nsecond line\x1b[201~"]
        );
        let restored = repo.runtime().unwrap();
        let queue = &restored.agents[&target.agent_id].instruction_queue;
        assert_eq!(queue.pending[0].id, "two");
        assert_eq!(queue.delivery.as_ref().unwrap().state, DeliveryState::Sent);
        dispatch(&HostDependencies {
            repository: repo.as_ref(),
            runner: fake.as_ref(),
            terminal: fake.as_ref(),
        })
        .unwrap();
        assert_eq!(fake.writes.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn replaced_pane_between_paste_and_enter_halts_without_retrying_or_retargeting() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Fake {
            replace_on_paste: true,
            ..Fake::default()
        };
        let repo = repository::at(dir.path());
        let target = fake.seed(repo.as_ref());
        let deps = HostDependencies {
            repository: repo.as_ref(),
            runner: &fake,
            terminal: &fake,
        };
        dispatch(&deps).unwrap();
        dispatch(&deps).unwrap();
        assert_eq!(fake.pasted.load(Ordering::SeqCst), 1);
        assert_eq!(fake.writes.load(Ordering::SeqCst), 0);
        let store = repo.runtime().unwrap();
        assert_eq!(
            store.agents[&target.agent_id]
                .instruction_queue
                .delivery
                .as_ref()
                .unwrap()
                .state,
            DeliveryState::Uncertain
        );
        assert!(store
            .agents
            .values()
            .filter(|a| a.identity != target)
            .all(|a| a.instruction_queue.is_empty()));
    }
}
