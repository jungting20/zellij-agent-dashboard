use dashboard_core::{
    view::View, Agent, Liveness, NavigationAction, NavigationRequest, PaneOutput, PaneVisit,
    Snapshot, SCHEMA_VERSION,
};
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
    preview_due: Option<std::time::Instant>,
    poll_due: Option<std::time::Instant>,
    pin_pending: bool,
    next_queue: dashboard_core::NextQueue,
    next_pending: bool,
    next_check_pending: bool,
    navigation_target: Option<PaneVisit>,
    dashboard_origin: Option<u32>,
    dashboard_navigation_pending: bool,
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
            && !self.view.source_session.is_empty()
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
        self.window_pending = true;
        self.host(
            "open-dashboard",
            Some(&self.view.source_session),
            "open-dashboard",
        );
    }

    fn start_next(&mut self) {
        if self.next_pending
            || !self.permissions
            || self.config_error.is_some()
            || self.view.source_session.is_empty()
        {
            return;
        }
        if self.next_queue.is_empty() || self.next_check_pending {
            return;
        }
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
            self.navigation_target = None;
            return;
        }
        let Some(action) = self.next_queue.pop_front() else {
            return;
        };
        let session = self.view.source_session.clone();
        let pane_id = clients
            .iter()
            .find(|c| Some(c.client_id) == self.client_id)
            .and_then(|c| match c.pane_id {
                PaneId::Terminal(id) => Some(id),
                _ => None,
            });
        let (session, pane_id) = self
            .navigation_target
            .as_ref()
            .map(|target| (target.session.clone(), Some(target.pane_id)))
            .unwrap_or((session, pane_id));
        self.next_pending = true;
        self.navigate(action, session, pane_id, "next");
    }

    fn navigate(
        &self,
        action: NavigationAction,
        session: String,
        pane_id: Option<u32>,
        kind: &str,
    ) {
        let request = NavigationRequest {
            action,
            session,
            pane_id,
        };
        self.host(
            "navigate",
            Some(&serde_json::to_string(&request).unwrap()),
            kind,
        );
    }

    fn navigate_dashboard(&mut self, action: NavigationAction) {
        if self.dashboard_navigation_pending || self.view.source_session.is_empty() {
            return;
        }
        self.dashboard_navigation_pending = true;
        self.message = "Checking target pane…".into();
        self.navigate(
            action,
            self.view.source_session.clone(),
            self.dashboard_origin,
            "focus",
        );
    }

    fn focus_visit(&mut self, target: &PaneVisit) {
        if target.session == self.view.source_session {
            focus_terminal_pane(target.pane_id, true, false);
        } else {
            switch_session_with_focus(&target.session, None, Some((target.pane_id, false)));
        }
        self.message = "Focused terminal pane".into();
    }

    fn finish_navigation_step(&mut self) {
        // A burst is resolved before switching sessions. The newly attached
        // instance handles the next key after the switch.
        if self.next_queue.is_empty() {
            if let Some(target) = self.navigation_target.take() {
                self.focus_visit(&target);
            }
        } else {
            self.start_next();
        }
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
        self.output_dirty = false;
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
        let previous_selection = self.view.selected_id.clone();
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
            BareKey::Char('Q') => self.open_menu(MenuKind::Queue),
            BareKey::Char('e') => self.open_neovim(),
            BareKey::Char('I') => {
                self.open_menu(MenuKind::Input);
                self.edit_instruction();
            }
            BareKey::Char('a') => self.open_menu(MenuKind::Add),
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
                let target = self.snapshot.as_ref().and_then(|snapshot| {
                    snapshot.agents.iter().find(|agent| {
                        Some(&agent.identity.agent_id) == self.view.selected_id.as_ref()
                    })
                });
                if let Some(agent) = target {
                    self.navigate_dashboard(NavigationAction::Select(agent.identity.clone()));
                }
            }
            BareKey::Char('b') if self.permissions => {
                self.navigate_dashboard(NavigationAction::Previous);
            }
            _ => {}
        }
        if self.view.selected_id != previous_selection {
            self.view.output = None;
            let already_scheduled = self.preview_due.is_some();
            self.preview_due =
                Some(std::time::Instant::now() + std::time::Duration::from_millis(80));
            if !already_scheduled {
                set_timeout(0.08);
            }
        }
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
            EventType::SessionUpdate,
            EventType::ListClients,
            EventType::PastedText,
        ]);
        if !self.collector {
            subscribe(&[EventType::PaneUpdate]);
        }
        // The initial grant dialog must be selectable, including collectors.
        // Only suppress collector selection after the permission result arrives.
        set_selectable(true);
        // Hiding after requesting permissions would hide the grant dialog too.
        self.request_permissions();
        self.poll_due = Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
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
                    self.start_next();
                    self.start_window();
                }
            }
            Event::Timer(_) => {
                let now = std::time::Instant::now();
                if self.poll_due.is_some_and(|due| now >= due) {
                    // Preview timers must not multiply the periodic polling loop.
                    self.poll_due = Some(now + std::time::Duration::from_secs(2));
                    if !self.permissions && !self.denied {
                        self.request_permissions();
                    }
                    self.refresh();
                    self.action_timer();
                    set_timeout(2.0);
                }
                if let Some(due) = self.preview_due {
                    if now >= due {
                        self.preview_due = None;
                        self.refresh_output();
                    } else {
                        set_timeout(due.duration_since(now).as_secs_f64());
                    }
                }
                // Commands update the visible state when their results arrive.
                return false;
            }
            Event::Visible(true) if !self.collector => self.refresh(),
            Event::ModeUpdate(info) => {
                if let Some(session) = info.session_name {
                    self.view.source_session = session;
                }
                self.start_next();
                self.start_window();
                return false;
            }
            Event::SessionUpdate(sessions, _) => {
                if let Some(session) = sessions.iter().find(|s| s.is_current_session) {
                    self.view.source_session = session.name.clone();
                }
                self.start_next();
                self.start_window();
                return false;
            }
            Event::ListClients(clients) => {
                self.window_clients(&clients);
                self.navigation_clients(clients);
                return false;
            }
            Event::PaneUpdate(manifest) if !self.collector => {
                self.dashboard_origin = manifest.panes.values().find_map(|panes| {
                    if !panes.iter().any(|pane| {
                        pane.is_plugin && pane.is_floating && Some(pane.id) == self.plugin_id
                    }) {
                        return None;
                    }
                    panes
                        .iter()
                        .find(|pane| {
                            pane.is_focused && !pane.is_plugin && !pane.is_floating && !pane.exited
                        })
                        .map(|pane| pane.id)
                });
                return false;
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
                if kind == "focus" {
                    self.dashboard_navigation_pending = false;
                }
                if code != Some(0) {
                    if kind == "next" {
                        self.next_pending = false;
                        self.next_queue.clear();
                        self.finish_navigation_step();
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
                    match serde_json::from_slice::<Option<PaneVisit>>(&stdout) {
                        Ok(Some(target)) => self.navigation_target = Some(target),
                        Ok(None) => (),
                        Err(e) => {
                            self.next_queue.clear();
                            self.message = format!("Invalid navigation response: {e}");
                        }
                    }
                    self.finish_navigation_step();
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
                            self.refresh_queue_menu();
                            if self.preview_due.is_none() {
                                self.refresh_output();
                            }
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
                    match serde_json::from_slice::<Option<PaneVisit>>(&stdout) {
                        Ok(Some(target)) => {
                            self.focus_visit(&target);
                            close_self();
                        }
                        Ok(None) => self.message = "No previous pane in navigation history".into(),
                        Err(e) => self.message = format!("Invalid navigation response: {e}"),
                    }
                }
                if kind == "snapshot" && self.dirty {
                    self.dirty = false;
                    self.refresh();
                }
                if kind == "output" && self.output_dirty && self.preview_due.is_none() {
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
            "agent-next" | "agent-prev" if self.collector => {
                self.next_messages += 1;
                self.next_source = Some(format!(
                    "{:?}, private={}",
                    message.source, message.is_private
                ));
                if !message.is_private {
                    return false;
                }
                let action = if message.name == "agent-prev" {
                    Ok(NavigationAction::Previous)
                } else {
                    dashboard_core::NextFilter::parse(message.payload.as_deref().unwrap_or(""))
                        .map(NavigationAction::Next)
                };
                match action {
                    Ok(action) => {
                        if !self.next_queue.push_back(action) {
                            self.message = "Navigation queue full; extra key ignored".into();
                        }
                        self.start_next();
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
                        "session_events": true,
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
        // Pipe requests only start asynchronous work or report diagnostics.
        false
    }

    fn render(&mut self, rows: usize, cols: usize) {
        if self.collector && self.permissions && self.config_error.is_none() {
            return;
        }
        let lines = if let Some(error) = self.config_error.as_ref() {
            vec![dashboard_core::view::truncate(
                error,
                cols.saturating_sub(1),
            )]
        } else if let Some(snapshot) = self.snapshot.as_ref() {
            self.view.render(snapshot, rows, cols, &self.message)
        } else {
            ["Agent Dashboard", self.message.as_str()]
                .into_iter()
                .map(|line| dashboard_core::view::truncate(line, cols.saturating_sub(1)))
                .collect()
        };
        print!("{}", crate::render::frame(&lines, rows));
    }
}
