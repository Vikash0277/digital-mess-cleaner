//! Metadata, full-text and natural-language search plus "Ask My Files" answers.

use crate::error::{AppError, AppResult};
use crate::fsutil;
use crate::models::*;
use crate::repo;
use chrono::{Datelike, Duration, NaiveDate, TimeZone, Utc};
use regex::Regex;
use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection};
use std::collections::HashSet;
use std::time::Instant;

pub struct ParsedQuery {
    pub tokens: Vec<String>,
    /// Exact phrases typed with double quotes, e.g. "electricity bill".
    pub phrases: Vec<String>,
    /// Whitelisted FTS column hints, e.g. `content:invoice`.
    pub fields: Vec<(String, String)>,
    pub interpretation: Vec<String>,
    pub filters: SearchFilters,
    pub sort: Option<String>,
}

fn re(pat: &str) -> Regex {
    Regex::new(pat).expect("built-in regex must compile")
}

fn size_multiplier(unit: &str) -> f64 {
    match unit.to_lowercase().as_str() {
        "kb" => 1024.0,
        "mb" => 1024.0 * 1024.0,
        "gb" => 1024.0 * 1024.0 * 1024.0,
        "tb" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => 1.0,
    }
}

fn midnight(days_ago: i64) -> i64 {
    let now = Utc::now();
    let date = now.date_naive() - Duration::days(days_ago);
    Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
        .timestamp()
}

