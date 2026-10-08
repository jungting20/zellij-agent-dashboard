//! Native SQLite adapter. SQL and filesystem details never enter the core.
use crate::repository::{CatalogState, Repository, TransactionBackend, UnitOfWork, WriteScope};
use dashboard_core::{ActionResult, Agent, Snapshot, Store, StoreData};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use std::{
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
                    Store::loaded(header(&conn)?, None)?;
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
                persist_full(&conn, &store)?;
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
    fn begin_scope(&self, scope: WriteScope) -> Result<UnitOfWork, String> {
        let conn = self.connect()?;
        conn.execute_batch("BEGIN IMMEDIATE").map_err(error)?;
        let store = match &scope {
            WriteScope::Full => load(&conn)?,
            WriteScope::Runtime => load_runtime(&conn, true)?,
            WriteScope::Action(id) => {
                let mut data = runtime_data(&conn, true)?;
                if let Some(request) = record(&conn, "requests", id)? {
                    data.requests.insert(id.clone(), request);
                }
                let count: i64 = conn
                    .query_row("SELECT count(*) FROM requests", [], |row| row.get(0))
                    .map_err(error)?;
                Store::loaded(data, Some(usize::try_from(count).map_err(error)?))?
            }
            WriteScope::Request(id) => {
                let mut data = header(&conn)?;
                if let Some(request) = record(&conn, "requests", id)? {
                    data.requests.insert(id.clone(), request);
                }
                if let Some(launch) = record(&conn, "launches", id)? {
                    data.launches.insert(id.clone(), launch);
                }
                data.recent_directories = records(&conn, "recent_directories")?
                    .into_iter()
                    .map(|(_, value)| value)
                    .collect();
                // Preserve migration changes for agent defaults even in a request transaction.
                if data.schema_version < dashboard_core::SCHEMA_VERSION {
                    data.agents = records(&conn, "agents")?.into_iter().collect();
                }
                Store::loaded(data, None)?
            }
        };
        Ok(UnitOfWork::new(
            store,
            Box::new(SqliteTransaction { conn, scope }),
        ))
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
            Ok(Store::loaded(data, None)?.agents.get(id).cloned())
        })
    }
    fn request(&self, id: &str) -> Result<Option<ActionResult>, String> {
        self.query(|conn| {
            Store::loaded(header(conn)?, None)?;
            record(conn, "requests", id)
        })
    }
    fn recent_requests(&self, limit: usize) -> Result<Vec<ActionResult>, String> {
        self.query(|conn| {
            Store::loaded(header(conn)?, None)?;
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
            let store = Store::loaded(data, None)?;
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
        self.begin_scope(WriteScope::Full)
    }
    fn begin_runtime(&self) -> Result<UnitOfWork, String> {
        self.begin_scope(WriteScope::Runtime)
    }
    fn begin_action(&self, id: &str) -> Result<UnitOfWork, String> {
        self.begin_scope(WriteScope::Action(id.into()))
    }
    fn begin_request(&self, id: &str) -> Result<UnitOfWork, String> {
        self.begin_scope(WriteScope::Request(id.into()))
    }
}

struct SqliteTransaction {
    conn: Connection,
    scope: WriteScope,
}

impl TransactionBackend for SqliteTransaction {
    fn commit(&mut self, store: &Store) -> Result<(), String> {
        store.check_version()?;
        validate_scope(store, &self.scope)?;
        if store.changes().replacement {
            prune_replacement(&self.conn, store)?;
        }
        persist_changes(&self.conn, store)?;
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

fn runtime_data(conn: &Connection, relationships: bool) -> Result<StoreData, String> {
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
    Ok(data)
}

fn load_runtime(conn: &Connection, relationships: bool) -> Result<Store, String> {
    Store::loaded(runtime_data(conn, relationships)?, None)
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
    Store::loaded(store, None)
}

fn metadata(store: &Store) -> Result<String, String> {
    serde_json::to_string(&store.metadata()).map_err(error)
}

fn validate_scope(store: &Store, scope: &WriteScope) -> Result<(), String> {
    let changes = store.changes();
    if changes.replacement && !matches!(scope, WriteScope::Full) {
        return Err("full restoration requires a full transaction".into());
    }
    match scope {
        WriteScope::Full => Ok(()),
        WriteScope::Runtime if changes.requests.is_empty() => Ok(()),
        WriteScope::Action(id) | WriteScope::Request(id)
            if changes.requests.iter().all(|key| key == id) =>
        {
            Ok(())
        }
        _ => Err("request mutation outside transaction scope".into()),
    }
}

fn prune_replacement(conn: &Connection, store: &Store) -> Result<(), String> {
    // Explicit full restoration only. Partial scopes never infer deletions.
    for (table, ids) in [
        (
            "agents",
            store
                .agents
                .keys()
                .collect::<std::collections::BTreeSet<_>>(),
        ),
        ("requests", store.requests.keys().collect()),
        ("launches", store.launches.keys().collect()),
    ] {
        let mut query = conn
            .prepare(&format!("SELECT id FROM {table}"))
            .map_err(error)?;
        let existing = query
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(error)?;
        let remove: Vec<_> = existing
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?
            .into_iter()
            .filter(|id| !ids.contains(id))
            .collect();
        for id in remove {
            conn.execute(&format!("DELETE FROM {table} WHERE id=?1"), [id])
                .map_err(error)?;
        }
    }
    Ok(())
}

fn upsert<T: Serialize>(conn: &Connection, table: &str, id: &str, value: &T) -> Result<(), String> {
    let body = serde_json::to_string(value).map_err(error)?;
    conn.execute(&format!("INSERT INTO {table}(id, body) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET body=excluded.body WHERE body<>excluded.body"), params![id, body]).map_err(error)?;
    Ok(())
}

fn replace_list<T: Serialize>(conn: &Connection, table: &str, values: &[T]) -> Result<(), String> {
    // Activity and directory lists are bounded to 50 and 100 records respectively.
    conn.execute(&format!("DELETE FROM {table}"), [])
        .map_err(error)?;
    for (index, value) in values.iter().enumerate() {
        upsert(conn, table, &index.to_string(), value)?;
    }
    Ok(())
}

fn persist_changes(conn: &Connection, store: &Store) -> Result<(), String> {
    let changes = store.changes();
    if changes.metadata {
        conn.execute("INSERT INTO metadata(id, body) VALUES ('store', ?1) ON CONFLICT(id) DO UPDATE SET body=excluded.body WHERE body<>excluded.body", [metadata(store)?]).map_err(error)?;
    }
    for id in &changes.removed_agents {
        conn.execute("DELETE FROM agents WHERE id=?1", [id])
            .map_err(error)?;
    }
    for id in &changes.agents {
        upsert(
            conn,
            "agents",
            id,
            store.agents.get(id).ok_or("changed agent missing")?,
        )?;
    }
    for id in &changes.launches {
        upsert(
            conn,
            "launches",
            id,
            store.launches.get(id).ok_or("changed launch missing")?,
        )?;
    }
    for id in &changes.requests {
        let record = store.requests.get(id).ok_or("changed request missing")?;
        let body = serde_json::to_string(record).map_err(error)?;
        conn.execute("INSERT INTO requests(id, body, at_ms) VALUES (?1, ?2, ?3) ON CONFLICT(id) DO UPDATE SET body=excluded.body, at_ms=excluded.at_ms WHERE body<>excluded.body OR at_ms<>excluded.at_ms", params![id, body, i64::try_from(record.at_ms).map_err(error)?]).map_err(error)?;
    }
    if changes.activities {
        replace_list(conn, "activities", &store.activities)?;
    }
    if changes.recent_directories {
        replace_list(conn, "recent_directories", &store.recent_directories)?;
    }
    Ok(())
}

fn persist_full(conn: &Connection, store: &Store) -> Result<(), String> {
    let restored = Store::restore(store.clone().into_data())?;
    prune_replacement(conn, &restored)?;
    persist_changes(conn, &restored)
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
        let mut store = fixture().into_data();
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
    fn launch_request(id: &str) -> ActionRequest {
        ActionRequest {
            request_id: id.into(),
            action: Action::Launch {
                session: "dev".into(),
                epoch: "epoch".into(),
                cwd: "/tmp".into(),
                tool: "codex".into(),
            },
        }
    }

    #[test]
    fn scoped_changes_skip_unrelated_payloads_preserve_rows_and_limit_mutations() {
        let dir = tempfile::tempdir().unwrap();
        let repo = SqliteRepository(dir.path().into());
        let mut tx = repo.begin().unwrap();
        tx.store = fixture();
        tx.commit().unwrap();
        let conn = repo.connect().unwrap();
        conn.execute("INSERT INTO requests VALUES ('unrelated', '{bad', 0)", [])
            .unwrap();
        conn.execute_batch("CREATE TRIGGER preserve_unrelated BEFORE UPDATE ON requests WHEN OLD.id='unrelated' BEGIN SELECT RAISE(ABORT, 'unrelated request changed'); END;").unwrap();
        let id = repo.snapshot(10000).unwrap().agents[0]
            .identity
            .agent_id
            .clone();
        let target = repo.agent(&id).unwrap().unwrap().identity;
        let mut tx = repo.begin_runtime().unwrap();
        assert!(tx.store.requests.is_empty());
        tx.store.set_alias(&target, "갱신").unwrap();
        tx.commit().unwrap();
        assert_eq!(repo.agent(&id).unwrap().unwrap().alias, "갱신");
        // A request-only transaction has no runtime agents or activities.
        conn.execute_batch("CREATE TRIGGER preserve_agents BEFORE UPDATE ON agents BEGIN SELECT RAISE(ABORT, 'agent rewritten by request'); END;").unwrap();
        let mut tx = repo.begin_request("pending").unwrap();
        assert!(tx.store.agents.is_empty());
        assert_eq!(tx.store.requests.len(), 1);
        tx.store
            .finish_request(
                "pending",
                dashboard_core::RequestState::Succeeded,
                "ok".into(),
                Some(8),
                None,
            )
            .unwrap();
        tx.commit().unwrap();
        assert_eq!(
            repo.request("pending").unwrap().unwrap().state,
            dashboard_core::RequestState::Succeeded
        );
        assert_eq!(
            conn.query_row(
                "SELECT body FROM requests WHERE id='unrelated'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "{bad"
        );
        assert_eq!(
            conn.query_row("SELECT count(*) FROM requests", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
        // Full replacement and out-of-scope claims are rejected rather than deleting omitted data.
        let mut tx = repo.begin_request("pending").unwrap();
        tx.store = Store::restore(StoreData::default()).unwrap();
        assert!(tx.commit().is_err());
        drop(tx);
        let mut tx = repo.begin_runtime().unwrap();
        tx.store
            .claim(&launch_request("wrong-scope"), 20000)
            .unwrap();
        assert!(tx.commit().is_err());
        drop(tx);
        assert!(repo.request("wrong-scope").unwrap().is_none());
        assert!(repo.agent(&id).unwrap().is_some());
    }

    #[test]
    fn action_scope_checks_global_limit_and_concurrent_claims_execute_once() {
        let dir = tempfile::tempdir().unwrap();
        let repo = SqliteRepository(dir.path().into());
        let conn = repo.connect().unwrap();
        let mut record = dashboard_core::ActionResult {
            request: launch_request("seed"),
            state: dashboard_core::RequestState::Succeeded,
            at_ms: 1,
            message: "ok".into(),
            pane_id: None,
            path: String::new(),
        };
        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        for index in 0..4096 {
            record.request.request_id = format!("seed-{index}");
            conn.execute(
                "INSERT INTO requests VALUES (?1, ?2, ?3)",
                params![
                    record.request.request_id,
                    serde_json::to_string(&record).unwrap(),
                    1
                ],
            )
            .unwrap();
        }
        conn.execute_batch("COMMIT").unwrap();
        let mut tx = repo.begin_action("new").unwrap();
        assert!(tx.store.requests.is_empty());
        assert_eq!(tx.store.request_count(), 4096);
        assert!(tx.store.claim(&launch_request("new"), 20000).is_err());
        drop(tx);
        let mut tx = repo.begin_action("seed-0").unwrap();
        assert!(!tx.store.claim(&launch_request("seed-0"), 20000).unwrap());
        drop(tx);
        conn.execute("DELETE FROM requests", []).unwrap();
        let barrier = Arc::new(Barrier::new(8));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let dir = dir.path().to_owned();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    let repo = SqliteRepository(dir);
                    let mut tx = repo.begin_action("once").unwrap();
                    let claimed = tx.store.claim(&launch_request("once"), 20000).unwrap();
                    tx.commit().unwrap();
                    claimed
                })
            })
            .collect();
        assert_eq!(
            threads
                .into_iter()
                .map(|thread| thread.join().unwrap() as usize)
                .sum::<usize>(),
            1
        );
        assert_eq!(repo.recent_requests(50).unwrap().len(), 1);
    }

    #[test]
    fn scoped_migration_and_explicit_agent_deletion_preserve_request_history() {
        let dir = tempfile::tempdir().unwrap();
        let repo = SqliteRepository(dir.path().into());
        let mut tx = repo.begin().unwrap();
        tx.store = fixture();
        tx.commit().unwrap();
        let conn = repo.connect().unwrap();
        let id = repo.snapshot(10000).unwrap().agents[0]
            .identity
            .agent_id
            .clone();
        for request_only in [false, true] {
            let mut data = repo.read().unwrap().into_data();
            data.schema_version = 2;
            data.agents.get_mut(&id).unwrap().pane.presence = dashboard_core::PanePresence::Present;
            data.agents.get_mut(&id).unwrap().pane.observed_at_ms = 123;
            let agent = serde_json::to_string(&data.agents[&id]).unwrap();
            let mut header = Store::loaded(data.clone(), None).unwrap().metadata();
            header.schema_version = 2;
            conn.execute(
                "UPDATE metadata SET body=?1",
                [serde_json::to_string(&header).unwrap()],
            )
            .unwrap();
            conn.execute("UPDATE agents SET body=?1 WHERE id=?2", params![agent, id])
                .unwrap();
            let mut tx = if request_only {
                repo.begin_request("pending").unwrap()
            } else {
                repo.begin_action("other").unwrap()
            };
            tx.commit().unwrap();
            let persisted: Agent = serde_json::from_str(
                &conn
                    .query_row("SELECT body FROM agents WHERE id=?1", [&id], |row| {
                        row.get::<_, String>(0)
                    })
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(
                persisted.pane.presence,
                dashboard_core::PanePresence::Unknown
            );
            assert_eq!(persisted.pane.observed_at_ms, 0);
        }
        let mut tx = repo.begin_runtime().unwrap();
        tx.store.reconcile(&[], 100_000_000);
        tx.commit().unwrap();
        assert!(repo.agent(&id).unwrap().is_none());
        assert!(repo.request("pending").unwrap().is_some());
    }

    #[test]
    fn failed_request_patch_rolls_back_metadata_and_preserves_pending_result() {
        let dir = tempfile::tempdir().unwrap();
        let repo = SqliteRepository(dir.path().into());
        let mut tx = repo.begin_action("pending").unwrap();
        tx.store.claim(&launch_request("pending"), 10).unwrap();
        tx.commit().unwrap();
        let revision = repo.snapshot(10).unwrap().revision;
        let conn = repo.connect().unwrap();
        conn.execute_batch("CREATE TRIGGER fail_completion BEFORE UPDATE ON requests BEGIN SELECT RAISE(ABORT, 'forced completion failure'); END;").unwrap();
        let mut tx = repo.begin_request("pending").unwrap();
        tx.store
            .finish_request(
                "pending",
                dashboard_core::RequestState::Succeeded,
                "ok".into(),
                None,
                None,
            )
            .unwrap();
        assert!(tx.commit().is_err());
        drop(tx);
        assert_eq!(repo.snapshot(10).unwrap().revision, revision);
        assert_eq!(
            repo.request("pending").unwrap().unwrap().state,
            dashboard_core::RequestState::Pending
        );
    }

    #[test]
    #[ignore = "manual repository timing experiment; no timing assertions"]
    fn repository_transaction_cost_with_request_history() {
        for count in [0, 4000] {
            let dir = tempfile::tempdir().unwrap();
            let repo = SqliteRepository(dir.path().into());
            let conn = repo.connect().unwrap();
            let mut record = dashboard_core::ActionResult {
                request: launch_request("seed"),
                state: dashboard_core::RequestState::Succeeded,
                at_ms: 1,
                message: "x".repeat(4096),
                pane_id: None,
                path: String::new(),
            };
            conn.execute_batch("BEGIN IMMEDIATE").unwrap();
            for index in 0..count {
                record.request.request_id = format!("seed-{index}");
                conn.execute(
                    "INSERT INTO requests VALUES (?1, ?2, 1)",
                    params![
                        record.request.request_id,
                        serde_json::to_string(&record).unwrap()
                    ],
                )
                .unwrap();
            }
            conn.execute_batch("COMMIT").unwrap();
            for (name, scope) in [("full", WriteScope::Full), ("runtime", WriteScope::Runtime)] {
                let mut samples = Vec::new();
                for index in 0..12 {
                    let start = Instant::now();
                    let mut tx = repo.begin_scope(scope.clone()).unwrap();
                    assert!(tx.store.claim_scan(&format!("sample-{index}"), 20000));
                    tx.commit().unwrap();
                    let mut release = repo.begin_scope(scope.clone()).unwrap();
                    release.store.release_scan(&format!("sample-{index}"));
                    release.commit().unwrap();
                    samples.push(start.elapsed().as_secs_f64() * 1000.0);
                }
                samples.sort_by(f64::total_cmp);
                println!(
                    "requests={count} scope={name} claim+release median_ms={:.3}",
                    (samples[5] + samples[6]) / 2.0
                );
            }
        }
    }
}
