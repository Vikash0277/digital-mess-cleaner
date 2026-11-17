//! SQLite schema, migrations and small query helpers.

use crate::error::AppResult;
use rusqlite::Connection;
use std::path::Path;

pub const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS roots (
  id           INTEGER PRIMARY KEY,
  path         TEXT NOT NULL UNIQUE,
  label        TEXT NOT NULL,
  kind         TEXT NOT NULL DEFAULT 'folder',
  enabled      INTEGER NOT NULL DEFAULT 1,
  created_at   INTEGER NOT NULL,
  last_scan_at INTEGER
);

CREATE TABLE IF NOT EXISTS files (
  id             INTEGER PRIMARY KEY,
  root_id        INTEGER NOT NULL REFERENCES roots(id) ON DELETE CASCADE,
  path           TEXT NOT NULL UNIQUE,
  parent_path    TEXT NOT NULL DEFAULT '',
  name           TEXT NOT NULL,
  ext            TEXT NOT NULL DEFAULT '',
  size           INTEGER NOT NULL DEFAULT 0,
  mtime          INTEGER NOT NULL DEFAULT 0,
  atime          INTEGER NOT NULL DEFAULT 0,
  ctime          INTEGER NOT NULL DEFAULT 0,
  category       TEXT NOT NULL DEFAULT 'other',
  mime           TEXT NOT NULL DEFAULT '',
  content_text   TEXT,
  content_status TEXT NOT NULL DEFAULT 'none',
  hash           TEXT,
  partial_hash   TEXT,
  phash          TEXT,
  width          INTEGER,
  height         INTEGER,
  tags           TEXT NOT NULL DEFAULT '[]',
  doc_type       TEXT,
  vendor         TEXT,
  amount         TEXT,
  invoice_no     TEXT,
  doc_date       TEXT,
  due_date       TEXT,
  confidence     REAL NOT NULL DEFAULT 0,
  sensitive      INTEGER NOT NULL DEFAULT 0,
  sensitive_kind TEXT,
  evidence       TEXT NOT NULL DEFAULT '[]',
  summary        TEXT,
  analyzed_at    INTEGER,
  indexed_at     INTEGER NOT NULL,
  last_seen_at   INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_files_root      ON files(root_id);
CREATE INDEX IF NOT EXISTS idx_files_size      ON files(size DESC);
CREATE INDEX IF NOT EXISTS idx_files_mtime     ON files(mtime);
CREATE INDEX IF NOT EXISTS idx_files_atime     ON files(atime);
CREATE INDEX IF NOT EXISTS idx_files_category  ON files(category);
CREATE INDEX IF NOT EXISTS idx_files_hash      ON files(hash);
CREATE INDEX IF NOT EXISTS idx_files_phash     ON files(phash);
CREATE INDEX IF NOT EXISTS idx_files_parent    ON files(parent_path);
CREATE INDEX IF NOT EXISTS idx_files_doc_type  ON files(doc_type);
CREATE INDEX IF NOT EXISTS idx_files_sensitive ON files(sensitive);
CREATE INDEX IF NOT EXISTS idx_files_size_mtime ON files(size, mtime);

CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
  name,
  path,
  content_text,
  tags,
  content='files',
  content_rowid='id',
  tokenize='unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS files_ai AFTER INSERT ON files BEGIN
  INSERT INTO files_fts(rowid, name, path, content_text, tags)
  VALUES (new.id, new.name, new.path, new.content_text, new.tags);
END;
CREATE TRIGGER IF NOT EXISTS files_ad AFTER DELETE ON files BEGIN
  INSERT INTO files_fts(files_fts, rowid, name, path, content_text, tags)
  VALUES ('delete', old.id, old.name, old.path, old.content_text, old.tags);
END;
CREATE TRIGGER IF NOT EXISTS files_au AFTER UPDATE OF name, path, content_text, tags ON files BEGIN
  INSERT INTO files_fts(files_fts, rowid, name, path, content_text, tags)
  VALUES ('delete', old.id, old.name, old.path, old.content_text, old.tags);
  INSERT INTO files_fts(rowid, name, path, content_text, tags)
  VALUES (new.id, new.name, new.path, new.content_text, new.tags);
END;

CREATE TABLE IF NOT EXISTS dirs (
  root_id     INTEGER NOT NULL REFERENCES roots(id) ON DELETE CASCADE,
  path        TEXT NOT NULL,
  parent_path TEXT NOT NULL DEFAULT '',
  depth       INTEGER NOT NULL DEFAULT 0,
  file_count  INTEGER NOT NULL DEFAULT 0,
  total_bytes INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (root_id, path)
);
CREATE INDEX IF NOT EXISTS idx_dirs_bytes ON dirs(total_bytes DESC);
CREATE INDEX IF NOT EXISTS idx_dirs_root  ON dirs(root_id);

CREATE TABLE IF NOT EXISTS quarantine (
  id              TEXT PRIMARY KEY,
  original_path   TEXT NOT NULL,
  quarantine_path TEXT NOT NULL,
  size            INTEGER NOT NULL DEFAULT 0,
  file_id         INTEGER,
  created_at      INTEGER NOT NULL,
  restored_at     INTEGER,
  state           TEXT NOT NULL DEFAULT 'held'
);
CREATE INDEX IF NOT EXISTS idx_quarantine_state ON quarantine(state);

CREATE TABLE IF NOT EXISTS operations (
  id         TEXT PRIMARY KEY,
  kind       TEXT NOT NULL,
  summary    TEXT NOT NULL,
  detail     TEXT NOT NULL DEFAULT '{}',
  reversible INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL,
  undone_at  INTEGER,
  bytes      INTEGER NOT NULL DEFAULT 0,
  item_count INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_operations_created ON operations(created_at DESC);

CREATE TABLE IF NOT EXISTS storage_snapshots (
  id         INTEGER PRIMARY KEY,
  taken_at   INTEGER NOT NULL,
  root_id    INTEGER,
  total_bytes INTEGER NOT NULL,
  file_count INTEGER NOT NULL,
  by_category TEXT NOT NULL DEFAULT '[]'
);
CREATE INDEX IF NOT EXISTS idx_snapshots_taken ON storage_snapshots(taken_at);

CREATE TABLE IF NOT EXISTS reminders (
  id         TEXT PRIMARY KEY,
  file_id    INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
  title      TEXT NOT NULL,
  due_date   TEXT NOT NULL,
  kind       TEXT NOT NULL DEFAULT 'document',
  created_at INTEGER NOT NULL,
  dismissed  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS automations (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  root_id     INTEGER NOT NULL REFERENCES roots(id) ON DELETE CASCADE,
  folder      TEXT NOT NULL,
  enabled     INTEGER NOT NULL DEFAULT 0,
  auto_apply  INTEGER NOT NULL DEFAULT 0,
  watch_new   INTEGER NOT NULL DEFAULT 1,
  rule        TEXT NOT NULL DEFAULT '{}',
  actions     TEXT NOT NULL DEFAULT '{}',
  created_at  INTEGER NOT NULL,
  last_run_at INTEGER
);

CREATE TABLE IF NOT EXISTS automation_runs (
  id            TEXT PRIMARY KEY,
  automation_id TEXT NOT NULL,
  started_at    INTEGER NOT NULL,
  finished_at   INTEGER NOT NULL,
  actions       INTEGER NOT NULL DEFAULT 0,
  summary       TEXT NOT NULL DEFAULT '',
  mode          TEXT NOT NULL DEFAULT 'suggest'
);
CREATE INDEX IF NOT EXISTS idx_runs_started ON automation_runs(started_at DESC);

CREATE TABLE IF NOT EXISTS relations (
  id         TEXT PRIMARY KEY,
  label      TEXT NOT NULL,
  kind       TEXT NOT NULL,
  confidence REAL NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS relation_members (
  relation_id TEXT NOT NULL REFERENCES relations(id) ON DELETE CASCADE,
  file_id     INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
  role        TEXT NOT NULL DEFAULT 'file',
  PRIMARY KEY (relation_id, file_id)
);

CREATE TABLE IF NOT EXISTS notifications (
  id         TEXT PRIMARY KEY,
  title      TEXT NOT NULL,
  body       TEXT NOT NULL,
  kind       TEXT NOT NULL DEFAULT 'info',
  created_at INTEGER NOT NULL,
  read       INTEGER NOT NULL DEFAULT 0
);
"#;

pub fn open(path: &Path) -> AppResult<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    // Keep the write-ahead log small and let short-lived writers wait instead of
    // failing. A single huge WAL (multiple gigabytes) is what made the app hang
    // during long scans on large drives.
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA temp_store = FILE;
         PRAGMA cache_size = -32768;
         PRAGMA wal_autocheckpoint = 1000;
         PRAGMA journal_size_limit = 67108864;",
    )?;
    conn.execute_batch(SCHEMA)?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(conn)
}

pub fn open_in_memory() -> AppResult<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

/// Fold the write-ahead log back into the database and truncate it. Cheap to
/// call after a scan because `wal_autocheckpoint` keeps the log small.
pub fn checkpoint(conn: &Connection) {
    let _ = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()));
}

pub fn get_setting(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    use rusqlite::OptionalExtension;
    Ok(conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?)
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO settings(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    )?;
    Ok(())
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_opens_and_fts_works() {
        let conn = open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO roots(id, path, label, kind, created_at) VALUES (1, 'C:/x', 'x', 'folder', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO files(id, root_id, path, parent_path, name, size, indexed_at, last_seen_at, content_text)
             VALUES (1, 1, 'C:/x/invoice.pdf', 'C:/x', 'invoice.pdf', 10, 0, 0, 'electricity bill vendor acme')",
            [],
        )
        .unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM files_fts WHERE files_fts MATCH 'electricity'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        conn.execute(
            "UPDATE files SET content_text = 'water bill' WHERE id = 1",
            [],
        )
        .unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM files_fts WHERE files_fts MATCH 'electricity'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }
}