fn year_bounds(year: i32) -> (i64, i64) {
    let start = NaiveDate::from_ymd_opt(year, 1, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    let end = NaiveDate::from_ymd_opt(year, 12, 31)
        .unwrap()
        .and_hms_opt(23, 59, 59)
        .unwrap();
    (
        Utc.from_utc_datetime(&start).timestamp(),
        Utc.from_utc_datetime(&end).timestamp(),
    )
}

pub fn parse_query(input: &str, mut filters: SearchFilters) -> ParsedQuery {
    let mut interpretation: Vec<String> = Vec::new();
    let mut sort: Option<String> = None;
    let mut phrases: Vec<String> = Vec::new();
    let mut fields: Vec<(String, String)> = Vec::new();

    // Pull out exact phrases first so their words are not eaten by the
    // natural-language filter rules below.
    let chars: Vec<char> = input.chars().collect();
    let mut without_phrases = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '"' {
            if let Some(relative) = chars[index + 1..].iter().position(|c| *c == '"') {
                let phrase: String = chars[index + 1..index + 1 + relative].iter().collect();
                let phrase = phrase.trim();
                if !phrase.is_empty() {
                    phrases.push(phrase.to_string());
                    interpretation.push(format!("Exact phrase: {phrase}"));
                }
                index += relative + 2;
                continue;
            }
        }
        without_phrases.push(chars[index]);
        index += 1;
    }

    let mut working = without_phrases;
    for (field, pattern) in [
        ("name", r#"(?i)\bname:\s*([^\s"]+)"#),
        ("path", r#"(?i)\bpath:\s*([^\s"]+)"#),
        ("content", r#"(?i)\bcontent:\s*([^\s"]+)"#),
    ] {
        let field_pattern = re(pattern);
        for captures in field_pattern.captures_iter(&working) {
            let value = captures[1].to_lowercase();
            if value.chars().count() >= 2 {
                fields.push((field.to_string(), value.clone()));
                interpretation.push(format!("Text in {field}: {value}"));
            }
        }
        working = field_pattern.replace_all(&working, " ").to_string();
    }

    let low = working.to_lowercase();

    // Sensitive / private intent.
    if ["confidential", "sensitive", "password", "passwords"]
        .iter()
        .any(|w| low.contains(w))
    {
        filters.sensitive_only = true;
        interpretation.push("Only sensitive files".into());
    }

    // Document types.
    let doc_types: [(&str, &[&str]); 8] = [
        ("invoice", &["invoice", "invoices", "bill", "bills"]),
        ("receipt", &["receipt", "receipts"]),
        ("contract", &["contract", "contracts", "agreement"]),
        ("resume", &["resume", "resumes", "cv"]),
        ("statement", &["statement", "statements"]),
        ("warranty", &["warranty", "warranties"]),
        ("certificate", &["certificate", "certificates"]),
        ("report", &["report", "reports"]),
    ];
    for (doc_type, words) in doc_types {
        for word in words {
            let pattern = format!(r"(?i)\b{}\b", regex::escape(word));
            if re(&pattern).is_match(&working) {
                filters.doc_types.push(doc_type.to_string());
                interpretation.push(format!("Document type: {doc_type}"));
                working = re(&pattern).replace_all(&working, " ").to_string();
                break;
            }
        }
    }

    // Categories and extensions.
    let cat_terms: [(&str, &[&str]); 5] = [
        (
            "images",
            &[
                "photo",
                "photos",
                "image",
                "images",
                "picture",
                "pictures",
                "screenshot",
                "screenshots",
            ],
        ),
        ("video", &["video", "videos", "movie", "movies"]),
        ("audio", &["audio", "music", "song", "songs"]),
        ("archives", &["archive", "archives", "zip", "zips"]),
        ("installers", &["installer", "installers"]),
    ];
    for (cat, words) in cat_terms {
        for word in words {
            let pattern = format!(r"(?i)\b{}\b", regex::escape(word));
            if re(&pattern).is_match(&working) {
                filters.categories.push(cat.to_string());
                interpretation.push(format!("Category: {}", fsutil::category_label(cat)));
                working = re(&pattern).replace_all(&working, " ").to_string();
                break;
            }
        }
    }

    let ext_terms: [(&str, &[&str]); 6] = [
        ("pdf", &["pdf", "pdfs"]),
        ("docx", &["word", "docx"]),
        ("xlsx", &["excel", "spreadsheet", "spreadsheets", "xlsx"]),
        (
            "pptx",
            &["powerpoint", "presentation", "presentations", "pptx"],
        ),
        ("zip", &["archive", "archives"]),
        ("exe", &["installer", "installers"]),
    ];
    for (ext, words) in ext_terms {
        for word in words {
            let pattern = format!(r"(?i)\b{}\b", regex::escape(word));
            if re(&pattern).is_match(&working) && !filters.exts.iter().any(|e| e == ext) {
                filters.exts.push(ext.to_string());
                interpretation.push(format!("File extension: .{ext}"));
                working = re(&pattern).replace_all(&working, " ").to_string();
                break;
            }
        }
    }

    // Time expressions.
    let now_year = Utc::now().year();
    let time_rules: Vec<(&str, i64, i64, &str)> = vec![
        (
            "last year",
            year_bounds(now_year - 1).0,
            year_bounds(now_year - 1).1,
            "Modified last year",
        ),
        (
            "this year",
            year_bounds(now_year).0,
            year_bounds(now_year).1,
            "Modified this year",
        ),
        (
            "last month",
            midnight(60),
            midnight(30),
            "Modified in the last two months",
        ),
        (
            "last week",
            midnight(7),
            midnight(0),
            "Modified in the last 7 days",
        ),
        ("today", midnight(0), i64::MAX, "Modified today"),
        ("yesterday", midnight(1), midnight(0), "Modified yesterday"),
    ];
    for (phrase, after, before, label) in time_rules {
        if working.to_lowercase().contains(phrase) {
            filters.modified_after = Some(after);
            if before != i64::MAX {
                filters.modified_before = Some(before);
            }
            interpretation.push(label.into());
            working = re(&format!(r"(?i){}", regex::escape(phrase)))
                .replace_all(&working, " ")
                .to_string();
        }
    }

    // "older than N days"
    if let Some(c) = re(r"(?i)(?:older than|not modified for|inactive for)\s*(\d+)\s*(day|days|month|months|year|years)")
        .captures(&working)
    {
        let n: i64 = c[1].parse().unwrap_or(0);
        let mult = match c[2].to_lowercase().as_str() {
            "month" | "months" => 30,
            "year" | "years" => 365,
            _ => 1,
        };
        filters.modified_before = Some(fsutil::now_secs() - n * mult * 86_400);
        interpretation.push(format!("Not modified for {n} {}", &c[2]));
        working = re(r"(?i)(?:older than|not modified for|inactive for)\s*\d+\s*(?:day|days|month|months|year|years)")
            .replace_all(&working, " ")
            .to_string();
    }

    // Explicit years like "2024".
    if let Some(c) = re(r"\b(19|20)\d{2}\b").find(&working) {
        if let Ok(year) = c.as_str().parse::<i32>() {
            let (a, b) = year_bounds(year);
            filters.modified_after = Some(a);
            filters.modified_before = Some(b);
            interpretation.push(format!("From {year}"));
            working = working.replace(c.as_str(), " ");
        }
    }

    // Sizes.
    if let Some(c) = re(
        r"(?i)(?:over|larger than|bigger than|more than|above)\s*(\d+(?:\.\d+)?)\s*(kb|mb|gb|tb)",
    )
    .captures(&working)
    {
        let v: f64 = c[1].parse().unwrap_or(0.0) * size_multiplier(&c[2]);
        filters.min_size = Some(v as i64);
        interpretation.push(format!("Larger than {}{}", &c[1], c[2].to_uppercase()));
        working = re(r"(?i)(?:over|larger than|bigger than|more than|above)\s*\d+(?:\.\d+)?\s*(?:kb|mb|gb|tb)")
            .replace_all(&working, " ")
            .to_string();
    }
    if let Some(c) =
        re(r"(?i)(?:under|less than|smaller than|below)\s*(\d+(?:\.\d+)?)\s*(kb|mb|gb|tb)")
            .captures(&working)
    {
        let v: f64 = c[1].parse().unwrap_or(0.0) * size_multiplier(&c[2]);
        filters.max_size = Some(v as i64);
        interpretation.push(format!("Smaller than {}{}", &c[1], c[2].to_uppercase()));
        working =
            re(r"(?i)(?:under|less than|smaller than|below)\s*\d+(?:\.\d+)?\s*(?:kb|mb|gb|tb)")
                .replace_all(&working, " ")
                .to_string();
    }

    // Sort intent.
    let lower = working.to_lowercase();
    if ["largest", "biggest", "most space", "taking the most"]
        .iter()
        .any(|w| lower.contains(w))
    {
        sort = Some("size_desc".into());
        interpretation.push("Sorted by largest".into());
    } else if ["oldest", "haven't opened", "have not opened", "not opened"]
        .iter()
        .any(|w| lower.contains(w))
    {
        sort = Some("mtime_asc".into());
        interpretation.push("Oldest first".into());
    } else if ["newest", "latest", "most recent"]
        .iter()
        .any(|w| lower.contains(w))
    {
        sort = Some("mtime_desc".into());
        interpretation.push("Newest first".into());
    }

    // Drop conversational filler.
    let stop: HashSet<&str> = [
        "find",
        "show",
        "give",
        "get",
        "me",
        "my",
        "mine",
        "all",
        "please",
        "the",
        "a",
        "an",
        "of",
        "in",
        "on",
        "at",
        "for",
        "from",
        "with",
        "and",
        "or",
        "to",
        "is",
        "are",
        "was",
        "were",
        "do",
        "does",
        "did",
        "where",
        "what",
        "which",
        "that",
        "this",
        "these",
        "those",
        "it",
        "its",
        "i",
        "we",
        "you",
        "file",
        "files",
        "document",
        "documents",
        "please",
        "sorted",
        "list",
        "about",
        "related",
        "anything",
        "something",
        "computer",
        "drive",
        "disk",
        "last",
        "year",
        "month",
        "week",
        "today",
        "yesterday",
        "taking",
        "most",
        "space",
        "largest",
        "biggest",
        "oldest",
        "newest",
        "latest",
        "recent",
        "more",
        "less",
        "than",
        "over",
        "under",
        "larger",
        "smaller",
        "above",
        "below",
    ]
    .into_iter()
    .collect();

    let tokens: Vec<String> = working
        .split(|c: char| !c.is_alphanumeric())
        .map(|t| t.trim().to_lowercase())
        .filter(|t| t.len() >= 2 && !stop.contains(t.as_str()))
        .filter(|t| !t.chars().all(|c| c.is_ascii_digit() && t.len() < 4))
        .collect();
    let mut seen = HashSet::new();
    let tokens: Vec<String> = tokens
        .into_iter()
        .filter(|t| seen.insert(t.clone()))
        .collect();

    if !tokens.is_empty() {
        interpretation.push(format!("Matching: {}", tokens.join(", ")));
    }

    ParsedQuery {
        tokens,
        phrases,
        fields,
        interpretation,
        filters,
        sort,
    }
}

fn push_filter_sql(filters: &SearchFilters, clauses: &mut Vec<String>, values: &mut Vec<Value>) {
    if !filters.categories.is_empty() {
        let marks = std::iter::repeat("?")
            .take(filters.categories.len())
            .collect::<Vec<_>>()
            .join(",");
        clauses.push(format!("f.category IN ({marks})"));
        for c in &filters.categories {
            values.push(Value::Text(c.clone()));
        }
    }
    if !filters.exts.is_empty() {
        let marks = std::iter::repeat("?")
            .take(filters.exts.len())
            .collect::<Vec<_>>()
            .join(",");
        clauses.push(format!("f.ext IN ({marks})"));
        for c in &filters.exts {
            values.push(Value::Text(c.trim_start_matches('.').to_lowercase()));
        }
    }
    if !filters.doc_types.is_empty() {
        let marks = std::iter::repeat("?")
            .take(filters.doc_types.len())
            .collect::<Vec<_>>()
            .join(",");
        clauses.push(format!("f.doc_type IN ({marks})"));
        for c in &filters.doc_types {
            values.push(Value::Text(c.clone()));
        }
    }
    if let Some(root) = filters.root_id {
        clauses.push("f.root_id = ?".into());
        values.push(Value::Integer(root));
    }
    if let Some(min) = filters.min_size {
        clauses.push("f.size >= ?".into());
        values.push(Value::Integer(min));
    }
    if let Some(max) = filters.max_size {
        clauses.push("f.size <= ?".into());
        values.push(Value::Integer(max));
    }
    if let Some(after) = filters.modified_after {
        clauses.push("f.mtime >= ?".into());
        values.push(Value::Integer(after));
    }
    if let Some(before) = filters.modified_before {
        clauses.push("f.mtime <= ?".into());
        values.push(Value::Integer(before));
    }
    if filters.sensitive_only {
        clauses.push("f.sensitive = 1".into());
    }
}

fn order_sql(sort: Option<&str>) -> &'static str {
    match sort.unwrap_or("mtime_desc") {
        "size_desc" | "largest" => "f.size DESC",
        "size_asc" => "f.size ASC",
        "mtime_asc" | "oldest" => "f.mtime ASC",
        "name_asc" => "f.name COLLATE NOCASE ASC",
        "name_desc" => "f.name COLLATE NOCASE DESC",
        _ => "f.mtime DESC",
    }
}

/// Full-text results can be ranked by FTS relevance, but the user's explicit
/// sort always wins. Returned strings are fixed SQL fragments, never input.
fn fts_order_sql(sort: Option<&str>) -> &'static str {
    match sort {
        None | Some("") | Some("relevance") => "rank",
        Some("newest") => "f.mtime DESC",
        Some("oldest") => "f.mtime ASC",
        Some("largest") => "f.size DESC",
        Some(_) => order_sql(sort),
    }
}

/// Keep only word characters. Quotes were handled by `parse_query`, and FTS5
/// operators never reach the expression from user input.
fn fts_term(raw: &str) -> String {
    raw.chars().filter(|c| c.is_alphanumeric()).collect()
}

/// Phrases keep their spaces; only characters that could alter FTS5 syntax are
/// dropped. The tokenizer still splits on punctuation inside the quotes.
fn fts_phrase(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect()
}

fn build_fts_expression(parsed: &ParsedQuery) -> AppResult<String> {
    let mut terms: Vec<String> = Vec::new();

    for phrase in &parsed.phrases {
        let clean = fts_phrase(phrase);
        if !clean.is_empty() {
            terms.push(format!("\"{clean}\""));
        }
    }
    for (field, value) in &parsed.fields {
        let column = match field.as_str() {
            "name" => "name",
            "path" => "path",
            "content" => "content_text",
            _ => continue,
        };
        let clean = fts_term(value);
        if !clean.is_empty() {
            terms.push(format!("{column}:\"{clean}\"*"));
        }
    }
    for token in &parsed.tokens {
        let clean = fts_term(token);
        if !clean.is_empty() {
            terms.push(format!("\"{clean}\"*"));
        }
    }

    if terms.len() > 12 {
        return Err(AppError::Invalid(
            "That search has too many words. Try fewer terms.".into(),
        ));
    }
    Ok(terms.join(" AND "))
}

/// SQLite writes these sentinels around matched words in `highlight()`.
const HL_OPEN: char = '\u{1}';
const HL_CLOSE: char = '\u{2}';

fn parse_highlighted(marked: &str) -> (Option<String>, Vec<SnippetPart>) {
    if !marked.contains(HL_OPEN) && !marked.contains(HL_CLOSE) {
        return (None, Vec::new());
    }
    let mut plain = String::new();
    let mut parts: Vec<SnippetPart> = Vec::new();
    let mut current = String::new();
    let mut highlight = false;
    for ch in marked.chars() {
        match ch {
            HL_OPEN => {
                if !current.is_empty() {
                    parts.push(SnippetPart {
                        text: std::mem::take(&mut current),
                        highlight,
                    });
                }
                highlight = true;
            }
            HL_CLOSE => {
                if !current.is_empty() {
                    parts.push(SnippetPart {
                        text: std::mem::take(&mut current),
                        highlight,
                    });
                }
                highlight = false;
            }
            '\n' | '\r' | '\t' => {
                current.push(' ');
                plain.push(' ');
            }
            _ => {
                current.push(ch);
                plain.push(ch);
            }
        }
    }
    if !current.is_empty() {
        parts.push(SnippetPart {
            text: current,
            highlight,
        });
    }
    if plain.is_empty() {
        (None, Vec::new())
    } else {
        (Some(plain), parts)
    }
}

fn plain_snippet_parts(text: &str) -> Vec<SnippetPart> {
    if text.is_empty() {
        Vec::new()
    } else {
        vec![SnippetPart {
            text: text.to_string(),
            highlight: false,
        }]
    }
}

pub fn search(conn: &Connection, query: &str, filters: SearchFilters) -> AppResult<SearchResponse> {
    let started = Instant::now();
    if query.chars().count() > 500 {
        return Err(AppError::Invalid(
            "That search is too long. Try a shorter search.".into(),
        ));
    }
    let parsed = parse_query(query, filters);
    // Natural-language words such as "oldest" override an explicit sort from
    // the UI; otherwise the caller's sort is the one that matters.
    let sort = parsed.sort.as_deref().or(parsed.filters.sort.as_deref());
    let limit = parsed.filters.limit.unwrap_or(200).clamp(1, 1000);
    let offset = parsed.filters.offset.unwrap_or(0).max(0);
    let columns = repo::FILE_COLUMNS
        .split(',')
        .map(|c| format!("f.{}", c.trim()))
        .collect::<Vec<_>>()
        .join(", ");

    let mut hits: Vec<SearchHit> = Vec::new();
    let mut mode = "metadata".to_string();
    let mut total = 0i64;

    let fts = build_fts_expression(&parsed)?;
    if !fts.is_empty() {
        let mut values: Vec<Value> = vec![Value::Text(fts)];
        let mut clauses: Vec<String> = Vec::new();
        push_filter_sql(&parsed.filters, &mut clauses, &mut values);
        let where_sql = if clauses.is_empty() {
            " FROM files f JOIN files_fts ON files_fts.rowid = f.id WHERE files_fts MATCH ?"
                .to_string()
        } else {
            format!(
                " FROM files f JOIN files_fts ON files_fts.rowid = f.id WHERE files_fts MATCH ? AND {}",
                clauses.join(" AND ")
            )
        };

        let count_sql = format!("SELECT count(*){where_sql}");
        total = conn.query_row(&count_sql, params_from_iter(values.iter()), |r| r.get(0))?;

        if total > 0 {
            let sql = format!(
                "SELECT {columns}, f.content_text, bm25(files_fts) AS rank, \
                 highlight(files_fts, 2, char(1), char(2)) AS fts_snippet \
                 {where_sql} ORDER BY {} LIMIT ? OFFSET ?",
                fts_order_sql(sort)
            );
            let mut row_values = values.clone();
            row_values.push(Value::Integer(limit));
            row_values.push(Value::Integer(offset));
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params_from_iter(row_values.iter()), |row| {
                let file = repo::row_to_file(row)?;
                let content: Option<String> = row.get(18)?;
                let rank: f64 = row.get(19).unwrap_or(0.0);
                let marked: Option<String> = row.get(20)?;
                Ok((file, content, rank, marked))
            })?;
            for (file, content, rank, marked) in rows.flatten() {
                let (snippet, parts) = match parse_highlighted(marked.as_deref().unwrap_or("")) {
                    (Some(plain), parts) if !plain.is_empty() => (plain, parts),
                    _ => {
                        let fallback = build_snippet(&parsed.tokens, content.as_deref(), &file);
                        let parts = plain_snippet_parts(&fallback);
                        (fallback, parts)
                    }
                };
                let matched_on = matched_field(&parsed.tokens, content.as_deref(), &file);
                hits.push(SearchHit {
                    file,
                    snippet,
                    snippet_parts: parts,
                    matched_on,
                    score: 1.0 / (1.0 + rank.abs()),
                });
            }
            if !hits.is_empty() {
                mode = "full-text".into();
            }
        }
    }

    // Best-effort substring fallback for words the tokenizer splits, e.g. a
    // search inside a longer identifier. Only runs when FTS found nothing.
    if total == 0 {
        let mut clauses: Vec<String> = Vec::new();
        let mut values: Vec<Value> = Vec::new();
        for token in &parsed.tokens {
            if token.is_empty() {
                continue;
            }
            clauses.push("(f.name LIKE ? COLLATE NOCASE OR f.path LIKE ? COLLATE NOCASE OR lower(COALESCE(f.content_text,'')) LIKE ?)".into());
            let prefix = format!("{token}%");
            let anywhere = format!("%{token}%");
            values.push(Value::Text(prefix.clone()));
            values.push(Value::Text(prefix));
            values.push(Value::Text(anywhere));
        }
        for phrase in &parsed.phrases {
            clauses.push(
                "(lower(COALESCE(f.content_text,'')) LIKE ? OR f.name LIKE ? COLLATE NOCASE)"
                    .into(),
            );
            let like = format!("%{}%", phrase.to_lowercase());
            values.push(Value::Text(like));
            values.push(Value::Text(format!("{phrase}%")));
        }
        push_filter_sql(&parsed.filters, &mut clauses, &mut values);
        let where_sql = if clauses.is_empty() {
            "1=1".to_string()
        } else {
            clauses.join(" AND ")
        };

        let count_sql = format!("SELECT count(*) FROM files f WHERE {where_sql}");
        total = conn.query_row(&count_sql, params_from_iter(values.iter()), |r| r.get(0))?;

        if total > 0 {
            let sql = format!(
                "SELECT {columns}, f.content_text FROM files f WHERE {where_sql} ORDER BY {} LIMIT ? OFFSET ?",
                order_sql(sort)
            );
            let mut row_values = values.clone();
            row_values.push(Value::Integer(limit));
            row_values.push(Value::Integer(offset));
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params_from_iter(row_values.iter()), |row| {
                let file = repo::row_to_file(row)?;
                let content: Option<String> = row.get(18)?;
                Ok((file, content))
            })?;
            for (file, content) in rows.flatten() {
                let snippet = build_snippet(&parsed.tokens, content.as_deref(), &file);
                let parts = plain_snippet_parts(&snippet);
                let matched_on = matched_field(&parsed.tokens, content.as_deref(), &file);
                hits.push(SearchHit {
                    file,
                    snippet,
                    snippet_parts: parts,
                    matched_on,
                    score: 0.5,
                });
            }
            if !hits.is_empty() {
                mode = "metadata".into();
            }
        }
    }

    Ok(SearchResponse {
        query: query.to_string(),
        interpretation: parsed.interpretation,
        hits,
        total,
        took_ms: started.elapsed().as_millis() as i64,
        mode,
    })
}

