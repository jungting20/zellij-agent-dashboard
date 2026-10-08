//! Native SQLite adapter. SQL and filesystem details never enter the core.
use crate::repository::{Repository, TransactionBackend, UnitOfWork};
use dashboard_core::Store;
use rusqlite::{params, Connection};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::PathBuf,
    time::{Duration, Instant},
};

const DATABASE_VERSION: i64 = 1;

pub struct SqliteRepository(pub PathBuf);

fn error(e: impl std::fmt::Display) -> String {
    format!("state database: {e}")
}

impl SqliteRepository {
    fn connect(&self) -> Result<Connection, String> {
        if !self.0.is_absolute() {
            return Err("state directory must be absolute".into());
        }
        fs::create_dir_all(&self.0).map_err(error)?;
        fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700)).map_err(error)?;
        let path = self.0.join("store.sqlite3");
        OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .mode(0o600)
            .open(&path)
            .map_err(error)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(error)?;
        let conn = Connection::open(&path).map_err(error)?;
        conn.busy_timeout(Duration::from_secs(2)).map_err(error)?;
        conn.pragma_update(None, "synchronous", "FULL")
            .map_err(error)?;
        let version = database_version(&conn)?;
        if version != 0 && version != DATABASE_VERSION {
            return Err(format!("unsupported database version {version}; preserved"));
        }
        // Configure WAL only during initialization; ordinary readers never take
        // a writer reservation. SQLite sidecars inherit the database permissions.
        if version == 0 {
            enable_wal(&conn)?;
            conn.execute_batch("BEGIN IMMEDIATE").map_err(error)?;
            let initialized = (|| {
                let current = database_version(&conn)?;
                if current == DATABASE_VERSION {
                    return Ok(());
                }
                if current != 0 {
                    return Err(format!("unsupported database version {current}; preserved"));
                }
                let tables: i64 = conn.query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
                    [], |r| r.get(0),
                ).map_err(error)?;
                if tables != 0 {
                    return Err("unversioned database has tables; preserved".into());
                }
                let store = crate::legacy_json::read_legacy(&self.0)?;
                conn.execute_batch(
                    "CREATE TABLE metadata (id TEXT PRIMARY KEY CHECK(id='store'), body TEXT NOT NULL);
                     CREATE TABLE agents (id TEXT PRIMARY KEY, body TEXT NOT NULL);
                     CREATE TABLE activities (id INTEGER PRIMARY KEY, body TEXT NOT NULL);
                     CREATE TABLE requests (id TEXT PRIMARY KEY, body TEXT NOT NULL);
                     CREATE TABLE launches (id TEXT PRIMARY KEY, body TEXT NOT NULL);
                     CREATE TABLE recent_directories (id INTEGER PRIMARY KEY, body TEXT NOT NULL);"
                ).map_err(error)?;
                persist(&conn, &Store::default(), &store)?;
                conn.pragma_update(None, "user_version", DATABASE_VERSION)
                    .map_err(error)?;
                Ok(())
            })();
            match initialized {
                Ok(()) => conn.execute_batch("COMMIT").map_err(error)?,
                Err(e) => {
                    let _ = conn.execute_batch("ROLLBACK");
                    return Err(e);
                }
            }
        }
        Ok(conn)
    }
}

// Changing journal mode can return BUSY without invoking SQLite's busy
// handler. Retry this initialization-only operation within the same 2s bound.
fn enable_wal(conn: &Connection) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match conn.query_row("PRAGMA journal_mode=WAL", [], |r| r.get::<_, String>(0)) {
            Ok(mode) if mode == "wal" => return Ok(()),
            Ok(mode) => return Err(format!("WAL unavailable: {mode}")),
            Err(rusqlite::Error::SqliteFailure(e, _))
                if matches!(
                    e.code,
                    rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
                ) && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(5))
            }
            Err(e) => return Err(error(e)),
        }
    }
}

fn database_version(conn: &Connection) -> Result<i64, String> {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(error)
}

impl Repository for SqliteRepository {
    fn read(&self) -> Result<Store, String> {
        let conn = self.connect()?;
        let tx = conn.unchecked_transaction().map_err(error)?;
        let store = load(&tx)?;
        tx.commit().map_err(error)?;
        Ok(store)
    }

    fn begin(&self) -> Result<UnitOfWork, String> {
        let conn = self.connect()?;
        conn.execute_batch("BEGIN IMMEDIATE").map_err(error)?;
        let previous = load(&conn)?;
        Ok(UnitOfWork::new(
            previous.clone(),
            Box::new(SqliteTransaction { conn, previous }),
        ))
    }
}

struct SqliteTransaction {
    conn: Connection,
    previous: Store,
}

