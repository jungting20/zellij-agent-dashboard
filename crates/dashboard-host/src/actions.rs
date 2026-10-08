use crate::{
    command, process,
    storage::{now_ms, LockedStore},
};
use dashboard_core::{
    Action, ActionRequest, ActionResult, Agent, AgentEvent, Catalog, EventKind, Identity,
    LaunchInfo, Liveness, RequestState, Status, SCHEMA_VERSION,
};
use std::{
    env, fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
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

pub fn catalog(dir: &Path) -> Result<Catalog, String> {
    let locked = LockedStore::open(dir)?;
    let inventory = process::inventory()?;
    let mut directories = locked.store.recent_directories.clone();
    directories.extend(locked.store.agents.values().map(|a| a.cwd.clone()));
    if let Ok(bytes) = command::output(
        Command::new("zoxide").args(["query", "--list"]),
        Duration::from_millis(500),
        128 * 1024,
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
    for agent in store
        .agents
        .values_mut()
        .filter(|a| a.liveness == Liveness::Live)
    {
        let Some(process) = inventory
            .processes
            .iter()
            .find(|p| p.pid == agent.identity.pid)
        else {
            continue;
        };
        let Some(id) = process::env_value(&process.command, "ZAD_LAUNCH_ID") else {
            continue;
        };
        let Some(launch) = store.launches.get_mut(&id) else {
            continue;
        };
        if launch.session != agent.identity.session_name
            || launch.epoch != agent.identity.session_epoch
            || launch.tool != agent.tool
            || launch
                .pane_id
                .is_some_and(|pane| pane != agent.identity.pane_id)
            || launch
                .agent_id
                .as_ref()
                .is_some_and(|old| old != &agent.identity.agent_id)
        {
            continue;
        }
        launch.pane_id = Some(agent.identity.pane_id);
        launch.agent_id = Some(agent.identity.agent_id.clone());
        agent.parent_id.clone_from(&launch.parent_id);
        agent.cwd.clone_from(&launch.cwd);
    }
}

fn checked(dir: &Path, target: &Identity) -> Result<Agent, String> {
    let mut locked = LockedStore::open(dir)?;
    let inventory = process::inventory()?;
    locked.store.reconcile(&inventory.found, now_ms());
    link_launches(&mut locked.store, &inventory);
    locked.store.validate_target(target)?;
    locked.save()?;
    Ok(locked.store.agents[&target.agent_id].clone())
}

fn zellij(session: &str, args: &[&str]) -> Result<String, String> {
    let bytes = command::output(
        Command::new("zellij")
            .args(["--session", session, "action"])
            .args(args),
        Duration::from_millis(1200),
        128 * 1024,
    )?;
    Ok(String::from_utf8_lossy(&bytes).trim().into())
}

fn pane_number(value: &str) -> Result<u32, String> {
    value
        .trim()
        .strip_prefix("terminal_")
        .ok_or("Zellij did not return a terminal pane ID")?
        .parse()
        .map_err(|_| "invalid created pane ID".into())
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

fn git(directory: &Path, args: &[&str]) -> Result<String, String> {
    let bytes = command::output(
        Command::new("git").arg("-C").arg(directory).args(args),
        Duration::from_secs(10),
        256 * 1024,
    )?;
    Ok(String::from_utf8_lossy(&bytes).trim().into())
}

fn launch(
    dir: &Path,
    request: &ActionRequest,
    session: &str,
    epoch: &str,
    directory: &str,
    tool: &str,
    parent: Option<&Identity>,
) -> Result<(String, Option<u32>, String), String> {
    let path = cwd(directory)?;
    let name = TOOLS
        .iter()
        .find(|(name, _)| *name == tool)
        .ok_or("지원하지 않는 에이전트 도구")?
        .1;
    let exe = executable(name)?;
    let inventory = process::inventory()?;
    if inventory.epochs.get(session).map(String::as_str) != Some(epoch) {
        return Err("Zellij session changed or exited".into());
    }
    if let Some(parent) = parent {
        checked(dir, parent)?;
    }
    let directory = path.to_string_lossy().to_string();
    {
        let mut locked = LockedStore::open(dir)?;
        locked.store.launches.insert(
            request.request_id.clone(),
            LaunchInfo {
                parent_id: parent.map(|p| p.agent_id.clone()),
                session: session.into(),
                epoch: epoch.into(),
                cwd: directory.clone(),
                tool: tool.into(),
                pane_id: None,
                agent_id: None,
            },
        );
        locked.save()?;
    }
    let marker = format!("ZAD_LAUNCH_ID={}", request.request_id);
    let pane = pane_number(&zellij(
        session,
        &[
            "new-pane",
            "--no-focus",
            "--cwd",
            &directory,
            "--name",
            &format!(
                "{tool} · {}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ),
            "--",
            "/usr/bin/env",
            &marker,
            &exe.to_string_lossy(),
        ],
    )?)?;
    let mut locked = LockedStore::open(dir)?;
    locked
        .store
        .launches
        .get_mut(&request.request_id)
        .unwrap()
        .pane_id = Some(pane);
    locked.store.remember_directory(&directory);
    locked.save()?;
    Ok((format!("{tool} 실행 완료"), Some(pane), directory))
}

fn send(dir: &Path, target: &Identity, text: &str) -> Result<(), String> {
    if text.trim().is_empty()
        || text.len() > 64 * 1024
        || text
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
    {
        return Err("입력은 64 KiB 이하의 텍스트여야 합니다".into());
    }
    checked(dir, target)?;
    // Paste preserves multiline instructions. Delivery failures are uncertain.
    let paste = format!("\x1b[200~{text}\x1b[201~");
    let pane = format!("terminal_{}", target.pane_id);
    zellij(
        &target.session_name,
        &["write-chars", "--pane-id", &pane, &paste],
    )?;
    checked(dir, target)?;
    zellij(&target.session_name, &["write", "--pane-id", &pane, "13"])?;
    let mut locked = LockedStore::open(dir)?;
    let at = now_ms();
    locked.store.validate_target(target)?;
    let agent = &locked.store.agents[&target.agent_id];
    let event = AgentEvent {
        schema_version: SCHEMA_VERSION,
        event_id: uuid::Uuid::new_v4().to_string(),
        identity: target.clone(),
        tool: agent.tool.clone(),
        kind: EventKind::TurnStarted,
        sequence: agent.sequence + 1,
        observed_at_ms: at,
        cwd: String::new(),
        summary: text.into(),
        detail: "dashboard input".into(),
    };
    locked.store.apply(&event)?;
    locked.save()
}

fn execute(dir: &Path, request: &ActionRequest) -> Result<(String, Option<u32>, String), String> {
    match &request.action {
        Action::Input { target, text } => {
            send(dir, target, text)?;
            Ok(("입력 전송 완료".into(), None, String::new()))
        }
        Action::Alias { target, alias } => {
            if alias.len() > 120 || alias.chars().any(char::is_control) {
                return Err("한 줄 태그를 입력해주세요 (120 bytes 이하)".into());
            }
            checked(dir, target)?;
            let mut locked = LockedStore::open(dir)?;
            locked.store.validate_target(target)?;
            locked.store.agents.get_mut(&target.agent_id).unwrap().alias = alias.trim().into();
            locked.store.revision += 1;
            locked.save()?;
            Ok(("태그 저장 완료".into(), None, String::new()))
        }
        Action::Close { target } => {
            checked(dir, target)?;
            // Serialize pin settings through the final protection check/close.
            let mut locked = LockedStore::open(dir)?;
            locked.store.validate_target(target)?;
            let mut id = Some(target.agent_id.as_str());
            let mut seen = std::collections::BTreeSet::new();
            while let Some(current) = id {
                if !seen.insert(current) {
                    break;
                }
                let Some(agent) = locked.store.agents.get(current) else {
                    break;
                };
                if agent.pinned {
                    return Err("고정된 에이전트는 종료할 수 없습니다".into());
                }
                id = agent.parent_id.as_deref();
            }
            zellij(
                &target.session_name,
                &[
                    "close-pane",
                    "--pane-id",
                    &format!("terminal_{}", target.pane_id),
                ],
            )?;
            let agent = locked.store.agents.get_mut(&target.agent_id).unwrap();
            agent.ended = true;
            agent.liveness = Liveness::Gone;
            locked.store.revision += 1;
            locked.save()?;
            Ok(("pane 종료 완료".into(), None, String::new()))
        }
        Action::Launch {
            session,
            epoch,
            cwd,
            tool,
        } => launch(dir, request, session, epoch, cwd, tool, None),
        Action::Worktree {
            parent,
            branch,
            tool,
        } => {
            let agent = checked(dir, parent)?;
            let root = cwd(&git(
                Path::new(&agent.cwd),
                &["rev-parse", "--show-toplevel"],
            )?)?;
            git(&root, &["check-ref-format", "--branch", branch])?;
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
            let mut locked = LockedStore::open(dir)?;
            locked
                .store
                .requests
                .get_mut(&request.request_id)
                .unwrap()
                .path = path.to_string_lossy().into();
            locked.save()?;
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
            )?;
            launch(
                dir,
                request,
                &parent.session_name,
                &parent.session_epoch,
                &path.to_string_lossy(),
                tool,
                Some(parent),
            )
        }
        Action::Lazygit { target } => {
            let agent = checked(dir, target)?;
            let exe = executable("lazygit")?;
            let pane = pane_number(&zellij(
                &target.session_name,
                &[
                    "new-pane",
                    "--floating",
                    "--close-on-exit",
                    "--cwd",
                    &agent.cwd,
                    "--name",
                    "lazygit",
                    "--",
                    &exe.to_string_lossy(),
                ],
            )?)?;
            Ok(("lazygit 실행 완료".into(), Some(pane), String::new()))
        }
        Action::Editor { target, text } => {
            let agent = checked(dir, target)?;
            let path = dir.join(format!("edit-{}.txt", uuid::Uuid::new_v4()));
            fs::write(&path, text).map_err(|e| e.to_string())?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                .map_err(|e| e.to_string())?;
            let editor = env::var("VISUAL")
                .or_else(|_| env::var("EDITOR"))
                .unwrap_or_else(|_| "vi".into());
            // EDITOR is a user command; the filename is a positional argument.
            let script = format!("exec {editor} \"$1\"");
            let pane = pane_number(&zellij(
                &target.session_name,
                &[
                    "new-pane",
                    "--floating",
                    "--close-on-exit",
                    "--cwd",
                    &agent.cwd,
                    "--name",
                    "지시 편집",
                    "--",
                    "/bin/sh",
                    "-c",
                    &script,
                    "editor",
                    &path.to_string_lossy(),
                ],
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
            checked(dir, parent)?;
            if shell.trim().is_empty() || shell.len() > 64 * 1024 {
                return Err("셸 명령어를 입력하세요".into());
            }
            let locked = LockedStore::open(dir)?;
            let children: Vec<_> = locked
                .store
                .agents
                .values()
                .filter(|a| {
                    a.parent_id.as_ref() == Some(&parent.agent_id) && a.liveness == Liveness::Live
                })
                .cloned()
                .collect();
            drop(locked);
            let mut results = Vec::new();
            let mut paths = std::collections::BTreeSet::new();
            for child in children {
                if !paths.insert(child.cwd.clone()) {
                    continue;
                }
                checked(dir, &child.identity)?;
                let result = command::output(
                    Command::new("/bin/sh")
                        .args(["-lc", shell])
                        .current_dir(cwd(&child.cwd)?),
                    Duration::from_secs(60),
                    64 * 1024,
                );
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
            let pa = checked(dir, parent)?;
            let ch = checked(dir, child)?;
            if ch.parent_id.as_ref() != Some(&parent.agent_id) {
                return Err("선택한 에이전트는 직접 자식이 아닙니다".into());
            }
            if !matches!(pa.status, Status::Idle | Status::Done)
                || !matches!(ch.status, Status::Idle | Status::Done)
            {
                return Err(
                    "부모와 자식이 idle 또는 done일 때 병합 지시를 보낼 수 있습니다".into(),
                );
            }
            let branch = git(Path::new(&ch.cwd), &["branch", "--show-current"])?;
            let instruction=format!("자식 worktree {}의 작업을 검토하고 현재 브랜치에 병합해주세요.\n자식 경로: {}\n자식 브랜치: {}\n충돌과 테스트 결과를 확인하고 처리 결과를 보고해주세요.",ch.project(),ch.cwd,branch);
            send(dir, parent, &instruction)?;
            Ok(("부모에게 병합 지시 전송 완료".into(), None, String::new()))
        }
    }
}

pub fn action(dir: &Path, request: ActionRequest) -> Result<ActionResult, String> {
    {
        let mut locked = LockedStore::open(dir)?;
        let inventory = process::inventory()?;
        locked.store.reconcile(&inventory.found, now_ms());
        link_launches(&mut locked.store, &inventory);
        if !locked.store.claim(&request, now_ms())? {
            return Ok(locked.store.requests[&request.request_id].clone());
        }
        locked.save()?;
    }
    let result = execute(dir, &request);
    let mut locked = LockedStore::open(dir)?;
    let record = locked
        .store
        .requests
        .get_mut(&request.request_id)
        .ok_or("request record disappeared")?;
    match result {
        Ok((message, pane, path)) => {
            record.state = RequestState::Succeeded;
            record.message = message;
            record.pane_id = pane;
            record.path = path;
        }
        Err(error) => {
            record.state = if matches!(request.action, Action::Alias { .. }) {
                RequestState::Failed
            } else {
                RequestState::Uncertain
            };
            record.message = error;
        }
    }
    let response = record.clone();
    locked.store.revision += 1;
    locked.save()?;
    Ok(response)
}

pub fn result(dir: &Path, id: &str) -> Result<ActionResult, String> {
    let locked = LockedStore::open(dir)?;
    locked
        .store
        .requests
        .get(id)
        .cloned()
        .ok_or("unknown request ID".into())
}

pub fn edited(dir: &Path, id: &str) -> Result<serde_json::Value, String> {
    let result = result(dir, id)?;
    let Action::Editor { target, .. } = &result.request.action else {
        return Err("request is not an editor action".into());
    };
    let panes = zellij(&target.session_name, &["list-panes", "--json"])?;
    let panes: serde_json::Value = serde_json::from_str(&panes).map_err(|e| e.to_string())?;
    let open = panes
        .as_array()
        .ok_or("invalid pane list")?
        .iter()
        .any(|p| p["is_plugin"] == false && p["id"].as_u64() == result.pane_id.map(u64::from));
    if open {
        return Ok(serde_json::json!({"ready":false}));
    }
    if fs::metadata(&result.path).map_err(|e| e.to_string())?.len() > 64 * 1024 {
        return Err("edited instruction exceeds 64 KiB".into());
    }
    let text = fs::read_to_string(&result.path).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"ready":true,"text":text,"target":target}))
}