fn matched_field(tokens: &[String], content: Option<&str>, file: &FileEntry) -> String {
    let low_name = file.name.to_lowercase();
    let low_path = file.path.to_lowercase();
    let low_content = content.unwrap_or("").to_lowercase();
    let mut fields = Vec::new();
    for token in tokens {
        if low_name.contains(token) {
            fields.push("name");
        } else if low_content.contains(token) {
            fields.push("content");
        } else if low_path.contains(token) {
            fields.push("path");
        }
    }
    fields.sort();
    fields.dedup();
    if fields.is_empty() {
        "filters".into()
    } else {
        fields.join(" + ")
    }
}

fn build_snippet(tokens: &[String], content: Option<&str>, file: &FileEntry) -> String {
    let source = content.unwrap_or("");
    if source.is_empty() {
        return file.path.clone();
    }

    // Work on characters, never raw byte offsets: file text is arbitrary UTF-8
    // and slicing at a byte boundary could panic the whole search command.
    let chars: Vec<char> = source.chars().collect();
    let lower: Vec<char> = chars
        .iter()
        .map(|c| c.to_lowercase().next().unwrap_or(*c))
        .collect();
    let mut best: Option<(usize, String)> = None;

    for token in tokens {
        let needle: Vec<char> = token.chars().collect();
        if needle.is_empty() || needle.len() > lower.len() {
            continue;
        }
        let mut found = None;
        for start in 0..=lower.len() - needle.len() {
            if lower[start..start + needle.len()] == needle[..] {
                found = Some(start);
                break;
            }
        }
        if let Some(pos) = found {
            let from = pos.saturating_sub(90);
            let to = (pos + needle.len() + 150).min(chars.len());
            let mut snippet = String::new();
            if from > 0 {
                snippet.push('…');
            }
            snippet.extend(chars[from..to].iter());
            if to < chars.len() {
                snippet.push('…');
            }
            if best.as_ref().map(|(p, _)| pos < *p).unwrap_or(true) {
                best = Some((pos, snippet));
            }
        }
    }

    best.map(|(_, s)| fsutil::normalize_ws(&s))
        .unwrap_or_else(|| fsutil::truncate_chars(&fsutil::normalize_ws(source), 220))
}

