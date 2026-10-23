//! One finite, serialized notification reconciliation per collector session.
use crate::{repository::Repository, terminal::SessionId, zellij::ZellijCli};
use std::{
    hash::{Hash, Hasher},
    path::Path,
};

pub fn sync(
    session: &str,
    dir: &Path,
    repository: &dyn Repository,
    terminal: &ZellijCli<'_>,
) -> Result<(), String> {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    session.hash(&mut hash);
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(format!("notification-window-{:x}.lock", hash.finish())))
        .map_err(|e| e.to_string())?;
    if fs2::FileExt::try_lock_exclusive(&lock).is_err() {
        return Ok(());
    }
    // Read inside the window lock so an older helper cannot undo a newer one.
    // No repository transaction is held during terminal I/O.
    let snapshot = repository.snapshot(crate::repository::now_ms())?;
    let enabled = !dashboard_core::notification::waiting(&snapshot).is_empty();
    let host = std::env::current_exe().map_err(|e| e.to_string())?;
    terminal.sync_notification(
        &SessionId(session.into()),
        &host.with_file_name("agent-notification.wasm"),
        &host,
        dir,
        enabled,
    )?;
    if enabled {
        terminal.update_notification(
            &SessionId(session.into()),
            &serde_json::to_string(&dashboard_core::notification::Notification::from_snapshot(
                &snapshot,
            ))
            .map_err(|e| e.to_string())?,
        )?;
    }
    Ok(())
}
