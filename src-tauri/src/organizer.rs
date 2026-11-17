//! Smart organisation: destination suggestions, smart names, safe moves and
//! user-defined rules. Nothing moves without an explicit apply call.

use crate::error::{AppError, AppResult};
use crate::fsutil;
use crate::intelligence;
use crate::models::*;
use crate::repo;
use chrono::{TimeZone, Utc};
use rusqlite::Connection;
use serde_json::json;
use std::path::{Path, PathBuf};

struct DocFields {
    doc_type: Option<String>,
    vendor: Option<String>,
    amount: Option<String>,
    doc_date: Option<String>,
    confidence: f64,
    content_status: String,
    evidence: Vec<Evidence>,
}

fn doc_fields(conn: &Connection, id: i64) -> AppResult<DocFields> {
    conn.query_row(
        "SELECT doc_type, vendor, amount, doc_date, confidence, content_status, evidence FROM files WHERE id=?1",
        [id],
        |r| {
            let evidence_raw: String = r.get(6)?;
            Ok(DocFields {
                doc_type: r.get(0)?,
                vendor: r.get(1)?,
                amount: r.get(2)?,
                doc_date: r.get(3)?,
                confidence: r.get(4)?,
                content_status: r.get(5)?,
                evidence: serde_json::from_str(&evidence_raw).unwrap_or_default(),
            })
        },
    )
    .map_err(|_| AppError::NotFound("File is not in the index.".into()))
}

fn suggested_subfolder(category: &str, doc_type: Option<&str>, name: &str) -> &'static str {
    match category {
        "documents" => match doc_type.unwrap_or("other") {
            "invoice" => "Documents/Finance/Invoices",
            "receipt" => "Documents/Finance/Receipts",
            "statement" => "Documents/Finance/Statements",
            "contract" => "Documents/Legal/Contracts",
            "warranty" => "Documents/Product/Warranty",
            "certificate" => "Documents/Certificates",
            "resume" => "Documents/Career/Resumes",
            "report" => "Documents/Reports",
            "id_document" => "Documents/Identity",
            _ => "Documents/Other",
        },
        "images" => {
            if crate::duplicates::is_screenshot(name) {
                "Media/Screenshots"
            } else {
                "Media/Photos"
            }
        }
        "video" => "Media/Videos",
        "audio" => "Media/Audio",
        "archives" => "Archives",
        "installers" => "Installers",
        _ => "Other",
    }
}

fn clean_token(input: &str) -> String {
    let mut out = String::new();
    let mut last_sep = false;
    for ch in input.chars() {
        if ch.is_alphanumeric() {
            out.push(ch);
            last_sep = false;
        } else if !last_sep {
            out.push('_');
            last_sep = true;
        }
    }
    out.trim_matches('_').to_string()
}

fn smart_name(file: &FileEntry, fields: &DocFields) -> Option<String> {
    if fields.confidence < 0.45 {
        return None;
    }
    let doc_type = fields.doc_type.as_deref().unwrap_or("other");
    let vendor = fields.vendor.as_deref().filter(|v| !v.trim().is_empty());
    let date = fields.doc_date.clone().or_else(|| {
        Utc.timestamp_opt(file.mtime, 0)
            .single()
            .map(|d| d.format("%Y-%m-%d").to_string())
    });
    let label = match doc_type {
        "invoice" => "Invoice",
        "receipt" => "Receipt",
        "contract" => "Contract",
        "warranty" => "Warranty",
        "statement" => "Statement",
        _ => return None,
    };
    let vendor_part = vendor.map(clean_token).filter(|v| !v.is_empty());
    let mut parts: Vec<String> = Vec::new();
    if let Some(v) = vendor_part {
        parts.push(v);
    }
    parts.push(label.to_string());
    if let Some(d) = date {
        parts.push(d);
    }
    let ext = if file.ext.is_empty() {
        String::new()
    } else {
        format!(".{}", file.ext)
    };
    let name = format!("{}{}", parts.join("_"), ext);
    let sanitized = fsutil::sanitize_filename(&name, &file.name);
    if sanitized.eq_ignore_ascii_case(&file.name) {
        None
    } else {
        Some(sanitized)
    }
}

