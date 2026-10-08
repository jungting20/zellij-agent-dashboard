use crate::{
    command::{self, CommandSpec},
    host::{pane_id, pane_number, HostDependencies},
    panes, process,
    repository::now_ms,
    terminal::{NewPane, SessionId},
};
use dashboard_core::{
    Action, ActionRequest, ActionResult, Agent, Catalog, Identity, LaunchInfo, Liveness,
    RequestState, StateSignal,
};
use std::{
    env, fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::Duration,
};

const TOOLS: &[(&str, &str)] = &[
    ("claude", "claude"),
    ("codex", "codex"),
    ("cursor", "agent"),
    ("gemini", "agy"),
    ("hermes", "hermes"),
    ("pi", "pi"),
];

fn executable(name: &str) -> Result<PathBuf, String> {
    let mut paths: Vec<PathBuf> =
        env::split_paths(&env::var_os("PATH").unwrap_or_default()).collect();
    if let Some(home) = env::var_os("HOME") {
        let home = PathBuf::from(home);
        paths.extend([
            home.join(".local/bin"),
            home.join("Library/pnpm"),
            home.join(".cargo/bin"),
        ]);
    }
    paths.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/bin"),
    ]);
    paths
        .iter()
        .map(|dir| dir.join(name))
        .find(|path| {
            fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
        .ok_or_else(|| format!("{name} 실행 파일을 찾을 수 없습니다"))
}

pub fn catalog(deps: &HostDependencies) -> Result<Catalog, String> {
    let store = deps.repository.catalog()?;
    let inventory = process::inventory(deps.runner)?;
    let mut directories = store.directories;
    directories.extend(store.agents.iter().map(|a| a.cwd.clone()));
    if let Ok(bytes) = command::output(
        deps.runner,
        CommandSpec::new("zoxide", Some(Duration::from_millis(500)), 128 * 1024)
            .args(["query", "--list"]),
    ) {
        directories.extend(String::from_utf8_lossy(&bytes).lines().map(String::from));
    }
    let mut seen = std::collections::BTreeSet::new();
    directories
        .retain(|p| Path::new(p).is_absolute() && Path::new(p).is_dir() && seen.insert(p.clone()));
    Ok(Catalog {
        directories,
        tools: TOOLS
            .iter()
            .filter(|(_, exe)| executable(exe).is_ok())
            .map(|(tool, _)| (*tool).into())
            .collect(),
        sessions: inventory.epochs.into_iter().collect(),
    })
}

pub fn link_launches(store: &mut dashboard_core::Store, inventory: &process::Inventory) {
    let observations: Vec<_> = store
        .agents
        .values()
        .filter_map(|agent| {
            let process = inventory
                .processes
                .iter()
                .find(|p| p.pid == agent.identity.pid)?;
            let id = process::env_value(&process.command, "ZAD_LAUNCH_ID")?;
            Some((agent.identity.clone(), id))
        })
        .collect();
    for (identity, id) in observations {
        store.associate_launch(&identity, &id);
    }
}

fn checked(target: &Identity, deps: &HostDependencies) -> Result<Agent, String> {
    let pane = panes::require_present(target, deps)?;
    let inventory_at = now_ms();
    let inventory = process::inventory(deps.runner)?;
    let mut locked = deps.repository.begin_runtime()?;
    locked.store.reconcile(&inventory.found, inventory_at);
    link_launches(&mut locked.store, &inventory);
    locked.store.validate_target(target)?;
    if !locked.store.observe_pane(target, pane)
        && locked.store.agents[&target.agent_id].pane.presence
            != dashboard_core::PanePresence::Present
    {
        return Err("agent pane changed during verification".into());
    }
    locked.commit()?;
    Ok(locked.store.agents[&target.agent_id].clone())
}

pub fn resolve(id: &str, deps: &HostDependencies) -> Result<Agent, String> {
    let agent = deps.repository.agent(id)?.ok_or("agent no longer exists")?;
    checked(&agent.identity, deps)
}

fn cwd(value: &str) -> Result<PathBuf, String> {
    if !Path::new(value).is_absolute() {
        return Err("작업 경로는 절대 경로여야 합니다".into());
    }
    let path = fs::canonicalize(value).map_err(|e| e.to_string())?;
    if !path.is_dir() {
        return Err("작업 경로가 디렉토리가 아닙니다".into());
    }
    Ok(path)
}

fn git(directory: &Path, args: &[&str], deps: &HostDependencies) -> Result<String, String> {
    let spec = CommandSpec::new("git", Some(Duration::from_secs(10)), 256 * 1024)
        .args([
            std::ffi::OsString::from("-C"),
            directory.as_os_str().to_owned(),
        ])
        .args(args.iter().copied());
    let bytes = command::output(deps.runner, spec)?;
    Ok(String::from_utf8_lossy(&bytes).trim().into())
}

struct LaunchTarget<'a> {
    session: &'a str,
    epoch: &'a str,
    directory: &'a str,
    tool: &'a str,
    parent: Option<&'a Identity>,
}

