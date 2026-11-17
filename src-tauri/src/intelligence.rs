//! Local intelligence: text extraction, document classification, field extraction.
//! Everything runs on-device. No content leaves the machine.

use crate::error::AppResult;
use crate::fsutil;
use crate::models::*;
use crate::repo;
use chrono::{NaiveDate, Utc};
use regex::Regex;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

const MAX_TEXT_CHARS: usize = 200_000;
const SUMMARY_CHARS: usize = 280;

/// Outcome of a single-file analysis pass, returned to the UI for display.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisOutcome {
    pub doc_type: String,
    pub confidence: f64,
    pub sensitive: bool,
    pub text_chars: usize,
}

/// A sensitive-data match found inside a file's text.
#[derive(Debug, Clone)]
pub struct SensitiveHit {
    pub kind: String,
    pub reason: String,
    pub severity: String,
    pub excerpt: String,
}

// ---------------------------------------------------------------------------
// Analysis
// ---------------------------------------------------------------------------

/// Analyse one indexed file: read its text locally, classify it, pull out the
/// fields we can support without guessing, and store the result on the row.
pub fn analyze_file(conn: &Connection, file_id: i64) -> AppResult<AnalysisOutcome> {
    let file = repo::load_file(conn, file_id)?;
    let path = Path::new(&file.path);
    let text = if path.is_file() {
        fsutil::extract_text(path, MAX_TEXT_CHARS).unwrap_or(None)
    } else {
        None
    };
    let text_ref = text.as_deref().unwrap_or("");
    let text_chars = text_ref.chars().count();

    let doc_type = classify(&file.name, text_ref);
    let confidence = confidence_for(doc_type, text_chars);
    let hit = detect_sensitive(text_ref);
    let amount = extract_amount(text_ref);
    let invoice_no = extract_invoice_no(text_ref);
    let due_date = extract_due_date(text_ref);
    let doc_date = extract_doc_date(text_ref);
    let vendor = extract_vendor(text_ref);
    let gstin = extract_gstin(text_ref);
    let summary = summarise(&file, doc_type, text_ref, amount.as_deref(), due_date);

    let mut evidence: Vec<Evidence> = Vec::new();
    if text_chars > 0 {
        evidence.push(Evidence {
            label: "Text layer".into(),
            value: format!("{text_chars} characters read on this device"),
            weight: 1.0,
        });
    } else {
        evidence.push(Evidence {
            label: "Text layer".into(),
            value: "No readable text layer (scanned image or unsupported format)".into(),
            weight: 0.6,
        });
    }
    if doc_type != "other" {
        evidence.push(Evidence {
            label: "Classification".into(),
            value: format!("Matched {doc_type} wording in the file name or text"),
            weight: 1.0,
        });
    }
    if let Some(found) = &amount {
        evidence.push(Evidence {
            label: "Amount".into(),
            value: found.clone(),
            weight: 1.0,
        });
    }
    if let Some(found) = &gstin {
        evidence.push(Evidence {
            label: "Tax ID".into(),
            value: found.clone(),
            weight: 1.0,
        });
    }
    if let Some(found) = &hit {
        evidence.push(Evidence {
            label: "Sensitive data".into(),
            value: found.reason.clone(),
            weight: 1.0,
        });
    }

    let content_status = if text.is_some() {
        "indexed"
    } else {
        "unsupported"
    };
    let evidence_json = serde_json::to_string(&evidence).unwrap_or_else(|_| "[]".into());

    conn.execute(
        "UPDATE files SET
            content_text = ?2,
            content_status = ?3,
            doc_type = ?4,
            confidence = ?5,
            sensitive = ?6,
            sensitive_kind = ?7,
            vendor = ?8,
            amount = ?9,
            invoice_no = ?10,
            doc_date = ?11,
            due_date = ?12,
            evidence = ?13,
            summary = ?14,
            analyzed_at = ?15
         WHERE id = ?1",
        params![
            file_id,
            text,
            content_status,
            if doc_type == "other" {
                None
            } else {
                Some(doc_type)
            },
            confidence,
            i64::from(hit.is_some()),
            hit.as_ref().map(|h| h.kind.clone()),
            vendor,
            amount,
            invoice_no,
            doc_date.map(|d| d.to_string()),
            due_date.map(|d| d.to_string()),
            evidence_json,
            summary,
            fsutil::now_secs(),
        ],
    )?;

    Ok(AnalysisOutcome {
        doc_type: doc_type.to_string(),
        confidence,
        sensitive: hit.is_some(),
        text_chars,
    })
}