pub fn analyze_folder(conn: &Connection, folder: &str, limit: i64) -> AppResult<i64> {
    let ids: Vec<i64> = {
        let mut stmt = conn.prepare(
            "SELECT id FROM files WHERE parent_path = ?1 AND content_status = 'none'
             ORDER BY CASE category WHEN 'documents' THEN 0 WHEN 'images' THEN 1 ELSE 2 END
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![folder, limit], |r| r.get(0))?;
        rows.filter_map(Result::ok).collect()
    };
    let mut done = 0;
    for id in ids {
        if intelligence::analyze_file(conn, id).is_ok() {
            done += 1;
        }
    }
    Ok(done)
}

pub fn plan(conn: &Connection, folder: &str, limit: i64) -> AppResult<OrganizePlan> {
    let folder_path = Path::new(folder);
    if !folder_path.is_dir() {
        return Err(AppError::NotFound(format!(
            "{} is not available. Reconnect the drive or rescan first.",
            folder
        )));
    }
    let columns = repo::FILE_COLUMNS
        .split(',')
        .map(|c| format!("f.{}", c.trim()))
        .collect::<Vec<_>>()
        .join(", ");
    let files: Vec<FileEntry> = {
        let mut stmt = conn.prepare(&format!(
            "SELECT {columns} FROM files f WHERE f.parent_path = ?1 ORDER BY f.size DESC LIMIT ?2"
        ))?;
        let rows = stmt.query_map(
            rusqlite::params![folder, limit.clamp(1, 1000)],
            repo::row_to_file,
        )?;
        rows.filter_map(Result::ok).collect()
    };

    let mut items: Vec<OrganizeItem> = Vec::new();
    let mut total_bytes = 0i64;
    let mut category_map: std::collections::HashMap<String, (i64, i64)> =
        std::collections::HashMap::new();
    let mut analyzed = 0i64;
    let mut no_change = 0i64;

    for file in files {
        total_bytes += file.size;
        let entry = category_map.entry(file.category.clone()).or_insert((0, 0));
        entry.0 += file.size;
        entry.1 += 1;

        let fields = doc_fields(conn, file.id)?;
        if fields.content_status == "indexed" {
            analyzed += 1;
        }
        let doc_type = fields
            .doc_type
            .clone()
            .or_else(|| Some(intelligence::classify_simple(&file.name).0.to_string()))
            .filter(|t| t != "other");
        let subfolder = suggested_subfolder(&file.category, doc_type.as_deref(), &file.name);
        let target_folder = folder_path.join(subfolder);
        let new_name = smart_name(&file, &fields).unwrap_or_else(|| file.name.clone());
        let full_target = target_folder.join(&new_name);
        let conflict = full_target.exists() && full_target != Path::new(&file.path);
        let already_there = Path::new(&file.path).parent() == Some(target_folder.as_path())
            && file.name == new_name;
        if already_there {
            no_change += 1;
            continue;
        }

        let mut evidence = fields.evidence.clone();
        evidence.push(Evidence {
            label: "Destination rule".into(),
            value: format!(
                "{} file → {subfolder}",
                fsutil::category_label(&file.category)
            ),
            weight: 1.0,
        });
        if fields.vendor.is_some() || fields.doc_date.is_some() || fields.amount.is_some() {
            evidence.push(Evidence {
                label: "Name signals".into(),
                value: format!(
                    "{} · {} · {}",
                    fields.vendor.clone().unwrap_or_else(|| "no vendor".into()),
                    fields
                        .doc_date
                        .clone()
                        .unwrap_or_else(|| "no extracted date".into()),
                    fields.amount.clone().unwrap_or_else(|| "no amount".into())
                ),
                weight: 1.0,
            });
        }
        let confidence = if fields.confidence > 0.0 {
            fields.confidence
        } else {
            (intelligence::classify_simple(&file.name).1 * 0.8).max(0.25)
        };
        let reason = if new_name != file.name {
            format!(
                "Recognised as a {}. Rename to a readable name and move to {}.",
                doc_type.clone().unwrap_or_else(|| "document".into()),
                subfolder
            )
        } else {
            format!(
                "Grouped as {}. Move to {}.",
                fsutil::category_label(&file.category),
                subfolder
            )
        };

        items.push(OrganizeItem {
            file,
            suggested_name: new_name,
            suggested_folder: target_folder.to_string_lossy().to_string(),
            full_target: full_target.to_string_lossy().to_string(),
            reason,
            confidence,
            doc_type,
            evidence,
            conflict,
        });
    }

    let category_counts = category_map
        .into_iter()
        .map(|(category, (bytes, count))| CategoryStat {
            category,
            bytes,
            count,
        })
        .collect();

    let mut notes = vec![
        "Destinations stay inside the folder you selected. Nothing is moved until you approve the plan.".into(),
        "Name conflicts are resolved by adding (2), (3) and so on — existing files are never overwritten.".into(),
    ];
    if analyzed < items.len() as i64 {
        notes.push(
            "Some files have not been content-analysed. Run “Analyse contents” for stronger names and destinations."
                .into(),
        );
    }
    if no_change > 0 {
        notes.push(format!(
            "{no_change} files are already in a sensible place and were left out of the plan."
        ));
    }

    Ok(OrganizePlan {
        folder: folder.to_string(),
        items,
        total_bytes,
        category_counts,
        notes,
    })
}