impl TransactionBackend for SqliteTransaction {
    fn commit(&mut self, store: &Store) -> Result<(), String> {
        // Reject a newer domain schema rather than saving unreadable data.
        let mut validated = store.clone();
        validated.migrate()?;
        persist(&self.conn, &self.previous, &validated)?;
        self.conn.execute_batch("COMMIT").map_err(error)
    }
}

impl Drop for SqliteTransaction {
    fn drop(&mut self) {
        if !self.conn.is_autocommit() {
            let _ = self.conn.execute_batch("ROLLBACK");
        }
    }
}

fn records<T: DeserializeOwned>(
    conn: &Connection,
    table: &str,
) -> Result<Vec<(String, T)>, String> {
    // Table names are internal constants, never external input.
    let mut query = conn
        .prepare(&format!(
            "SELECT CAST(id AS TEXT), body FROM {table} ORDER BY id"
        ))
        .map_err(error)?;
    let rows = query
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(error)?;
    rows.map(|row| {
        let (id, body) = row.map_err(error)?;
        Ok((id, serde_json::from_str(&body).map_err(error)?))
    })
    .collect()
}

fn load(conn: &Connection) -> Result<Store, String> {
    let body: String = conn
        .query_row("SELECT body FROM metadata WHERE id='store'", [], |r| {
            r.get(0)
        })
        .map_err(error)?;
    let mut store: Store = serde_json::from_str(&body).map_err(error)?;
    store.agents = records(conn, "agents")?.into_iter().collect();
    store.activities = records(conn, "activities")?
        .into_iter()
        .map(|(_, v)| v)
        .collect();
    store.requests = records(conn, "requests")?.into_iter().collect();
    store.launches = records(conn, "launches")?.into_iter().collect();
    store.recent_directories = records(conn, "recent_directories")?
        .into_iter()
        .map(|(_, v)| v)
        .collect();
    store.migrate()?;
    Ok(store)
}

fn metadata(store: &Store) -> Result<String, String> {
    let mut value = store.clone();
    value.agents.clear();
    value.activities.clear();
    value.requests.clear();
    value.launches.clear();
    value.recent_directories.clear();
    serde_json::to_string(&value).map_err(error)
}

fn encoded<T: Serialize>(
    values: impl IntoIterator<Item = (String, T)>,
) -> Result<BTreeMap<String, String>, String> {
    values
        .into_iter()
        .map(|(id, v)| Ok((id, serde_json::to_string(&v).map_err(error)?)))
        .collect()
}

fn sync(
    conn: &Connection,
    table: &str,
    old: BTreeMap<String, String>,
    new: BTreeMap<String, String>,
) -> Result<(), String> {
    let mut delete = conn
        .prepare(&format!("DELETE FROM {table} WHERE id=?1"))
        .map_err(error)?;
    for id in old.keys().filter(|id| !new.contains_key(*id)) {
        delete.execute([id]).map_err(error)?;
    }
    let mut upsert = conn.prepare(&format!(
        "INSERT INTO {table} (id, body) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET body=excluded.body"
    )).map_err(error)?;
    for (id, body) in new {
        if old.get(&id) != Some(&body) {
            upsert.execute(params![id, body]).map_err(error)?;
        }
    }
    Ok(())
}

