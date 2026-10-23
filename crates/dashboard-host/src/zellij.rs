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
    pub fn update_notification(&self, session: &SessionId, payload: &str) -> Result<(), String> {
        let mut spec = CommandSpec::new(
            self.executable.clone(),
            Some(Duration::from_millis(1200)),
            0,
        )
        .args([
            "--session",
            &session.0,
            "pipe",
            "--name",
            "agent-waiting-notification",
            "--",
            payload,
        ]);
        spec.capture = false;
        command::output(self.runner, spec).map(|_| ())
    }
    pub fn sync_notification(
        &self,
        session: &SessionId,
        wasm: &std::path::Path,
        host: &std::path::Path,
        state: &std::path::Path,
        enabled: bool,
    ) -> Result<(), String> {
        for path in [wasm, host, state] {
            if !path.is_absolute() || path.to_string_lossy().contains(',') {
                return Err("notification paths must be absolute and cannot contain commas".into());
            }
        }
        let url = format!("file:{}", wasm.display());
        let configuration = format!(
            "mode=notification,host_path={},state_dir={}",
            host.display(),
            state.display()
        );
        let bytes = self.action(
            session,
            vec!["list-panes".into(), "--all".into(), "--json".into()],
            Duration::from_millis(1200),
            1024 * 1024,
        )?;
        let panes: Vec<CliPane> = parse_panes(session, &bytes)?;
        let mut tab_widths = std::collections::BTreeMap::new();
        for pane in panes.iter().filter(|p| !p.is_floating && !p.is_suppressed) {
            let width = tab_widths.entry(pane.tab_id).or_insert(0usize);
            *width = (*width).max(pane.pane_x.saturating_add(pane.pane_columns));
        }
        let mut existing = std::collections::BTreeSet::new();
        for pane in panes
            .iter()
            .filter(|p| p.is_plugin && p.plugin_url.as_deref() == Some(&url))
        {
            if !enabled || !pane.is_floating || pane.is_suppressed || !existing.insert(pane.tab_id)
            {
                self.action(
                    session,
                    vec![
                        "close-pane".into(),
                        "--pane-id".into(),
                        format!("plugin_{}", pane.id).into(),
                    ],
                    Duration::from_millis(1200),
                    128 * 1024,
                )?;
            } else {
                // Coordinates are the left edge: x=100% would produce a zero-width pane.
                let width = tab_widths.get(&pane.tab_id).copied().unwrap_or(40);
                self.action(
                    session,
                    vec![
                        "change-floating-pane-coordinates".into(),
                        "--pane-id".into(),
                        format!("plugin_{}", pane.id).into(),
                        "--x".into(),
                        width.saturating_sub(40).to_string().into(),
                        "--y".into(),
                        "0".into(),
                        "--width".into(),
                        width.min(40).to_string().into(),
                        "--height".into(),
                        "4".into(),
                        "--borderless".into(),
                        "true".into(),
                        "--pinned".into(),
                        "true".into(),
                    ],
                    Duration::from_millis(1200),
                    128 * 1024,
                )?;
            }
        }
        if enabled {
            if !wasm.is_file() {
                return Err(format!("notification plugin not found: {}", wasm.display()));
            }
            let tabs: std::collections::BTreeSet<_> = panes
                .iter()
                .filter(|p| !p.is_plugin)
                .filter_map(|p| p.tab_id)
                .collect();
            for tab in tabs {
                if existing.contains(&Some(tab)) {
                    continue;
                }
                let width = tab_widths.get(&Some(tab)).copied().unwrap_or(40);
                self.action(
                    session,
                    vec![
                        "new-pane".into(),
                        "--plugin".into(),
                        url.clone().into(),
                        "--configuration".into(),
                        configuration.clone().into(),
                        "--tab-id".into(),
                        tab.to_string().into(),
                        "--floating".into(),
                        "--no-focus".into(),
                        "--name".into(),
                        "Agent Waiting".into(),
                        "--x".into(),
                        width.saturating_sub(40).to_string().into(),
                        "--y".into(),
                        "0".into(),
                        "--width".into(),
                        width.min(40).to_string().into(),
                        "--height".into(),
                        "4".into(),
                        "--borderless".into(),
                        "true".into(),
                        "--pinned".into(),
                        "true".into(),
                    ],
                    Duration::from_secs(5),
                    128 * 1024,
                )?;
            }
        }
        Ok(())
    }

    pub fn open_dashboard(
        &self,
        session: &SessionId,
        wasm: &std::path::Path,
        host: &std::path::Path,
        state: &std::path::Path,
    ) -> Result<(), String> {
        for path in [wasm, host, state] {
            if !path.is_absolute() || path.to_string_lossy().contains(',') {
                return Err("dashboard paths must be absolute and cannot contain commas".into());
            }
        }
        let url = format!("file:{}", wasm.display());
        let configuration = format!(
            "mode=dashboard,host_path={},state_dir={}",
            host.display(),
            state.display()
        );
        let bytes = self.action(
            session,
            vec!["list-panes".into(), "--json".into()],
            Duration::from_millis(1200),
            1024 * 1024,
        )?;
        let panes: Vec<CliPane> = parse_panes(session, &bytes)?;
        if panes.iter().any(|pane| {
            pane.is_plugin
                && pane.is_floating
                && !pane.is_suppressed
                && pane.plugin_url.as_deref() == Some(&url)
        }) {
            self.action(
                session,
                vec![
                    "launch-or-focus-plugin".into(),
                    url.into(),
                    "--floating".into(),
                    "--move-to-focused-tab".into(),
                    "--configuration".into(),
                    configuration.into(),
                ],
                Duration::from_secs(5),
                128 * 1024,
            )?;
        } else {
            command::output(
                self.runner,
                CommandSpec::new(
                    self.executable.clone(),
                    Some(Duration::from_secs(5)),
                    128 * 1024,
                )
                .args([
                    "--session",
                    &session.0,
                    "plugin",
                    "--floating",
                    "--x",
                    "5%",
                    "--y",
                    "5%",
                    "--width",
                    "90%",
                    "--height",
                    "90%",
                    "--configuration",
                    &configuration,
                    "--",
                    &url,
                ]),
            )?;
        }
        Ok(())
    }

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
    is_floating: bool,
    #[serde(default)]
    is_suppressed: bool,
    #[serde(default)]
    pane_x: usize,
    #[serde(default)]
    pane_columns: usize,
    #[serde(default)]
    plugin_url: Option<String>,
    #[serde(default)]
    tab_id: Option<u32>,
    #[serde(default)]
    tab_name: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    pane_cwd: Option<String>,
}