pub fn list_files(
    conn: &Connection,
    filters: SearchFilters,
    limit: i64,
    offset: i64,
) -> AppResult<Page<FileEntry>> {
    let columns = repo::FILE_COLUMNS
        .split(',')
        .map(|c| format!("f.{}", c.trim()))
        .collect::<Vec<_>>()
        .join(", ");
    let mut clauses: Vec<String> = Vec::new();
    let mut values: Vec<Value> = Vec::new();
    push_filter_sql(&filters, &mut clauses, &mut values);
    let where_sql = if clauses.is_empty() {
        "1=1".to_string()
    } else {
        clauses.join(" AND ")
    };
    let total: i64 = {
        let sql = format!("SELECT count(*) FROM files f WHERE {where_sql}");
        conn.query_row(&sql, params_from_iter(values.iter()), |r| r.get(0))?
    };
    let sql = format!(
        "SELECT {columns} FROM files f WHERE {where_sql} ORDER BY {} LIMIT ? OFFSET ?",
        order_sql(filters.sort.as_deref())
    );
    values.push(Value::Integer(limit));
    values.push(Value::Integer(offset));
    let mut stmt = conn.prepare(&sql)?;
    let items = stmt
        .query_map(params_from_iter(values.iter()), repo::row_to_file)?
        .filter_map(Result::ok)
        .collect();
    Ok(Page {
        items,
        total,
        offset,
        limit,
    })
}

