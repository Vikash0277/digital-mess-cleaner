//! Small repository helpers shared by commands and engines.

use crate::error::{AppError, AppResult};
use crate::fsutil;
use crate::models::*;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::json;

pub const FILE_COLUMNS: &str =
    "id, root_id, path, parent_path, name, ext, size, mtime, atime, ctime, \
     category, mime, tags, doc_type, confidence, sensitive, content_status, hash";

pub fn row_to_file(row: &Row<'_>) -> rusqlite::Result<FileEntry> {
    let tags_raw: String = row.get(12)?;
    let tags: Vec<String> = serde_json::from_str(&tags_raw).unwrap_or_default();
    let content_status: String = row.get(16)?;
    Ok(FileEntry {
        id: row.get(0)?,
        root_id: row.get(1)?,
        path: row.get(2)?,
        parent_path: row.get(3)?,
        name: row.get(4)?,
        ext: row.get(5)?,
        size: row.get(6)?,
        mtime: row.get(7)?,
        atime: row.get(8)?,
        ctime: row.get(9)?,
        category: row.get(10)?,
        mime: row.get(11)?,
        tags,
        doc_type: row.get(13)?,
        confidence: row.get(14)?,
        sensitive: row.get::<_, i64>(15)? != 0,
        analyzed: content_status == "indexed",
        hash: row.get(17)?,
        snippet: None,
    })
}

pub fn load_file(conn: &Connection, id: i64) -> AppResult<FileEntry> {
    let sql = format!("SELECT {FILE_COLUMNS} FROM files WHERE id = ?1");
    conn.query_row(&sql, [id], row_to_file).map_err(|_| {
        AppError::NotFound(format!(
            "File {id} is not in the index. Scan its folder first."
        ))
    })
}

pub fn load_files(conn: &Connection, ids: &[i64]) -> AppResult<Vec<FileEntry>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = std::iter::repeat("?")
        .take(ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!("SELECT {FILE_COLUMNS} FROM files WHERE id IN ({placeholders})");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(ids.iter()), row_to_file)?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn load_files_at_paths(conn: &Connection, paths: &[String]) -> AppResult<Vec<FileEntry>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = std::iter::repeat("?")
        .take(paths.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!("SELECT {FILE_COLUMNS} FROM files WHERE path IN ({placeholders})");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(paths.iter()), row_to_file)?;
    Ok(rows.filter_map(Result::ok).collect())
}

/// Roots without touching the files table. Counts and sizes are left at zero
/// because a per-root `sum(size)` over millions of rows is a full index walk;
/// `storage::roots_with_totals` fills them from the cached totals instead.
pub fn list_roots_basic(conn: &Connection) -> AppResult<Vec<RootInfo>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.path, r.label, r.kind, r.enabled, r.last_scan_at
         FROM roots r ORDER BY r.created_at ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(RootInfo {
            id: row.get(0)?,
            path: row.get(1)?,
            label: row.get(2)?,
            kind: row.get(3)?,
            enabled: row.get::<_, i64>(4)? != 0,
            last_scan_at: row.get(5)?,
            file_count: 0,
            total_bytes: 0,
        })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn root_paths(conn: &Connection) -> AppResult<Vec<(i64, std::path::PathBuf)>> {
    let mut stmt = conn.prepare("SELECT id, path FROM roots WHERE enabled = 1")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    Ok(rows
        .filter_map(Result::ok)
        .map(|(id, p)| (id, std::path::PathBuf::from(p)))
        .collect())
}

pub fn ensure_user_path(conn: &Connection, path: &std::path::Path) -> AppResult<()> {
    if let Some(reason) = fsutil::protected_reason(path) {
        return Err(AppError::Permission(reason));
    }
    let roots = root_paths(conn)?;
    if roots.is_empty() {
        return Err(AppError::Invalid(
            "Add a folder to your library first. Digital Mess Cleaner only works inside folders you choose.".into(),
        ));
    }
    if roots.iter().any(|(_, root)| fsutil::is_within(path, root)) {
        Ok(())
    } else {
        Err(AppError::Permission(
            "That path is outside the folders you added. Add it first so every action stays within your library.".into(),
        ))
    }
}

pub fn recent_operations(conn: &Connection, limit: i64) -> AppResult<Vec<OperationEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, summary, detail, reversible, created_at, undone_at, item_count, bytes
         FROM operations ORDER BY created_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |row| {
        Ok(OperationEntry {
            id: row.get(0)?,
            kind: row.get(1)?,
            summary: row.get(2)?,
            detail: row.get(3)?,
            reversible: row.get::<_, i64>(4)? != 0,
            created_at: row.get(5)?,
            undone_at: row.get(6)?,
            item_count: row.get(7)?,
            bytes: row.get(8)?,
        })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub struct OperationItem {
    pub from: String,
    pub to: String,
    pub quarantine_id: Option<String>,
}

pub fn log_operation(
    conn: &Connection,
    kind: &str,
    summary: &str,
    items: &[OperationItem],
    bytes: i64,
    reversible: bool,
) -> AppResult<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let detail = json!({
        "items": items.iter().map(|i| json!({
            "from": i.from, "to": i.to, "quarantineId": i.quarantine_id
        })).collect::<Vec<_>>()
    })
    .to_string();
    conn.execute(
        "INSERT INTO operations(id, kind, summary, detail, reversible, created_at, bytes, item_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, kind, summary, detail, reversible as i64, fsutil::now_secs(), bytes, items.len() as i64],
    )?;
    Ok(id)
}

pub fn get_operation(conn: &Connection, id: &str) -> AppResult<OperationEntry> {
    conn.query_row(
        "SELECT id, kind, summary, detail, reversible, created_at, undone_at, item_count, bytes
         FROM operations WHERE id = ?1",
        [id],
        |row| {
            Ok(OperationEntry {
                id: row.get(0)?,
                kind: row.get(1)?,
                summary: row.get(2)?,
                detail: row.get(3)?,
                reversible: row.get::<_, i64>(4)? != 0,
                created_at: row.get(5)?,
                undone_at: row.get(6)?,
                item_count: row.get(7)?,
                bytes: row.get(8)?,
            })
        },
    )
    .map_err(|_| AppError::NotFound(format!("Operation {id} was not found.")))
}

pub fn insert_notification(
    conn: &Connection,
    title: &str,
    body: &str,
    kind: &str,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO notifications(id, title, body, kind, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            uuid::Uuid::new_v4().to_string(),
            title,
            body,
            kind,
            fsutil::now_secs()
        ],
    )?;
    Ok(())
}

pub fn setting_i64(conn: &Connection, key: &str, default: i64) -> i64 {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
        r.get::<_, String>(0)
    })
    .optional()
    .ok()
    .flatten()
    .and_then(|v| v.parse().ok())
    .unwrap_or(default)
}