/// Files that still need content analysis, most useful first.
pub fn pending_targets(conn: &Connection, limit: i64) -> AppResult<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, path FROM files
         WHERE content_status = 'none'
         ORDER BY CASE category
                    WHEN 'documents' THEN 0
                    WHEN 'images' THEN 1
                    ELSE 2
                  END, size DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit.clamp(1, 100_000)], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.filter_map(Result::ok).collect())
}

/// Analyse up to `limit` files that have not been read yet, reporting progress
/// and honouring the shared cancellation flag.
pub fn analyze_pending(
    conn: &mut Connection,
    limit: i64,
    cancel: &Arc<AtomicBool>,
    mut progress: impl FnMut(i64, i64, &str),
) -> AppResult<AnalyzeReport> {
    let started = Instant::now();
    let targets = pending_targets(conn, limit)?;

    let total = targets.len() as i64;
    let mut analyzed = 0i64;
    let mut failed = 0i64;
    let mut cancelled = false;

    for (index, (file_id, file_path)) in targets.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }
        progress(index as i64, total, file_path);
        match analyze_file(conn, *file_id) {
            Ok(_) => analyzed += 1,
            Err(_) => failed += 1,
        }
    }

    Ok(AnalyzeReport {
        analyzed,
        failed,
        cancelled,
        took_ms: started.elapsed().as_millis() as i64,
    })
}

/// Rebuild the "version" relations used by related-file and version views.
pub fn rebuild_relations(conn: &Connection) -> AppResult<usize> {
    conn.execute(
        "DELETE FROM relation_members WHERE relation_id IN
           (SELECT id FROM relations WHERE kind = 'version')",
        [],
    )?;
    conn.execute("DELETE FROM relations WHERE kind = 'version'", [])?;

    let columns = prefixed_columns();
    let files: Vec<FileEntry> = {
        let mut stmt = conn.prepare(&format!(
            "SELECT {columns} FROM files f
             WHERE f.category = 'documents' AND f.content_status IN ('indexed', 'unsupported')
             LIMIT 5000"
        ))?;
        let rows = stmt.query_map([], repo::row_to_file)?;
        rows.filter_map(Result::ok).collect()
    };

    let mut groups: std::collections::BTreeMap<String, Vec<FileEntry>> =
        std::collections::BTreeMap::new();
    for file in files {
        let key = version_key(&file.name);
        if key.is_empty() {
            continue;
        }
        groups.entry(key).or_default().push(file);
    }

    let mut created = 0usize;
    for (key, mut members) in groups {
        if members.len() < 2 {
            continue;
        }
        members.sort_by_key(|f| std::cmp::Reverse(f.mtime));
        let relation_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO relations(id, label, kind, confidence, created_at)
             VALUES (?1, ?2, 'version', ?3, ?4)",
            params![relation_id, key, 0.8, fsutil::now_secs()],
        )?;
        for member in &members {
            conn.execute(
                "INSERT OR IGNORE INTO relation_members(relation_id, file_id, role)
                 VALUES (?1, ?2, 'file')",
                params![relation_id, member.id],
            )?;
        }
        created += 1;
    }
    Ok(created)
}

