use crate::{
    command::{self, CommandRunner, CommandSpec},
    host::{pane_id, pane_number},
    terminal::{NewPane, PaneId, SessionId, TerminalHost, TerminalPane},
};
use serde::Deserialize;
use std::{ffi::OsString, time::Duration};

pub struct ZellijCli<'a> {
    executable: OsString,
    runner: &'a dyn CommandRunner,
}

impl<'a> ZellijCli<'a> {
    pub fn new(executable: impl Into<OsString>, runner: &'a dyn CommandRunner) -> Self {
        Self {
            executable: executable.into(),
            runner,
        }
    }

    fn action(
        &self,
        session: &SessionId,
        args: Vec<OsString>,
        timeout: Duration,
        limit: usize,
    ) -> Result<Vec<u8>, String> {
        command::output(
            self.runner,
            CommandSpec::new(self.executable.clone(), Some(timeout), limit)
                .args([
                    OsString::from("--session"),
                    session.0.clone().into(),
                    "action".into(),
                ])
                .args(args),
        )
    }

    fn pane_action(
        &self,
        session: &SessionId,
        pane: &PaneId,
        action: &str,
        args: Vec<OsString>,
    ) -> Result<(), String> {
        let mut argv = vec![
            action.into(),
            "--pane-id".into(),
            format!("terminal_{}", pane_number(pane)?).into(),
        ];
        argv.extend(args);
        self.action(session, argv, Duration::from_millis(1200), 128 * 1024)
            .map(|_| ())
    }
}

#[derive(Deserialize)]
struct CliPane {
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

impl TerminalHost for ZellijCli<'_> {
    fn list_panes(
        &self,
        session: &SessionId,
        all_tabs: bool,
        timeout: Duration,
    ) -> Result<Vec<TerminalPane>, String> {
        let mut args = vec!["list-panes".into()];
        if all_tabs {
            args.push("--all".into());
        }
        args.push("--json".into());
        let bytes = self.action(
            session,
            args,
            timeout,
            if all_tabs { 1024 * 1024 } else { 128 * 1024 },
        )?;
        let panes: Vec<CliPane> = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        Ok(panes
            .into_iter()
            .filter(|p| !p.is_plugin)
            .map(|p| TerminalPane {
                id: pane_id(p.id),
                tab_id: p.tab_id,
                tab_name: p.tab_name,
                title: p.title,
                cwd: p.pane_cwd,
            })
            .collect())
    }

