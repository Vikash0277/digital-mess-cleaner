//! Safe cleanup: quarantine, restore, permanent delete (separate step),
//! operation history and undo. Nothing is deleted without quarantine first.

use crate::error::{AppError, AppResult};
use crate::fsutil;
use crate::models::*;
use crate::organizer;
use crate::repo;
use rusqlite::Connection;
use serde_json::json;
use std::path::{Path, PathBuf};

pub fn quarantine(
    conn: &mut Connection,
    data_dir: &Path,
    file_ids: &[i64],
) -> AppResult<QuarantineResult> {
    if file_ids.is_empty() {
        return Err(AppError::Invalid(
            "Select at least one file to quarantine.".into(),
        ));
    }
    let quarantine_root = data_dir.join("quarantine");
    std::fs::create_dir_all(&quarantine_root)?;
    let mut entries: Vec<QuarantineEntry> = Vec::new();
    let mut outcomes: Vec<MoveOutcome> = Vec::new();
    let mut op_items: Vec<repo::OperationItem> = Vec::new();
    let mut total_bytes = 0i64;
    let now = fsutil::now_secs();

    for id in file_ids {
        let file = match repo::load_file(conn, *id) {
            Ok(f) => f,
            Err(e) => {
                outcomes.push(MoveOutcome {
                    file_id: *id,
                    from: String::new(),
                    to: String::new(),
                    ok: false,
                    error: Some(e.to_string()),
                });
                continue;
            }
        };
        let source = PathBuf::from(&file.path);
        let result = (|| -> AppResult<(String, PathBuf)> {
            repo::ensure_user_path(conn, &source)?;
            if !source.exists() {
                return Err(AppError::NotFound(format!(
                    "{} no longer exists at that location.",
                    file.name
                )));
            }
            let qid = uuid::Uuid::new_v4().to_string();
            let item_dir = quarantine_root.join(&qid);
            std::fs::create_dir_all(&item_dir)?;
            let target = fsutil::unique_destination(&item_dir, &file.name);
            organizer::move_path(&source, &target)?;
            let manifest = json!({
                "id": qid,
                "originalPath": file.path,
                "quarantinePath": target.to_string_lossy(),
                "size": file.size,
                "quarantinedAt": now,
                "restorable": true,
            });
            let _ = std::fs::write(
                item_dir.join("manifest.json"),
                serde_json::to_vec_pretty(&manifest).unwrap_or_default(),
            );
            Ok((qid, target))
        })();

        match result {
            Ok((qid, target)) => {
                conn.execute(
                    "INSERT INTO quarantine(id, original_path, quarantine_path, size, file_id, created_at, state)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'held')",
                    rusqlite::params![qid, file.path, target.to_string_lossy().to_string(), file.size, file.id, now],
                )?;
                conn.execute("DELETE FROM files WHERE id = ?1", [file.id])?;
                op_items.push(repo::OperationItem {
                    from: file.path.clone(),
                    to: target.to_string_lossy().to_string(),
                    quarantine_id: Some(qid.clone()),
                });
                total_bytes += file.size;
                entries.push(QuarantineEntry {
                    id: qid,
                    original_path: file.path.clone(),
                    quarantine_path: target.to_string_lossy().to_string(),
                    name: file.name.clone(),
                    size: file.size,
                    created_at: now,
                    restored_at: None,
                    state: "held".into(),
                    file_exists: true,
                    days_held: 0,
                });
                outcomes.push(MoveOutcome {
                    file_id: file.id,
                    from: file.path,
                    to: target.to_string_lossy().to_string(),
                    ok: true,
                    error: None,
                });
            }
            Err(e) => outcomes.push(MoveOutcome {
                file_id: file.id,
                from: file.path.clone(),
                to: String::new(),
                ok: false,
                error: Some(e.to_string()),
            }),
        }
    }

    let operation_id = if op_items.is_empty() {
        String::new()
    } else {
        repo::log_operation(
            conn,
            "quarantine",
            &format!(
                "Quarantined {} file{}",
                entries.len(),
                if entries.len() == 1 { "" } else { "s" }
            ),
            &op_items,
            total_bytes,
            true,
        )?
    };

    let failed = outcomes.iter().filter(|o| !o.ok).count() as i64;
    Ok(QuarantineResult {
        operation_id,
        moved: entries.len() as i64,
        failed,
        total_bytes,
        entries,
        outcomes,
    })
}