/// Documents that have enough extracted information to show in Document Intelligence.
pub fn list_documents(
    conn: &Connection,
    doc_type: Option<&str>,
    limit: i64,
) -> AppResult<Vec<DocumentRecord>> {
    let ids: Vec<i64> = {
        let mut stmt = conn.prepare(
            "SELECT id FROM files
             WHERE doc_type IS NOT NULL AND doc_type <> 'other'
               AND (?1 IS NULL OR doc_type = ?1)
             ORDER BY COALESCE(analyzed_at, indexed_at) DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![doc_type, limit.clamp(1, 2000)], |r| r.get(0))?;
        rows.filter_map(Result::ok).collect()
    };

    Ok(ids
        .into_iter()
        .filter_map(|id| document_record(conn, id).ok())
        .collect())
}

/// Full detail for one document, including related files and reminders.
pub fn document_detail(conn: &Connection, file_id: i64) -> AppResult<DocumentRecord> {
    document_record(conn, file_id)
}

/// Files that look like versions of the same document.
pub fn list_versions(conn: &Connection) -> AppResult<Vec<VersionGroup>> {
    let columns = prefixed_columns();
    let files: Vec<FileEntry> = {
        let mut stmt = conn.prepare(&format!(
            "SELECT {columns} FROM files f
             WHERE f.category = 'documents'
             ORDER BY f.mtime DESC
             LIMIT 5000"
        ))?;
        let rows = stmt.query_map([], repo::row_to_file)?;
        rows.filter_map(Result::ok).collect()
    };

    let mut groups: std::collections::BTreeMap<String, Vec<FileEntry>> =
        std::collections::BTreeMap::new();
    for file in files {
        let key = version_key(&file.name);
        if key.is_empty() {
            continue;
        }
        groups.entry(key).or_default().push(file);
    }

    let mut out = Vec::new();
    for (key, mut members) in groups {
        if members.len() < 2 {
            continue;
        }
        members.sort_by_key(|f| std::cmp::Reverse(f.mtime));
        let latest_id = members.first().map(|f| f.id).unwrap_or_default();
        out.push(VersionGroup {
            key,
            latest_id,
            reason: "Same base filename with a version marker (for example v2, final or copy)."
                .into(),
            files: members,
        });
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(out)
}

/// Upcoming document dates. Past reminders are hidden unless explicitly requested.
pub fn list_reminders(conn: &Connection, include_past: bool) -> AppResult<Vec<Reminder>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.file_id, r.title, r.due_date, r.kind, f.name, f.path
         FROM reminders r JOIN files f ON f.id = r.file_id
         WHERE r.dismissed = 0
         ORDER BY r.due_date ASC",
    )?;
    let today = Utc::now().date_naive();
    let rows = stmt.query_map([], |row| {
        let due_date: String = row.get(3)?;
        let days_left = parse_date(&due_date)
            .map(|due| (due - today).num_days())
            .unwrap_or(0);
        Ok(Reminder {
            id: row.get(0)?,
            file_id: row.get(1)?,
            title: row.get(2)?,
            due_date,
            kind: row.get(4)?,
            file_name: row.get(5)?,
            path: row.get(6)?,
            days_left,
        })
    })?;

    let mut out: Vec<Reminder> = rows.filter_map(Result::ok).collect();
    if !include_past {
        out.retain(|r| r.days_left >= 0);
    }
    Ok(out)
}

