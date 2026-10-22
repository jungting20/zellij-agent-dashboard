use super::*;
use dashboard_core::{
    menu::{Menu, MenuKind, TextInput, ALIASES},
    Action, ActionRequest, ActionResult, Catalog, Identity, RequestState,
};

impl Dashboard {
    pub(super) fn refresh_queue_menu(&mut self) {
        let Some(menu) = self
            .view
            .menu
            .as_mut()
            .filter(|m| m.kind == MenuKind::Queue && !m.busy)
        else {
            return;
        };
        let Some(agent) = self.snapshot.as_ref().and_then(|s| {
            s.agents
                .iter()
                .find(|a| Some(&a.identity) == menu.target.as_ref())
        }) else {
            return;
        };
        let chosen = menu.chosen();
        menu.options.clear();
        if let Some(delivery) = &agent.instruction_queue.delivery {
            menu.options.push((
                format!(
                    "[{}] {}",
                    delivery.message,
                    delivery.instruction.text.lines().next().unwrap_or("")
                ),
                delivery.instruction.id.clone(),
            ));
        }
        menu.options
            .extend(
                agent
                    .instruction_queue
                    .pending
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        (
                            format!(
                                "대기 {} · {}",
                                index + 1,
                                item.text.lines().next().unwrap_or("")
                            ),
                            item.id.clone(),
                        )
                    }),
            );
        menu.selected = chosen
            .and_then(|id| menu.options.iter().position(|(_, value)| *value == id))
            .unwrap_or(0);
        menu.error = if menu.options.is_empty() {
            "예약된 입력이 없습니다".into()
        } else {
            String::new()
        };
    }

    pub(super) fn open_neovim(&mut self) {
        if !self.permissions || self.action_pending || self.active_request.is_some() {
            return;
        }
        if self.view.source_session.is_empty() {
            self.message = "대시보드의 Zellij 세션을 확인할 수 없습니다".into();
            return;
        }
        let target = self.snapshot.as_ref().and_then(|s| {
            s.agents
                .iter()
                .find(|a| Some(&a.identity.agent_id) == self.view.selected_id.as_ref())
                .map(|a| a.identity.clone())
        });
        let Some(target) = target else {
            self.message = "선택한 에이전트가 없습니다".into();
            return;
        };
        self.submit(Action::Neovim {
            target,
            session: self.view.source_session.clone(),
        });
    }

    pub(super) fn open_menu(&mut self, kind: MenuKind) {
        if !self.permissions || self.action_pending {
            return;
        }
        let agent = self
            .snapshot
            .as_ref()
            .and_then(|s| {
                s.agents
                    .iter()
                    .find(|a| Some(&a.identity.agent_id) == self.view.selected_id.as_ref())
            })
            .cloned();
        if kind != MenuKind::Recent && kind != MenuKind::Requests && agent.is_none() {
            self.message = "선택한 에이전트가 없습니다".into();
            return;
        }
        let title = match kind {
            MenuKind::Input => "프롬프트 · working 중 자동 예약",
            MenuKind::Queue => "예약 입력 큐",
            MenuKind::QueueResume => "예약 재개 확인",
            MenuKind::Alias => "태그",
            MenuKind::Recent => "새 에이전트",
            MenuKind::Worktree => "Worktree",
            MenuKind::Requests => "최근 조작 결과 · 에디터 복원",
            _ => "작업",
        };
        let mut menu = Menu::new(kind, title, agent.as_ref().map(|a| a.identity.clone()));
        if kind == MenuKind::Alias {
            menu.options = ALIASES
                .iter()
                .map(|(l, v)| (l.to_string(), v.to_string()))
                .collect();
            menu.selected = agent
                .as_ref()
                .and_then(|a| ALIASES.iter().position(|(_, v)| *v == a.alias))
                .unwrap_or(ALIASES.len() - 1);
            menu.input = TextInput::new(agent.as_ref().unwrap().alias.clone());
        }
        if kind == MenuKind::Worktree {
            menu.directory = agent.as_ref().unwrap().cwd.clone();
            menu.options = vec![
                ("a: 추가".into(), "add".into()),
                ("s: 자식 worktree 셸 명령".into(), "shell".into()),
                ("m: 병합 지시".into(), "merge".into()),
                ("t: 자식 이동".into(), "children".into()),
                ("g: lazygit".into(), "lazygit".into()),
            ];
        }
        if matches!(kind, MenuKind::Recent | MenuKind::Worktree) {
            self.host("catalog", None, "catalog");
            if kind == MenuKind::Recent {
                menu.busy = true;
            }
        }
        if kind == MenuKind::Requests {
            menu.busy = true;
            self.host("requests", None, "requests");
        }
        self.view.menu = Some(menu);
        self.refresh_queue_menu();
    }

    pub(super) fn submit(&mut self, action: Action) {
        if self.action_pending || self.active_request.is_some() {
            return;
        }
        self.request_serial += 1;
        let request = ActionRequest {
            request_id: format!(
                "{}:{}:{}:{}",
                self.plugin_id.unwrap_or_default(),
                self.client_id.unwrap_or_default(),
                self.started_ns,
                self.request_serial
            ),
            action,
        };
        let Ok(payload) = serde_json::to_string(&request) else {
            return;
        };
        let context = BTreeMap::from([
            ("kind".into(), "action".into()),
            ("request_id".into(), request.request_id.clone()),
        ]);
        self.run_host(&["action", &payload], context);
        if let Some(menu) = self.view.menu.as_mut() {
            menu.busy = true;
            menu.request_id = Some(request.request_id.clone());
            menu.error.clear();
        }
        self.active_request = Some(request);
        self.action_pending = true;
        self.message = "처리 중…".into();
    }

    pub(super) fn action_timer(&mut self) {
        if self.collector || !self.permissions {
            return;
        }
        if !self.action_pending {
            if let Some(request) = &self.active_request {
                self.host("result", Some(&request.request_id), "action-status");
                self.action_pending = true;
            }
        }
        if !self.edited_pending {
            if let Some(id) = &self.editor_request {
                self.host("edited", Some(id), "edited");
                self.edited_pending = true;
            }
        }
    }

    fn children_menu(&mut self, merge: bool) {
        let Some(menu) = self.view.menu.as_mut() else {
            return;
        };
        let Some(parent) = &menu.target else {
            return;
        };
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        menu.kind = if merge {
            MenuKind::Merge
        } else {
            MenuKind::Children
        };
        menu.title = if merge {
            "병합할 자식 worktree 선택"
        } else {
            "자식 worktree 이동"
        }
        .into();
        menu.selected = 0;
        menu.options = snapshot
            .agents
            .iter()
            .filter(|a| {
                a.parent_id.as_ref() == Some(&parent.agent_id) && a.liveness == Liveness::Live
            })
            .map(|a| {
                (
                    format!("{} [{}] {}", a.project(), a.status.label(), a.cwd),
                    a.identity.agent_id.clone(),
                )
            })
            .collect();
        if menu.options.is_empty() {
            menu.error = "자식 worktree 에이전트가 없습니다".into();
        }
    }

    pub(super) fn open_merge(&mut self) {
        self.open_menu(MenuKind::Worktree);
        self.children_menu(true);
    }

    pub(super) fn edit_instruction(&mut self) {
        let Some(menu) = self.view.menu.as_ref() else {
            return;
        };
        let Some(target) = menu.target.clone() else {
            return;
        };
        self.submit(Action::Editor {
            target,
            text: menu.input.text.clone(),
        });
    }

    pub(super) fn menu_key(&mut self, key: KeyWithModifier) {
        let Some(menu) = self.view.menu.as_mut() else {
            return;
        };
        if key.bare_key == BareKey::Esc
            || (key.bare_key == BareKey::Char('c')
                && key.key_modifiers.contains(&KeyModifier::Ctrl))
        {
            self.view.menu = None;
            return;
        }
        if menu.busy {
            return;
        }
        if menu.kind == MenuKind::Result {
            match key.bare_key {
                BareKey::Up | BareKey::Char('k') => menu.offset = menu.offset.saturating_sub(1),
                BareKey::Down | BareKey::Char('j') => menu.offset = menu.offset.saturating_add(1),
                BareKey::PageUp => menu.offset = menu.offset.saturating_sub(10),
                BareKey::PageDown => menu.offset = menu.offset.saturating_add(10),
                BareKey::Home => menu.offset = 0,
                _ => {}
            }
            return;
        }
        if menu.kind == MenuKind::Input
            && key.bare_key == BareKey::Char('e')
            && key.key_modifiers.contains(&KeyModifier::Ctrl)
        {
            self.edit_instruction();
            return;
        }
        if menu.kind == MenuKind::Input
            && ((key.bare_key == BareKey::Enter
                && (key.key_modifiers.contains(&KeyModifier::Alt)
                    || key.key_modifiers.contains(&KeyModifier::Shift)))
                || (key.bare_key == BareKey::Char('j')
                    && key.key_modifiers.contains(&KeyModifier::Ctrl)))
        {
            menu.input.insert("\n", true);
            return;
        }
        if menu.kind == MenuKind::Queue {
            if key.bare_key == BareKey::Char('d') {
                if let (Some(target), Some(queued_id)) = (menu.target.clone(), menu.chosen()) {
                    self.submit(Action::CancelQueued { target, queued_id });
                }
                return;
            }
            if key.bare_key == BareKey::Char('r') {
                let uncertain = self
                    .snapshot
                    .as_ref()
                    .and_then(|s| {
                        s.agents
                            .iter()
                            .find(|a| Some(&a.identity) == menu.target.as_ref())
                    })
                    .and_then(|a| a.instruction_queue.delivery.as_ref())
                    .is_some_and(|d| d.state == dashboard_core::DeliveryState::Uncertain);
                if !uncertain {
                    menu.error = "확인이 필요한 전송이 없습니다".into();
                    return;
                }
                menu.kind = MenuKind::QueueResume;
                menu.title = "전송 여부를 직접 확인한 뒤 재개하세요".into();
                menu.options = vec![
                    ("돌아가기".into(), "no".into()),
                    (
                        "확인 완료 · 불명확한 항목 재전송 없이 다음부터 재개".into(),
                        "yes".into(),
                    ),
                ];
                menu.selected = 0;
                return;
            }
        }
        let shortcut = if menu.kind == MenuKind::Worktree {
            match key.bare_key {
                BareKey::Char('a') => Some("add"),
                BareKey::Char('s') => Some("shell"),
                BareKey::Char('m') => Some("merge"),
                BareKey::Char('t') => Some("children"),
                BareKey::Char('g') => Some("lazygit"),
                _ => None,
            }
        } else {
            None
        };
        if key.bare_key == BareKey::Enter || shortcut.is_some() {
            let kind = menu.kind;
            let target = menu.target.clone();
            let chosen = shortcut.map(String::from).or_else(|| menu.chosen());
            match kind {
                MenuKind::Queue => {
                    let Some(id) = chosen else { return };
                    let text = self
                        .snapshot
                        .as_ref()
                        .and_then(|s| {
                            s.agents.iter().find(|a| {
                                Some(&a.identity.agent_id) == target.as_ref().map(|t| &t.agent_id)
                            })
                        })
                        .and_then(|a| {
                            a.instruction_queue
                                .pending
                                .iter()
                                .find(|i| i.id == id)
                                .or_else(|| {
                                    a.instruction_queue
                                        .delivery
                                        .as_ref()
                                        .map(|d| &d.instruction)
                                        .filter(|i| i.id == id)
                                })
                        })
                        .map(|i| i.text.clone())
                        .unwrap_or_else(|| {
                            "예약 항목이 변경되었습니다. Q로 다시 확인하세요".into()
                        });
                    menu.kind = MenuKind::Result;
                    menu.title = "예약 입력 내용".into();
                    menu.result = text;
                    menu.options.clear();
                }
                MenuKind::QueueResume => {
                    if chosen.as_deref() == Some("yes") {
                        if let Some(target) = target {
                            self.submit(Action::ResumeQueue { target });
                        }
                    } else {
                        self.view.menu = None;
                    }
                }
                MenuKind::Input => {
                    let text = menu.input.text.clone();
                    if !text.trim().is_empty() {
                        if let Some(target) = target {
                            self.submit(Action::Input { target, text });
                        }
                    }
                }
                MenuKind::Alias => {
                    let Some(alias) = chosen else {
                        return;
                    };
                    if alias == "custom" {
                        menu.kind = MenuKind::AliasCustom;
                        menu.options.clear();
                    } else if let Some(target) = target {
                        self.submit(Action::Alias { target, alias });
                    }
                }
                MenuKind::AliasCustom => {
                    let alias = menu.input.text.clone();
                    if let Some(target) = target {
                        self.submit(Action::Alias { target, alias });
                    }
                }
                MenuKind::Recent => {
                    let directory = if menu.input.text.starts_with('/') {
                        menu.input.text.clone()
                    } else {
                        chosen.unwrap_or_default()
                    };
                    if directory.is_empty() {
                        menu.error = "경로를 입력하거나 선택하세요".into();
                        return;
                    }
                    menu.directory = directory;
                    menu.kind = MenuKind::Tool;
                    menu.options = menu
                        .catalog
                        .tools
                        .iter()
                        .map(|t| (t.clone(), t.clone()))
                        .collect();
                    menu.selected = 0;
                    if menu.options.is_empty() {
                        menu.error = "설치된 에이전트가 없습니다".into();
                    }
                }
                MenuKind::Tool => {
                    let Some(tool) = chosen else {
                        return;
                    };
                    let directory = menu.directory.clone();
                    let session = if self.view.source_session.is_empty() {
                        target
                            .as_ref()
                            .map(|t| t.session_name.clone())
                            .unwrap_or_default()
                    } else {
                        self.view.source_session.clone()
                    };
                    let Some(epoch) = menu
                        .catalog
                        .sessions
                        .iter()
                        .find(|(s, _)| s == &session)
                        .map(|(_, e)| e.clone())
                    else {
                        menu.error = "실행할 Zellij 세션이 없습니다".into();
                        return;
                    };
                    self.submit(Action::Launch {
                        session,
                        epoch,
                        cwd: directory,
                        tool,
                    });
                }
                MenuKind::Worktree => match chosen.as_deref() {
                    Some("add") => {
                        menu.kind = MenuKind::WorktreeName;
                        menu.title = "Worktree 브랜치명".into();
                        menu.options.clear();
                        menu.input = TextInput::default();
                    }
                    Some("shell") => {
                        menu.kind = MenuKind::Shell;
                        menu.title = "자식 worktree 셸 명령".into();
                        menu.options.clear();
                        menu.input = TextInput::default();
                    }
                    Some("merge") => self.children_menu(true),
                    Some("children") => self.children_menu(false),
                    Some("lazygit") => {
                        if let Some(target) = target {
                            if self.view.source_session.is_empty() {
                                menu.error = "대시보드의 Zellij 세션을 확인할 수 없습니다".into();
                                return;
                            }
                            self.submit(Action::Lazygit {
                                target,
                                session: Some(self.view.source_session.clone()),
                            });
                        }
                    }
                    _ => {}
                },
                MenuKind::WorktreeName => {
                    if menu.input.text.trim().is_empty() {
                        menu.error = "브랜치명을 입력하세요".into();
                        return;
                    }
                    menu.branch = menu.input.text.clone();
                    menu.kind = MenuKind::WorktreeTool;
                    menu.title = "Worktree 에이전트 종류".into();
                    menu.options = menu
                        .catalog
                        .tools
                        .iter()
                        .map(|t| (t.clone(), t.clone()))
                        .collect();
                    menu.selected = 0;
                }
                MenuKind::WorktreeTool => {
                    if let (Some(parent), Some(tool)) = (target, chosen) {
                        let branch = menu.branch.clone();
                        self.submit(Action::Worktree {
                            parent,
                            branch,
                            tool,
                        });
                    }
                }
                MenuKind::Shell => {
                    if let Some(parent) = target {
                        let command = menu.input.text.clone();
                        if !command.trim().is_empty() {
                            self.submit(Action::ShellChildren { parent, command });
                        }
                    }
                }
                MenuKind::Children | MenuKind::Merge => {
                    let child = chosen.and_then(|id| {
                        self.snapshot
                            .as_ref()?
                            .agents
                            .iter()
                            .find(|a| a.identity.agent_id == id)
                            .map(|a| a.identity.clone())
                    });
                    if let Some(child) = child {
                        if kind == MenuKind::Children {
                            self.host("resolve", Some(&child.agent_id), "focus");
                            self.view.menu = None;
                        } else if let Some(parent) = target {
                            self.submit(Action::Merge { parent, child });
                        }
                    }
                }
                MenuKind::Requests => {
                    if let Some(id) = chosen {
                        menu.busy = true;
                        self.host("result", Some(&id), "history-result");
                    }
                }
                _ => {}
            }
            return;
        }
        let editing = menu.editing();
        match key.bare_key {
            BareKey::Down => menu.move_selection(1),
            BareKey::Up => menu.move_selection(-1),
            BareKey::Char('j') if !editing => menu.move_selection(1),
            BareKey::Char('k') if !editing => menu.move_selection(-1),
            BareKey::Backspace if editing => menu.input.backspace(),
            BareKey::Delete if editing => menu.input.delete(),
            BareKey::Left if editing => menu.input.left(),
            BareKey::Right if editing => menu.input.right(),
            BareKey::Home if editing => menu.input.cursor = 0,
            BareKey::End if editing => menu.input.cursor = menu.input.text.len(),
            BareKey::Char(c)
                if editing
                    && !key.key_modifiers.contains(&KeyModifier::Ctrl)
                    && !key.key_modifiers.contains(&KeyModifier::Alt) =>
            {
                menu.input
                    .insert(&c.to_string(), menu.kind == MenuKind::Input);
                menu.selected = 0;
            }
            _ => {}
        }
    }

    pub(super) fn control_result(
        &mut self,
        kind: &str,
        stdout: &[u8],
        context: &BTreeMap<String, String>,
    ) {
        match kind {
            "catalog" => {
                if let (Ok(catalog), Some(menu)) = (
                    serde_json::from_slice::<Catalog>(stdout),
                    self.view.menu.as_mut(),
                ) {
                    if matches!(
                        menu.kind,
                        MenuKind::Recent
                            | MenuKind::Worktree
                            | MenuKind::WorktreeName
                            | MenuKind::WorktreeTool
                    ) {
                        menu.busy = false;
                        if menu.kind == MenuKind::Recent {
                            menu.options = catalog
                                .directories
                                .iter()
                                .map(|p| (p.clone(), p.clone()))
                                .collect();
                        }
                        menu.catalog = catalog;
                    }
                }
            }
            "requests" => {
                if let (Ok(records), Some(menu)) = (
                    serde_json::from_slice::<Vec<ActionResult>>(stdout),
                    self.view.menu.as_mut(),
                ) {
                    if menu.kind == MenuKind::Requests {
                        menu.busy = false;
                        menu.options = records
                            .iter()
                            .map(|r| {
                                (
                                    format!(
                                        "{:?} {} · {}",
                                        r.state,
                                        r.request.request_id,
                                        r.message.lines().next().unwrap_or("")
                                    ),
                                    r.request.request_id.clone(),
                                )
                            })
                            .collect();
                    }
                }
            }
            "edited" => {
                self.edited_pending = false;
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(stdout) {
                    if value["ready"] == true {
                        if let (Ok(target), Some(text)) = (
                            serde_json::from_value::<Identity>(value["target"].clone()),
                            value["text"].as_str(),
                        ) {
                            let mut menu =
                                Menu::new(MenuKind::Input, "편집한 프롬프트", Some(target));
                            menu.input = TextInput::new(text.into());
                            self.view.menu = Some(menu);
                            self.editor_request = None;
                        }
                    }
                }
            }
            "action" | "action-status" | "history-result" => {
                self.action_pending = false;
                let Ok(result) = serde_json::from_slice::<ActionResult>(stdout) else {
                    return;
                };
                if kind == "action" && context.get("request_id") != Some(&result.request.request_id)
                {
                    return;
                }
                if kind == "action-status"
                    && self.active_request.as_ref().map(|r| &r.request_id)
                        != Some(&result.request.request_id)
                {
                    return;
                }
                if result.state == RequestState::Pending {
                    self.message = "조작 결과 확인 중 · 자동 재전송 없음".into();
                    return;
                }
                self.active_request = None;
                self.message = result.message.clone();
                if matches!(result.request.action, Action::Editor { .. })
                    && result.state == RequestState::Succeeded
                {
                    self.editor_request = Some(result.request.request_id.clone());
                    self.view.menu = None;
                    return;
                }
                if kind == "history-result"
                    || result.state != RequestState::Succeeded
                    || matches!(result.request.action, Action::ShellChildren { .. })
                {
                    let mut menu = Menu::new(
                        MenuKind::Result,
                        "조작 결과",
                        result.request.action.target().cloned(),
                    );
                    menu.result = format!(
                        "{}\n{:?}\n{}\n{}",
                        result.request.request_id, result.state, result.message, result.path
                    );
                    self.view.menu = Some(menu);
                } else {
                    self.view.menu = None;
                }
                self.refresh();
            }
            _ => {}
        }
    }
}
