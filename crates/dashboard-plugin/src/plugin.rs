use dashboard_core::{view::View, Agent, Liveness, PaneOutput, Snapshot, SCHEMA_VERSION};
use std::{collections::BTreeMap, path::Path};
use zellij_tile::prelude::*;
#[path = "controls.rs"]
mod controls;
use dashboard_core::menu::MenuKind;

#[derive(Default)]
pub struct Dashboard {
    active_request: Option<dashboard_core::ActionRequest>,
    action_pending: bool,
    request_serial: u64,
    started_ns: u128,
    editor_request: Option<String>,
    edited_pending: bool,
    collector: bool,
    host_path: String,
    state_dir: String,
    permissions: bool,
    denied: bool,
    plugin_id: Option<u32>,
    client_id: Option<u16>,
    pending: bool,
    dirty: bool,
    polls: u64,
    snapshot: Option<Snapshot>,
    view: View,
    message: String,
    config_error: Option<String>,
    output_pending: bool,
    output_dirty: bool,
    pin_pending: bool,
}

impl Dashboard {
    fn request_permissions(&self) {
        request_permission(&[
            PermissionType::RunCommands,
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::ReadCliPipes,
        ]);
    }

    fn refresh(&mut self) {
        if !self.permissions || self.config_error.is_some() {
            return;
        }
        if self.pending {
            self.dirty = true;
            return;
        }
        self.pending = true;
        self.polls += 1;
        self.host(
            if self.collector { "scan" } else { "snapshot" },
            None,
            "snapshot",
        );
    }

    fn host(&self, command: &str, argument: Option<&str>, kind: &str) {
        let mut argv = vec![
            self.host_path.as_str(),
            "--state-dir",
            self.state_dir.as_str(),
            command,
        ];
        if let Some(arg) = argument {
            argv.push(arg);
        }
        let mut context = BTreeMap::from([("kind".into(), kind.into())]);
        if let Some(arg) = argument {
            context.insert("agent_id".into(), arg.into());
        }
        run_command(&argv, context);
    }

    fn refresh_output(&mut self) {
        if self.collector || !self.permissions {
            return;
        }
        let agent = self.snapshot.as_ref().and_then(|s| {
            s.agents.iter().find(|a| {
                Some(&a.identity.agent_id) == self.view.selected_id.as_ref()
                    && a.liveness == Liveness::Live
            })
        });
        let Some(agent) = agent else {
            self.view.output = None;
            return;
        };
        if self.output_pending {
            self.output_dirty = true;
            return;
        }
        self.output_pending = true;
        self.host("preview", Some(&agent.identity.agent_id), "output");
    }

    fn pin(&mut self) {
        if self.pin_pending {
            return;
        }
        let agent = self.snapshot.as_ref().and_then(|s| {
            s.agents
                .iter()
                .find(|a| Some(&a.identity.agent_id) == self.view.selected_id.as_ref())
        });
        let Some(agent) = agent else {
            return;
        };
        let desired = if agent.pinned { "false" } else { "true" };
        let context = BTreeMap::from([
            ("kind".into(), "pin".into()),
            ("agent_id".into(), agent.identity.agent_id.clone()),
        ]);
        run_command(
            &[
                &self.host_path,
                "--state-dir",
                &self.state_dir,
                "pin",
                &agent.identity.agent_id,
                desired,
            ],
            context,
        );
        self.pin_pending = true;
        self.message = "Updating pin…".into();
    }

