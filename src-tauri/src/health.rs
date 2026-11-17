//! File health, app leftovers, temporary candidates, migration checklist,
//! memory timeline and storage growth.

use crate::error::{AppError, AppResult};
use crate::fsutil;
use crate::models::*;
use crate::repo;
use chrono::{TimeZone, Utc};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

fn columns() -> String {
    repo::FILE_COLUMNS
        .split(',')
        .map(|c| format!("f.{}", c.trim()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn files_sql(
    conn: &Connection,
    sql: &str,
    args: &[rusqlite::types::Value],
    limit: i64,
) -> AppResult<Vec<FileEntry>> {
    let mut values = args.to_vec();
    values.push(rusqlite::types::Value::Integer(limit));
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(values.iter()), repo::row_to_file)?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn health_check(conn: &Connection) -> AppResult<Vec<HealthIssue>> {
    let mut issues: Vec<HealthIssue> = Vec::new();
    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();

    // 1. Missing files (verified by filesystem check, not just the index flag).
    let sample = files_sql(
        conn,
        &format!(
            "SELECT {} FROM files f ORDER BY f.mtime DESC LIMIT ?",
            columns()
        ),
        &[],
        2000,
    )?;
    for file in sample {
        if !seen.insert(file.id) {
            continue;
        }
        if !Path::new(&file.path).exists() {
            issues.push(HealthIssue {
                id: format!("missing-{}", file.id),
                kind: "missing".into(),
                severity: "medium".into(),
                title: "File is missing from disk".into(),
                detail: "The index still lists this file, but it is no longer at that location. It may have been moved or deleted outside the app.".into(),
                path: Some(file.path.clone()),
                bytes: file.size,
                verified: true,
                file_id: Some(file.id),
            });
            if issues.len() >= 100 {
                break;
            }
        }
    }

    // 2. Zero-byte documents (verified size, suspected problem).
    let zero = files_sql(
        conn,
        &format!(
            "SELECT {} FROM files f WHERE f.size = 0 AND f.category='documents' LIMIT ?",
            columns()
        ),
        &[],
        60,
    )?;
    for file in zero {
        if !seen.insert(file.id) {
            continue;
        }
        issues.push(HealthIssue {
            id: format!("empty-{}", file.id),
            kind: "empty".into(),
            severity: "low".into(),
            title: "Empty document".into(),
            detail: "This document is 0 bytes. It may be a failed download or a placeholder."
                .into(),
            path: Some(file.path),
            bytes: 0,
            verified: true,
            file_id: Some(file.id),
        });
    }

    // 3. Damaged ZIP archives (verified by actually opening the central directory).
    let archives = files_sql(
        conn,
        &format!(
            "SELECT {} FROM files f WHERE f.ext='zip' ORDER BY f.size DESC LIMIT ?",
            columns()
        ),
        &[],
        40,
    )?;
    for file in archives {
        let path = Path::new(&file.path);
        if !path.exists() {
            continue;
        }
        let ok = std::fs::File::open(path)
            .map(|f| zip::ZipArchive::new(f).is_ok())
            .unwrap_or(false);
        if !ok && seen.insert(file.id) {
            issues.push(HealthIssue {
                id: format!("archive-{}", file.id),
                kind: "damaged_archive".into(),
                severity: "medium".into(),
                title: "Archive cannot be opened".into(),
                detail: "The ZIP central directory could not be read. The file may be corrupted or only partly downloaded.".into(),
                path: Some(file.path),
                bytes: file.size,
                verified: true,
                file_id: Some(file.id),
            });
        }
    }

    // 4. Unreadable documents (permission or lock).
    let docs = files_sql(
        conn,
        &format!(
            "SELECT {} FROM files f WHERE f.category='documents' ORDER BY f.size DESC LIMIT ?",
            columns()
        ),
        &[],
        120,
    )?;
    for file in docs {
        if !seen.insert(file.id) {
            continue;
        }
        let path = Path::new(&file.path);
        if path.exists() && std::fs::File::open(path).is_err() {
            issues.push(HealthIssue {
                id: format!("unreadable-{}", file.id),
                kind: "unreadable".into(),
                severity: "medium".into(),
                title: "File cannot be read".into(),
                detail: "The file exists but could not be opened. It may be locked by another app or permission-protected.".into(),
                path: Some(file.path),
                bytes: file.size,
                verified: false,
                file_id: Some(file.id),
            });
        }
    }

    // 5. Dangerous double extensions (suspected, never auto-acted on).
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM files f WHERE lower(f.name) LIKE '%.pdf.exe' OR lower(f.name) LIKE '%.doc.exe' OR lower(f.name) LIKE '%.jpg.exe' LIMIT 30",
        columns()
    ))?;
    let suspicious: Vec<FileEntry> = stmt
        .query_map([], repo::row_to_file)?
        .filter_map(Result::ok)
        .collect();
    for file in suspicious {
        if !seen.insert(file.id) {
            continue;
        }
        issues.push(HealthIssue {
            id: format!("double-ext-{}", file.id),
            kind: "suspicious_name".into(),
            severity: "high".into(),
            title: "Suspicious double extension".into(),
            detail: "The name hides an executable behind a document extension. Do not open it unless you trust where it came from.".into(),
            path: Some(file.path),
            bytes: file.size,
            verified: false,
            file_id: Some(file.id),
        });
    }

    let order = |s: &str| match s {
        "high" => 0,
        "medium" => 1,
        _ => 2,
    };
    issues.sort_by_key(|i| (order(&i.severity), i.title.clone()));
    Ok(issues)
}

fn app_data_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if cfg!(target_os = "windows") {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            roots.push(PathBuf::from(local));
        }
        if let Ok(roaming) = std::env::var("APPDATA") {
            roots.push(PathBuf::from(roaming));
        }
        if let Ok(programdata) = std::env::var("PROGRAMDATA") {
            roots.push(PathBuf::from(programdata));
        }
    } else if cfg!(target_os = "macos") {
        if let Ok(home) = std::env::var("HOME") {
            roots.push(PathBuf::from(&home).join("Library/Application Support"));
            roots.push(PathBuf::from(&home).join("Library/Caches"));
            roots.push(PathBuf::from(&home).join("Library/Logs"));
        }
    } else if let Ok(home) = std::env::var("HOME") {
        roots.push(PathBuf::from(&home).join(".local/share"));
        roots.push(PathBuf::from(&home).join(".cache"));
        roots.push(PathBuf::from(&home).join(".config"));
    }
    roots.into_iter().filter(|p| p.is_dir()).collect()
}

