use crate::{
    actions,
    host::{pane_id, HostDependencies},
    process::{self, Inventory},
    terminal::SessionId,
};
use dashboard_core::{
    NavigationAction, NavigationHistory, NavigationRequest, NextRequest, PaneVisit,
};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    time::Duration,
};

const HISTORY_FILE: &str = "navigation-v1.json";

fn visit(
    inventory: &Inventory,
    session: &str,
    id: u32,
    preferred: Option<&dashboard_core::Identity>,
) -> Option<PaneVisit> {
    let session_epoch = inventory.epochs.get(session)?;
    let agent = inventory
        .found
        .iter()
        .filter(|found| found.identity.slot() == (session, id))
        .min_by_key(|found| (Some(&found.identity) != preferred, found.identity.pid))
        .map(|found| found.identity.clone());
    Some(PaneVisit {
        session: session.into(),
        session_epoch: session_epoch.clone(),
        pane_id: id,
        agent,
    })
}

pub fn navigate(
    dir: &Path,
    request: NavigationRequest,
    deps: &HostDependencies,
) -> Result<Option<PaneVisit>, String> {
    let target = match request.action {
        NavigationAction::Next(filter) => {
            let agent = actions::next_agent(
                &NextRequest {
                    filter,
                    session: request.session.clone(),
                    pane_id: request.pane_id,
                },
                deps,
            )?;
            match agent {
                Some(agent) => Some(agent.identity),
                None => return Ok(None),
            }
        }
        NavigationAction::Select(identity) => Some(actions::checked(&identity, deps)?.identity),
        NavigationAction::Previous => None,
    };

    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("navigation.lock"))
        .map_err(|e| e.to_string())?;
    fs2::FileExt::lock_exclusive(&lock).map_err(|e| e.to_string())?;
    let path = dir.join(HISTORY_FILE);
    let mut history: NavigationHistory = match File::open(&path) {
        Ok(file) => serde_json::from_reader(file).map_err(|e| e.to_string())?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => NavigationHistory::default(),
        Err(e) => return Err(e.to_string()),
    };

    let inventory = process::inventory(deps.runner)?;
    let previous_agent = history.current().and_then(|current| current.agent.as_ref());
    let from = request
        .pane_id
        .and_then(|id| visit(&inventory, &request.session, id, previous_agent));
    let destination = if let Some(target) = target {
        let to = visit(
            &inventory,
            &target.session_name,
            target.pane_id,
            Some(&target),
        )
        .filter(|to| to.agent.as_ref() == Some(&target))
        .ok_or("agent process changed or exited")?;
        if !history.advance(from, to.clone()) {
            return Ok(Some(to));
        }
        Some(to)
    } else {
        let Some(from) = from else {
            return Ok(None);
        };
        history.back(&from, |entry| {
            if visit(
                &inventory,
                &entry.session,
                entry.pane_id,
                entry.agent.as_ref(),
            )
            .as_ref()
                != Some(entry)
            {
                return Ok(false);
            }
            let panes = deps.terminal.list_panes(
                &SessionId(entry.session.clone()),
                true,
                Duration::from_millis(500),
            )?;
            Ok(panes.iter().any(|pane| pane.id == pane_id(entry.pane_id)))
        })?
    };

    let temporary = dir.join("navigation-v1.tmp");
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(&history).map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    fs::rename(temporary, path).map_err(|e| e.to_string())?;
    Ok(destination)
}