pub fn ask(conn: &Connection, question: &str) -> AppResult<AskAnswer> {
    let started = Instant::now();
    let low = question.to_lowercase();
    let mut evidence: Vec<Evidence> = Vec::new();

    // Storage intent.
    if ["space", "storage", "biggest", "largest", "full"]
        .iter()
        .any(|w| low.contains(w))
    {
        let total: i64 =
            conn.query_row("SELECT COALESCE(sum(size),0) FROM files", [], |r| r.get(0))?;
        let count: i64 = conn.query_row("SELECT count(*) FROM files", [], |r| r.get(0))?;
        let cat = crate::storage::category_breakdown(conn)?;
        let top_cat = cat.first().cloned();
        let largest = largest_record(conn)?;
        let mut answer = format!(
            "You have indexed {} files using {}. ",
            count,
            crate::format::bytes(total)
        );
        if let Some(c) = &top_cat {
            answer.push_str(&format!(
                "{} take up the most room at {} ({} files). ",
                fsutil::category_label(&c.category),
                crate::format::bytes(c.bytes),
                c.count
            ));
        }
        if let Some((name, size, path)) = &largest {
            answer.push_str(&format!(
                "The single largest file is {name} at {}. It is at {path}.",
                crate::format::bytes(*size)
            ));
        }
        evidence.push(Evidence {
            label: "Indexed files".into(),
            value: count.to_string(),
            weight: 1.0,
        });
        evidence.push(Evidence {
            label: "Total indexed size".into(),
            value: crate::format::bytes(total),
            weight: 1.0,
        });
        if let Some(c) = top_cat {
            evidence.push(Evidence {
                label: "Largest category".into(),
                value: fsutil::category_label(&c.category).into(),
                weight: 1.0,
            });
        }
        return Ok(AskAnswer {
            question: question.into(),
            answer,
            references: Vec::new(),
            evidence,
            confidence: if count > 0 { 0.9 } else { 0.3 },
            caveat: if count == 0 {
                Some(
                    "No folders have been scanned yet, so this answer is based on an empty index."
                        .into(),
                )
            } else {
                None
            },
            took_ms: started.elapsed().as_millis() as i64,
        });
    }

    // Forgotten / unopened intent.
    if [
        "haven't opened",
        "have not opened",
        "not opened",
        "forgotten",
        "old",
        "inactive",
    ]
    .iter()
    .any(|w| low.contains(w))
    {
        let days = if low.contains("two years") || low.contains("2 years") {
            730
        } else {
            365
        };
        let files = crate::storage::forgotten_files(conn, days, 0, 10)?;
        let total: i64 = files.iter().map(|f| f.size).sum();
        let answer = if files.is_empty() {
            format!("I did not find any files untouched for {days} days in the current index.")
        } else {
            format!(
                "I found {} files not modified in {days} days, using about {}. The largest is “{}” ({}). Review them before removing anything — old does not mean unnecessary.",
                files.len(),
                crate::format::bytes(total),
                files[0].name,
                crate::format::bytes(files[0].size)
            )
        };
        evidence.push(Evidence {
            label: "Inactivity window".into(),
            value: format!("{days} days"),
            weight: 1.0,
        });
        evidence.push(Evidence {
            label: "Candidates".into(),
            value: files.len().to_string(),
            weight: 1.0,
        });
        let references = files
            .into_iter()
            .take(6)
            .map(|file| SearchHit {
                snippet: file.path.clone(),
                snippet_parts: Vec::new(),
                file,
                matched_on: "last modified".into(),
                score: 0.6,
            })
            .collect();
        return Ok(AskAnswer {
            question: question.into(),
            answer,
            references,
            evidence,
            confidence: 0.75,
            caveat: Some("Old files can still be important (tax records, certificates). Nothing is removed without your approval.".into()),
            took_ms: started.elapsed().as_millis() as i64,
        });
    }

    // Default: retrieval answer with references.
    let response = search(conn, question, SearchFilters::default())?;
    let hits = response.hits;
    let (answer, confidence, caveat) = if hits.is_empty() {
        (
            "I could not find a matching file in the indexed folders. Try a broader phrase, scan the folder that contains it, or run content analysis so text inside documents becomes searchable.".to_string(),
            0.2,
            Some("Nothing is sent off this device. Answers are limited to files you have indexed.".into()),
        )
    } else {
        let top = &hits[0];
        let mut text = format!(
            "I found {} matching file{}. The closest is “{}” at {} ({}).",
            hits.len(),
            if hits.len() == 1 { "" } else { "s" },
            top.file.name,
            top.file.path,
            crate::format::bytes(top.file.size)
        );
        if let Some(doc_type) = &top.file.doc_type {
            text.push_str(&format!(" It looks like a {doc_type}."));
        }
        if top.snippet != top.file.path {
            text.push_str(&format!(
                " Relevant text: “{}”.",
                fsutil::truncate_chars(&top.snippet, 200)
            ));
        }
        text.push_str(" Sources are listed below.");
        (text, 0.78, None)
    };
    evidence.push(Evidence {
        label: "Search mode".into(),
        value: response.mode.clone(),
        weight: 1.0,
    });
    evidence.push(Evidence {
        label: "Matches".into(),
        value: hits.len().to_string(),
        weight: 1.0,
    });
    evidence.push(Evidence {
        label: "Search time".into(),
        value: format!("{} ms", response.took_ms),
        weight: 1.0,
    });

    Ok(AskAnswer {
        question: question.into(),
        answer,
        references: hits.into_iter().take(8).collect(),
        evidence,
        confidence,
        caveat,
        took_ms: started.elapsed().as_millis() as i64,
    })
}