fn shared_app_names() -> &'static [&'static str] {
    &[
        "microsoft",
        "packages",
        "temp",
        "temporary",
        "common files",
        "crashdumps",
        "programs",
        "windows",
        "google",
        "mozilla",
        "apple",
        "nvidia",
        "intel",
        "amd",
        "adobe",
        "zoom",
        "slack",
        "discord",
        "code",
        "opencode",
        "digitalmesscleaner",
        "digital mess cleaner",
    ]
}

pub fn app_leftovers(limit: i64) -> AppResult<Vec<Leftover>> {
    let roots = app_data_roots();
    if roots.is_empty() {
        return Ok(Vec::new());
    }
    let mut out: Vec<Leftover> = Vec::new();
    let cutoff = fsutil::now_secs() - 365 * 86_400;
    let mut examined = 0;
    for root in roots {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            examined += 1;
            if examined > 500 {
                break;
            }
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let lower = name.to_lowercase();
            let shared = shared_app_names()
                .iter()
                .any(|s| lower == *s || lower.starts_with(&format!("{s} ")));
            if shared {
                continue;
            }
            let (bytes, _) = fsutil::dir_size(&path);
            if bytes < 2 * 1024 * 1024 {
                continue;
            }
            let last_modified = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let looks_like_leftover = [
                "cache",
                "log",
                "crash",
                "temp",
                "update",
                "installer",
                "old",
            ]
            .iter()
            .any(|needle| lower.contains(needle));
            let mut confidence: f64 = 0.3;
            if last_modified < cutoff {
                confidence += 0.25;
            }
            if looks_like_leftover {
                confidence += 0.2;
            }
            if name.ends_with("-old") || name.ends_with(".old") {
                confidence += 0.15;
            }
            let mut evidence = vec![
                Evidence {
                    label: "Location".into(),
                    value: path.to_string_lossy().to_string(),
                    weight: 1.0,
                },
                Evidence {
                    label: "Last changed".into(),
                    value: if last_modified > 0 {
                        format!("{} days ago", (fsutil::now_secs() - last_modified) / 86_400)
                    } else {
                        "unknown".into()
                    },
                    weight: 1.0,
                },
            ];
            if looks_like_leftover {
                evidence.push(Evidence {
                    label: "Name signal".into(),
                    value: "Folder name suggests cache, logs or temporary data".into(),
                    weight: 0.5,
                });
            }
            out.push(Leftover {
                path: path.to_string_lossy().to_string(),
                app: name,
                bytes: bytes as i64,
                last_modified,
                confidence: confidence.min(0.85),
                evidence,
                shared: false,
            });
        }
    }
    out.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    out.truncate(limit.clamp(1, 100) as usize);
    Ok(out)
}

