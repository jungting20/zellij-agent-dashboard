use dashboard_core::Store;
use fs2::FileExt;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub struct LockedStore {
    _lock: File,
    dir: PathBuf,
    pub store: Store,
}

impl LockedStore {
    pub fn open(dir: &Path) -> Result<Self, String> {
        if !dir.is_absolute() {
            return Err("state directory must be absolute".into());
        }
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(dir.join("store.lock"))
            .map_err(|e| e.to_string())?;
        fs::set_permissions(dir.join("store.lock"), fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match lock.try_lock_exclusive() {
                Ok(()) => break,
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(e) => return Err(format!("state lock: {e}")),
            }
        }
        let path = dir.join("store.json");
        let store = if path.exists() {
            let mut bytes = Vec::new();
            File::open(path)
                .map_err(|e| e.to_string())?
                .take(16 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > 16 * 1024 * 1024 {
                return Err("state file exceeds limit".into());
            }
            serde_json::from_slice::<Store>(&bytes)
                .map_err(|e| format!("state file invalid; preserved: {e}"))?
        } else {
            Store::default()
        };
        store.check_version()?;
        Ok(Self {
            _lock: lock,
            dir: dir.into(),
            store,
        })
    }

    pub fn save(&self) -> Result<(), String> {
        let path = self
            .dir
            .join(format!(".store-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|e| e.to_string())?;
            file.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|e| e.to_string())?;
            serde_json::to_writer(&mut file, &self.store).map_err(|e| e.to_string())?;
            file.flush().map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
            fs::rename(&path, self.dir.join("store.json")).map_err(|e| e.to_string())?;
            File::open(&self.dir)
                .and_then(|f| f.sync_all())
                .map_err(|e| e.to_string())
        })();
        if result.is_err() {
            let _ = fs::remove_file(path);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrent_writers_serialize_and_recover_all_updates() {
        let dir = tempfile::tempdir().unwrap();
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let path = dir.path().to_owned();
                std::thread::spawn(move || {
                    let mut locked = LockedStore::open(&path).unwrap();
                    locked.store.revision += 1;
                    locked.save().unwrap();
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        assert_eq!(LockedStore::open(dir.path()).unwrap().store.revision, 8);
        assert_eq!(
            fs::metadata(dir.path().join("store.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    fn corrupt_or_newer_store_is_never_silently_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.json");
        fs::write(&path, "{incomplete").unwrap();
        assert!(LockedStore::open(dir.path()).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{incomplete");
        let store = Store {
            schema_version: 999,
            ..Store::default()
        };
        fs::write(&path, serde_json::to_string(&store).unwrap()).unwrap();
        assert!(LockedStore::open(dir.path()).is_err());
    }
}