fn parse_panes(session: &SessionId, bytes: &[u8]) -> Result<Vec<CliPane>, String> {
    serde_json::from_slice(bytes).map_err(|error| {
        let response: String = String::from_utf8_lossy(bytes)
            .chars()
            .take(240)
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        format!(
            "Zellij list-panes --json 세션 {:?}: {error}; 응답: {response:?}",
            session.0
        )
    })
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
        let panes: Vec<CliPane> = parse_panes(session, &bytes)?;
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

    fn styled_screen(&self, session: &SessionId, pane: &PaneId) -> Result<String, String> {
        let bytes = self.action(
            session,
            vec![
                "dump-screen".into(),
                "--ansi".into(),
                "--pane-id".into(),
                format!("terminal_{}", pane_number(pane)?).into(),
            ],
            Duration::from_millis(500),
            512 * 1024,
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
        if let Some(tab) = options.tab_id {
            args.extend(["--tab-id".into(), tab.to_string().into()]);
        }
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
    use std::path::Path;

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
    fn notification_anchors_with_positive_width_and_reuses_one_window_per_tab() {
        let dir = tempfile::tempdir().unwrap();
        let wasm = dir.path().join("notification.wasm");
        std::fs::write(&wasm, []).unwrap();
        let url = format!("file:{}", wasm.display());
        let data = serde_json::json!([
            {"id":0,"is_plugin":false,"tab_id":0,"pane_x":0,"pane_columns":120},
            {"id":1,"is_plugin":true,"is_floating":true,"tab_id":0,"plugin_url":url,"pane_x":120,"pane_columns":0},
            {"id":2,"is_plugin":true,"is_floating":true,"tab_id":0,"plugin_url":url},
            {"id":3,"is_plugin":false,"tab_id":1,"pane_x":0,"pane_columns":20}
        ]);
        let runner = RecordingRunner::new(&serde_json::to_vec(&data).unwrap());
        ZellijCli::new("zellij", &runner)
            .sync_notification(
                &SessionId("dev".into()),
                &wasm,
                Path::new("/app/dashboard-host"),
                dir.path(),
                true,
            )
            .unwrap();
        let calls = runner.calls.borrow();
        let args: Vec<Vec<String>> = calls
            .iter()
            .map(|c| {
                c.args
                    .iter()
                    .map(|s| s.to_string_lossy().into_owned())
                    .collect()
            })
            .collect();
        let positioned = args
            .iter()
            .find(|a| a[3] == "change-floating-pane-coordinates")
            .unwrap();
        assert!(positioned.windows(2).any(|w| w == ["--x", "80"]));
        assert!(positioned.windows(2).any(|w| w == ["--width", "40"]));
        assert!(args
            .iter()
            .any(|a| a[3] == "close-pane" && a.last().unwrap() == "plugin_2"));
        let created: Vec<_> = args.iter().filter(|a| a[3] == "new-pane").collect();
        assert_eq!(created.len(), 1);
        assert!(created[0].windows(2).any(|w| w == ["--x", "0"]));
        assert!(created[0].windows(2).any(|w| w == ["--width", "20"]));
        assert!(created[0].iter().any(|s| s == "--no-focus"));
    }

    #[test]
    fn notification_cleanup_only_closes_its_own_plugin() {
        let runner = RecordingRunner::new(br#"[
            {"id":1,"is_plugin":true,"is_floating":true,"tab_id":0,"plugin_url":"file:/app/agent-notification.wasm"},
            {"id":2,"is_plugin":true,"is_floating":true,"tab_id":0,"plugin_url":"file:/app/agent-notification.wasm"},
            {"id":3,"is_plugin":true,"is_floating":true,"tab_id":0,"plugin_url":"file:/app/agent-dashboard.wasm"},
            {"id":4,"is_plugin":false,"tab_id":0}
        ]"#);
        let host = ZellijCli::new("zellij", &runner);
        // Cleanup must work even after the notification artifact was removed.
        host.sync_notification(
            &SessionId("운영".into()),
            Path::new("/app/agent-notification.wasm"),
            Path::new("/app/dashboard-host"),
            Path::new("/state"),
            false,
        )
        .unwrap();
        let calls = runner.calls.borrow();
        let targets: Vec<_> = calls
            .iter()
            .skip(1)
            .map(|c| c.args.last().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(targets, ["plugin_1", "plugin_2"]);
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
            tab_id: Some(9),
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
                "--tab-id",
                "9",
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
    fn invalid_inventory_reports_session_and_bounded_response() {
        for bytes in [b"No session found\n".as_slice(), b"", &[27; 400]] {
            let error = match parse_panes(&SessionId("dev".into()), bytes) {
                Err(error) => error,
                Ok(_) => panic!("invalid inventory accepted"),
            };
            assert!(error.contains("dev") && error.contains("list-panes --json"));
            assert!(!error.contains('\n') && !error.contains('\x1b'));
            assert!(error.len() < 500);
        }
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

    #[test]
    fn dashboard_is_created_at_full_size_and_existing_window_is_reused() {
        let runner = RecordingRunner::new(br#"[{"id":0,"is_plugin":true,"is_suppressed":true,"plugin_url":"file:/app/agent-dashboard.wasm"}]"#);
        let host = ZellijCli::new("zellij", &runner);
        let session = SessionId("dev".into());
        let wasm = std::path::Path::new("/app/agent-dashboard.wasm");
        let executable = std::path::Path::new("/app/dashboard-host");
        let state = std::path::Path::new("/state");
        host.open_dashboard(&session, wasm, executable, state)
            .unwrap();
        let args = argv(&runner);
        assert_eq!(args[2], "plugin");
        assert!(args.windows(2).any(|a| a == ["--width", "90%"]));
        assert!(args.windows(2).any(|a| a == ["--height", "90%"]));
        assert_eq!(runner.calls.borrow().len(), 2);

        runner.calls.borrow_mut().clear();
        *runner.response.borrow_mut() = Ok(br#"[{"id":4,"is_plugin":true,"is_floating":true,"plugin_url":"file:/app/agent-dashboard.wasm"}]"#.to_vec());
        host.open_dashboard(&session, wasm, executable, state)
            .unwrap();
        assert_eq!(argv(&runner)[3], "launch-or-focus-plugin");
        assert_eq!(runner.calls.borrow().len(), 2);

        runner.calls.borrow_mut().clear();
        *runner.response.borrow_mut() = Ok(b"invalid inventory".to_vec());
        assert!(host
            .open_dashboard(&session, wasm, executable, state)
            .is_err());
        assert_eq!(runner.calls.borrow().len(), 1);
    }
}