pub fn temp_candidates(limit: i64) -> AppResult<Vec<Leftover>> {
    let temp = std::env::temp_dir();
    let cutoff = fsutil::now_secs() - 7 * 86_400;
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&temp) {
        for entry in entries.flatten() {
            let path = entry.path();
            let meta = entry.metadata().ok();
            let Some(meta) = meta else { continue };
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            if mtime > cutoff {
                continue;
            }
            let (bytes, _) = if meta.is_dir() {
                fsutil::dir_size(&path)
            } else {
                (meta.len(), 1)
            };
            if bytes < 5 * 1024 * 1024 {
                continue;
            }
            out.push(Leftover {
                path: path.to_string_lossy().to_string(),
                app: "System temporary folder".into(),
                bytes: bytes as i64,
                last_modified: mtime,
                confidence: 0.45,
                evidence: vec![
                    Evidence {
                        label: "Location".into(),
                        value: temp.to_string_lossy().to_string(),
                        weight: 1.0,
                    },
                    Evidence {
                        label: "Rule".into(),
                        value: "At least 5 MB and unchanged for 7 days".into(),
                        weight: 1.0,
                    },
                ],
                shared: true,
            });
        }
    }
    out.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    out.truncate(limit.clamp(1, 100) as usize);
    Ok(out)
}

pub fn migration_checklist(conn: &Connection) -> AppResult<MigrationChecklist> {
    let important = files_sql(
        conn,
        &format!(
            "SELECT {} FROM files f WHERE f.doc_type IN ('invoice','receipt','contract','certificate','warranty','id_document','resume','statement')
             ORDER BY f.doc_date DESC LIMIT ?",
            columns()
        ),
        &[],
        200,
    )?;
    let photos = files_sql(
        conn,
        &format!(
            "SELECT {} FROM files f WHERE f.category='images' ORDER BY f.size DESC LIMIT ?",
            columns()
        ),
        &[],
        200,
    )?;
    let projects = files_sql(
        conn,
        &format!(
            "SELECT {} FROM files f WHERE f.category='documents' AND (f.doc_type IS NULL OR f.doc_type NOT IN ('invoice','receipt','contract','certificate','warranty','id_document','resume','statement'))
             ORDER BY f.mtime DESC LIMIT ?",
            columns()
        ),
        &[],
        200,
    )?;
    let other = files_sql(
        conn,
        &format!(
            "SELECT {} FROM files f WHERE f.category='other' ORDER BY f.size DESC LIMIT ?",
            columns()
        ),
        &[],
        100,
    )?;
    let section = |title: &str, detail: &str, files: Vec<FileEntry>| MigrationSection {
        title: title.into(),
        detail: detail.into(),
        bytes: files.iter().map(|f| f.size).sum(),
        files,
    };
    let sections = vec![
        section(
            "Must-keep documents",
            "Invoices, receipts, contracts, warranties, certificates and identity papers.",
            important,
        ),
        section(
            "Photos and memories",
            "Large photo collections take the longest to copy. Keep a second copy.",
            photos,
        ),
        section(
            "Projects and work files",
            "Recent documents that are not in the must-keep list. Review before excluding.",
            projects,
        ),
        section(
            "Everything else",
            "Other files. Check for niche formats that may need their original app.",
            other,
        ),
    ];
    let total_bytes = sections.iter().map(|s| s.bytes).sum();
    let file_count = sections.iter().map(|s| s.files.len() as i64).sum();
    Ok(MigrationChecklist {
        generated_at: fsutil::now_secs(),
        total_bytes,
        file_count,
        sections,
        notes: vec![
            "This is a checklist, not a copy. Nothing leaves your computer.".into(),
            "Copy important documents first, then media, then everything else.".into(),
            "Keep the old computer available until the new one is verified.".into(),
            "External drives and cloud accounts are not included unless you add and scan them."
                .into(),
        ],
    })
}