fn largest_record(conn: &Connection) -> AppResult<Option<(String, i64, String)>> {
    use rusqlite::OptionalExtension;
    Ok(conn
        .query_row(
            "SELECT name, size, path FROM files ORDER BY size DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_natural_language() {
        let parsed = parse_query(
            "show me all PDF invoices from last year larger than 5 MB",
            SearchFilters::default(),
        );
        assert!(parsed.filters.doc_types.contains(&"invoice".to_string()));
        assert!(parsed.filters.exts.contains(&"pdf".to_string()));
        assert!(parsed.filters.min_size.unwrap() >= 5 * 1024 * 1024);
        assert!(parsed.filters.modified_after.is_some());
        assert!(parsed.interpretation.len() >= 3);
    }

    #[test]
    fn separates_free_text_from_document_type_filters() {
        let parsed = parse_query("electricity bills", SearchFilters::default());
        assert_eq!(parsed.tokens, vec!["electricity"]);
        assert!(parsed.filters.doc_types.contains(&"invoice".to_string()));
    }

    #[test]
    fn size_parsing() {
        let parsed = parse_query("videos over 2 gb", SearchFilters::default());
        assert_eq!(parsed.filters.min_size, Some(2 * 1024 * 1024 * 1024));
    }

    #[test]
    fn parses_phrases_and_field_hints() {
        let parsed = parse_query(
            "content:invoice \"electricity bill\"",
            SearchFilters::default(),
        );
        assert_eq!(parsed.phrases, vec!["electricity bill".to_string()]);
        assert_eq!(
            parsed.fields,
            vec![("content".to_string(), "invoice".to_string())]
        );
        let expression = build_fts_expression(&parsed).unwrap();
        assert_eq!(
            expression,
            "\"electricity bill\" AND content_text:\"invoice\"*"
        );
    }

    fn seed_search_db() -> Connection {
        let conn = crate::db::open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO roots(id,path,label,kind,created_at) VALUES (1,'C:\\Docs','Docs','folder',0)",
            [],
        )
        .unwrap();
        for (id, name, text) in [
            (1, "one.txt", "electricity bill for april"),
            (2, "two.txt", "electricity statement for may"),
            (3, "three.txt", "water bill for june"),
        ] {
            conn.execute(
                "INSERT INTO files(id,root_id,path,parent_path,name,ext,size,indexed_at,last_seen_at,content_text,category)
                 VALUES (?1,1,?2,'C:\\Docs',?3,'txt',10,0,0,?4,'documents')",
                rusqlite::params![id, format!("C:\\Docs\\{name}"), name, text],
            )
            .unwrap();
        }
        conn
    }

    #[test]
    fn search_returns_real_total_and_pages() {
        let conn = seed_search_db();
        let first = search(
            &conn,
            "electricity",
            SearchFilters {
                limit: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(first.total, 2);
        assert_eq!(first.hits.len(), 1);
        assert_eq!(first.mode, "full-text");

        let second = search(
            &conn,
            "electricity",
            SearchFilters {
                limit: Some(1),
                offset: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(second.total, 2);
        assert_ne!(second.hits[0].file.id, first.hits[0].file.id);
    }

    #[test]
    fn explicit_sort_applies_to_full_text_results() {
        let conn = seed_search_db();
        let sorted = search(
            &conn,
            "electricity",
            SearchFilters {
                sort: Some("name_desc".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(sorted.hits.len(), 2);
        assert_eq!(sorted.hits[0].file.name, "two.txt");
        assert_eq!(sorted.hits[1].file.name, "one.txt");
    }

    #[test]
    fn phrase_and_field_searches_are_safe() {
        let conn = seed_search_db();
        let phrase = search(&conn, "\"electricity bill\"", SearchFilters::default()).unwrap();
        assert_eq!(phrase.total, 1);
        assert_eq!(phrase.hits[0].file.id, 1);

        let field = search(&conn, "content:water", SearchFilters::default()).unwrap();
        assert_eq!(field.total, 1);
        assert_eq!(field.hits[0].file.id, 3);

        // FTS5 operators are never passed through: both terms must match.
        let operators = search(&conn, "electricity OR \"bill\"", SearchFilters::default()).unwrap();
        assert_eq!(operators.total, 1);

        let long = "word ".repeat(120);
        assert!(search(&conn, &long, SearchFilters::default()).is_err());
    }

    #[test]
    fn fts_triggers_keep_index_in_sync() {
        let conn = seed_search_db();
        conn.execute(
            "UPDATE files SET content_text='solar panel guide' WHERE id=1",
            [],
        )
        .unwrap();
        assert_eq!(
            search(&conn, "electricity", SearchFilters::default())
                .unwrap()
                .total,
            1
        );
        assert_eq!(
            search(&conn, "solar", SearchFilters::default())
                .unwrap()
                .total,
            1
        );
        conn.execute("DELETE FROM files WHERE id=1", []).unwrap();
        assert_eq!(
            search(&conn, "solar", SearchFilters::default())
                .unwrap()
                .total,
            0
        );
    }

    #[test]
    fn snippets_are_utf8_safe_and_highlighted() {
        let conn = seed_search_db();
        let hit = search(&conn, "electricity", SearchFilters::default())
            .unwrap()
            .hits
            .remove(0);
        assert!(hit
            .snippet_parts
            .iter()
            .any(|part| { part.highlight && part.text.to_lowercase().contains("electricity") }));

        let unicode = format!("{} bill 😀 {}", "é".repeat(120), "notes".repeat(40));
        let file = FileEntry {
            id: 99,
            root_id: 1,
            path: "C:\\Docs\\unicode.txt".into(),
            parent_path: "C:\\Docs".into(),
            name: "unicode.txt".into(),
            ext: "txt".into(),
            size: 10,
            mtime: 0,
            atime: 0,
            ctime: 0,
            category: "documents".into(),
            mime: "text/plain".into(),
            tags: Vec::new(),
            doc_type: None,
            confidence: 1.0,
            sensitive: false,
            analyzed: true,
            hash: None,
            snippet: None,
        };
        let snippet = build_snippet(&["bill".to_string()], Some(&unicode), &file);
        assert!(snippet.contains("bill"));
    }
}