fn launch(
    request: &ActionRequest,
    target: LaunchTarget<'_>,
    deps: &HostDependencies,
) -> Result<(String, Option<u32>, String), String> {
    let LaunchTarget {
        session,
        epoch,
        directory,
        tool,
        parent,
    } = target;
    let path = cwd(directory)?;
    let name = TOOLS
        .iter()
        .find(|(name, _)| *name == tool)
        .ok_or("지원하지 않는 에이전트 도구")?
        .1;
    let exe = executable(name)?;
    let inventory = process::inventory(deps.runner)?;
    if inventory.epochs.get(session).map(String::as_str) != Some(epoch) {
        return Err("Zellij session changed or exited".into());
    }
    if let Some(parent) = parent {
        checked(parent, deps)?;
    }
    let directory = path.to_string_lossy().to_string();
    {
        let mut locked = deps.repository.begin_request(&request.request_id)?;
        locked.store.record_launch(
            &request.request_id,
            LaunchInfo {
                parent_id: parent.map(|p| p.agent_id.clone()),
                session: session.into(),
                epoch: epoch.into(),
                cwd: directory.clone(),
                tool: tool.into(),
                pane_id: None,
                agent_id: None,
            },
        )?;
        locked.commit()?;
    }
    let marker = format!("ZAD_LAUNCH_ID={}", request.request_id);
    let pane = pane_number(&deps.terminal.new_pane(
        &SessionId(session.into()),
        &NewPane {
            cwd: path.clone(),
            title: format!(
                "{tool} · {}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ),
            no_focus: true,
            floating: false,
            close_on_exit: false,
            program: "/usr/bin/env".into(),
            args: vec![marker.into(), exe.into_os_string()],
        },
    )?)?;
    let mut locked = deps.repository.begin_request(&request.request_id)?;
    locked.store.record_launch_pane(&request.request_id, pane)?;
    locked.store.remember_directory(&directory);
    locked.commit()?;
    Ok((format!("{tool} 실행 완료"), Some(pane), directory))
}

fn send(target: &Identity, text: &str, deps: &HostDependencies) -> Result<(), String> {
    if text.trim().is_empty()
        || text.len() > 64 * 1024
        || text
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
    {
        return Err("입력은 64 KiB 이하의 텍스트여야 합니다".into());
    }
    checked(target, deps)?;
    // Paste preserves multiline instructions. Delivery failures are uncertain.
    let paste = format!("\x1b[200~{text}\x1b[201~");
    let session = SessionId(target.session_name.clone());
    let pane = pane_id(target.pane_id);
    deps.terminal.write_text(&session, &pane, &paste)?;
    checked(target, deps)?;
    deps.terminal.write_bytes(&session, &pane, &[13])?;
    let mut locked = deps.repository.begin_runtime()?;
    let at = now_ms();
    locked.store.validate_target(target)?;
    locked.store.apply_signal(&StateSignal::Instruction {
        identity: target.clone(),
        observed_at_ms: at,
        text: text.into(),
    })?;
    locked.commit()
}

fn execute(
    dir: &Path,
    request: &ActionRequest,
    deps: &HostDependencies,
) -> Result<(String, Option<u32>, String), String> {
    match &request.action {
        Action::Input { target, text } => {
            send(target, text, deps)?;
            Ok(("입력 전송 완료".into(), None, String::new()))
        }
        Action::Alias { target, alias } => {
            checked(target, deps)?;
            let mut locked = deps.repository.begin_runtime()?;
            locked.store.set_alias(target, alias)?;
            locked.commit()?;
            Ok(("태그 저장 완료".into(), None, String::new()))
        }
        Action::Close { target } => {
            checked(target, deps)?;
            // Serialize pin settings through the final protection check/close.
            let mut locked = deps.repository.begin_runtime()?;
            locked.store.validate_close(target)?;
            deps.terminal.close_pane(
                &SessionId(target.session_name.clone()),
                &pane_id(target.pane_id),
            )?;
            locked.store.close_confirmed(target)?;
            locked.commit()?;
            Ok(("pane 종료 완료".into(), None, String::new()))
        }
        Action::Launch {
            session,
            epoch,
            cwd,
            tool,
        } => launch(
            request,
            LaunchTarget {
                session,
                epoch,
                directory: cwd,
                tool,
                parent: None,
            },
            deps,
        ),
        Action::Worktree {
            parent,
            branch,
            tool,
        } => {
            let agent = checked(parent, deps)?;
            let root = cwd(&git(
                Path::new(&agent.cwd),
                &["rev-parse", "--show-toplevel"],
                deps,
            )?)?;
            git(&root, &["check-ref-format", "--branch", branch], deps)?;
            if branch.starts_with('-') {
                return Err("invalid branch name".into());
            }
            let name = format!(
                "{}-{}",
                branch.replace('/', "-"),
                request
                    .request_id
                    .chars()
                    .filter(char::is_ascii_alphanumeric)
                    .take(8)
                    .collect::<String>()
            );
            let path = root
                .parent()
                .ok_or("repository has no parent directory")?
                .join(format!(
                    "{}-{}",
                    root.file_name().unwrap_or_default().to_string_lossy(),
                    name
                ));
            let mut locked = deps.repository.begin_request(&request.request_id)?;
            locked
                .store
                .record_request_path(&request.request_id, path.to_string_lossy().into())?;
            locked.commit()?;
            drop(locked);
            git(
                &root,
                &[
                    "worktree",
                    "add",
                    "-b",
                    &name,
                    "--",
                    &path.to_string_lossy(),
                    "HEAD",
                ],
                deps,
            )?;
            launch(
                request,
                LaunchTarget {
                    session: &parent.session_name,
                    epoch: &parent.session_epoch,
                    directory: &path.to_string_lossy(),
                    tool,
                    parent: Some(parent),
                },
                deps,
            )
        }
        Action::Lazygit { target } => {
            let agent = checked(target, deps)?;
            let exe = executable("lazygit")?;
            let pane = pane_number(&deps.terminal.new_pane(
                &SessionId(target.session_name.clone()),
                &NewPane {
                    cwd: agent.cwd.into(),
                    title: "lazygit".into(),
                    floating: true,
                    close_on_exit: true,
                    no_focus: false,
                    program: exe.into_os_string(),
                    args: vec![],
                },
            )?)?;
            Ok(("lazygit 실행 완료".into(), Some(pane), String::new()))
        }
        Action::Editor { target, text } => {
            let agent = checked(target, deps)?;
            let path = dir.join(format!("edit-{}.txt", uuid::Uuid::new_v4()));
            fs::write(&path, text).map_err(|e| e.to_string())?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                .map_err(|e| e.to_string())?;
            let editor = env::var("VISUAL")
                .or_else(|_| env::var("EDITOR"))
                .unwrap_or_else(|_| "vi".into());
            // EDITOR is a user command; the filename is a positional argument.
            let script = format!("exec {editor} \"$1\"");
            let pane = pane_number(&deps.terminal.new_pane(
                &SessionId(target.session_name.clone()),
                &NewPane {
                    cwd: agent.cwd.into(),
                    title: "지시 편집".into(),
                    floating: true,
                    close_on_exit: true,
                    no_focus: false,
                    program: "/bin/sh".into(),
                    args: vec![
                        "-c".into(),
                        script.into(),
                        "editor".into(),
                        path.as_os_str().to_owned(),
                    ],
                },
            )?)?;
            Ok((
                "에디터를 닫으면 편집한 지시를 복원합니다".into(),
                Some(pane),
                path.to_string_lossy().into(),
            ))
        }
        Action::ShellChildren {
            parent,
            command: shell,
        } => {
            checked(parent, deps)?;
            if shell.trim().is_empty() || shell.len() > 64 * 1024 {
                return Err("셸 명령어를 입력하세요".into());
            }
            let store = deps.repository.runtime()?;
            let children: Vec<_> = store
                .agents
                .values()
                .filter(|a| {
                    a.parent_id.as_ref() == Some(&parent.agent_id) && a.liveness == Liveness::Live
                })
                .cloned()
                .collect();
            drop(store);
            let mut results = Vec::new();
            let mut paths = std::collections::BTreeSet::new();
            for child in children {
                if !paths.insert(child.cwd.clone()) {
                    continue;
                }
                checked(&child.identity, deps)?;
                let mut spec =
                    CommandSpec::new("/bin/sh", Some(Duration::from_secs(60)), 64 * 1024)
                        .args(["-lc", shell]);
                spec.cwd = Some(cwd(&child.cwd)?);
                let result = command::output(deps.runner, spec);
                results.push(format!(
                    "{}\n{}",
                    child.cwd,
                    match result {
                        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
                        Err(e) => format!("실패: {e}"),
                    }
                ));
            }
            if results.is_empty() {
                return Err("실행할 자식 worktree가 없습니다".into());
            }
            Ok((results.join("\n\n"), None, String::new()))
        }
        Action::Merge { parent, child } => {
            checked(parent, deps)?;
            let ch = checked(child, deps)?;
            deps.repository.runtime()?.validate_merge(parent, child)?;
            let branch = git(Path::new(&ch.cwd), &["branch", "--show-current"], deps)?;
            let instruction=format!("자식 worktree {}의 작업을 검토하고 현재 브랜치에 병합해주세요.\n자식 경로: {}\n자식 브랜치: {}\n충돌과 테스트 결과를 확인하고 처리 결과를 보고해주세요.",ch.project(),ch.cwd,branch);
            send(parent, &instruction, deps)?;
            Ok(("부모에게 병합 지시 전송 완료".into(), None, String::new()))
        }
    }
}

pub fn action(
    dir: &Path,
    request: ActionRequest,
    deps: &HostDependencies,
) -> Result<ActionResult, String> {
    {
        let inventory_at = now_ms();
        let inventory = process::inventory(deps.runner)?;
        let mut locked = deps.repository.begin_action(&request.request_id)?;
        locked.store.reconcile(&inventory.found, inventory_at);
        link_launches(&mut locked.store, &inventory);
        if !locked.store.claim(&request, now_ms())? {
            return Ok(locked.store.requests[&request.request_id].clone());
        }
        locked.commit()?;
    }
    let result = execute(dir, &request, deps);
    let mut locked = deps.repository.begin_request(&request.request_id)?;
    let response = match result {
        Ok((message, pane, path)) => locked.store.finish_request(
            &request.request_id,
            RequestState::Succeeded,
            message,
            pane,
            Some(path),
        )?,
        Err(error) => {
            let state = if matches!(request.action, Action::Alias { .. }) {
                RequestState::Failed
            } else {
                RequestState::Uncertain
            };
            locked
                .store
                .finish_request(&request.request_id, state, error, None, None)?
        }
    };
    locked.commit()?;
    Ok(response)
}

pub fn result(
    repository: &dyn crate::repository::Repository,
    id: &str,
) -> Result<ActionResult, String> {
    repository.request(id)?.ok_or("unknown request ID".into())
}

pub fn edited(id: &str, deps: &HostDependencies) -> Result<serde_json::Value, String> {
    let result = result(deps.repository, id)?;
    let Action::Editor { target, .. } = &result.request.action else {
        return Err("request is not an editor action".into());
    };
    let panes = deps.terminal.list_panes(
        &SessionId(target.session_name.clone()),
        false,
        Duration::from_millis(1200),
    )?;
    let open = panes
        .iter()
        .any(|pane| result.pane_id.is_some_and(|id| pane.id == pane_id(id)));
    if open {
        return Ok(serde_json::json!({"ready":false}));
    }
    if fs::metadata(&result.path).map_err(|e| e.to_string())?.len() > 64 * 1024 {
        return Err("edited instruction exceeds 64 KiB".into());
    }
    let text = fs::read_to_string(&result.path).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"ready":true,"text":text,"target":target}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        command::{CommandError, CommandOutput, CommandRunner},
        terminal::{PaneId, TerminalHost, TerminalPane},
    };
    use std::cell::{Cell, RefCell};

    #[derive(Default)]
    struct FakeHost {
        changed: Cell<bool>,
        missing_pane: Cell<bool>,
        change_after_paste: Cell<bool>,
        calls: RefCell<Vec<String>>,
    }
    impl FakeHost {
        fn inventory_text(&self) -> String {
            let started = if self.changed.get() {
                "10:02:00"
            } else {
                "10:01:00"
            };
            format!("10 1 Mon Oct 5 10:00:00 2026 zellij --server /tmp/dev\n20 10 Mon Oct 5 {started} 2026 /bin/codex ZELLIJ_SESSION_NAME=dev ZELLIJ_PANE_ID=7 PWD=/tmp\n{} 1 Mon Oct 5 10:00:00 2026 /bin/test\n", std::process::id())
        }
        fn seed(&self, dir: &Path) -> Identity {
            let inventory_at = now_ms();
            let inventory = process::parse_inventory(&self.inventory_text()).unwrap();
            let target = inventory.found[0].identity.clone();
            let mut locked = crate::repository::at(dir).begin().unwrap();
            locked.store.reconcile(&inventory.found, inventory_at);
            locked.commit().unwrap();
            target
        }
    }
    impl CommandRunner for FakeHost {
        fn run(&self, spec: &CommandSpec) -> Result<CommandOutput, CommandError> {
            assert_eq!(spec.program, "ps");
            assert_eq!(
                spec.args,
                ["axeww", "-o", "pid=,ppid=,lstart=,command="].map(std::ffi::OsString::from)
            );
            assert_eq!(
                spec.env,
                vec![
                    ("LC_ALL".into(), None),
                    ("LC_TIME".into(), Some("C".into())),
                    ("LC_CTYPE".into(), Some("en_US.UTF-8".into()))
                ]
            );
            Ok(CommandOutput {
                success: true,
                code: Some(0),
                stdout: self.inventory_text().into_bytes(),
                stderr: vec![],
            })
        }
    }
    impl TerminalHost for FakeHost {
        fn list_panes(
            &self,
            _: &SessionId,
            _: bool,
            _: Duration,
        ) -> Result<Vec<TerminalPane>, String> {
            Ok(if self.missing_pane.get() {
                vec![]
            } else {
                vec![TerminalPane {
                    id: pane_id(7),
                    tab_id: None,
                    tab_name: String::new(),
                    title: String::new(),
                    cwd: None,
                }]
            })
        }
        fn screen(&self, _: &SessionId, _: &PaneId) -> Result<String, String> {
            panic!("unexpected screen")
        }
        fn write_text(&self, session: &SessionId, pane: &PaneId, text: &str) -> Result<(), String> {
            assert_eq!(session.0, "dev");
            assert_eq!(*pane, pane_id(7));
            self.calls.borrow_mut().push(text.into());
            if self.change_after_paste.get() {
                self.changed.set(true);
            }
            Ok(())
        }
        fn write_bytes(&self, _: &SessionId, _: &PaneId, bytes: &[u8]) -> Result<(), String> {
            assert_eq!(bytes, [13]);
            self.calls.borrow_mut().push("enter".into());
            Ok(())
        }
        fn close_pane(&self, _: &SessionId, _: &PaneId) -> Result<(), String> {
            self.calls.borrow_mut().push("close".into());
            Ok(())
        }
        fn new_pane(&self, _: &SessionId, _: &NewPane) -> Result<PaneId, String> {
            panic!("unexpected new pane")
        }
        fn notify_changed(&self, _: &SessionId, _: &str) -> Result<(), String> {
            panic!("unexpected notify")
        }
    }

    #[test]
    fn duplicate_input_does_not_repeat_paste_or_enter() {
        let directory = tempfile::tempdir().unwrap();
        let fake = FakeHost::default();
        let target = fake.seed(directory.path());
        let repository = crate::repository::at(directory.path());
        let deps = HostDependencies {
            repository: repository.as_ref(),
            runner: &fake,
            terminal: &fake,
        };
        let request = ActionRequest {
            request_id: "input-once".into(),
            action: Action::Input {
                target,
                text: "한글\n두 줄".into(),
            },
        };
        assert_eq!(
            action(directory.path(), request.clone(), &deps)
                .unwrap()
                .state,
            RequestState::Succeeded
        );
        assert_eq!(
            action(directory.path(), request, &deps).unwrap().state,
            RequestState::Succeeded
        );
        assert_eq!(
            *fake.calls.borrow(),
            ["\x1b[200~한글\n두 줄\x1b[201~", "enter"]
        );
    }

    #[test]
    fn replaced_process_after_paste_blocks_enter_and_retry() {
        let directory = tempfile::tempdir().unwrap();
        let fake = FakeHost::default();
        fake.change_after_paste.set(true);
        let target = fake.seed(directory.path());
        let repository = crate::repository::at(directory.path());
        let deps = HostDependencies {
            repository: repository.as_ref(),
            runner: &fake,
            terminal: &fake,
        };
        let request = ActionRequest {
            request_id: "changed-input".into(),
            action: Action::Input {
                target,
                text: "hello".into(),
            },
        };
        assert_eq!(
            action(directory.path(), request.clone(), &deps)
                .unwrap()
                .state,
            RequestState::Uncertain
        );
        assert_eq!(
            action(directory.path(), request, &deps).unwrap().state,
            RequestState::Uncertain
        );
        assert_eq!(*fake.calls.borrow(), ["\x1b[200~hello\x1b[201~"]);
    }

    #[test]
    fn pin_protection_blocks_close_before_adapter_call() {
        let directory = tempfile::tempdir().unwrap();
        let fake = FakeHost::default();
        let target = fake.seed(directory.path());
        {
            let mut locked = crate::repository::at(directory.path()).begin().unwrap();
            locked.store.set_pinned(&target.agent_id, true).unwrap();
            locked.commit().unwrap();
        }
        let repository = crate::repository::at(directory.path());
        let deps = HostDependencies {
            repository: repository.as_ref(),
            runner: &fake,
            terminal: &fake,
        };
        let request = ActionRequest {
            request_id: "pinned-close".into(),
            action: Action::Close { target },
        };
        let error = action(directory.path(), request, &deps).unwrap_err();
        assert!(error.contains("고정"));
        assert!(fake.calls.borrow().is_empty());
    }

    #[test]
    fn missing_pane_blocks_resolution_and_input_even_when_process_is_live() {
        let dir = tempfile::tempdir().unwrap();
        let fake = FakeHost::default();
        let target = fake.seed(dir.path());
        fake.missing_pane.set(true);
        let repository = crate::repository::at(dir.path());
        let deps = HostDependencies {
            repository: repository.as_ref(),
            runner: &fake,
            terminal: &fake,
        };
        assert!(resolve(&target.agent_id, &deps)
            .unwrap_err()
            .contains("pane no longer exists"));
        let result = action(
            dir.path(),
            ActionRequest {
                request_id: "missing".into(),
                action: Action::Input {
                    target,
                    text: "must not send".into(),
                },
            },
            &deps,
        )
        .unwrap();
        assert_eq!(result.state, RequestState::Uncertain);
        assert!(fake.calls.borrow().is_empty());
    }
}
