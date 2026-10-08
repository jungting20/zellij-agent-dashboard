//! One-time import of the legacy JSON store. Never writes legacy state.
use dashboard_core::Store;
use fs2::FileExt;
use std::{
    fs::File,
    io::Read,
    path::Path,
    time::{Duration, Instant},
};

pub fn read_legacy(dir: &Path) -> Result<Store, String> {
    let path = dir.join("store.json");
    if !path.exists() {
        return Ok(Store::default());
    }
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join("store.lock"))
        .map_err(|e| e.to_string())?;
    use std::os::unix::fs::PermissionsExt;
    lock.set_permissions(std::fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match lock.try_lock_exclusive() {
            Ok(()) => break,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(5))
            }
            Err(e) => return Err(format!("legacy state lock: {e}")),
        }
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("state file exceeds limit; preserved".into());
    }
    let mut store: Store = serde_json::from_slice(&bytes)
        .map_err(|e| format!("state file invalid; preserved: {e}"))?;
    store.migrate()?;
    Ok(store)
}