pub fn list_quarantine(
    conn: &Connection,
    include_restored: bool,
) -> AppResult<Vec<QuarantineEntry>> {
    let sql = if include_restored {
        "SELECT id, original_path, quarantine_path, size, created_at, restored_at, state FROM quarantine ORDER BY created_at DESC"
    } else {
        "SELECT id, original_path, quarantine_path, size, created_at, restored_at, state FROM quarantine WHERE state='held' ORDER BY created_at DESC"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| {
        let path: String = r.get(2)?;
        let created: i64 = r.get(4)?;
        Ok(QuarantineEntry {
            id: r.get(0)?,
            original_path: r.get(1)?,
            quarantine_path: path.clone(),
            name: Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "file".into()),
            size: r.get(3)?,
            created_at: created,
            restored_at: r.get(5)?,
            state: r.get(6)?,
            file_exists: Path::new(&path).exists(),
            days_held: ((fsutil::now_secs() - created) / 86_400).max(0),
        })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn restore(conn: &mut Connection, ids: &[String]) -> AppResult<QuarantineResult> {
    if ids.is_empty() {
        return Err(AppError::Invalid(
            "Select at least one quarantined file to restore.".into(),
        ));
    }
    let mut entries = Vec::new();
    let mut outcomes = Vec::new();
    let mut op_items = Vec::new();
    let mut total_bytes = 0;
    let now = fsutil::now_secs();

    for id in ids {
        let row: Result<(String, String, i64), _> = conn.query_row(
            "SELECT original_path, quarantine_path, size FROM quarantine WHERE id=?1 AND state='held'",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        );
        let (original, qpath, size) = match row {
            Ok(v) => v,
            Err(_) => {
                outcomes.push(MoveOutcome {
                    file_id: 0,
                    from: String::new(),
                    to: String::new(),
                    ok: false,
                    error: Some("That item is not held in quarantine.".into()),
                });
                continue;
            }
        };
        let source = PathBuf::from(&qpath);
        let original_path = PathBuf::from(&original);
        let result = (|| -> AppResult<PathBuf> {
            repo::ensure_user_path(conn, &original_path)?;
            if let Some(parent) = original_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            if !source.exists() {
                return Err(AppError::NotFound(
                    "The quarantined copy is missing.".into(),
                ));
            }
            let name = original_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "file".into());
            let target = if original_path.exists() {
                original_path
                    .parent()
                    .map(|p| fsutil::unique_destination(p, &name))
                    .unwrap_or_else(|| original_path.clone())
            } else {
                original_path.clone()
            };
            organizer::move_path(&source, &target)?;
            Ok(target)
        })();
        match result {
            Ok(target) => {
                conn.execute(
                    "UPDATE quarantine SET state='restored', restored_at=?1 WHERE id=?2",
                    rusqlite::params![now, id],
                )?;
                let _ = std::fs::remove_file(
                    source
                        .parent()
                        .map(|p| p.join("manifest.json"))
                        .unwrap_or_default(),
                );
                op_items.push(repo::OperationItem {
                    from: qpath.clone(),
                    to: target.to_string_lossy().to_string(),
                    quarantine_id: Some(id.clone()),
                });
                total_bytes += size;
                entries.push(QuarantineEntry {
                    id: id.clone(),
                    original_path: original.clone(),
                    quarantine_path: qpath.clone(),
                    name: Path::new(&original)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "file".into()),
                    size,
                    created_at: now,
                    restored_at: Some(now),
                    state: "restored".into(),
                    file_exists: target.exists(),
                    days_held: 0,
                });
                outcomes.push(MoveOutcome {
                    file_id: 0,
                    from: qpath,
                    to: target.to_string_lossy().to_string(),
                    ok: true,
                    error: None,
                });
            }
            Err(e) => outcomes.push(MoveOutcome {
                file_id: 0,
                from: qpath,
                to: original,
                ok: false,
                error: Some(e.to_string()),
            }),
        }
    }

    let operation_id = if op_items.is_empty() {
        String::new()
    } else {
        repo::log_operation(
            conn,
            "restore",
            &format!(
                "Restored {} file{} from quarantine",
                entries.len(),
                if entries.len() == 1 { "" } else { "s" }
            ),
            &op_items,
            total_bytes,
            false,
        )?
    };
    let failed = outcomes.iter().filter(|o| !o.ok).count() as i64;
    Ok(QuarantineResult {
        operation_id,
        moved: entries.len() as i64,
        failed,
        total_bytes,
        entries,
        outcomes,
    })
}