pub fn write_migration_checklist(path: &Path, checklist: &MigrationChecklist) -> AppResult<()> {
    if let Some(reason) = fsutil::protected_reason(path) {
        return Err(AppError::Permission(reason));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut out = String::new();
    out.push_str("# Digital Mess Cleaner — migration checklist\n\n");
    out.push_str(&format!(
        "Generated: {}\n\nIncludes {} files, {} total.\n\n",
        Utc.timestamp_opt(checklist.generated_at, 0)
            .single()
            .map(|d| d.to_rfc2822())
            .unwrap_or_default(),
        checklist.file_count,
        crate::format::bytes(checklist.total_bytes)
    ));
    for section in &checklist.sections {
        out.push_str(&format!(
            "## {} ({})\n\n{}\n\n",
            section.title,
            crate::format::bytes(section.bytes),
            section.detail
        ));
        for file in &section.files {
            out.push_str(&format!(
                "- [ ] `{}` — {}\n",
                file.path,
                crate::format::bytes(file.size)
            ));
        }
        out.push('\n');
    }
    out.push_str("## Notes\n\n");
    for note in &checklist.notes {
        out.push_str(&format!("- {note}\n"));
    }
    std::fs::write(path, out)?;
    Ok(())
}

pub fn timeline(
    conn: &Connection,
    file_id: Option<i64>,
    limit: i64,
) -> AppResult<Vec<TimelineEvent>> {
    let mut events: Vec<TimelineEvent> = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT id, kind, summary, created_at FROM operations ORDER BY created_at DESC LIMIT 60",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(TimelineEvent {
            at: r.get(3)?,
            kind: "operation".into(),
            label: r.get(2)?,
            detail: r.get(1)?,
            file_id: None,
        })
    })?;
    events.extend(rows.filter_map(Result::ok));

    if let Some(id) = file_id {
        if let Ok(file) = repo::load_file(conn, id) {
            let mut push = |at: i64, kind: &str, label: &str| {
                if at > 0 {
                    events.push(TimelineEvent {
                        at,
                        kind: kind.into(),
                        label: label.into(),
                        detail: file.path.clone(),
                        file_id: Some(file.id),
                    });
                }
            };
            push(file.ctime, "created", "Created");
            push(file.mtime, "modified", "Last modified");
            push(file.atime, "accessed", "Last opened");
            if let Ok(Some(doc_date)) =
                conn.query_row("SELECT doc_date FROM files WHERE id=?1", [id], |r| {
                    r.get::<_, Option<String>>(0)
                })
            {
                if let Ok(d) = chrono::NaiveDate::parse_from_str(&doc_date, "%Y-%m-%d") {
                    events.push(TimelineEvent {
                        at: Utc
                            .from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap())
                            .timestamp(),
                        kind: "document_date".into(),
                        label: "Date printed on the document".into(),
                        detail: doc_date,
                        file_id: Some(id),
                    });
                }
            }
        }
    } else {
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM files f ORDER BY f.mtime DESC LIMIT ?1",
            columns()
        ))?;
        let rows = stmt.query_map([limit], repo::row_to_file)?;
        for file in rows.flatten() {
            events.push(TimelineEvent {
                at: file.mtime,
                kind: "modified".into(),
                label: format!("{} modified", file.name),
                detail: file.path,
                file_id: Some(file.id),
            });
        }
    }
    events.sort_by(|a, b| b.at.cmp(&a.at));
    events.truncate(limit.clamp(1, 500) as usize);
    Ok(events)
}