/// Files linked to this one by a rebuilt relation (versions, duplicates, topics).
pub fn related_files(conn: &Connection, file_id: i64) -> AppResult<Vec<FileEntry>> {
    let columns = prefixed_columns();
    let sql = format!(
        "SELECT {columns} FROM files f
         JOIN relation_members m ON m.file_id = f.id
         WHERE m.relation_id IN
               (SELECT relation_id FROM relation_members WHERE file_id = ?1)
           AND f.id <> ?1
         LIMIT 50"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([file_id], repo::row_to_file)?;
    Ok(rows.filter_map(Result::ok).collect())
}

/// Files flagged as containing sensitive data, newest and largest first.
pub fn sensitive_findings(conn: &Connection, limit: i64) -> AppResult<Vec<SensitiveFinding>> {
    let ids: Vec<i64> = {
        let mut stmt =
            conn.prepare("SELECT id FROM files WHERE sensitive = 1 ORDER BY size DESC LIMIT ?1")?;
        let rows = stmt.query_map([limit.clamp(1, 5000)], |r| r.get(0))?;
        rows.filter_map(Result::ok).collect()
    };

    let mut out = Vec::new();
    for id in ids {
        let file = match repo::load_file(conn, id) {
            Ok(file) => file,
            Err(_) => continue,
        };
        let (kind, text): (Option<String>, Option<String>) = conn.query_row(
            "SELECT sensitive_kind, content_text FROM files WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let kind = kind.unwrap_or_else(|| "sensitive".into());
        out.push(SensitiveFinding {
            file,
            severity: severity_for(&kind).to_string(),
            excerpt: text.as_deref().map(redact).unwrap_or_default(),
            confidence: 0.9,
            evidence: vec![Evidence {
                label: "Local pattern scan".into(),
                value: format!("Matched a {kind} pattern in the file text"),
                weight: 1.0,
            }],
            kind,
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Classification helpers (also used by the organiser)
// ---------------------------------------------------------------------------

/// Cheap filename-only classification used when a file has not been read yet.
pub fn classify_simple(filename: &str) -> (&'static str, f64) {
    let doc_type = classify(filename, "");
    let confidence = if doc_type == "other" { 0.3 } else { 0.7 };
    (doc_type, confidence)
}

/// Classify a file from its name and (where available) extracted text.
pub fn classify(filename: &str, text: &str) -> &'static str {
    let haystack = format!("{} {}", filename.to_lowercase(), text.to_lowercase());
    let has = |needles: &[&str]| needles.iter().any(|needle| haystack.contains(needle));

    if has(&["invoice", "tax invoice", "bill to", "invoice no"]) {
        "invoice"
    } else if has(&["receipt", "cash memo", "payment received"]) {
        "receipt"
    } else if has(&[
        "resume",
        "curriculum vitae",
        "work experience",
        "education",
        "skills",
    ]) {
        "resume"
    } else if has(&[
        "bank statement",
        "account statement",
        "statement of account",
        "opening balance",
        "closing balance",
    ]) {
        "bank_statement"
    } else if has(&[
        "agreement",
        "contract",
        "terms and conditions",
        "signed between",
    ]) {
        "contract"
    } else if has(&["certificate", "certified", "awarded to"]) {
        "certificate"
    } else if has(&["presentation", "slide deck"]) {
        "presentation"
    } else {
        "other"
    }
}

fn confidence_for(doc_type: &str, text_chars: usize) -> f64 {
    let base = match doc_type {
        "invoice" | "receipt" | "bank_statement" => 0.86,
        "resume" | "contract" | "certificate" => 0.8,
        "presentation" => 0.7,
        _ => 0.3,
    };
    if text_chars > 0 {
        base
    } else {
        (base - 0.15).max(0.2)
    }
}

/// Stable grouping key that strips version markers and punctuation.
pub fn version_key(name: &str) -> String {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name);
    let cleaned: String = stem
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let mut words: Vec<&str> = cleaned.split_whitespace().collect();
    while let Some(last) = words.last().copied() {
        let version_word = matches!(
            last,
            "copy" | "final" | "old" | "new" | "backup" | "draft" | "revised" | "version"
        );
        // Short trailing numbers are copy counters (`file 001`). Longer ones are
        // usually years or reference numbers and must stay in the key.
        let numbered = last.len() <= 3 && last.chars().all(|c| c.is_ascii_digit());
        let v_number = last.len() > 1
            && last.starts_with('v')
            && last[1..].chars().all(|c| c.is_ascii_digit());
        if version_word || numbered || v_number {
            words.pop();
        } else {
            break;
        }
    }
    words.join(" ")
}

/// Parse the date formats commonly found in documents.
pub fn parse_date(raw: &str) -> Option<NaiveDate> {
    let cleaned = raw.trim().trim_end_matches([',', '.', ';', ')', ']']);
    const FORMATS: &[&str] = &[
        "%Y-%m-%d",
        "%d/%m/%Y",
        "%m/%d/%Y",
        "%d-%m-%Y",
        "%d.%m.%Y",
        "%Y/%m/%d",
        "%d %b %Y",
        "%d %B %Y",
        "%b %d, %Y",
        "%B %d, %Y",
        "%b %d %Y",
        "%B %d %Y",
    ];
    FORMATS
        .iter()
        .find_map(|format| NaiveDate::parse_from_str(cleaned, format).ok())
}

// ---------------------------------------------------------------------------
// Field extraction
// ---------------------------------------------------------------------------

pub fn extract_amount(text: &str) -> Option<String> {
    let found = amount_re().find(text)?.as_str().trim();
    if found.is_empty() {
        None
    } else {
        Some(found.to_string())
    }
}

pub fn extract_invoice_no(text: &str) -> Option<String> {
    let captures = invoice_re().captures(text)?;
    Some(captures.get(1)?.as_str().trim().to_string())
}

pub fn extract_due_date(text: &str) -> Option<NaiveDate> {
    let captures = due_re().captures(text)?;
    parse_date(captures.get(1)?.as_str())
}

pub fn extract_gstin(text: &str) -> Option<String> {
    let captures = gstin_re().captures(text)?;
    Some(captures.get(1)?.as_str().to_string())
}

fn extract_doc_date(text: &str) -> Option<NaiveDate> {
    let captures = doc_date_re().captures(text)?;
    parse_date(captures.get(1)?.as_str())
}

fn extract_vendor(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| (2..=80).contains(&line.len()))
        .find(|line| {
            let lower = line.to_lowercase();
            line.chars().any(|c| c.is_alphabetic())
                && !lower.contains("invoice")
                && !lower.contains("receipt")
                && !lower.contains("statement")
                && !lower.contains("date")
                && !lower.contains("total")
        })
        .map(|line| line.to_string())
}

fn summarise(
    file: &FileEntry,
    doc_type: &str,
    text: &str,
    amount: Option<&str>,
    due_date: Option<NaiveDate>,
) -> String {
    let label = match doc_type {
        "invoice" => "Invoice",
        "receipt" => "Receipt",
        "resume" => "Resume",
        "bank_statement" => "Bank statement",
        "contract" => "Contract",
        "certificate" => "Certificate",
        "presentation" => "Presentation",
        _ => "File",
    };
    let mut parts = vec![format!("{label}: {}", file.name)];
    if let Some(value) = amount {
        parts.push(format!("amount {value}"));
    }
    if let Some(due) = due_date {
        parts.push(format!("due {due}"));
    }
    if text.is_empty() {
        parts.push("no readable text was found, so this is based on the file name only".into());
    } else {
        let snippet = fsutil::truncate_chars(text, SUMMARY_CHARS);
        parts.push(snippet);
    }
    parts.join(". ")
}

// ---------------------------------------------------------------------------
// Sensitive-data detection
// ---------------------------------------------------------------------------

pub fn detect_sensitive(text: &str) -> Option<SensitiveHit> {
    if text.is_empty() {
        return None;
    }

    for captures in card_re().captures_iter(text) {
        let raw = captures.get(0)?.as_str();
        let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
        if (13..=19).contains(&digits.len()) && luhn_ok(&digits) {
            return Some(SensitiveHit {
                kind: "payment_card".into(),
                reason: "Looks like a payment card number (passed the Luhn check)".into(),
                severity: "high".into(),
                excerpt: format!("Card ending {}", &digits[digits.len() - 4..]),
            });
        }
    }

    if let Some(captures) = aadhaar_re().captures(text) {
        let value = captures.get(0)?.as_str();
        let digits: String = value.chars().filter(char::is_ascii_digit).collect();
        if digits.len() == 12 {
            return Some(SensitiveHit {
                kind: "national_id".into(),
                reason: "Looks like a 12-digit national ID number".into(),
                severity: "high".into(),
                excerpt: format!("ID ending {}", &digits[digits.len() - 4..]),
            });
        }
    }

    if pan_re().is_match(text) {
        return Some(SensitiveHit {
            kind: "tax_id".into(),
            reason: "Looks like a permanent account number (PAN)".into(),
            severity: "medium".into(),
            excerpt: "Tax identifier masked".into(),
        });
    }

    None
}

/// Mask most digits, keeping only the last four characters of a match.
fn redact(text: &str) -> String {
    let digits: Vec<char> = text.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 4 {
        let tail: String = digits[digits.len() - 4..].iter().collect();
        format!("•••• {tail}")
    } else {
        fsutil::truncate_chars(text, 120)
    }
}

fn severity_for(kind: &str) -> &'static str {
    match kind {
        "payment_card" | "national_id" => "high",
        "tax_id" => "medium",
        _ => "low",
    }
}

fn luhn_ok(digits: &str) -> bool {
    let mut sum = 0u32;
    let mut double = false;
    for ch in digits.chars().rev() {
        let mut value = match ch.to_digit(10) {
            Some(value) => value,
            None => return false,
        };
        if double {
            value *= 2;
            if value > 9 {
                value -= 9;
            }
        }
        sum += value;
        double = !double;
    }
    sum % 10 == 0
}

// ---------------------------------------------------------------------------
// Shared helpers and compiled patterns
// ---------------------------------------------------------------------------

fn prefixed_columns() -> String {
    repo::FILE_COLUMNS
        .split(',')
        .map(|column| format!("f.{}", column.trim()))
        .collect::<Vec<_>>()
        .join(", ")
}

type DocRow = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    f64,
    Option<String>,
    String,
);

fn document_record(conn: &Connection, file_id: i64) -> AppResult<DocumentRecord> {
    let file = repo::load_file(conn, file_id)?;
    let (doc_type, vendor, amount, invoice_no, doc_date, due_date, confidence, summary, evidence): DocRow = conn.query_row(
        "SELECT doc_type, vendor, amount, invoice_no, doc_date, due_date, confidence, summary, evidence
         FROM files WHERE id = ?1",
        [file_id],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
            ))
        },
    )?;

    Ok(DocumentRecord {
        doc_type: doc_type.unwrap_or_else(|| "other".into()),
        vendor,
        amount,
        invoice_no,
        doc_date,
        due_date,
        confidence,
        summary: summary.unwrap_or_default(),
        evidence: serde_json::from_str(&evidence).unwrap_or_default(),
        related: related_files(conn, file_id).unwrap_or_default(),
        reminders: reminders_for(conn, file_id),
        file,
    })
}