pub fn permanent_delete(
    conn: &mut Connection,
    data_dir: &Path,
    ids: &[String],
    confirmation: &str,
) -> AppResult<QuarantineResult> {
    if confirmation.trim() != "DELETE PERMANENTLY" {
        return Err(AppError::Invalid(
            "Permanent deletion needs the exact confirmation phrase “DELETE PERMANENTLY”.".into(),
        ));
    }
    if ids.is_empty() {
        return Err(AppError::Invalid(
            "Select at least one quarantined file.".into(),
        ));
    }
    let quarantine_root = dunce::canonicalize(data_dir.join("quarantine"))
        .unwrap_or_else(|_| data_dir.join("quarantine"));
    let mut deleted = 0i64;
    let mut total_bytes = 0i64;
    let mut outcomes = Vec::new();
    let now = fsutil::now_secs();

    for id in ids {
        let row: Result<(String, i64), _> = conn.query_row(
            "SELECT quarantine_path, size FROM quarantine WHERE id=?1 AND state='held'",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        );
        let (qpath, size) = match row {
            Ok(v) => v,
            Err(_) => continue,
        };
        let path = PathBuf::from(&qpath);
        // Defence in depth: only ever delete inside our own quarantine area.
        let canonical = dunce::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if !fsutil::is_within(&canonical, &quarantine_root) {
            outcomes.push(MoveOutcome {
                file_id: 0,
                from: qpath,
                to: String::new(),
                ok: false,
                error: Some("Refused: not inside the app quarantine area.".into()),
            });
            continue;
        }
        let result = if path.exists() {
            std::fs::remove_file(&path)
        } else {
            Ok(())
        };
        match result {
            Ok(()) => {
                conn.execute(
                    "UPDATE quarantine SET state='deleted', restored_at=?1 WHERE id=?2",
                    rusqlite::params![now, id],
                )?;
                if let Some(parent) = path.parent() {
                    let _ = std::fs::remove_file(parent.join("manifest.json"));
                    let _ = std::fs::remove_dir(parent);
                }
                total_bytes += size;
                deleted += 1;
                outcomes.push(MoveOutcome {
                    file_id: 0,
                    from: qpath,
                    to: String::new(),
                    ok: true,
                    error: None,
                });
            }
            Err(e) => outcomes.push(MoveOutcome {
                file_id: 0,
                from: qpath,
                to: String::new(),
                ok: false,
                error: Some(e.to_string()),
            }),
        }
    }

    let operation_id = repo::log_operation(
        conn,
        "permanent_delete",
        &format!(
            "Permanently deleted {deleted} quarantined file{}",
            if deleted == 1 { "" } else { "s" }
        ),
        &[],
        total_bytes,
        false,
    )?;
    let failed = outcomes.iter().filter(|o| !o.ok).count() as i64;
    Ok(QuarantineResult {
        operation_id,
        moved: deleted,
        failed,
        total_bytes,
        entries: Vec::new(),
        outcomes,
    })
}

pub fn operations(conn: &Connection, limit: i64) -> AppResult<Vec<OperationEntry>> {
    repo::recent_operations(conn, limit)
}