    fn key(&mut self, key: KeyWithModifier) {
        if self.view.menu.is_some() {
            self.menu_key(key);
            return;
        }
        if key.bare_key == BareKey::Char('c') && key.key_modifiers.contains(&KeyModifier::Ctrl) {
            close_self();
            return;
        }
        if self.view.instruction_open {
            match key.bare_key {
                BareKey::Char('p' | 'q') | BareKey::Esc => self.view.instruction_open = false,
                BareKey::Down | BareKey::Char('j') => {
                    self.view.instruction_offset = self.view.instruction_offset.saturating_add(1)
                }
                BareKey::Up | BareKey::Char('k') => {
                    self.view.instruction_offset = self.view.instruction_offset.saturating_sub(1)
                }
                BareKey::PageDown => {
                    self.view.instruction_offset = self.view.instruction_offset.saturating_add(10)
                }
                BareKey::PageUp => {
                    self.view.instruction_offset = self.view.instruction_offset.saturating_sub(10)
                }
                BareKey::Home => self.view.instruction_offset = 0,
                BareKey::End => self.view.instruction_offset = usize::MAX,
                _ => {}
            }
            return;
        }
        if self.view.searching {
            match key.bare_key {
                BareKey::Esc | BareKey::Enter => self.view.searching = false,
                BareKey::Backspace => {
                    self.view.query.pop();
                }
                BareKey::Char(c)
                    if !key.key_modifiers.contains(&KeyModifier::Ctrl)
                        && !key.key_modifiers.contains(&KeyModifier::Alt) =>
                {
                    self.view.query.push(c)
                }
                _ => {}
            }
            if let Some(snapshot) = self.snapshot.as_ref() {
                self.view.reconcile_selection(snapshot);
            }
            self.refresh_output();
            return;
        }
        match key.bare_key {
            BareKey::Char('i') => self.open_menu(MenuKind::Input),
            BareKey::Char('I') => {
                self.open_menu(MenuKind::Input);
                self.edit_instruction();
            }
            BareKey::Char('a') => self.open_menu(MenuKind::Alias),
            BareKey::Char('n') => self.open_menu(MenuKind::Recent),
            BareKey::Char('g') => self.open_menu(MenuKind::Worktree),
            BareKey::Char('m') => self.open_merge(),
            BareKey::Char('u') => self.open_menu(MenuKind::Requests),
            BareKey::Char('d') => {
                if let Some(target) = self
                    .snapshot
                    .as_ref()
                    .and_then(|s| {
                        s.agents
                            .iter()
                            .find(|a| Some(&a.identity.agent_id) == self.view.selected_id.as_ref())
                    })
                    .map(|a| a.identity.clone())
                {
                    self.submit(dashboard_core::Action::Close { target });
                }
            }
            BareKey::Char('q') | BareKey::Esc => close_self(),
            BareKey::Char('/') => self.view.searching = true,
            BareKey::Char('r') | BareKey::Char('R') => self.refresh(),
            BareKey::Tab => {
                if let Some(snapshot) = self.snapshot.as_ref() {
                    self.view.toggle_panel(snapshot);
                }
            }
            BareKey::Char(' ') if self.permissions => self.pin(),
            BareKey::Char('p') => {
                if let Some(snapshot) = self.snapshot.as_ref() {
                    self.view.open_instruction(snapshot);
                }
            }
            BareKey::Down | BareKey::Char('j') => {
                if let Some(snapshot) = self.snapshot.as_ref() {
                    self.view.select(snapshot, 1);
                }
            }
            BareKey::Up | BareKey::Char('k') => {
                if let Some(snapshot) = self.snapshot.as_ref() {
                    self.view.select(snapshot, -1);
                }
            }
            BareKey::Char(c @ '1'..='9') => {
                if let Some(snapshot) = self.snapshot.as_ref() {
                    self.view.select_number(snapshot, (c as u8 - b'1') as usize);
                }
            }
            BareKey::Enter if self.permissions => {
                if let Some(id) = self.view.selected_id.as_deref() {
                    // Resolve a fresh process identity before focusing a recycled pane.
                    self.host("resolve", Some(id), "focus");
                    self.message = "Checking selected pane…".into();
                }
            }
            _ => {}
        }
        self.refresh_output();
    }
}