fn reminders_for(conn: &Connection, file_id: i64) -> Vec<Reminder> {
    let today = Utc::now().date_naive();
    let mut stmt = match conn.prepare(
        "SELECT id, title, due_date, kind FROM reminders
         WHERE file_id = ?1 AND dismissed = 0 ORDER BY due_date ASC",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map([file_id], |row| {
        let due_date: String = row.get(2)?;
        Ok(Reminder {
            id: row.get(0)?,
            file_id,
            title: row.get(1)?,
            days_left: parse_date(&due_date)
                .map(|due| (due - today).num_days())
                .unwrap_or(0),
            due_date,
            kind: row.get(3)?,
            file_name: String::new(),
            path: String::new(),
        })
    });
    match rows {
        Ok(rows) => rows.filter_map(Result::ok).collect(),
        Err(_) => Vec::new(),
    }
}

fn amount_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(?:rs\.?|inr|usd|eur|gbp|\$|₹|€|£)\s*[0-9][0-9,]*(?:\.[0-9]{1,2})?")
            .expect("amount pattern is valid")
    })
}

fn invoice_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)(?:invoice|bill)\s*(?:no\.?|number|num|#|:)\s*[:#-]?\s*([A-Za-z0-9][A-Za-z0-9/_\-]{2,24})",
        )
        .expect("invoice pattern is valid")
    })
}

