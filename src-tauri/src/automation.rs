//! Automation Centre: saved rules, manual runs and a conservative background
//! watcher. Auto-apply only happens when the user explicitly enabled it.

use crate::error::{AppError, AppResult};
use crate::fsutil;
use crate::models::*;
use crate::organizer;
use crate::repo;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

fn parse_automation(row: &rusqlite::Row<'_>) -> rusqlite::Result<Automation> {
    let rule_raw: String = row.get(8)?;
    let actions_raw: String = row.get(9)?;
    Ok(Automation {
        id: row.get(0)?,
        name: row.get(1)?,
        root_id: row.get(2)?,
        folder: row.get(3)?,
        enabled: row.get::<_, i64>(4)? != 0,
        auto_apply: row.get::<_, i64>(5)? != 0,
        watch_new: row.get::<_, i64>(6)? != 0,
        rule: serde_json::from_str(&rule_raw).unwrap_or(AutomationRule {
            name_contains: None,
            extensions: Vec::new(),
            doc_types: Vec::new(),
            older_than_days: None,
            min_size: None,
        }),
        actions: serde_json::from_str(&actions_raw).unwrap_or(AutomationActions {
            destination: String::new(),
            rename_template: None,
        }),
        created_at: row.get(7)?,
        last_run_at: row.get(10)?,
    })
}