pub fn undo_operation(
    conn: &mut Connection,
    operation_id: &str,
    data_dir: &Path,
) -> AppResult<OperationEntry> {
    let op = repo::get_operation(conn, operation_id)?;
    if op.undone_at.is_some() {
        return Err(AppError::Invalid(
            "This operation was already undone.".into(),
        ));
    }
    if !op.reversible {
        return Err(AppError::Invalid("This action cannot be reversed.".into()));
    }
    let detail: serde_json::Value = serde_json::from_str(&op.detail).unwrap_or(json!({}));
    let items = detail
        .get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    match op.kind.as_str() {
        "organize" => {
            let mut reversed: Vec<repo::OperationItem> = Vec::new();
            for item in items {
                let from = item.get("from").and_then(|v| v.as_str()).unwrap_or("");
                let to = item.get("to").and_then(|v| v.as_str()).unwrap_or("");
                let target = PathBuf::from(to);
                let original = PathBuf::from(from);
                if !target.exists() {
                    continue;
                }
                repo::ensure_user_path(conn, &original)?;
                if let Some(parent) = original.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let name = original
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "file".into());
                let destination = if original.exists() {
                    original
                        .parent()
                        .map(|p| fsutil::unique_destination(p, &name))
                        .unwrap_or_else(|| original.clone())
                } else {
                    original.clone()
                };
                organizer::move_path(&target, &destination)?;
                let parent = destination
                    .parent()
                    .unwrap_or(Path::new(""))
                    .to_string_lossy()
                    .to_string();
                conn.execute(
                    "UPDATE files SET path=?1, parent_path=?2, name=?3 WHERE path=?4",
                    rusqlite::params![
                        destination.to_string_lossy().to_string(),
                        parent,
                        destination
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        to
                    ],
                )?;
                reversed.push(repo::OperationItem {
                    from: to.to_string(),
                    to: destination.to_string_lossy().to_string(),
                    quarantine_id: None,
                });
            }
            repo::log_operation(
                conn,
                "undo",
                &format!("Undid: {}", op.summary),
                &reversed,
                op.bytes,
                false,
            )?;
        }
        "quarantine" => {
            let ids: Vec<String> = items
                .iter()
                .filter_map(|i| {
                    i.get("quarantineId")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .collect();
            restore(conn, &ids)?;
            repo::log_operation(
                conn,
                "undo",
                &format!("Undid: {}", op.summary),
                &[],
                op.bytes,
                false,
            )?;
        }
        _ => {
            return Err(AppError::Invalid(format!(
                "Undo is not available for “{}” operations.",
                op.kind
            )));
        }
    }

    let _ = data_dir;
    conn.execute(
        "UPDATE operations SET undone_at = ?1 WHERE id = ?2",
        rusqlite::params![fsutil::now_secs(), operation_id],
    )?;
    repo::get_operation(conn, operation_id)
}

pub fn remove_empty_folders(conn: &mut Connection, folders: &[String]) -> AppResult<i64> {
    if folders.is_empty() {
        return Err(AppError::Invalid("No folders were selected.".into()));
    }
    let mut removed = 0i64;
    let mut paths: Vec<PathBuf> = folders.iter().map(PathBuf::from).collect();
    paths.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for path in paths {
        repo::ensure_user_path(conn, &path)?;
        match std::fs::read_dir(&path) {
            Ok(mut entries) => {
                if entries.next().is_some() {
                    continue;
                }
            }
            Err(_) => continue,
        }
        if std::fs::remove_dir(&path).is_ok() {
            conn.execute(
                "DELETE FROM dirs WHERE path = ?1",
                [path.to_string_lossy().to_string()],
            )?;
            removed += 1;
        }
    }
    if removed > 0 {
        repo::log_operation(
            conn,
            "remove_empty_folders",
            &format!(
                "Removed {removed} empty folder{}",
                if removed == 1 { "" } else { "s" }
            ),
            &[],
            0,
            false,
        )?;
    }
    Ok(removed)
}

pub fn purge_quarantine(conn: &mut Connection, data_dir: &Path) -> AppResult<i64> {
    let ids: Vec<String> = {
        let mut stmt = conn.prepare("SELECT id FROM quarantine WHERE state='held'")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.filter_map(Result::ok).collect()
    };
    if ids.is_empty() {
        return Ok(0);
    }
    let result = permanent_delete(conn, data_dir, &ids, "DELETE PERMANENTLY")?;
    Ok(result.moved)
}

pub fn quarantine_paths(
    conn: &mut Connection,
    data_dir: &Path,
    paths: &[String],
    label: &str,
) -> AppResult<QuarantineResult> {
    if paths.is_empty() {
        return Err(AppError::Invalid(
            "Select at least one item to quarantine.".into(),
        ));
    }
    let quarantine_root = data_dir.join("quarantine");
    std::fs::create_dir_all(&quarantine_root)?;
    let data_canonical = dunce::canonicalize(data_dir).unwrap_or_else(|_| data_dir.to_path_buf());
    let roots = repo::root_paths(conn)?;
    let mut entries = Vec::new();
    let mut outcomes = Vec::new();
    let mut op_items = Vec::new();
    let mut total_bytes = 0i64;
    let now = fsutil::now_secs();

    for raw in paths {
        let source = PathBuf::from(raw);
        let canonical = dunce::canonicalize(&source).unwrap_or_else(|_| source.clone());
        if let Some(reason) = fsutil::protected_reason(&source) {
            outcomes.push(MoveOutcome {
                file_id: 0,
                from: raw.clone(),
                to: String::new(),
                ok: false,
                error: Some(reason),
            });
            continue;
        }
        if fsutil::is_within(&canonical, &data_canonical) {
            outcomes.push(MoveOutcome {
                file_id: 0,
                from: raw.clone(),
                to: String::new(),
                ok: false,
                error: Some("Refused: this is inside the app's own data folder.".into()),
            });
            continue;
        }
        if roots.iter().any(|(_, r)| {
            dunce::canonicalize(r)
                .map(|c| c == canonical)
                .unwrap_or(false)
        }) {
            outcomes.push(MoveOutcome {
                file_id: 0,
                from: raw.clone(),
                to: String::new(),
                ok: false,
                error: Some(
                    "Refused: this is a library root. Remove it from the library instead.".into(),
                ),
            });
            continue;
        }
        if !source.exists() {
            outcomes.push(MoveOutcome {
                file_id: 0,
                from: raw.clone(),
                to: String::new(),
                ok: false,
                error: Some("Item no longer exists.".into()),
            });
            continue;
        }
        let is_dir = source.is_dir();
        let size = if is_dir {
            fsutil::dir_size(&source).0 as i64
        } else {
            std::fs::metadata(&source)
                .map(|m| m.len() as i64)
                .unwrap_or(0)
        };
        let name = source
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "item".into());
        let qid = uuid::Uuid::new_v4().to_string();
        let item_dir = quarantine_root.join(&qid);
        std::fs::create_dir_all(&item_dir)?;
        let target = fsutil::unique_destination(&item_dir, &name);
        let result = if is_dir {
            move_directory(&source, &target)
        } else {
            organizer::move_path(&source, &target)
        };
        match result {
            Ok(()) => {
                let manifest = json!({
                    "id": qid,
                    "originalPath": raw,
                    "quarantinePath": target.to_string_lossy(),
                    "size": size,
                    "quarantinedAt": now,
                    "kind": if is_dir { "folder" } else { "file" },
                    "source": label,
                    "restorable": true,
                });
                let _ = std::fs::write(
                    item_dir.join("manifest.json"),
                    serde_json::to_vec_pretty(&manifest).unwrap_or_default(),
                );
                conn.execute(
                    "INSERT INTO quarantine(id, original_path, quarantine_path, size, file_id, created_at, state)
                     VALUES (?1, ?2, ?3, ?4, NULL, ?5, 'held')",
                    rusqlite::params![qid, raw, target.to_string_lossy().to_string(), size, now],
                )?;
                op_items.push(repo::OperationItem {
                    from: raw.clone(),
                    to: target.to_string_lossy().to_string(),
                    quarantine_id: Some(qid.clone()),
                });
                total_bytes += size;
                entries.push(QuarantineEntry {
                    id: qid,
                    original_path: raw.clone(),
                    quarantine_path: target.to_string_lossy().to_string(),
                    name,
                    size,
                    created_at: now,
                    restored_at: None,
                    state: "held".into(),
                    file_exists: true,
                    days_held: 0,
                });
                outcomes.push(MoveOutcome {
                    file_id: 0,
                    from: raw.clone(),
                    to: target.to_string_lossy().to_string(),
                    ok: true,
                    error: None,
                });
            }
            Err(e) => outcomes.push(MoveOutcome {
                file_id: 0,
                from: raw.clone(),
                to: String::new(),
                ok: false,
                error: Some(e.to_string()),
            }),
        }
    }

    let operation_id = if op_items.is_empty() {
        String::new()
    } else {
        repo::log_operation(
            conn,
            "quarantine",
            &format!(
                "Quarantined {} {}",
                entries.len(),
                if entries.len() == 1 { "item" } else { "items" }
            ),
            &op_items,
            total_bytes,
            true,
        )?
    };
    let failed = outcomes.iter().filter(|o| !o.ok).count() as i64;
    Ok(QuarantineResult {
        operation_id,
        moved: entries.len() as i64,
        failed,
        total_bytes,
        entries,
        outcomes,
    })
}