pub fn storage_growth(conn: &Connection, days: i64) -> AppResult<StorageGrowth> {
    let snapshots = crate::storage::trends(conn, days)?;
    if snapshots.len() < 2 {
        return Ok(StorageGrowth {
            since: snapshots.first().map(|s| s.taken_at).unwrap_or(0),
            until: fsutil::now_secs(),
            categories: Vec::new(),
            notes: vec!["Not enough history yet. A snapshot is saved after each scan; growth appears after the second snapshot.".into()],
        });
    }
    let first = snapshots.first().unwrap();
    let last = snapshots.last().unwrap();
    let mut map: std::collections::HashMap<String, (i64, i64)> = std::collections::HashMap::new();
    for c in &first.by_category {
        map.entry(c.category.clone()).or_insert((0, 0)).0 = c.bytes;
    }
    for c in &last.by_category {
        map.entry(c.category.clone()).or_insert((0, 0)).1 = c.bytes;
    }
    let mut categories: Vec<CategoryDelta> = map
        .into_iter()
        .map(|(category, (from_bytes, to_bytes))| CategoryDelta {
            category,
            from_bytes,
            to_bytes,
            delta_bytes: to_bytes - from_bytes,
        })
        .collect();
    categories.sort_by(|a, b| b.delta_bytes.cmp(&a.delta_bytes));
    Ok(StorageGrowth {
        since: first.taken_at,
        until: last.taken_at,
        categories,
        notes: vec!["Category growth compares the first and latest storage snapshots.".into()],
    })
}

pub fn app_info(conn: &Connection, data_dir: &Path) -> AppResult<AppInfo> {
    let indexed_files: i64 = conn.query_row("SELECT count(*) FROM files", [], |r| r.get(0))?;
    let root_count: i64 = conn.query_row("SELECT count(*) FROM roots", [], |r| r.get(0))?;
    let db_bytes = std::fs::metadata(data_dir.join("digital-mess-cleaner.db"))
        .map(|m| m.len() as i64)
        .unwrap_or(0);
    Ok(AppInfo {
        app_version: env!("CARGO_PKG_VERSION").into(),
        data_dir: data_dir.to_string_lossy().to_string(),
        platform: std::env::consts::OS.into(),
        db_bytes,
        indexed_files,
        root_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn timeline_returns_events() {
        let conn = db::open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO roots(id,path,label,kind,created_at) VALUES (1,'x','x','folder',0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO files(id,root_id,path,parent_path,name,size,mtime,category,indexed_at,last_seen_at)
             VALUES (1,1,'x/a.txt','x','a.txt',1,100,'documents',0,0)",
            [],
        )
        .unwrap();
        let events = timeline(&conn, None, 10).unwrap();
        assert!(!events.is_empty());
    }

    #[test]
    fn migration_checklist_counts_sections() {
        let conn = db::open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO roots(id,path,label,kind,created_at) VALUES (1,'x','x','folder',0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO files(id,root_id,path,parent_path,name,size,doc_type,category,indexed_at,last_seen_at)
             VALUES (1,1,'x/a.pdf','x','a.pdf',10,'invoice','documents',0,0)",
            [],
        )
        .unwrap();
        let checklist = migration_checklist(&conn).unwrap();
        assert_eq!(checklist.file_count, 1);
        assert_eq!(checklist.sections.len(), 4);
    }
}
