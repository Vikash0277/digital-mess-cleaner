//! Duplicate detection.
//! Exact groups require identical size + identical full BLAKE3 hash.
//! Similar images use a perceptual average hash and are never called exact.

use crate::error::AppResult;
use crate::fsutil;
use crate::models::*;
use crate::repo;
use rusqlite::Connection;
use std::collections::HashMap;

fn file_columns() -> String {
    repo::FILE_COLUMNS
        .split(',')
        .map(|c| format!("f.{}", c.trim()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn candidate_files(
    conn: &Connection,
    categories: &[&str],
    limit: i64,
) -> AppResult<Vec<FileEntry>> {
    let category_sql = if categories.is_empty() {
        String::new()
    } else {
        format!(
            "AND f.category IN ({})",
            categories
                .iter()
                .map(|c| format!("'{}'", c))
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    let sql = format!(
        "SELECT {} FROM files f
         WHERE f.size > 0
           AND f.size IN (SELECT size FROM files WHERE size > 0 GROUP BY size HAVING count(*) > 1)
           {category_sql}
         ORDER BY f.size DESC LIMIT ?1",
        file_columns()
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([limit], repo::row_to_file)?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn exact_groups(
    conn: &mut Connection,
    categories: &[&str],
    compute_missing: bool,
) -> AppResult<DuplicateReport> {
    let mut files = candidate_files(conn, categories, 8000)?;
    let candidates = files.len() as i64;
    let mut hashed = 0i64;
    let mut notes: Vec<String> = Vec::new();

    if compute_missing {
        for file in files.iter_mut() {
            if file.hash.is_some() {
                continue;
            }
            let path = std::path::Path::new(&file.path);
            if !path.exists() {
                let _ = conn.execute(
                    "UPDATE files SET content_status='missing' WHERE id=?1",
                    [file.id],
                );
                continue;
            }
            match fsutil::hash_file(path) {
                Ok(hash) => {
                    conn.execute(
                        "UPDATE files SET hash = ?1 WHERE id = ?2",
                        rusqlite::params![hash, file.id],
                    )?;
                    file.hash = Some(hash);
                    hashed += 1;
                }
                Err(_) => {
                    notes.push(format!(
                        "Could not read {}. It may be locked or permission-protected.",
                        file.name
                    ));
                }
            }
        }
    }

    let mut groups: HashMap<String, Vec<FileEntry>> = HashMap::new();
    for file in files {
        if let Some(hash) = &file.hash {
            groups.entry(hash.clone()).or_default().push(file);
        }
    }

    let mut out: Vec<DuplicateGroup> = groups
        .into_iter()
        .filter(|(_, members)| members.len() > 1)
        .map(|(hash, mut members)| {
            members.sort_by(|a, b| a.path.len().cmp(&b.path.len()).then(a.path.cmp(&b.path)));
            let size = members.first().map(|f| f.size).unwrap_or(0);
            let wasted = size * (members.len() as i64 - 1);
            DuplicateGroup {
                id: format!("exact-{}", &hash[..hash.len().min(12)]),
                kind: "exact".into(),
                signature: format!("BLAKE3 {}", &hash[..hash.len().min(16)]),
                wasted_bytes: wasted,
                similarity: Some(1.0),
                files: members,
            }
        })
        .collect();

    out.sort_by(|a, b| b.wasted_bytes.cmp(&a.wasted_bytes));
    let total_wasted = out.iter().map(|g| g.wasted_bytes).sum();
    if candidates > 8000 {
        notes.push("More than 8,000 duplicate candidates were found; only the 8,000 largest were compared this run.".into());
    }
    if !compute_missing {
        notes.push("Only files with an existing full hash were compared. Run the scan again to hash new candidates.".into());
    }
    notes.push(
        "Exact means every byte matches. Grouping happens only after a full cryptographic hash."
            .into(),
    );

    Ok(DuplicateReport {
        mode: "exact".into(),
        groups: out,
        total_wasted,
        candidate_files: candidates,
        hashed_files: hashed,
        notes,
    })
}

pub fn similar_images(
    conn: &mut Connection,
    threshold: u32,
    compute_missing: bool,
) -> AppResult<DuplicateReport> {
    let sql = format!(
        "SELECT {} FROM files f WHERE f.category='images' AND f.phash IS NULL ORDER BY f.size DESC LIMIT 600",
        file_columns()
    );
    let mut pending: Vec<FileEntry> = {
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], repo::row_to_file)?;
        rows.filter_map(Result::ok).collect()
    };
    let mut notes = Vec::new();
    let mut hashed = 0;
    if compute_missing {
        for file in pending.iter_mut() {
            let path = std::path::Path::new(&file.path);
            if !path.exists() {
                continue;
            }
            match fsutil::perceptual_hash(path) {
                Some((hash, _w, _h)) => {
                    conn.execute(
                        "UPDATE files SET phash = ?1 WHERE id = ?2",
                        rusqlite::params![hash, file.id],
                    )?;
                    hashed += 1;
                }
                None => {
                    // Mark unsupported so we do not retry forever.
                    conn.execute("UPDATE files SET phash = '-' WHERE id = ?1", [file.id])?;
                }
            }
        }
    }

    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM files f WHERE f.category='images' AND phash IS NOT NULL AND phash != '-' ORDER BY f.size DESC LIMIT 4000",
        file_columns()
    ))?;
    let files: Vec<FileEntry> = stmt
        .query_map([], repo::row_to_file)?
        .filter_map(Result::ok)
        .collect();
    let phashes: HashMap<i64, String> = {
        let mut stmt =
            conn.prepare("SELECT id, phash FROM files WHERE phash IS NOT NULL AND phash != '-'")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        rows.filter_map(Result::ok).collect()
    };

    let mut assigned: HashMap<i64, usize> = HashMap::new();
    let mut clusters: Vec<Vec<FileEntry>> = Vec::new();
    for file in files {
        let Some(hash) = phashes.get(&file.id) else {
            continue;
        };
        let mut best: Option<(usize, u32)> = None;
        for (idx, members) in clusters.iter().enumerate() {
            if assigned.contains_key(&file.id) {
                break;
            }
            let rep = &members[0];
            let Some(rep_hash) = phashes.get(&rep.id) else {
                continue;
            };
            let Some(dist) = fsutil::hamming_hex(hash, rep_hash) else {
                continue;
            };
            if dist <= threshold {
                match best {
                    Some((_, d)) if d <= dist => {}
                    _ => best = Some((idx, dist)),
                }
            }
        }
        match best {
            Some((idx, _)) => {
                assigned.insert(file.id, idx);
                clusters[idx].push(file);
            }
            None => {
                assigned.insert(file.id, clusters.len());
                clusters.push(vec![file]);
            }
        }
    }

    let mut out: Vec<DuplicateGroup> = Vec::new();
    for members in clusters.into_iter().filter(|m| m.len() > 1) {
        let sizes: Vec<i64> = members.iter().map(|f| f.size).collect();
        let max = sizes.iter().copied().max().unwrap_or(0);
        let wasted = sizes.iter().sum::<i64>() - max;
        let rep = &members[0];
        let rep_hash = phashes.get(&rep.id).cloned().unwrap_or_default();
        let mut min_dist = 64u32;
        for m in members.iter().skip(1) {
            if let (Some(a), Some(b)) = (phashes.get(&m.id), Some(&rep_hash)) {
                if let Some(d) = fsutil::hamming_hex(a, b) {
                    min_dist = min_dist.min(d);
                }
            }
        }
        let similarity = 1.0 - min_dist as f64 / 64.0;
        out.push(DuplicateGroup {
            id: format!("similar-{}", &rep_hash[..rep_hash.len().min(12)]),
            kind: "similar".into(),
            signature: format!(
                "aHash {} · ≤ {threshold}/64 bits apart",
                &rep_hash[..rep_hash.len().min(16)]
            ),
            wasted_bytes: wasted.max(0),
            similarity: Some(similarity),
            files: members,
        });
    }
    out.sort_by(|a, b| b.wasted_bytes.cmp(&a.wasted_bytes));
    let total_wasted = out.iter().map(|g| g.wasted_bytes).sum();
    notes.push("Similar images are grouped by perceptual hash. They are not byte-identical unless you also confirm them under Exact duplicates.".into());
    if hashed > 0 {
        notes.push(format!(
            "Computed perceptual hashes for {hashed} images this run."
        ));
    }
    if out.is_empty() {
        notes.push("No visually similar images passed the threshold. Try a looser threshold if you expected matches.".into());
    }

    Ok(DuplicateReport {
        mode: "similar".into(),
        groups: out,
        total_wasted,
        candidate_files: phashes.len() as i64,
        hashed_files: hashed,
        notes,
    })
}

pub fn analyze(conn: &mut Connection, mode: &str, threshold: u32) -> AppResult<DuplicateReport> {
    match mode {
        "similar" | "images" => similar_images(conn, threshold, true),
        "video" => {
            let mut report = exact_groups(conn, &["video"], true)?;
            report.mode = "video".into();
            if report.groups.is_empty() {
                report.notes.push("No byte-identical videos found. Similar-encoding detection needs FFmpeg and is not enabled in this build.".into());
            }
            Ok(report)
        }
        "audio" => {
            let mut report = exact_groups(conn, &["audio"], true)?;
            report.mode = "audio".into();
            Ok(report)
        }
        "documents" => {
            let mut report = exact_groups(conn, &["documents"], true)?;
            report.mode = "documents".into();
            report.notes.push("These are byte-identical documents only. Related versions are under Document Intelligence → Versions.".into());
            Ok(report)
        }
        "screenshots" => {
            let mut report = exact_groups(conn, &["images"], false)?;
            report.mode = "screenshots".into();
            let mut filtered: Vec<DuplicateGroup> = report
                .groups
                .into_iter()
                .filter(|g| g.files.iter().any(|f| is_screenshot(&f.name)))
                .collect();
            for g in &mut filtered {
                g.files.retain(|f| is_screenshot(&f.name));
            }
            filtered.retain(|g| g.files.len() > 1);
            report.groups = filtered;
            report.total_wasted = report.groups.iter().map(|g| g.wasted_bytes).sum();
            report
                .notes
                .push("Only files whose names look like screenshots are shown.".into());
            Ok(report)
        }
        _ => exact_groups(conn, &[], true),
    }
}

pub fn is_screenshot(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("screenshot")
        || n.contains("screen shot")
        || n.starts_with("screen_shot")
        || n.contains("snip")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn exact_groups_require_same_content() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        let c = dir.path().join("c.txt");
        std::fs::write(&a, b"same content").unwrap();
        std::fs::write(&b, b"same content").unwrap();
        std::fs::write(&c, b"different!!").unwrap();
        let mut conn = db::open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO roots(id,path,label,kind,created_at) VALUES (1,'x','x','folder',0)",
            [],
        )
        .unwrap();
        for (i, p) in [&a, &b, &c].iter().enumerate() {
            conn.execute(
                "INSERT INTO files(id,root_id,path,parent_path,name,size,category,indexed_at,last_seen_at)
                 VALUES (?1,1,?2,?3,?4,?5,'documents',0,0)",
                rusqlite::params![
                    i as i64 + 1,
                    p.to_string_lossy().to_string(),
                    dir.path().to_string_lossy().to_string(),
                    p.file_name().unwrap().to_string_lossy().to_string(),
                    std::fs::metadata(p).unwrap().len() as i64
                ],
            )
            .unwrap();
        }
        let report = exact_groups(&mut conn, &[], true).unwrap();
        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.groups[0].files.len(), 2);
        assert!(report.total_wasted > 0);
    }

    #[test]
    fn screenshots_detected() {
        assert!(is_screenshot("Screenshot 2026-11-12 101010.png"));
        assert!(!is_screenshot("holiday.png"));
    }
}
