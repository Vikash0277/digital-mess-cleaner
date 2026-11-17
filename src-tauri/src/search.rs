//! Metadata, full-text and natural-language search plus "Ask My Files" answers.

use crate::error::AppResult;
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
    let mut working = input.to_string();
    let mut interpretation: Vec<String> = Vec::new();
    let mut sort: Option<String> = None;

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

pub fn search(conn: &Connection, query: &str, filters: SearchFilters) -> AppResult<SearchResponse> {
    let started = Instant::now();
    let parsed = parse_query(query, filters);
    let limit = parsed.filters.limit.unwrap_or(200).clamp(1, 1000);
    let mut hits: Vec<SearchHit> = Vec::new();
    let mut mode = "metadata".to_string();

    if !parsed.tokens.is_empty() {
        let fts = parsed
            .tokens
            .iter()
            .map(|t| format!("\"{}\"*", t.replace('"', "")))
            .collect::<Vec<_>>()
            .join(" AND ");
        let columns = repo::FILE_COLUMNS
            .split(',')
            .map(|c| format!("f.{}", c.trim()))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT {columns}, f.content_text, bm25(files_fts) AS rank
             FROM files f JOIN files_fts ON files_fts.rowid = f.id
             WHERE files_fts MATCH ?"
        );
        let mut values: Vec<Value> = vec![Value::Text(fts.clone())];
        let mut clauses: Vec<String> = Vec::new();
        push_filter_sql(&parsed.filters, &mut clauses, &mut values);
        let mut sql = sql;
        if !clauses.is_empty() {
            sql.push_str(" AND ");
            sql.push_str(&clauses.join(" AND "));
        }
        sql.push_str(" ORDER BY rank LIMIT ?");
        values.push(Value::Integer(limit));
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(values.iter()), |row| {
            let file = repo::row_to_file(row)?;
            let content: Option<String> = row.get(18)?;
            let rank: f64 = row.get(19).unwrap_or(0.0);
            Ok((file, content, rank))
        })?;
        for (file, content, rank) in rows.flatten() {
            let snippet = build_snippet(&parsed.tokens, content.as_deref(), &file);
            let matched_on = matched_field(&parsed.tokens, content.as_deref(), &file);
            hits.push(SearchHit {
                file,
                snippet,
                matched_on,
                score: 1.0 / (1.0 + rank.abs()),
            });
        }
        if !hits.is_empty() {
            mode = "full-text".into();
        }
    }

    // Fallback: metadata-only matching so new or unanalyzed files still appear.
    if hits.is_empty() {
        let columns = repo::FILE_COLUMNS
            .split(',')
            .map(|c| format!("f.{}", c.trim()))
            .collect::<Vec<_>>()
            .join(", ");
        let mut clauses: Vec<String> = Vec::new();
        let mut values: Vec<Value> = Vec::new();
        for token in &parsed.tokens {
            clauses.push("(lower(f.name) LIKE ? OR lower(f.path) LIKE ? OR lower(COALESCE(f.content_text,'')) LIKE ?)".into());
            let like = format!("%{token}%");
            values.push(Value::Text(like.clone()));
            values.push(Value::Text(like.clone()));
            values.push(Value::Text(like));
        }
        push_filter_sql(&parsed.filters, &mut clauses, &mut values);
        let where_sql = if clauses.is_empty() {
            "1=1".to_string()
        } else {
            clauses.join(" AND ")
        };
        let sql = format!(
            "SELECT {columns}, f.content_text FROM files f WHERE {where_sql} ORDER BY {} LIMIT ?",
            order_sql(parsed.sort.as_deref())
        );
        values.push(Value::Integer(limit));
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(values.iter()), |row| {
            let file = repo::row_to_file(row)?;
            let content: Option<String> = row.get(18)?;
            Ok((file, content))
        })?;
        for (file, content) in rows.flatten() {
            let snippet = build_snippet(&parsed.tokens, content.as_deref(), &file);
            let matched_on = matched_field(&parsed.tokens, content.as_deref(), &file);
            hits.push(SearchHit {
                file,
                snippet,
                matched_on,
                score: 0.5,
            });
        }
        if !hits.is_empty() {
            mode = "metadata".into();
        }
    }

    let total = hits.len() as i64;
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
    let lower = source.to_lowercase();
    let mut best: Option<(usize, String)> = None;
    for token in tokens {
        if let Some(pos) = lower.find(token) {
            let start = pos.saturating_sub(90);
            let end = (pos + token.len() + 150).min(source.len());
            let mut snippet = String::new();
            if start > 0 {
                snippet.push('…');
            }
            let slice = &source[start..end];
            snippet.push_str(slice.trim());
            if end < source.len() {
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
}