fn persist(conn: &Connection, old: &Store, new: &Store) -> Result<(), String> {
    conn.execute("INSERT INTO metadata (id, body) VALUES ('store', ?1) ON CONFLICT(id) DO UPDATE SET body=excluded.body", [metadata(new)?]).map_err(error)?;
    sync(
        conn,
        "agents",
        encoded(old.agents.iter().map(|(k, v)| (k.clone(), v)))?,
        encoded(new.agents.iter().map(|(k, v)| (k.clone(), v)))?,
    )?;
    sync(
        conn,
        "requests",
        encoded(old.requests.iter().map(|(k, v)| (k.clone(), v)))?,
        encoded(new.requests.iter().map(|(k, v)| (k.clone(), v)))?,
    )?;
    sync(
        conn,
        "launches",
        encoded(old.launches.iter().map(|(k, v)| (k.clone(), v)))?,
        encoded(new.launches.iter().map(|(k, v)| (k.clone(), v)))?,
    )?;
    sync(
        conn,
        "activities",
        encoded(
            old.activities
                .iter()
                .enumerate()
                .map(|(k, v)| (k.to_string(), v)),
        )?,
        encoded(
            new.activities
                .iter()
                .enumerate()
                .map(|(k, v)| (k.to_string(), v)),
        )?,
    )?;
    sync(
        conn,
        "recent_directories",
        encoded(
            old.recent_directories
                .iter()
                .enumerate()
                .map(|(k, v)| (k.to_string(), v)),
        )?,
        encoded(
            new.recent_directories
                .iter()
                .enumerate()
                .map(|(k, v)| (k.to_string(), v)),
        )?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashboard_core::{Action, ActionRequest, LaunchInfo, ScanLease};
    use std::sync::{Arc, Barrier};

    fn fixture() -> Store {
        let inventory = crate::process::parse_inventory(
            "10 1 Mon Oct 5 10:00:00 2026 zellij --server /tmp/dev\n20 10 Mon Oct 5 10:01:00 2026 /bin/codex ZELLIJ_SESSION_NAME=dev ZELLIJ_PANE_ID=7 PWD=/tmp\n"
        ).unwrap();
        let mut store = Store::default();
        store.reconcile(&inventory.found, 10_000);
        let id = store.agents.keys().next().unwrap().clone();
        store.set_pinned(&id, true).unwrap();
        store.agents.get_mut(&id).unwrap().alias = "한글 태그".into();
        store.remember_directory("/tmp/한글");
        store
            .claim(
                &ActionRequest {
                    request_id: "pending".into(),
                    action: Action::Launch {
                        session: "dev".into(),
                        epoch: "epoch".into(),
                        cwd: "/tmp".into(),
                        tool: "codex".into(),
                    },
                },
                10_001,
            )
            .unwrap();
        store.launches.insert(
            "pending".into(),
            LaunchInfo {
                parent_id: Some(id),
                session: "dev".into(),
                epoch: "epoch".into(),
                cwd: "/tmp".into(),
                tool: "codex".into(),
                pane_id: Some(8),
                agent_id: None,
            },
        );
        store.scan_lease = Some(ScanLease {
            token: "expired".into(),
            expires_at_ms: 1,
        });
        store
    }

    #[test]
    fn concurrent_initialization_and_writers_keep_all_updates() {
        let dir = tempfile::tempdir().unwrap();
        let initial = fixture();
        let bytes = serde_json::to_vec(&initial).unwrap();
        fs::write(dir.path().join("store.json"), &bytes).unwrap();
        let barrier = Arc::new(Barrier::new(8));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let path = dir.path().to_owned();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    let repo = SqliteRepository(path);
                    let mut tx = repo.begin().unwrap();
                    tx.store.revision += 1;
                    tx.commit().unwrap();
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(
            SqliteRepository(dir.path().into()).read().unwrap().revision,
            initial.revision + 8
        );
        assert_eq!(fs::read(dir.path().join("store.json")).unwrap(), bytes);
        assert_eq!(
            fs::metadata(dir.path().join("store.sqlite3"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(dir.path()).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }

    #[test]
    fn reads_during_write_see_committed_state_and_drop_rolls_back() {
        let dir = tempfile::tempdir().unwrap();
        let repo = SqliteRepository(dir.path().into());
        let mut tx = repo.begin().unwrap();
        tx.store.revision = 42;
        for name in ["store.sqlite3-wal", "store.sqlite3-shm"] {
            assert_eq!(
                fs::metadata(dir.path().join(name))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        assert_eq!(repo.read().unwrap().revision, 0);
        drop(tx);
        assert_eq!(repo.read().unwrap().revision, 0);
        let mut tx = repo.begin().unwrap();
        tx.store = fixture();
        let expected = serde_json::to_value(&tx.store).unwrap();
        tx.commit().unwrap();
        assert!(tx.commit().is_err());
        assert_eq!(
            serde_json::to_value(repo.read().unwrap()).unwrap(),
            expected
        );
        // A committed unit still in scope must not hold the writer reservation.
        let _next = repo.begin().unwrap();
    }

    #[test]
    fn import_is_atomic_once_and_preserves_legacy_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.json");
        let store = fixture();
        let bytes = serde_json::to_vec(&store).unwrap();
        fs::write(&path, &bytes).unwrap();
        let repo = SqliteRepository(dir.path().into());
        assert_eq!(
            serde_json::to_value(repo.read().unwrap()).unwrap(),
            serde_json::to_value(&store).unwrap()
        );
        assert_eq!(fs::read(&path).unwrap(), bytes);
        let mut tx = repo.begin().unwrap();
        tx.store.revision += 10;
        tx.commit().unwrap();
        // After successful import SQLite is authoritative, even if JSON changes.
        fs::write(&path, "broken legacy file").unwrap();
        assert_eq!(repo.read().unwrap().revision, store.revision + 10);
    }

    #[test]
    fn legacy_v1_import_restores_hook_source() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = fixture();
        store.schema_version = 1;
        let agent = store.agents.values_mut().next().unwrap();
        agent.sequence = 7;
        agent.last_report_ms = Some(123);
        fs::write(
            dir.path().join("store.json"),
            serde_json::to_vec(&store).unwrap(),
        )
        .unwrap();
        let restored = SqliteRepository(dir.path().into()).read().unwrap();
        assert_eq!(restored.schema_version, 2);
        let agent = restored.agents.values().next().unwrap();
        assert_eq!(agent.status_source, dashboard_core::StatusSource::Hook);
        assert_eq!(agent.last_hook_report_ms, Some(123));
    }

    #[test]
    fn corrupt_and_newer_legacy_files_are_preserved_and_import_can_retry() {
        for bytes in [
            b"{incomplete".to_vec(),
            serde_json::to_vec(&Store {
                schema_version: 999,
                ..Store::default()
            })
            .unwrap(),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("store.json");
            fs::write(&path, &bytes).unwrap();
            let repo = SqliteRepository(dir.path().into());
            assert!(repo.read().is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes);
            let conn = Connection::open(dir.path().join("store.sqlite3")).unwrap();
            assert_eq!(database_version(&conn).unwrap(), 0);
            assert_eq!(
                conn.query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
                0
            );
            fs::write(&path, serde_json::to_vec(&fixture()).unwrap()).unwrap();
            assert!(!repo.read().unwrap().agents.is_empty());
        }
    }

    #[test]
    fn corrupt_and_newer_databases_are_not_reinitialized() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.sqlite3");
        fs::write(&path, b"broken sqlite").unwrap();
        let repo = SqliteRepository(dir.path().into());
        assert!(repo.read().is_err());
        assert_eq!(fs::read(&path).unwrap(), b"broken sqlite");
        fs::remove_file(&path).unwrap();
        let conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", 999).unwrap();
        drop(conn);
        let bytes = fs::read(&path).unwrap();
        assert!(repo.read().is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }

    #[test]
    fn unknown_domain_schema_and_unversioned_tables_are_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let repo = SqliteRepository(dir.path().into());
        let conn = repo.connect().unwrap();
        let body = serde_json::to_string(&Store {
            schema_version: 999,
            ..Store::default()
        })
        .unwrap();
        conn.execute("UPDATE metadata SET body=?1 WHERE id='store'", [&body])
            .unwrap();
        assert!(repo.read().is_err());
        assert!(repo.begin().is_err());
        assert_eq!(
            conn.query_row("SELECT body FROM metadata", [], |r| r.get::<_, String>(0))
                .unwrap(),
            body
        );

        let other = tempfile::tempdir().unwrap();
        let conn = Connection::open(other.path().join("store.sqlite3")).unwrap();
        conn.execute_batch(
            "CREATE TABLE unrelated (body TEXT); INSERT INTO unrelated VALUES ('keep');",
        )
        .unwrap();
        assert!(SqliteRepository(other.path().into()).read().is_err());
        assert_eq!(database_version(&conn).unwrap(), 0);
        assert_eq!(
            conn.query_row("SELECT body FROM unrelated", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "keep"
        );
    }

    #[test]
    fn row_updates_delete_removed_records_and_leave_unchanged_rows_alone() {
        let dir = tempfile::tempdir().unwrap();
        let repo = SqliteRepository(dir.path().into());
        let mut tx = repo.begin().unwrap();
        tx.store = fixture();
        tx.commit().unwrap();
        let conn = repo.connect().unwrap();
        conn.execute_batch("CREATE TRIGGER preserve_agent BEFORE UPDATE ON agents BEGIN SELECT RAISE(ABORT, 'unchanged agent rewritten'); END;").unwrap();
        let mut tx = repo.begin().unwrap();
        tx.store.requests.clear();
        tx.store.launches.clear();
        tx.store.activities.clear();
        tx.store.recent_directories.clear();
        tx.store.revision += 1;
        tx.commit().unwrap();
        let restored = repo.read().unwrap();
        assert!(!restored.agents.is_empty());
        assert!(
            restored.requests.is_empty()
                && restored.launches.is_empty()
                && restored.activities.is_empty()
                && restored.recent_directories.is_empty()
        );
    }

    #[test]
    fn failed_commit_rolls_back_all_tables() {
        let dir = tempfile::tempdir().unwrap();
        let repo = SqliteRepository(dir.path().into());
        let conn = repo.connect().unwrap();
        conn.execute_batch("CREATE TRIGGER fail_request BEFORE INSERT ON requests BEGIN SELECT RAISE(ABORT, 'forced failure'); END;").unwrap();
        let mut tx = repo.begin().unwrap();
        tx.store = fixture();
        assert!(tx.commit().is_err());
        drop(tx);
        let restored = repo.read().unwrap();
        assert_eq!(restored.revision, 0);
        assert!(restored.agents.is_empty() && restored.activities.is_empty());
    }
}
