use dashboard_core::{view::View, Agent, Liveness, PaneOutput, Snapshot, SCHEMA_VERSION};
use std::{
    collections::{BTreeMap, VecDeque},
    path::Path,
};
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
    next_queue: VecDeque<dashboard_core::NextFilter>,
    next_pending: bool,
    next_check_pending: bool,
    next_origin: Option<(String, u32)>,
    next_messages: u64,
    next_source: Option<String>,
    window_requested: bool,
    window_check_pending: bool,
    window_pending: bool,
}

impl Dashboard {
    fn start_window(&mut self) {
        if self.window_requested
            && self.permissions
            && self.config_error.is_none()
            && !self.window_check_pending
            && !self.window_pending
        {
            self.window_check_pending = true;
            list_clients();
        }
    }

    fn window_clients(&mut self, clients: &[ClientInfo]) {
        if !self.window_check_pending {
            return;
        }
        self.window_check_pending = false;
        self.window_requested = false;
        if clients.iter().map(|c| c.client_id).min() != self.client_id {
            return;
        }
        let session = get_session_list().ok().and_then(|s| {
            s.live_sessions
                .into_iter()
                .find(|s| s.is_current_session)
                .map(|s| s.name)
        });
        if let Some(session) = session {
            self.window_pending = true;
            self.host("open-dashboard", Some(&session), "open-dashboard");
        }
    }

    fn start_next(&mut self, origin: Option<(String, u32)>) {
        if self.next_pending || !self.permissions || self.config_error.is_some() {
            return;
        }
        if self.next_queue.is_empty() || self.next_check_pending {
            return;
        }
        self.next_origin = origin;
        self.next_check_pending = true;
        list_clients();
    }

    fn navigation_clients(&mut self, clients: Vec<ClientInfo>) {
        if !self.next_check_pending {
            return;
        }
        self.next_check_pending = false;
        // Keybind pipes reach retained and other-client instances too. Elect
        // one currently attached client before starting any host command.
        if clients.iter().map(|c| c.client_id).min() != self.client_id {
            self.next_queue.clear();
            self.next_origin = None;
            return;
        }
        let Some(filter) = self.next_queue.pop_front() else {
            return;
        };
        let session = get_session_list().ok().and_then(|s| {
            s.live_sessions
                .into_iter()
                .find(|s| s.is_current_session)
                .map(|s| s.name)
        });
        let Some(session) = session else {
            self.next_queue.clear();
            self.message = "Cannot determine current Zellij session".into();
            return;
        };
        self.view.source_session = session.clone();
        let pane_id = clients
            .iter()
            .find(|c| Some(c.client_id) == self.client_id)
            .and_then(|c| match c.pane_id {
                PaneId::Terminal(id) => Some(id),
                _ => None,
            });
        let (session, pane_id) = self
            .next_origin
            .take()
            .map(|(s, p)| (s, Some(p)))
            .unwrap_or((session, pane_id));
        let request = dashboard_core::NextRequest {
            filter,
            session,
            pane_id,
        };
        self.next_pending = true;
        self.host(
            "next",
            Some(&serde_json::to_string(&request).unwrap()),
            "next",
        );
    }

    fn focus_agent(&mut self, agent: &Agent) {
        if agent.identity.session_name == self.view.source_session {
            focus_terminal_pane(agent.identity.pane_id, true, false);
        } else {
            switch_session_with_focus(
                &agent.identity.session_name,
                None,
                Some((agent.identity.pane_id, false)),
            );
        }
        self.message = "Focused agent pane".into();
    }
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
        let mut argv = vec![command];
        if let Some(arg) = argument {
            argv.push(arg);
        }
        let mut context = BTreeMap::from([("kind".into(), kind.into())]);
        if let Some(arg) = argument {
            context.insert("agent_id".into(), arg.into());
        }
        self.run_host(&argv, context);
    }

    fn run_host(&self, arguments: &[&str], context: BTreeMap<String, String>) {
        // Expand home paths in the host environment, not the WASI filesystem.
        // Configuration and payloads remain positional arguments, never shell code.
        let mut argv = vec![
            "/bin/sh",
            "-c",
            r#"host=$1; state=$2; shift 2
case "$host" in '~/'*) host="$HOME/${host#\~/}" ;; esac
case "$state" in '~/'*) state="$HOME/${state#\~/}" ;; esac
if [ -n "$state" ]; then
    exec "$host" --state-dir "$state" "$@"
else
    exec "$host" "$@"
fi"#,
            "dashboard-host",
            &self.host_path,
            &self.state_dir,
        ];
        argv.extend_from_slice(arguments);
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
        self.run_host(&["pin", &agent.identity.agent_id, desired], context);
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
                    self.view.select_next_working(snapshot);
                }
            }
            BareKey::Left | BareKey::Char('h') => {
                if let Some(snapshot) = self.snapshot.as_ref() {
                    self.view.focus_panel(snapshot, true);
                }
            }
            BareKey::Right | BareKey::Char('l') => {
                if let Some(snapshot) = self.snapshot.as_ref() {
                    self.view.focus_panel(snapshot, false);
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
        self.host_path = configuration
            .get("host_path")
            .cloned()
            .unwrap_or_else(|| "~/.config/zellij/plugins/dashboard-host".into());
        self.state_dir = configuration.get("state_dir").cloned().unwrap_or_default();
        let valid_path = |path: &str| Path::new(path).is_absolute() || path.starts_with("~/");
        if !valid_path(&self.host_path)
            || (!self.state_dir.is_empty() && !valid_path(&self.state_dir))
        {
            self.config_error =
                Some("host_path and state_dir must be absolute or start with ~/".into());
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
            EventType::ListClients,
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
                    self.start_next(None);
                    self.start_window();
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
            Event::ListClients(clients) => {
                self.window_clients(&clients);
                self.navigation_clients(clients);
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
                if kind == "open-dashboard" {
                    self.window_pending = false;
                }
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
                    if kind == "next" {
                        self.next_pending = false;
                        self.next_queue.clear();
                    }
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
                } else if kind == "next" {
                    self.next_pending = false;
                    match serde_json::from_slice::<Option<Agent>>(&stdout) {
                        Ok(Some(agent)) => {
                            // Finish a burst before switching sessions: detached
                            // instances must not execute the remaining keys.
                            if self.next_queue.is_empty() {
                                self.focus_agent(&agent);
                            }
                            self.start_next(Some((
                                agent.identity.session_name,
                                agent.identity.pane_id,
                            )));
                        }
                        Ok(None) => self.start_next(None),
                        Err(e) => {
                            self.next_queue.clear();
                            self.message = format!("Invalid navigation response: {e}");
                        }
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
                            self.focus_agent(&agent);
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
            "agent-dashboard-open" if self.collector && message.is_private => {
                if !self.window_pending {
                    self.window_requested = true;
                    self.start_window();
                }
            }
            "agent-next" if self.collector => {
                self.next_messages += 1;
                self.next_source = Some(format!(
                    "{:?}, private={}",
                    message.source, message.is_private
                ));
                if !message.is_private {
                    return false;
                }
                match dashboard_core::NextFilter::parse(message.payload.as_deref().unwrap_or("")) {
                    Ok(filter) => {
                        self.next_queue.push_back(filter);
                        self.start_next(None);
                    }
                    Err(error) => eprintln!("{error}"),
                }
            }
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
                        "next_messages": self.next_messages,
                        "next_source": self.next_source,
                        "next_pending": self.next_pending,
                        "next_check_pending": self.next_check_pending,
                        "next_queued": self.next_queue.len(),
                        "message": self.message,
                        "source_session": self.view.source_session,
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