/// Move/rename approved items. Returns per-item outcomes and logs one reversible operation.
pub fn apply(conn: &mut Connection, items: &[OrganizeRequestItem]) -> AppResult<ApplyResult> {
    if items.is_empty() {
        return Err(AppError::Invalid(
            "Select at least one file to organise.".into(),
        ));
    }
    let mut outcomes: Vec<MoveOutcome> = Vec::new();
    let mut operation_items: Vec<repo::OperationItem> = Vec::new();
    let mut total_bytes = 0i64;
    let mut moved = 0i64;

    for item in items {
        let file = match repo::load_file(conn, item.file_id) {
            Ok(f) => f,
            Err(e) => {
                outcomes.push(MoveOutcome {
                    file_id: item.file_id,
                    from: String::new(),
                    to: String::new(),
                    ok: false,
                    error: Some(e.to_string()),
                });
                continue;
            }
        };
        let source = PathBuf::from(&file.path);
        let target_dir = PathBuf::from(&item.target_folder);
        let result = (|| -> AppResult<(PathBuf, PathBuf)> {
            repo::ensure_user_path(conn, &source)?;
            repo::ensure_user_path(conn, &target_dir)?;
            let desired_name = item.new_name.clone().unwrap_or_else(|| file.name.clone());
            let desired = fsutil::sanitize_filename(&desired_name, &file.name);
            std::fs::create_dir_all(&target_dir)?;
            let target = fsutil::unique_destination(&target_dir, &desired);
            if target == source {
                return Ok((source.clone(), source));
            }
            move_path(&source, &target)?;
            Ok((source, target))
        })();

        match result {
            Ok((source, target)) => {
                if source == target {
                    outcomes.push(MoveOutcome {
                        file_id: file.id,
                        from: source.to_string_lossy().to_string(),
                        to: target.to_string_lossy().to_string(),
                        ok: true,
                        error: None,
                    });
                    continue;
                }
                let name = target
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&file.name)
                    .to_string();
                let parent = target
                    .parent()
                    .unwrap_or(Path::new(""))
                    .to_string_lossy()
                    .to_string();
                let ext = fsutil::ext_of(&target);
                conn.execute(
                    "UPDATE files SET path=?1, parent_path=?2, name=?3, ext=?4 WHERE id=?5",
                    rusqlite::params![
                        target.to_string_lossy().to_string(),
                        parent,
                        name,
                        ext,
                        file.id
                    ],
                )?;
                operation_items.push(repo::OperationItem {
                    from: source.to_string_lossy().to_string(),
                    to: target.to_string_lossy().to_string(),
                    quarantine_id: None,
                });
                total_bytes += file.size;
                moved += 1;
                outcomes.push(MoveOutcome {
                    file_id: file.id,
                    from: source.to_string_lossy().to_string(),
                    to: target.to_string_lossy().to_string(),
                    ok: true,
                    error: None,
                });
            }
            Err(e) => outcomes.push(MoveOutcome {
                file_id: file.id,
                from: file.path.clone(),
                to: target_dir.to_string_lossy().to_string(),
                ok: false,
                error: Some(e.to_string()),
            }),
        }
    }

    let operation_id = if operation_items.is_empty() {
        String::new()
    } else {
        repo::log_operation(
            conn,
            "organize",
            &format!(
                "Organised {moved} file{}",
                if moved == 1 { "" } else { "s" }
            ),
            &operation_items,
            total_bytes,
            true,
        )?
    };

    let failed = outcomes.iter().filter(|o| !o.ok).count() as i64;
    Ok(ApplyResult {
        operation_id,
        moved,
        failed,
        outcomes,
    })
}