fn due_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)(?:due\s*(?:date|on)|payment\s*due)\s*[:#-]?\s*([0-9]{1,2}[./-][0-9]{1,2}[./-][0-9]{2,4}|[0-9]{4}-[0-9]{2}-[0-9]{2}|[0-9]{1,2}\s+[A-Za-z]{3,9}\s+[0-9]{4}|[A-Za-z]{3,9}\s+[0-9]{1,2},\s*[0-9]{4})",
        )
        .expect("due-date pattern is valid")
    })
}

fn doc_date_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)(?:invoice\s*date|date\s*of\s*issue|issued\s*on|(?:^|\s)date)\s*[:#-]?\s*([0-9]{1,2}[./-][0-9]{1,2}[./-][0-9]{2,4}|[0-9]{4}-[0-9]{2}-[0-9]{2}|[0-9]{1,2}\s+[A-Za-z]{3,9}\s+[0-9]{4}|[A-Za-z]{3,9}\s+[0-9]{1,2},\s*[0-9]{4})",
        )
        .expect("document-date pattern is valid")
    })
}

fn gstin_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\b([0-9]{2}[A-Z]{5}[0-9]{4}[A-Z][1-9A-Z]Z[0-9A-Z])\b")
            .expect("GSTIN pattern is valid")
    })
}