pub fn list(conn: &Connection) -> AppResult<Vec<Automation>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, root_id, folder, enabled, auto_apply, watch_new, created_at, rule, actions, last_run_at
         FROM automations ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], parse_automation)?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn save(conn: &Connection, mut automation: Automation) -> AppResult<Vec<Automation>> {
    if automation.name.trim().is_empty() {
        return Err(AppError::Invalid("Give the automation a name.".into()));
    }
    if automation.folder.trim().is_empty() {
        return Err(AppError::Invalid("Choose the folder to watch.".into()));
    }
    if automation.actions.destination.trim().is_empty() {
        return Err(AppError::Invalid(
            "Choose where organised files should go.".into(),
        ));
    }
    if !Path::new(&automation.folder).exists() {
        return Err(AppError::NotFound(format!(
            "{} is not available.",
            automation.folder
        )));
    }
    if automation.id.trim().is_empty() {
        automation.id = uuid::Uuid::new_v4().to_string();
    }
    automation.created_at = if automation.created_at == 0 {
        fsutil::now_secs()
    } else {
        automation.created_at
    };
    conn.execute(
        "INSERT INTO automations(id, name, root_id, folder, enabled, auto_apply, watch_new, rule, actions, created_at, last_run_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(id) DO UPDATE SET
           name=excluded.name, root_id=excluded.root_id, folder=excluded.folder, enabled=excluded.enabled,
           auto_apply=excluded.auto_apply, watch_new=excluded.watch_new, rule=excluded.rule,
           actions=excluded.actions, last_run_at=excluded.last_run_at",
        rusqlite::params![
            automation.id,
            automation.name,
            automation.root_id,
            automation.folder,
            automation.enabled as i64,
            automation.auto_apply as i64,
            automation.watch_new as i64,
            serde_json::to_string(&automation.rule).unwrap_or_else(|_| "{}".into()),
            serde_json::to_string(&automation.actions).unwrap_or_else(|_| "{}".into()),
            automation.created_at,
            automation.last_run_at
        ],
    )?;
    list(conn)
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<Vec<Automation>> {
    conn.execute("DELETE FROM automations WHERE id=?1", [id])?;
    list(conn)
}

pub fn runs(conn: &Connection, limit: i64) -> AppResult<Vec<AutomationRun>> {
    let mut stmt = conn.prepare(
        "SELECT id, automation_id, started_at, finished_at, actions, summary, mode
         FROM automation_runs ORDER BY started_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |r| {
        Ok(AutomationRun {
            id: r.get(0)?,
            automation_id: r.get(1)?,
            started_at: r.get(2)?,
            finished_at: r.get(3)?,
            actions: r.get(4)?,
            summary: r.get(5)?,
            mode: r.get(6)?,
        })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

fn matches(file: &FileEntry, rule: &AutomationRule, vendor: &str, date: &str) -> bool {
    let _ = (vendor, date);
    if let Some(contains) = &rule.name_contains {
        if !contains.trim().is_empty()
            && !file.name.to_lowercase().contains(&contains.to_lowercase())
        {
            return false;
        }
    }
    if !rule.extensions.is_empty()
        && !rule
            .extensions
            .iter()
            .any(|e| e.trim_start_matches('.').eq_ignore_ascii_case(&file.ext))
    {
        return false;
    }
    if !rule.doc_types.is_empty() {
        match &file.doc_type {
            Some(t) if rule.doc_types.iter().any(|x| x == t) => {}
            _ => return false,
        }
    }
    if let Some(days) = rule.older_than_days {
        if file.mtime > fsutil::now_secs() - days.clamp(1, 36500) * 86_400 {
            return false;
        }
    }
    if let Some(min) = rule.min_size {
        if file.size < min {
            return false;
        }
    }
    true
}

fn files_in_folder(conn: &Connection, folder: &str) -> AppResult<Vec<FileEntry>> {
    let columns = repo::FILE_COLUMNS
        .split(',')
        .map(|c| format!("f.{}", c.trim()))
        .collect::<Vec<_>>()
        .join(", ");
    let mut stmt = conn.prepare(&format!(
        "SELECT {columns} FROM files f WHERE f.parent_path = ?1 ORDER BY f.mtime DESC LIMIT 500"
    ))?;
    let rows = stmt.query_map([folder], repo::row_to_file)?;
    Ok(rows.filter_map(Result::ok).collect())
}

fn doc_meta(conn: &Connection, id: i64) -> (String, String) {
    conn.query_row(
        "SELECT COALESCE(vendor,''), COALESCE(doc_date,'') FROM files WHERE id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .unwrap_or_default()
}

pub fn run(conn: &mut Connection, id: &str, apply_now: bool) -> AppResult<AutomationRun> {
    let automation = list(conn)?
        .into_iter()
        .find(|a| a.id == id)
        .ok_or_else(|| AppError::NotFound("Automation not found.".into()))?;
    let started = fsutil::now_secs();
    let folder = PathBuf::from(&automation.folder);
    let destination = {
        let d = PathBuf::from(&automation.actions.destination);
        if d.is_absolute() {
            d
        } else {
            folder.join(d)
        }
    };
    let files = files_in_folder(conn, &automation.folder)?;
    let mut requests = Vec::new();
    let mut planned = Vec::new();
    for file in files {
        let (vendor, date) = doc_meta(conn, file.id);
        if !matches(&file, &automation.rule, &vendor, &date) {
            continue;
        }
        let new_name = automation.actions.rename_template.as_ref().map(|t| {
            fsutil::sanitize_filename(
                &organizer::render_template(
                    t,
                    &file,
                    file.doc_type.as_deref().unwrap_or("document"),
                    &vendor,
                    &date,
                ),
                &file.name,
            )
        });
        if planned.len() < 100 {
            planned.push(format!(
                "{} → {}",
                file.name,
                new_name.clone().unwrap_or_else(|| file.name.clone())
            ));
        }
        requests.push(OrganizeRequestItem {
            file_id: file.id,
            target_folder: destination.to_string_lossy().to_string(),
            new_name,
        });
    }

    let (actions, summary, mode) = if apply_now && !requests.is_empty() {
        let result = organizer::apply(conn, &requests)?;
        (
            result.moved,
            format!(
                "Moved {} file{} and skipped {}.",
                result.moved,
                if result.moved == 1 { "" } else { "s" },
                result.failed
            ),
            "apply".to_string(),
        )
    } else {
        let summary = if requests.is_empty() {
            "No new matching files.".to_string()
        } else {
            format!(
                "{} file{} match. Review before applying.",
                requests.len(),
                if requests.len() == 1 { "" } else { "s" }
            )
        };
        (0, summary, "suggest".to_string())
    };

    let finished = fsutil::now_secs();
    let run_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO automation_runs(id, automation_id, started_at, finished_at, actions, summary, mode)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![run_id, automation.id, started, finished, actions, summary, mode],
    )?;
    conn.execute(
        "UPDATE automations SET last_run_at=?1 WHERE id=?2",
        rusqlite::params![finished, automation.id],
    )?;
    if !requests.is_empty() {
        repo::insert_notification(
            conn,
            &format!(
                "{}: {}",
                automation.name,
                if apply_now {
                    "files organised"
                } else {
                    "files ready to review"
                }
            ),
            &summary,
            if apply_now { "success" } else { "info" },
        )?;
    }
    let _ = planned;

    Ok(AutomationRun {
        id: run_id,
        automation_id: automation.id,
        started_at: started,
        finished_at: finished,
        actions,
        summary,
        mode,
    })
}

pub fn notifications(conn: &Connection, limit: i64) -> AppResult<Vec<Notification>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, body, kind, created_at, read FROM notifications ORDER BY created_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |r| {
        Ok(Notification {
            id: r.get(0)?,
            title: r.get(1)?,
            body: r.get(2)?,
            kind: r.get(3)?,
            created_at: r.get(4)?,
            read: r.get::<_, i64>(5)? != 0,
        })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn mark_notifications_read(conn: &Connection) -> AppResult<()> {
    conn.execute("UPDATE notifications SET read=1", [])?;
    Ok(())
}

/// Conservative background tick: snapshots and auto-apply automations only.
pub fn tick(conn: &mut Connection) -> AppResult<Vec<String>> {
    let mut messages = Vec::new();
    let _ = crate::storage::record_snapshot_if_due(conn);
    let automations = list(conn)?;
    for automation in automations {
        if !automation.enabled || !automation.auto_apply || !automation.watch_new {
            continue;
        }
        let due = automation
            .last_run_at
            .map(|t| fsutil::now_secs() - t > 120)
            .unwrap_or(true);
        if !due {
            continue;
        }
        match run(conn, &automation.id, true) {
            Ok(result) if result.actions > 0 => {
                messages.push(format!("{}: {}", automation.name, result.summary))
            }
            _ => {}
        }
    }
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_matching() {
        let file = FileEntry {
            id: 1,
            root_id: 1,
            path: "C:/d/Invoice_Amazon.pdf".into(),
            parent_path: "C:/d".into(),
            name: "Invoice_Amazon.pdf".into(),
            ext: "pdf".into(),
            size: 1000,
            mtime: 0,
            atime: 0,
            ctime: 0,
            category: "documents".into(),
            mime: "application/pdf".into(),
            tags: vec![],
            doc_type: Some("invoice".into()),
            confidence: 0.9,
            sensitive: false,
            analyzed: true,
            hash: None,
            snippet: None,
        };
        let rule = AutomationRule {
            name_contains: Some("invoice".into()),
            extensions: vec!["pdf".into()],
            doc_types: vec![],
            older_than_days: None,
            min_size: None,
        };
        assert!(matches(&file, &rule, "", ""));
        let no = AutomationRule {
            extensions: vec!["docx".into()],
            ..rule
        };
        assert!(!matches(&file, &no, "", ""));
    }
}