impl ZellijPlugin for Dashboard {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.started_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        self.collector = configuration.get("mode").map(String::as_str) == Some("collector");
        self.host_path = configuration.get("host_path").cloned().unwrap_or_default();
        self.state_dir = configuration.get("state_dir").cloned().unwrap_or_default();
        if !Path::new(&self.host_path).is_absolute() || !Path::new(&self.state_dir).is_absolute() {
            self.config_error =
                Some("host_path and state_dir must be absolute; use scripts/dashboard.sh".into());
        }
        if !matches!(
            configuration.get("mode").map(String::as_str),
            Some("collector" | "dashboard")
        ) {
            self.config_error = Some("mode must be collector or dashboard".into());
        }
        self.message = "Waiting for permissions".into();
        subscribe(&[
            EventType::Key,
            EventType::Timer,
            EventType::PermissionRequestResult,
            EventType::RunCommandResult,
            EventType::Visible,
            EventType::ModeUpdate,
            EventType::PastedText,
        ]);
        // The initial grant dialog must be selectable, including collectors.
        // Only suppress collector selection after the permission result arrives.
        set_selectable(true);
        // Hiding after requesting permissions would hide the grant dialog too.
        self.request_permissions();
        set_timeout(2.0);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PermissionRequestResult(status) => {
                self.permissions = status == PermissionStatus::Granted;
                self.denied = !self.permissions;
                self.message = if self.permissions {
                    "Loading agent state"
                } else {
                    "Permissions denied"
                }
                .into();
                if self.permissions {
                    let ids = get_plugin_ids();
                    self.plugin_id = Some(ids.plugin_id);
                    self.client_id = Some(ids.client_id);
                    if !self.collector {
                        rename_plugin_pane(ids.plugin_id, "Agent Dashboard");
                    }
                    if self.collector {
                        hide_self();
                        set_selectable(false);
                    }
                    self.refresh();
                }
            }
            Event::Timer(_) => {
                // A cached grant can arrive before the first client attaches.
                // Retry pending requests, but respect an explicit denial.
                if !self.permissions && !self.denied {
                    self.request_permissions();
                }
                self.refresh();
                self.action_timer();
                set_timeout(2.0);
            }
            Event::Visible(true) if !self.collector => self.refresh(),
            Event::ModeUpdate(info) => {
                self.view.source_session = info.session_name.unwrap_or_default()
            }
            Event::Key(key) if !self.collector => self.key(key),
            Event::PastedText(text) if !self.collector => {
                if let Some(menu) = self.view.menu.as_mut() {
                    if menu.editing() && !menu.busy {
                        menu.input.insert(&text, menu.kind == MenuKind::Input);
                        menu.selected = 0;
                    }
                }
            }
            Event::RunCommandResult(code, stdout, stderr, context) => {
                let kind = context.get("kind").map(String::as_str).unwrap_or("");
                if kind == "snapshot" {
                    self.pending = false;
                }
                if kind == "pin" {
                    self.pin_pending = false;
                }
                if kind == "output" {
                    self.output_pending = false;
                }
                if code != Some(0) {
                    if matches!(kind, "action" | "action-status" | "history-result") {
                        self.action_pending = false;
                        self.active_request = None;
                    }
                    if kind == "edited" {
                        self.edited_pending = false;
                    }
                    if let Some(menu) = self.view.menu.as_mut() {
                        menu.busy = false;
                        menu.error = String::from_utf8_lossy(&stderr).trim().into();
                    }
                    if kind != "output" {
                        self.message =
                            format!("Host error: {}", String::from_utf8_lossy(&stderr).trim());
                        eprintln!("{}", self.message);
                    }
                    if kind == "output" && context.get("agent_id") == self.view.selected_id.as_ref()
                    {
                        self.view.output = None;
                    }
                } else if matches!(
                    kind,
                    "catalog"
                        | "action"
                        | "action-status"
                        | "edited"
                        | "requests"
                        | "history-result"
                ) {
                    self.control_result(kind, &stdout, &context);
                } else if kind == "snapshot" {
                    match serde_json::from_slice::<Snapshot>(&stdout) {
                        Ok(snapshot) if snapshot.schema_version == SCHEMA_VERSION => {
                            self.view.reconcile_selection(&snapshot);
                            self.message = if snapshot.now_ms.saturating_sub(snapshot.last_scan_ms)
                                >= 10_000
                            {
                                "Collector offline · showing cached state"
                            } else {
                                "Connected"
                            }
                            .into();
                            self.snapshot = Some(snapshot);
                            self.refresh_output();
                        }
                        Ok(_) => self.message = "Unsupported state schema version".into(),
                        Err(e) => self.message = format!("Invalid host response: {e}"),
                    }
                } else if kind == "pin" {
                    match serde_json::from_slice::<Agent>(&stdout) {
                        Ok(agent) if context.get("agent_id") == Some(&agent.identity.agent_id) => {
                            self.view.pinned_only = agent.pinned;
                            self.view.selected_id = Some(agent.identity.agent_id.clone());
                            if let Some(snapshot) = self.snapshot.as_mut() {
                                if let Some(row) = snapshot
                                    .agents
                                    .iter_mut()
                                    .find(|a| a.identity == agent.identity)
                                {
                                    *row = agent;
                                }
                                self.view.reconcile_selection(snapshot);
                            }
                            self.refresh();
                        }
                        _ => self.message = "Selected agent changed; refresh the list".into(),
                    }
                } else if kind == "output" {
                    if let Ok(output) = serde_json::from_slice::<PaneOutput>(&stdout) {
                        if Some(&output.agent_id) == self.view.selected_id.as_ref()
                            && context.get("agent_id") == Some(&output.agent_id)
                        {
                            self.view.output = Some(output);
                        }
                    }
                } else if kind == "focus" {
                    match serde_json::from_slice::<Agent>(&stdout) {
                        Ok(agent) if context.get("agent_id") == Some(&agent.identity.agent_id) => {
                            switch_session_with_focus(
                                &agent.identity.session_name,
                                None,
                                Some((agent.identity.pane_id, false)),
                            );
                            self.message = "Focused agent pane".into();
                        }
                        _ => self.message = "Selected agent changed; refresh the list".into(),
                    }
                }
                if kind == "snapshot" && self.dirty {
                    self.dirty = false;
                    self.refresh();
                }
                if kind == "output" && self.output_dirty {
                    self.output_dirty = false;
                    self.refresh_output();
                }
            }
            _ => return false,
        }
        !self.collector
    }

    fn pipe(&mut self, message: PipeMessage) -> bool {
        match message.name.as_str() {
            "agent-dashboard-changed" => self.refresh(),
            "agent-dashboard-ping" => {
                if !self.permissions && !self.denied {
                    self.request_permissions();
                }
                if let PipeSource::Cli(pipe_id) = &message.source {
                    let response = serde_json::json!({
                        "mode": if self.collector { "collector" } else { "dashboard" },
                        "plugin_id": self.plugin_id,
                        "client_id": self.client_id,
                        "permissions": self.permissions,
                        "polls": self.polls,
                        "revision": self.snapshot.as_ref().map(|s| s.revision),
                        "selected_id": self.view.selected_id,
                        "query": self.view.query,
                        "pinned_panel":self.view.pinned_only,
                        "instruction_open":self.view.instruction_open,
                        "menu":self.view.menu.as_ref().map(|m|format!("{:?}",m.kind)),
                        "menu_busy":self.view.menu.as_ref().map(|m|m.busy),
                        "output_id":self.view.output.as_ref().map(|o|&o.agent_id),
                        "error": self.config_error,
                    });
                    cli_pipe_output(pipe_id, &format!("{response}\n"));
                    unblock_cli_pipe_input(pipe_id);
                }
            }
            _ => return false,
        }
        !self.collector
    }

    fn render(&mut self, rows: usize, cols: usize) {
        if let Some(error) = self.config_error.as_ref() {
            println!(
                "{}",
                dashboard_core::view::truncate(error, cols.saturating_sub(1))
            );
            return;
        }
        if self.collector && self.permissions {
            return;
        }
        match self.snapshot.as_ref() {
            Some(snapshot) => {
                for line in self.view.render(snapshot, rows, cols, &self.message) {
                    println!("{line}");
                }
            }
            None => {
                for line in ["Agent Dashboard", self.message.as_str()] {
                    println!(
                        "{}",
                        dashboard_core::view::truncate(line, cols.saturating_sub(1))
                    );
                }
            }
        }
    }
}