fn card_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b(?:\d[ -]?){13,19}\b").expect("card pattern is valid"))
}

fn aadhaar_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\b[2-9][0-9]{3}\s?[0-9]{4}\s?[0-9]{4}\b").expect("ID pattern is valid")
    })
}

fn pan_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b[A-Z]{5}[0-9]{4}[A-Z]\b").expect("PAN pattern is valid"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luhn_accepts_valid_card_and_rejects_invalid() {
        assert!(luhn_ok("4111111111111111"));
        assert!(!luhn_ok("4111111111111112"));
    }

    #[test]
    fn classifies_from_filename() {
        assert_eq!(classify_simple("Invoice-2024-01.pdf").0, "invoice");
        assert_eq!(classify_simple("Resume.pdf").0, "resume");
        assert_eq!(classify_simple("holiday.jpg").0, "other");
    }

    #[test]
    fn version_key_strips_markers() {
        assert_eq!(version_key("Contract v2.pdf"), "contract");
        assert_eq!(version_key("Contract-final.pdf"), "contract");
        assert_eq!(version_key("Budget 2024.xlsx"), "budget 2024");
    }

    #[test]
    fn extracts_invoice_fields() {
        let text = "Tax Invoice\nInvoice No: INV-1042\nTotal: INR 1,299.00\nDue Date: 15/03/2025";
        assert_eq!(extract_invoice_no(text).as_deref(), Some("INV-1042"));
        assert!(extract_amount(text).is_some());
        assert_eq!(
            extract_due_date(text).map(|d| d.to_string()).as_deref(),
            Some("2025-03-15")
        );
    }

    #[test]
    fn detects_cards_and_masks_them() {
        let hit = detect_sensitive("Card 4111 1111 1111 1111 expires soon").unwrap();
        assert_eq!(hit.kind, "payment_card");
        assert!(hit.excerpt.contains("1111"));
        assert!(!hit.excerpt.contains("4111 1111"));
    }
}