pub fn move_path(source: &Path, target: &Path) -> AppResult<()> {
    match std::fs::rename(source, target) {
        Ok(()) => Ok(()),
        Err(rename_err) => {
            // Cross-volume moves cannot be renamed; copy then remove.
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(source, target).map_err(|copy_err| {
                AppError::Io(format!(
                    "Could not move the file ({rename_err}; {copy_err}). It may be open in another app."
                ))
            })?;
            std::fs::remove_file(source)?;
            Ok(())
        }
    }
}

fn load_rules(conn: &Connection) -> AppResult<Vec<OrganizerRule>> {
    let raw = crate::db::get_setting(conn, "organizer_rules")?.unwrap_or_else(|| "[]".into());
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

fn store_rules(conn: &Connection, rules: &[OrganizerRule]) -> AppResult<()> {
    crate::db::set_setting(
        conn,
        "organizer_rules",
        &serde_json::to_string(rules).unwrap_or_else(|_| "[]".into()),
    )
}

pub fn rules(conn: &Connection) -> AppResult<Vec<OrganizerRule>> {
    load_rules(conn)
}

pub fn save_rule(conn: &Connection, mut rule: OrganizerRule) -> AppResult<Vec<OrganizerRule>> {
    if rule.name.trim().is_empty() {
        return Err(AppError::Invalid("Give the rule a name.".into()));
    }
    if rule.actions.destination.trim().is_empty() {
        return Err(AppError::Invalid(
            "Choose a destination folder for the rule.".into(),
        ));
    }
    if rule.id.trim().is_empty() {
        rule.id = uuid::Uuid::new_v4().to_string();
    }
    let mut all = load_rules(conn)?;
    if let Some(existing) = all.iter_mut().find(|r| r.id == rule.id) {
        *existing = rule;
    } else {
        all.push(rule);
    }
    store_rules(conn, &all)?;
    Ok(all)
}

pub fn delete_rule(conn: &Connection, id: &str) -> AppResult<Vec<OrganizerRule>> {
    let mut all = load_rules(conn)?;
    all.retain(|r| r.id != id);
    store_rules(conn, &all)?;
    Ok(all)
}

pub fn render_template(
    template: &str,
    file: &FileEntry,
    doc_type: &str,
    vendor: &str,
    date: &str,
) -> String {
    let stem = file
        .name
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(&file.name);
    template
        .replace("{name}", stem)
        .replace("{ext}", &file.ext)
        .replace("{docType}", doc_type)
        .replace("{year}", &date.chars().take(4).collect::<String>())
        .replace("{date}", date)
        .replace("{vendor}", vendor)
}

pub fn run_rule(conn: &mut Connection, rule: &OrganizerRule) -> AppResult<ApplyResult> {
    let folder = rule
        .folder
        .clone()
        .ok_or_else(|| AppError::Invalid("This rule has no source folder.".into()))?;
    let folder_path = PathBuf::from(&folder);
    let destination = {
        let d = PathBuf::from(&rule.actions.destination);
        if d.is_absolute() {
            d
        } else {
            folder_path.join(d)
        }
    };
    let columns = repo::FILE_COLUMNS
        .split(',')
        .map(|c| format!("f.{}", c.trim()))
        .collect::<Vec<_>>()
        .join(", ");
    let files: Vec<FileEntry> = {
        let mut stmt = conn.prepare(&format!(
            "SELECT {columns} FROM files f WHERE f.parent_path = ?1"
        ))?;
        let rows = stmt.query_map([&folder], repo::row_to_file)?;
        rows.filter_map(Result::ok).collect()
    };
    let cutoff = rule
        .rule
        .older_than_days
        .map(|d| fsutil::now_secs() - d * 86_400);
    let mut requests = Vec::new();
    for file in files {
        if let Some(contains) = &rule.rule.name_contains {
            if !file.name.to_lowercase().contains(&contains.to_lowercase()) {
                continue;
            }
        }
        if !rule.rule.extensions.is_empty()
            && !rule
                .rule
                .extensions
                .iter()
                .any(|e| e.trim_start_matches('.').eq_ignore_ascii_case(&file.ext))
        {
            continue;
        }
        if !rule.rule.doc_types.is_empty() {
            match &file.doc_type {
                Some(t) if rule.rule.doc_types.iter().any(|x| x == t) => {}
                _ => continue,
            }
        }
        if let Some(cut) = cutoff {
            if file.mtime > cut {
                continue;
            }
        }
        if let Some(min) = rule.rule.min_size {
            if file.size < min {
                continue;
            }
        }
        let fields = doc_fields(conn, file.id).unwrap_or(DocFields {
            doc_type: None,
            vendor: None,
            amount: None,
            doc_date: None,
            confidence: 0.0,
            content_status: "none".into(),
            evidence: Vec::new(),
        });
        let new_name = rule.actions.rename_template.as_ref().map(|t| {
            fsutil::sanitize_filename(
                &render_template(
                    t,
                    &file,
                    fields.doc_type.as_deref().unwrap_or("document"),
                    fields.vendor.as_deref().unwrap_or(""),
                    fields
                        .doc_date
                        .as_deref()
                        .or(file.doc_type.as_deref())
                        .unwrap_or(""),
                ),
                &file.name,
            )
        });
        requests.push(OrganizeRequestItem {
            file_id: file.id,
            target_folder: destination.to_string_lossy().to_string(),
            new_name,
        });
    }
    if requests.is_empty() {
        return Ok(ApplyResult {
            operation_id: String::new(),
            moved: 0,
            failed: 0,
            outcomes: Vec::new(),
        });
    }
    apply(conn, &requests)
}

pub fn seed_default_rules(conn: &Connection) -> AppResult<()> {
    if !load_rules(conn)?.is_empty() {
        return Ok(());
    }
    let rules = vec![OrganizerRule {
        id: "default-invoices".into(),
        name: "Invoices from selected folder".into(),
        enabled: false,
        folder: None,
        rule: AutomationRule {
            name_contains: Some("invoice".into()),
            extensions: vec!["pdf".into()],
            doc_types: vec![],
            older_than_days: None,
            min_size: None,
        },
        actions: AutomationActions {
            destination: "Invoices".into(),
            rename_template: None,
        },
    }];
    store_rules(conn, &rules)
}

pub fn rule_summary(rule: &OrganizerRule) -> String {
    json!({
        "name": rule.name,
        "enabled": rule.enabled,
        "folder": rule.folder,
        "destination": rule.actions.destination,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subfolder_rules_are_sensible() {
        assert_eq!(
            suggested_subfolder("documents", Some("invoice"), "x.pdf"),
            "Documents/Finance/Invoices"
        );
        assert_eq!(
            suggested_subfolder("images", None, "Screenshot 2026.png"),
            "Media/Screenshots"
        );
        assert_eq!(suggested_subfolder("video", None, "a.mp4"), "Media/Videos");
    }

    #[test]
    fn template_renders() {
        let file = FileEntry {
            id: 1,
            root_id: 1,
            path: "C:/x/a.pdf".into(),
            parent_path: "C:/x".into(),
            name: "a.pdf".into(),
            ext: "pdf".into(),
            size: 0,
            mtime: 0,
            atime: 0,
            ctime: 0,
            category: "documents".into(),
            mime: "application/pdf".into(),
            tags: vec![],
            doc_type: None,
            confidence: 0.0,
            sensitive: false,
            analyzed: false,
            hash: None,
            snippet: None,
        };
        assert_eq!(
            render_template(
                "{vendor}_{docType}_{year}.{ext}",
                &file,
                "invoice",
                "Acme",
                "2026-11-12"
            ),
            "Acme_invoice_2026.pdf"
        );
    }
}
