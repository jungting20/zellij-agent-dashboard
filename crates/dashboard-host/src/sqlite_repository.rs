//! Native SQLite adapter. SQL and filesystem details never enter the core.
use crate::repository::{CatalogState, Repository, TransactionBackend, UnitOfWork};
use dashboard_core::{ActionResult, Agent, Snapshot, Store, StoreData};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::PathBuf,
    time::{Duration, Instant},
};

const DATABASE_VERSION: i64 = 2;

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
        if !(0..=DATABASE_VERSION).contains(&version) {
            return Err(format!("unsupported database version {version}; preserved"));
        }
        // Configure WAL only during initialization; ordinary readers never take
        // a writer reservation. SQLite sidecars inherit the database permissions.
        if version < DATABASE_VERSION {
            if version == 0 {
                enable_wal(&conn)?;
            }
            conn.execute_batch("BEGIN IMMEDIATE").map_err(error)?;
            let initialized = (|| {
                let current = database_version(&conn)?;
                if current == DATABASE_VERSION {
                    return Ok(());
                }
                if current == 1 {
                    Store::restore(header(&conn)?)?;
                    conn.execute_batch(
                        "ALTER TABLE requests ADD COLUMN at_ms INTEGER NOT NULL DEFAULT 0;",
                    )
                    .map_err(error)?;
                    for (id, record) in records::<ActionResult>(&conn, "requests")? {
                        conn.execute(
                            "UPDATE requests SET at_ms=?1 WHERE id=?2",
                            params![i64::try_from(record.at_ms).map_err(error)?, id],
                        )
                        .map_err(error)?;
                    }
                    conn.execute_batch(
                        "CREATE INDEX requests_recent ON requests(at_ms DESC, id ASC);",
                    )
                    .map_err(error)?;
                    conn.pragma_update(None, "user_version", DATABASE_VERSION)
                        .map_err(error)?;
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
                     CREATE TABLE requests (id TEXT PRIMARY KEY, body TEXT NOT NULL, at_ms INTEGER NOT NULL);
                     CREATE INDEX requests_recent ON requests(at_ms DESC, id ASC);
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
    fn query<T>(&self, read: impl FnOnce(&Connection) -> Result<T, String>) -> Result<T, String> {
        let conn = self.connect()?;
        let tx = conn.unchecked_transaction().map_err(error)?;
        let value = read(&tx)?;
        tx.commit().map_err(error)?;
        Ok(value)
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

    fn snapshot(&self, at: u64) -> Result<Snapshot, String> {
        self.query(|conn| Ok(load_runtime(conn, false)?.snapshot(at)))
    }
    fn agent(&self, id: &str) -> Result<Option<Agent>, String> {
        self.query(|conn| {
            let mut data = header(conn)?;
            if let Some(agent) = record::<Agent>(conn, "agents", id)? {
                data.agents.insert(id.into(), agent);
            }
            Ok(Store::restore(data)?.agents.get(id).cloned())
        })
    }
    fn request(&self, id: &str) -> Result<Option<ActionResult>, String> {
        self.query(|conn| {
            Store::restore(header(conn)?)?;
            record(conn, "requests", id)
        })
    }
    fn recent_requests(&self, limit: usize) -> Result<Vec<ActionResult>, String> {
        self.query(|conn| {
            Store::restore(header(conn)?)?;
            let mut query = conn
                .prepare("SELECT body FROM requests ORDER BY at_ms DESC, id ASC LIMIT ?1")
                .map_err(error)?;
            let rows = query
                .query_map([limit.min(4096) as i64], |row| row.get::<_, String>(0))
                .map_err(error)?;
            rows.map(|row| serde_json::from_str(&row.map_err(error)?).map_err(error))
                .collect()
        })
    }
    fn catalog(&self) -> Result<CatalogState, String> {
        self.query(|conn| {
            let mut data = header(conn)?;
            data.agents = records(conn, "agents")?.into_iter().collect();
            data.recent_directories = records(conn, "recent_directories")?
                .into_iter()
                .map(|(_, value)| value)
                .collect();
            let store = Store::restore(data)?;
            Ok(CatalogState {
                directories: store.recent_directories.clone(),
                agents: store.agents.values().cloned().collect(),
            })
        })
    }
    fn runtime(&self) -> Result<Store, String> {
        self.query(|conn| load_runtime(conn, true))
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

fn header(conn: &Connection) -> Result<StoreData, String> {
    let body: String = conn
        .query_row("SELECT body FROM metadata WHERE id='store'", [], |row| {
            row.get(0)
        })
        .map_err(error)?;
    serde_json::from_str(&body).map_err(error)
}

fn record<T: DeserializeOwned>(
    conn: &Connection,
    table: &str,
    id: &str,
) -> Result<Option<T>, String> {
    let body: Option<String> = conn
        .query_row(
            &format!("SELECT body FROM {table} WHERE id=?1"),
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(error)?;
    body.map(|body| serde_json::from_str(&body).map_err(error))
        .transpose()
}

fn load_runtime(conn: &Connection, relationships: bool) -> Result<Store, String> {
    let mut data = header(conn)?;
    data.agents = records(conn, "agents")?.into_iter().collect();
    data.activities = records(conn, "activities")?
        .into_iter()
        .map(|(_, value)| value)
        .collect();
    if relationships {
        data.launches = records(conn, "launches")?.into_iter().collect();
        data.recent_directories = records(conn, "recent_directories")?
            .into_iter()
            .map(|(_, value)| value)
            .collect();
    }
    Store::restore(data)
}

fn load(conn: &Connection) -> Result<Store, String> {
    let mut store = header(conn)?;
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
    Store::restore(store)
}

fn metadata(store: &Store) -> Result<String, String> {
    let mut value = store.clone().into_data();
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
    for (id, body) in new {
        if old.get(&id) != Some(&body) {
            if table == "requests" {
                let record: ActionResult = serde_json::from_str(&body).map_err(error)?;
                conn.execute("INSERT INTO requests(id, body, at_ms) VALUES (?1, ?2, ?3) ON CONFLICT(id) DO UPDATE SET body=excluded.body, at_ms=excluded.at_ms", params![id, body, i64::try_from(record.at_ms).map_err(error)?]).map_err(error)?;
            } else {
                conn.execute(&format!("INSERT INTO {table} (id, body) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET body=excluded.body"), params![id, body]).map_err(error)?;
            }
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
        let target = store.agents[&id].identity.clone();
        store.set_alias(&target, "한글 태그").unwrap();
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
        store
            .record_launch(
                "pending",
                LaunchInfo {
                    parent_id: Some(id),
                    session: "dev".into(),
                    epoch: "epoch".into(),
                    cwd: "/tmp".into(),
                    tool: "codex".into(),
                    pane_id: Some(8),
                    agent_id: None,
                },
            )
            .unwrap();
        crate::repository::edit_fixture(&mut store, |data| {
            data.scan_lease = Some(ScanLease {
                token: "expired".into(),
                expires_at_ms: 1,
            })
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
                    crate::repository::edit_fixture(&mut tx.store, |data| data.revision += 1);
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
        crate::repository::edit_fixture(&mut tx.store, |data| data.revision = 42);
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
        crate::repository::edit_fixture(&mut tx.store, |data| data.revision += 10);
        tx.commit().unwrap();
        // After successful import SQLite is authoritative, even if JSON changes.
        fs::write(&path, "broken legacy file").unwrap();
        assert_eq!(repo.read().unwrap().revision, store.revision + 10);
    }

    #[test]
    fn legacy_v1_import_restores_hook_source() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = fixture();
        crate::repository::edit_fixture(&mut store, |data| {
            data.schema_version = 1;
            let agent = data.agents.values_mut().next().unwrap();
            agent.sequence = 7;
            agent.last_report_ms = Some(123);
        });
        fs::write(
            dir.path().join("store.json"),
            serde_json::to_vec(&store).unwrap(),
        )
        .unwrap();
        let restored = SqliteRepository(dir.path().into()).read().unwrap();
        assert_eq!(restored.schema_version, dashboard_core::SCHEMA_VERSION);
        let agent = restored.agents.values().next().unwrap();
        assert_eq!(agent.status_source, dashboard_core::StatusSource::Hook);
        assert_eq!(agent.last_hook_report_ms, Some(123));
    }

    #[test]
    fn corrupt_and_newer_legacy_files_are_preserved_and_import_can_retry() {
        for bytes in [
            b"{incomplete".to_vec(),
            serde_json::to_vec(&dashboard_core::StoreData {
                schema_version: 999,
                ..dashboard_core::StoreData::default()
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
        let body = serde_json::to_string(&dashboard_core::StoreData {
            schema_version: 999,
            ..dashboard_core::StoreData::default()
        })
        .unwrap();
        conn.execute("UPDATE metadata SET body=?1 WHERE id='store'", [&body])
            .unwrap();
        assert!(repo.read().is_err());
        assert!(repo.begin().is_err());
        assert!(repo.snapshot(1).is_err());
        assert!(repo.agent("missing").is_err());
        assert!(repo.request("missing").is_err());
        assert!(repo.catalog().is_err());
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
        crate::repository::edit_fixture(&mut tx.store, |data| {
            data.requests.clear();
            data.launches.clear();
            data.activities.clear();
            data.recent_directories.clear();
        });
        crate::repository::edit_fixture(&mut tx.store, |data| data.revision += 1);
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
    fn downgrade_to_v1(conn: &Connection) {
        conn.execute_batch(
            "DROP INDEX requests_recent;
            ALTER TABLE requests RENAME TO requests_v2;
            CREATE TABLE requests(id TEXT PRIMARY KEY, body TEXT NOT NULL);
            INSERT INTO requests(id, body) SELECT id, body FROM requests_v2;
            DROP TABLE requests_v2; PRAGMA user_version=1;",
        )
        .unwrap();
    }

    #[test]
    fn v1_migration_backfills_index_and_preserves_requests_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let repo = SqliteRepository(dir.path().into());
        let mut tx = repo.begin().unwrap();
        tx.store = fixture();
        tx.commit().unwrap();
        let conn = repo.connect().unwrap();
        let original: String = conn
            .query_row("SELECT body FROM requests WHERE id='pending'", [], |r| {
                r.get(0)
            })
            .unwrap();
        downgrade_to_v1(&conn);
        let result = repo.request("pending").unwrap().unwrap();
        assert_eq!(result.at_ms, 10001);
        assert_eq!(database_version(&conn).unwrap(), 2);
        assert_eq!(
            conn.query_row("SELECT body FROM requests", [], |r| r.get::<_, String>(0))
                .unwrap(),
            original
        );
        assert_eq!(
            conn.query_row("SELECT at_ms FROM requests", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            10001
        );
        let plan: String = conn
            .query_row(
                "EXPLAIN QUERY PLAN SELECT body FROM requests ORDER BY at_ms DESC, id ASC LIMIT 50",
                [],
                |r| r.get(3),
            )
            .unwrap();
        assert!(plan.contains("requests_recent"), "{plan}");
    }

    #[test]
    fn failed_v1_migration_rolls_back_columns_version_and_original_body() {
        let dir = tempfile::tempdir().unwrap();
        let repo = SqliteRepository(dir.path().into());
        let conn = repo.connect().unwrap();
        downgrade_to_v1(&conn);
        conn.execute("INSERT INTO requests VALUES ('broken', '{bad')", [])
            .unwrap();
        assert!(repo.request("broken").is_err());
        assert_eq!(database_version(&conn).unwrap(), 1);
        assert!(conn.prepare("SELECT at_ms FROM requests").is_err());
        assert_eq!(
            conn.query_row("SELECT body FROM requests", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "{bad"
        );
        conn.execute("DELETE FROM requests", []).unwrap();
        assert!(repo.snapshot(10000).is_ok());
        assert_eq!(database_version(&conn).unwrap(), 2);
    }

    #[test]
    fn purpose_specific_reads_skip_unrelated_payloads_and_keep_committed_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let repo = SqliteRepository(dir.path().into());
        let mut tx = repo.begin().unwrap();
        tx.store = fixture();
        tx.commit().unwrap();
        let id = repo.snapshot(10000).unwrap().agents[0]
            .identity
            .agent_id
            .clone();
        let conn = repo.connect().unwrap();
        conn.execute("INSERT INTO requests VALUES ('unrelated', '{bad', 0)", [])
            .unwrap();
        assert!(repo.read().is_err());
        assert_eq!(repo.snapshot(10000).unwrap().agents.len(), 1);
        assert!(repo.agent(&id).unwrap().is_some());
        assert!(repo.catalog().is_ok());
        assert_eq!(repo.runtime().unwrap().requests.len(), 0);
        assert!(repo.request("pending").unwrap().is_some());
        assert_eq!(
            repo.recent_requests(1).unwrap()[0].request.request_id,
            "pending"
        );
        assert!(repo.request("unrelated").is_err());
        conn.execute("DELETE FROM requests WHERE id='unrelated'", [])
            .unwrap();
        let mut tx = repo.begin().unwrap();
        tx.store.set_pinned(&id, false).unwrap();
        assert!(repo.snapshot(10000).unwrap().agents[0].pinned);
        tx.commit().unwrap();
        assert!(!repo.snapshot(10000).unwrap().agents[0].pinned);
    }

    #[test]
    fn recent_requests_apply_limit_and_stable_tie_order() {
        let dir = tempfile::tempdir().unwrap();
        let repo = SqliteRepository(dir.path().into());
        let mut tx = repo.begin().unwrap();
        for (id, at) in [("c", 20), ("b", 20), ("a", 10)] {
            tx.store
                .claim(
                    &ActionRequest {
                        request_id: id.into(),
                        action: Action::Launch {
                            session: "s".into(),
                            epoch: "e".into(),
                            cwd: "/tmp".into(),
                            tool: "codex".into(),
                        },
                    },
                    at,
                )
                .unwrap();
        }
        tx.commit().unwrap();
        assert!(repo.recent_requests(0).unwrap().is_empty());
        let ids: Vec<_> = repo
            .recent_requests(2)
            .unwrap()
            .into_iter()
            .map(|r| r.request.request_id)
            .collect();
        assert_eq!(ids, ["b", "c"]);
        assert!(repo.request("missing").unwrap().is_none());
    }
}