fn move_directory(source: &Path, target: &Path) -> AppResult<()> {
    if std::fs::rename(source, target).is_ok() {
        return Ok(());
    }
    let mut count = 0usize;
    for entry in walkdir::WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        let rel = entry
            .path()
            .strip_prefix(source)
            .map_err(|e| AppError::Io(e.to_string()))?;
        let dest = target.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &dest)?;
            count += 1;
        }
    }
    if count > 0 || target.exists() {
        std::fs::remove_dir_all(source)?;
        Ok(())
    } else {
        Err(AppError::Io("Folder could not be copied.".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn setup() -> (tempfile::TempDir, Connection, i64) {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path().join("library");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::write(library.join("a.txt"), b"hello").unwrap();
        let conn = db::open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO roots(id,path,label,kind,created_at) VALUES (1,?1,'lib','folder',0)",
            [library.to_string_lossy().to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO files(id,root_id,path,parent_path,name,size,category,indexed_at,last_seen_at)
             VALUES (1,1,?1,?2,'a.txt',5,'documents',0,0)",
            rusqlite::params![
                library.join("a.txt").to_string_lossy().to_string(),
                library.to_string_lossy().to_string()
            ],
        )
        .unwrap();
        (dir, conn, 1)
    }

    #[test]
    fn quarantine_then_restore_round_trips() {
        let (dir, mut conn, file_id) = setup();
        let data = dir.path().join("appdata");
        std::fs::create_dir_all(&data).unwrap();
        let result = quarantine(&mut conn, &data, &[file_id]).unwrap();
        assert_eq!(result.moved, 1);
        assert!(!dir.path().join("library/a.txt").exists());
        let held = list_quarantine(&conn, false).unwrap();
        assert_eq!(held.len(), 1);
        let restored = restore(&mut conn, &[held[0].id.clone()]).unwrap();
        assert_eq!(restored.moved, 1);
        assert!(dir.path().join("library/a.txt").exists());
    }

    #[test]
    fn permanent_delete_requires_confirmation() {
        let (dir, mut conn, file_id) = setup();
        let data = dir.path().join("appdata");
        std::fs::create_dir_all(&data).unwrap();
        quarantine(&mut conn, &data, &[file_id]).unwrap();
        let held = list_quarantine(&conn, false).unwrap();
        assert!(permanent_delete(&mut conn, &data, &[held[0].id.clone()], "yes").is_err());
        let done = permanent_delete(
            &mut conn,
            &data,
            &[held[0].id.clone()],
            "DELETE PERMANENTLY",
        )
        .unwrap();
        assert_eq!(done.moved, 1);
        assert!(list_quarantine(&conn, false).unwrap().is_empty());
    }

    #[test]
    fn undo_organize_moves_back() {
        let (dir, mut conn, file_id) = setup();
        let src = dir.path().join("library/a.txt");
        let target_dir = dir.path().join("library/Documents");
        let res = organizer::apply(
            &mut conn,
            &[OrganizeRequestItem {
                file_id,
                target_folder: target_dir.to_string_lossy().to_string(),
                new_name: Some("renamed.txt".into()),
            }],
        )
        .unwrap();
        assert_eq!(res.moved, 1);
        assert!(target_dir.join("renamed.txt").exists());
        undo_operation(&mut conn, &res.operation_id, &dir.path().join("appdata")).unwrap();
        assert!(src.exists());
        assert!(!target_dir.join("renamed.txt").exists());
    }
}