    fn screen(&self, session: &SessionId, pane: &PaneId) -> Result<String, String> {
        let bytes = self.action(
            session,
            vec![
                "dump-screen".into(),
                "--pane-id".into(),
                format!("terminal_{}", pane_number(pane)?).into(),
            ],
            Duration::from_millis(500),
            64 * 1024,
        )?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    fn write_text(&self, session: &SessionId, pane: &PaneId, text: &str) -> Result<(), String> {
        self.pane_action(session, pane, "write-chars", vec![text.into()])
    }
    fn write_bytes(&self, session: &SessionId, pane: &PaneId, bytes: &[u8]) -> Result<(), String> {
        self.pane_action(
            session,
            pane,
            "write",
            bytes.iter().map(|b| b.to_string().into()).collect(),
        )
    }
    fn close_pane(&self, session: &SessionId, pane: &PaneId) -> Result<(), String> {
        self.pane_action(session, pane, "close-pane", vec![])
    }
    fn new_pane(&self, session: &SessionId, options: &NewPane) -> Result<PaneId, String> {
        let mut args = vec!["new-pane".into()];
        if options.no_focus {
            args.push("--no-focus".into());
        }
        if options.floating {
            args.push("--floating".into());
        }
        if options.close_on_exit {
            args.push("--close-on-exit".into());
        }
        args.extend([
            "--cwd".into(),
            options.cwd.clone().into_os_string(),
            "--name".into(),
            options.title.clone().into(),
            "--".into(),
            options.program.clone(),
        ]);
        args.extend(options.args.clone());
        let bytes = self.action(session, args, Duration::from_millis(1200), 128 * 1024)?;
        let value = String::from_utf8_lossy(&bytes);
        let id = value
            .trim()
            .strip_prefix("terminal_")
            .ok_or("Zellij did not return a terminal pane ID")?;
        let number = id.parse::<u32>().map_err(|_| "invalid created pane ID")?;
        Ok(pane_id(number))
    }
    fn notify_changed(&self, session: &SessionId, event_id: &str) -> Result<(), String> {
        let mut spec =
            CommandSpec::new(self.executable.clone(), Some(Duration::from_millis(500)), 0).args([
                "--session",
                &session.0,
                "pipe",
                "--name",
                "agent-dashboard-changed",
                "--",
                event_id,
            ]);
        spec.capture = false;
        command::output(self.runner, spec).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CommandError, CommandOutput};
    use std::cell::RefCell;

    struct RecordingRunner {
        calls: RefCell<Vec<CommandSpec>>,
        response: RefCell<Result<Vec<u8>, CommandError>>,
    }
    impl RecordingRunner {
        fn new(bytes: &[u8]) -> Self {
            Self {
                calls: RefCell::new(vec![]),
                response: RefCell::new(Ok(bytes.to_vec())),
            }
        }
    }
    impl CommandRunner for RecordingRunner {
        fn run(&self, spec: &CommandSpec) -> Result<CommandOutput, CommandError> {
            self.calls.borrow_mut().push(spec.clone());
            match &*self.response.borrow() {
                Ok(bytes) => Ok(CommandOutput {
                    success: true,
                    code: Some(0),
                    stdout: bytes.clone(),
                    stderr: vec![],
                }),
                Err(_) => Err(CommandError::Timeout),
            }
        }
    }
    fn argv(runner: &RecordingRunner) -> Vec<String> {
        runner
            .calls
            .borrow()
            .last()
            .unwrap()
            .args
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn pane_list_filters_plugins_and_parses_metadata() {
        let runner = RecordingRunner::new(br#"[{"id":3,"is_plugin":true},{"id":3,"is_plugin":false,"tab_id":2,"tab_name":"dev","pane_cwd":"/a path"}]"#);
        let host = ZellijCli::new("/custom/zellij", &runner);
        let panes = host
            .list_panes(
                &SessionId("한글 세션".into()),
                true,
                Duration::from_millis(250),
            )
            .unwrap();
        assert_eq!(panes.len(), 1);
        assert_eq!(panes[0].id, pane_id(3));
        assert_eq!(panes[0].tab_id, Some(2));
        assert_eq!(panes[0].cwd.as_deref(), Some("/a path"));
        assert_eq!(
            argv(&runner),
            [
                "--session",
                "한글 세션",
                "action",
                "list-panes",
                "--all",
                "--json"
            ]
        );
        let calls = runner.calls.borrow();
        assert_eq!(calls[0].program, "/custom/zellij");
        assert_eq!(calls[0].timeout, Some(Duration::from_millis(250)));
        assert_eq!(calls[0].limit, 1024 * 1024);
    }

    #[test]
    fn creation_preserves_argv_and_validates_terminal_id() {
        let runner = RecordingRunner::new(b"terminal_42\n");
        let host = ZellijCli::new("zellij", &runner);
        let options = NewPane {
            cwd: "/한글 경로".into(),
            title: "작업 창".into(),
            floating: true,
            no_focus: true,
            close_on_exit: true,
            program: "/bin/echo".into(),
            args: vec!["$(literal) `text`".into()],
        };
        assert_eq!(
            host.new_pane(&SessionId("dev".into()), &options).unwrap(),
            pane_id(42)
        );
        assert_eq!(
            argv(&runner),
            [
                "--session",
                "dev",
                "action",
                "new-pane",
                "--no-focus",
                "--floating",
                "--close-on-exit",
                "--cwd",
                "/한글 경로",
                "--name",
                "작업 창",
                "--",
                "/bin/echo",
                "$(literal) `text`"
            ]
        );
        for invalid in ["plugin_42", "terminal_no", "terminal_4294967296"] {
            *runner.response.borrow_mut() = Ok(invalid.as_bytes().to_vec());
            assert!(host.new_pane(&SessionId("dev".into()), &options).is_err());
        }
    }

    #[test]
    fn input_screen_close_and_notification_use_separate_operations() {
        let runner = RecordingRunner::new(b"output\n");
        let host = ZellijCli::new("zellij", &runner);
        let session = SessionId("dev".into());
        let pane = pane_id(7);
        host.write_text(&session, &pane, "한글\n두 줄").unwrap();
        assert_eq!(
            argv(&runner),
            [
                "--session",
                "dev",
                "action",
                "write-chars",
                "--pane-id",
                "terminal_7",
                "한글\n두 줄"
            ]
        );
        host.write_bytes(&session, &pane, &[13, 27]).unwrap();
        assert_eq!(
            argv(&runner),
            [
                "--session",
                "dev",
                "action",
                "write",
                "--pane-id",
                "terminal_7",
                "13",
                "27"
            ]
        );
        assert_eq!(host.screen(&session, &pane).unwrap(), "output\n");
        host.close_pane(&session, &pane).unwrap();
        assert_eq!(argv(&runner)[3], "close-pane");
        host.notify_changed(&session, "event id").unwrap();
        assert_eq!(
            argv(&runner),
            [
                "--session",
                "dev",
                "pipe",
                "--name",
                "agent-dashboard-changed",
                "--",
                "event id"
            ]
        );
        let calls = runner.calls.borrow();
        assert!(!calls.last().unwrap().capture);
        assert_eq!(
            calls.last().unwrap().timeout,
            Some(Duration::from_millis(500))
        );
    }

    #[test]
    fn malformed_json_and_execution_errors_propagate() {
        let runner = RecordingRunner::new(b"{}");
        let host = ZellijCli::new("zellij", &runner);
        assert!(host
            .list_panes(&SessionId("dev".into()), false, Duration::from_secs(1))
            .is_err());
        *runner.response.borrow_mut() = Err(CommandError::Timeout);
        assert!(host
            .close_pane(&SessionId("dev".into()), &pane_id(7))
            .unwrap_err()
            .contains("timed out"));
    }
}
