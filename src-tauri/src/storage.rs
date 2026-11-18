//! Storage analysis: dashboards, category breakdowns, largest items, forgotten
//! files, snapshots, growth prediction and cleanup suggestions.

use crate::db;
use crate::error::AppResult;
use crate::fsutil;
use crate::models::*;
use crate::repo;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sysinfo::Disks;

/// Every aggregate that needs a pass over the whole `files` table, collected in
/// one query. On a multi-million-row index a single pass costs seconds, so the
/// dashboard must never run these separately: the old code ran a dozen of them
/// one after another and took minutes to open on a 3.5M-file drive.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileTotals {
    pub total_bytes: i64,
    pub file_count: i64,
    pub forgotten_count: i64,
    pub forgotten_bytes: i64,
    pub unused_year_bytes: i64,
    pub unanalyzed_count: i64,
    pub missing_count: i64,
    pub sensitive_count: i64,
    /// Counted on `dirs`, not `files`, but it is cached with everything else so
    /// a dashboard load never scans the 700k-row directory table either.
    pub empty_folder_count: i64,
    pub by_category: Vec<CategoryStat>,
    pub by_root: Vec<RootTotals>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootTotals {
    pub root_id: i64,
    pub file_count: i64,
    pub total_bytes: i64,
}

#[derive(Serialize, Deserialize)]
struct CachedTotals {
    at: i64,
    totals: FileTotals,
}

const TOTALS_KEY: &str = "cache.file_totals.v1";
/// Backstop only. Every mutation of the index clears the cache, so the numbers
/// are recomputed after a scan, a cleanup, an analysis or a rule run. The limit
/// exists only so that a missed invalidation cannot be wrong for ever; it is
/// deliberately long because expiring it forces a full index scan.
const TOTALS_TTL_SECS: i64 = 30 * 86_400;

/// Drop the cached aggregates. Call this from anything that inserts, updates or
/// deletes rows in `files` or `dirs`.
pub fn invalidate_totals(conn: &Connection) {
    let _ = conn.execute("DELETE FROM settings WHERE key = ?1", [TOTALS_KEY]);
}

/// Cached entry point used by the UI. A cold call scans the index once; after
/// that it is a single row read until something changes the index.
pub fn file_totals(conn: &Connection) -> AppResult<FileTotals> {
    if let Some(raw) = db::get_setting(conn, TOTALS_KEY)? {
        if let Ok(cached) = serde_json::from_str::<CachedTotals>(&raw) {
            if fsutil::now_secs() - cached.at <= TOTALS_TTL_SECS {
                return Ok(cached.totals);
            }
        }
    }
    let totals = compute_file_totals(conn)?;
    let entry = CachedTotals {
        at: fsutil::now_secs(),
        totals: totals.clone(),
    };
    if let Ok(raw) = serde_json::to_string(&entry) {
        let _ = db::set_setting(conn, TOTALS_KEY, &raw);
    }
    Ok(totals)
}

/// The single pass. `NOT INDEXED` is deliberate: letting SQLite drive this from
/// `idx_files_category` makes it fetch every row from the table by rowid, which
/// took 75 seconds on a 3.2 GB index. A plain table scan takes about six.
///
/// `atime <= ? AND mtime <= ?` is the same test as `MAX(atime, mtime) <= ?`
/// but, unlike `MAX`, it can use an index.
fn compute_file_totals(conn: &Connection) -> AppResult<FileTotals> {
    let forgotten_cutoff = fsutil::now_secs() - 180 * 86_400;
    let year_cutoff = fsutil::now_secs() - 365 * 86_400;
    let mut stmt = conn.prepare(
        "SELECT root_id, category,
                COALESCE(sum(size), 0),
                count(*),
                COALESCE(sum(CASE WHEN atime <= ?1 AND mtime <= ?1 THEN 1 ELSE 0 END), 0),
                COALESCE(sum(CASE WHEN atime <= ?1 AND mtime <= ?1 THEN size ELSE 0 END), 0),
                COALESCE(sum(CASE WHEN atime <= ?2 AND mtime <= ?2 THEN size ELSE 0 END), 0),
                COALESCE(sum(CASE WHEN content_status = 'none' THEN 1 ELSE 0 END), 0),
                COALESCE(sum(CASE WHEN content_status = 'missing' THEN 1 ELSE 0 END), 0),
                COALESCE(sum(CASE WHEN sensitive = 1 THEN 1 ELSE 0 END), 0)
         FROM files NOT INDEXED
         GROUP BY root_id, category",
    )?;
    let rows = stmt.query_map(rusqlite::params![forgotten_cutoff, year_cutoff], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, i64>(5)?,
            r.get::<_, i64>(6)?,
            r.get::<_, i64>(7)?,
            r.get::<_, i64>(8)?,
            r.get::<_, i64>(9)?,
        ))
    })?;

    let mut totals = FileTotals::default();
    for row in rows {
        let (
            root_id,
            category,
            bytes,
            count,
            forgotten,
            forgotten_bytes,
            unused_year,
            unanalyzed,
            missing,
            sensitive,
        ) = row?;
        totals.total_bytes += bytes;
        totals.file_count += count;
        totals.forgotten_count += forgotten;
        totals.forgotten_bytes += forgotten_bytes;
        totals.unused_year_bytes += unused_year;
        totals.unanalyzed_count += unanalyzed;
        totals.missing_count += missing;
        totals.sensitive_count += sensitive;

        match totals
            .by_category
            .iter_mut()
            .find(|c| c.category == category)
        {
            Some(existing) => {
                existing.bytes += bytes;
                existing.count += count;
            }
            None => totals.by_category.push(CategoryStat {
                category,
                bytes,
                count,
            }),
        }
        match totals.by_root.iter_mut().find(|r| r.root_id == root_id) {
            Some(existing) => {
                existing.total_bytes += bytes;
                existing.file_count += count;
            }
            None => totals.by_root.push(RootTotals {
                root_id,
                file_count: count,
                total_bytes: bytes,
            }),
        }
    }
    totals.by_category.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    // `dirs` is much smaller than `files`, but a cold read of it still costs
    // seconds on a 3 GB index, so count the empty folders in the same cached
    // payload rather than on every dashboard visit.
    totals.empty_folder_count = conn.query_row(
        "SELECT count(*) FROM dirs WHERE file_count=0 AND depth>0",
        [],
        |r| r.get(0),
    )?;
    Ok(totals)
}

/// Roots with their file counts and sizes taken from the cached totals, so this
/// costs nothing on the UI thread even with millions of indexed files.
pub fn roots_with_totals(conn: &Connection) -> AppResult<Vec<RootInfo>> {
    let totals = file_totals(conn)?;
    let mut roots = repo::list_roots_basic(conn)?;
    for root in &mut roots {
        if let Some(entry) = totals.by_root.iter().find(|r| r.root_id == root.id) {
            root.file_count = entry.file_count;
            root.total_bytes = entry.total_bytes;
        }
    }
    Ok(roots)
}

pub fn category_breakdown(conn: &Connection) -> AppResult<Vec<CategoryStat>> {
    Ok(file_totals(conn)?.by_category)
}

/// Break one category down into extensions, folders and its largest files.
/// Called only when a user opens a category on the dashboard, so it can afford
/// the full-table groupings that the dashboard itself deliberately avoids.
pub fn category_details(
    conn: &Connection,
    category: &str,
    limit: i64,
) -> AppResult<CategoryDetails> {
    let limit = limit.clamp(1, 100);
    let (total_bytes, file_count): (i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(size),0), COUNT(*) FROM files WHERE category = ?1",
        [category],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    let mut by_extension = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT LOWER(ext) AS ext, COUNT(*) AS n, COALESCE(SUM(size),0) AS bytes
             FROM files WHERE category = ?1
             GROUP BY LOWER(ext) ORDER BY bytes DESC, n DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![category, limit], |r| {
            Ok(ExtensionStat {
                ext: r.get(0)?,
                count: r.get(1)?,
                bytes: r.get(2)?,
            })
        })?;
        for row in rows.flatten() {
            by_extension.push(row);
        }
    }

    let mut by_folder = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT parent_path, COUNT(*) AS n, COALESCE(SUM(size),0) AS bytes
             FROM files WHERE category = ?1
             GROUP BY parent_path ORDER BY bytes DESC, n DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![category, limit], |r| {
            Ok(FolderStat {
                path: r.get(0)?,
                count: r.get(1)?,
                bytes: r.get(2)?,
            })
        })?;
        for row in rows.flatten() {
            by_folder.push(row);
        }
    }

    let columns = repo::FILE_COLUMNS
        .split(',')
        .map(|c| format!("f.{}", c.trim()))
        .collect::<Vec<_>>()
        .join(", ");
    let largest_files = {
        let mut stmt = conn.prepare(&format!(
            "SELECT {columns} FROM files f WHERE f.category = ?1
             ORDER BY f.size DESC LIMIT ?2"
        ))?;
        let rows = stmt.query_map(rusqlite::params![category, limit], repo::row_to_file)?;
        rows.flatten().collect()
    };

    Ok(CategoryDetails {
        category: category.to_string(),
        total_bytes,
        file_count,
        by_extension,
        by_folder,
        largest_files,
    })
}

pub fn largest_files(conn: &Connection, limit: i64) -> AppResult<Vec<FileEntry>> {
    let columns = repo::FILE_COLUMNS
        .split(',')
        .map(|c| format!("f.{}", c.trim()))
        .collect::<Vec<_>>()
        .join(", ");
    let mut stmt = conn.prepare(&format!(
        "SELECT {columns} FROM files f ORDER BY f.size DESC LIMIT ?1"
    ))?;
    let rows = stmt.query_map([limit], repo::row_to_file)?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn largest_folders(conn: &Connection, limit: i64) -> AppResult<Vec<FolderStat>> {
    let mut stmt = conn.prepare(
        "SELECT path, total_bytes, file_count FROM dirs
         WHERE total_bytes > 0 ORDER BY total_bytes DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |r| {
        Ok(FolderStat {
            path: r.get(0)?,
            bytes: r.get(1)?,
            count: r.get(2)?,
        })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn child_folders(conn: &Connection, parent: &str) -> AppResult<Vec<FolderStat>> {
    let mut stmt = conn.prepare(
        "SELECT path, total_bytes, file_count FROM dirs
         WHERE parent_path = ?1 AND path != ?1 ORDER BY total_bytes DESC LIMIT 200",
    )?;
    let rows = stmt.query_map([parent], |r| {
        Ok(FolderStat {
            path: r.get(0)?,
            bytes: r.get(1)?,
            count: r.get(2)?,
        })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn forgotten_files(
    conn: &Connection,
    days: i64,
    min_size: i64,
    limit: i64,
) -> AppResult<Vec<FileEntry>> {
    let cutoff = fsutil::now_secs() - days.clamp(1, 36500) * 86_400;
    let columns = repo::FILE_COLUMNS
        .split(',')
        .map(|c| format!("f.{}", c.trim()))
        .collect::<Vec<_>>()
        .join(", ");
    let mut stmt = conn.prepare(&format!(
        "SELECT {columns} FROM files f
         WHERE f.atime <= ?1 AND f.mtime <= ?1 AND f.size >= ?2
         ORDER BY f.size DESC LIMIT ?3"
    ))?;
    let rows = stmt.query_map(
        rusqlite::params![cutoff, min_size, limit],
        repo::row_to_file,
    )?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn empty_folders(conn: &Connection) -> AppResult<Vec<FolderStat>> {
    let mut stmt = conn.prepare(
        "SELECT path, 0, 0 FROM dirs WHERE file_count = 0
         AND depth > 0 ORDER BY length(path) DESC LIMIT 500",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(FolderStat {
            path: r.get(0)?,
            bytes: 0,
            count: 0,
        })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn duplicate_summary(conn: &Connection) -> AppResult<(i64, i64)> {
    // Only confirmed hashes; never inflate this with size-only guesses.
    let row = conn.query_row(
        "SELECT COALESCE(SUM(size * (cnt - 1)), 0), COUNT(*) FROM
           (SELECT size, count(*) AS cnt FROM files WHERE hash IS NOT NULL GROUP BY hash HAVING cnt > 1)",
        [],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
    )?;
    Ok(row)
}

pub fn potential_duplicate_summary(conn: &Connection) -> AppResult<(i64, i64)> {
    let row = conn.query_row(
        "SELECT COALESCE(SUM(size * (cnt - 1)), 0), COUNT(*) FROM
           (SELECT size, count(*) AS cnt FROM files WHERE size > 0 GROUP BY size HAVING cnt > 1)",
        [],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
    )?;
    Ok(row)
}

pub fn disks() -> Vec<DiskInfo> {
    let disks = Disks::new_with_refreshed_list();
    let mut out: Vec<DiskInfo> = disks
        .iter()
        .filter(|d| d.total_space() > 0)
        .map(|d| DiskInfo {
            name: d.name().to_string_lossy().to_string(),
            mount: d.mount_point().to_string_lossy().to_string(),
            total: d.total_space() as i64,
            available: d.available_space() as i64,
            kind: format!("{:?}", d.kind()),
            removable: d.is_removable(),
        })
        .collect();
    out.sort_by(|a, b| a.mount.cmp(&b.mount));
    out.dedup_by(|a, b| a.mount == b.mount);
    out
}

pub fn available_space_for(roots: &[RootInfo], disks: &[DiskInfo]) -> i64 {
    let mut best = 0i64;
    for root in roots {
        let root_path = std::path::Path::new(&root.path);
        let mut chosen: Option<&DiskInfo> = None;
        for disk in disks {
            if fsutil::is_within(root_path, std::path::Path::new(&disk.mount)) {
                match chosen {
                    Some(existing) if existing.mount.len() >= disk.mount.len() => {}
                    _ => chosen = Some(disk),
                }
            }
        }
        if let Some(d) = chosen {
            best = best.max(d.available);
        }
    }
    if best == 0 {
        disks.iter().map(|d| d.available).max().unwrap_or(0)
    } else {
        best
    }
}

pub fn take_snapshot(conn: &Connection) -> AppResult<Snapshot> {
    let totals = file_totals(conn)?;
    let taken_at = fsutil::now_secs();
    conn.execute(
        "INSERT INTO storage_snapshots(taken_at, total_bytes, file_count, by_category) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            taken_at,
            totals.total_bytes,
            totals.file_count,
            serde_json::to_string(&totals.by_category).unwrap_or_else(|_| "[]".into())
        ],
    )?;
    Ok(Snapshot {
        taken_at,
        total_bytes: totals.total_bytes,
        file_count: totals.file_count,
        by_category: totals.by_category,
    })
}

pub fn record_snapshot_if_due(conn: &Connection) -> AppResult<()> {
    let last: Option<i64> = conn
        .query_row("SELECT MAX(taken_at) FROM storage_snapshots", [], |r| {
            r.get(0)
        })
        .ok()
        .flatten();
    if last
        .map(|t| fsutil::now_secs() - t > 20 * 3600)
        .unwrap_or(true)
    {
        let _ = take_snapshot(conn)?;
    }
    Ok(())
}

pub fn trends(conn: &Connection, days: i64) -> AppResult<Vec<Snapshot>> {
    let cutoff = fsutil::now_secs() - days.max(1) * 86_400;
    let mut stmt = conn.prepare(
        "SELECT taken_at, total_bytes, file_count, by_category FROM storage_snapshots
         WHERE taken_at >= ?1 ORDER BY taken_at ASC",
    )?;
    let rows = stmt.query_map([cutoff], |r| {
        let cats_raw: String = r.get(3)?;
        Ok(Snapshot {
            taken_at: r.get(0)?,
            total_bytes: r.get(1)?,
            file_count: r.get(2)?,
            by_category: serde_json::from_str(&cats_raw).unwrap_or_default(),
        })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn prediction(conn: &Connection, available: i64) -> AppResult<Option<Prediction>> {
    let snapshots = trends(conn, 180)?;
    if snapshots.len() < 2 {
        return Ok(None);
    }
    let first = snapshots.first().unwrap();
    let last = snapshots.last().unwrap();
    let span_days = ((last.taken_at - first.taken_at) as f64 / 86_400.0).max(1.0);
    let growth = (last.total_bytes - first.total_bytes) as f64 / span_days;
    if growth <= 0.0 {
        return Ok(Some(Prediction {
            available_bytes: available,
            growth_per_day: growth.max(0.0),
            days_until_full: None,
            based_on_days: span_days as i64,
            message: "Your indexed storage is not growing right now.".into(),
            confidence: 0.5,
        }));
    }
    let days = if available > 0 {
        (available as f64 / growth).round() as i64
    } else {
        0
    };
    let confidence = (0.35 + span_days / 90.0).min(0.85);
    Ok(Some(Prediction {
        available_bytes: available,
        growth_per_day: growth,
        days_until_full: Some(days),
        based_on_days: span_days as i64,
        message: format!(
            "At the current rate ({} per day) this drive has about {} days before it is full.",
            crate::format::bytes(growth as i64),
            days
        ),
        confidence,
    }))
}

pub fn clutter_score(conn: &Connection) -> AppResult<ClutterScore> {
    let totals = file_totals(conn)?;
    clutter_score_from(conn, &totals)
}

fn clutter_score_from(conn: &Connection, totals: &FileTotals) -> AppResult<ClutterScore> {
    let total = totals.total_bytes;
    if total == 0 {
        return Ok(ClutterScore {
            score: 0,
            band: "No data yet".into(),
            factors: vec![Evidence {
                label: "Index".into(),
                value: "Add and scan a folder to calculate a clutter score.".into(),
                weight: 0.0,
            }],
        });
    }
    let category_bytes = |name: &str| {
        totals
            .by_category
            .iter()
            .find(|c| c.category == name)
            .map(|c| c.bytes)
            .unwrap_or(0)
    };
    let unused = totals.unused_year_bytes;
    let installers = category_bytes("installers");
    let archives = category_bytes("archives");
    let (dup_bytes, _) = duplicate_summary(conn)?;
    let empty = totals.empty_folder_count;
    let unanalyzed = totals.unanalyzed_count;
    let count = totals.file_count;

    let pct = |v: i64| (v as f64 / total as f64 * 100.0).clamp(0.0, 100.0);
    let factors = vec![
        Evidence {
            label: "Not opened in a year".into(),
            value: format!("{} ({:.0}%)", crate::format::bytes(unused), pct(unused)),
            weight: pct(unused) * 0.25 / 100.0,
        },
        Evidence {
            label: "Confirmed duplicates".into(),
            value: crate::format::bytes(dup_bytes),
            weight: pct(dup_bytes) * 0.3 / 100.0,
        },
        Evidence {
            label: "Old installers".into(),
            value: crate::format::bytes(installers),
            weight: pct(installers) * 0.15 / 100.0,
        },
        Evidence {
            label: "Archives".into(),
            value: crate::format::bytes(archives),
            weight: pct(archives) * 0.1 / 100.0,
        },
        Evidence {
            label: "Empty folders".into(),
            value: empty.to_string(),
            weight: (empty as f64 / 50.0).min(1.0) * 0.05,
        },
        Evidence {
            label: "Not yet analysed".into(),
            value: format!("{unanalyzed} of {count} files"),
            weight: (unanalyzed as f64 / count.max(1) as f64) * 0.1,
        },
    ];
    let score = (factors.iter().map(|f| f.weight).sum::<f64>() * 100.0).round() as i64;
    let score = score.clamp(0, 100);
    let band = match score {
        0..=24 => "Clear",
        25..=49 => "Manageable",
        50..=74 => "Cluttered",
        _ => "Heavy clutter",
    };
    Ok(ClutterScore {
        score,
        band: band.into(),
        factors,
    })
}

pub fn dashboard(conn: &Connection) -> AppResult<Dashboard> {
    // One pass over the files table, cached until the index changes. Every
    // number below that used to come from its own full scan now comes from it.
    let totals = file_totals(conn)?;
    let roots = roots_with_totals(conn)?;
    let by_category = totals.by_category.clone();
    let total_bytes = totals.total_bytes;
    let file_count = totals.file_count;
    let largest = largest_folders(conn, 8)?;
    let (duplicate_bytes, duplicate_groups) = duplicate_summary(conn)?;
    let quarantine_count: i64 = conn.query_row(
        "SELECT count(*) FROM quarantine WHERE state='held'",
        [],
        |r| r.get(0),
    )?;
    let quarantine_bytes: i64 = conn.query_row(
        "SELECT COALESCE(sum(size),0) FROM quarantine WHERE state='held'",
        [],
        |r| r.get(0),
    )?;
    let forgotten_count = totals.forgotten_count;
    let forgotten_bytes = totals.forgotten_bytes;
    let sensitive_count = totals.sensitive_count;
    let unanalyzed_count = totals.unanalyzed_count;
    let empty_folder_count = totals.empty_folder_count;
    let reminders_due: i64 = conn.query_row(
        "SELECT count(*) FROM reminders WHERE dismissed=0 AND due_date <= date('now','+60 day')",
        [],
        |r| r.get(0),
    )?;
    let missing = totals.missing_count;
    let disks = disks();
    let available = available_space_for(&roots, &disks);
    let last_scan_at: Option<i64> = conn
        .query_row("SELECT MAX(last_scan_at) FROM roots", [], |r| r.get(0))
        .ok()
        .flatten();
    let clutter = clutter_score_from(conn, &totals)?;
    let recent_activity = repo::recent_operations(conn, 6)?;

    Ok(Dashboard {
        roots,
        total_bytes,
        file_count,
        by_category,
        largest_folders: largest,
        duplicate_bytes,
        duplicate_groups,
        quarantine_count,
        quarantine_bytes,
        forgotten_count,
        forgotten_bytes,
        sensitive_count,
        unanalyzed_count,
        empty_folder_count,
        reminders_due,
        health_issues: missing,
        disks,
        last_scan_at,
        prediction: prediction(conn, available)?,
        recent_activity,
        clutter,
    })
}

pub fn cleanup_suggestions(conn: &Connection) -> AppResult<Vec<CleanupSuggestion>> {
    let mut out = Vec::new();

    let installers = files_where(
        conn,
        "category='installers' AND mtime <= ?1",
        &[fsutil::now_secs() - 180 * 86_400],
        60,
    )?;
    if !installers.is_empty() {
        let bytes = installers.iter().map(|f| f.size).sum();
        out.push(CleanupSuggestion {
            id: "old-installers".into(),
            kind: "installers".into(),
            title: "Old application installers".into(),
            detail: format!(
                "{} installer files have not changed in six months. Keep any you still need to reinstall software.",
                installers.len()
            ),
            bytes,
            confidence: 0.72,
            files: installers,
            evidence: vec![Evidence { label: "Rule".into(), value: "Category = installers and modified more than 180 days ago".into(), weight: 1.0 }],
            safety: "Moved to quarantine first. You can restore them.".into(),
        });
    }

    let archives = files_where(
        conn,
        "category='archives' AND atime <= ?1 AND mtime <= ?1",
        &[fsutil::now_secs() - 365 * 86_400],
        60,
    )?;
    if !archives.is_empty() {
        let bytes = archives.iter().map(|f| f.size).sum();
        out.push(CleanupSuggestion {
            id: "unused-archives".into(),
            kind: "archives".into(),
            title: "Archives not opened in a year".into(),
            detail: format!("{} ZIP or archive files have not been opened for at least a year. They may still hold your only copy of something.", archives.len()),
            bytes,
            confidence: 0.55,
            files: archives,
            evidence: vec![Evidence { label: "Rule".into(), value: "Archives with last access and modification older than 365 days".into(), weight: 1.0 }],
            safety: "Review the contents first. Quarantine is reversible.".into(),
        });
    }

    let large_old = files_where(
        conn,
        "size >= ?1 AND atime <= ?2 AND mtime <= ?2",
        &[1024 * 1024 * 1024, fsutil::now_secs() - 730 * 86_400],
        40,
    )?;
    if !large_old.is_empty() {
        let bytes = large_old.iter().map(|f| f.size).sum();
        out.push(CleanupSuggestion {
            id: "large-inactive".into(),
            kind: "large".into(),
            title: "Large files untouched for two years".into(),
            detail: format!("{} files over 1 GB have not changed in two years. Check each one before removing it.", large_old.len()),
            bytes,
            confidence: 0.5,
            files: large_old,
            evidence: vec![Evidence { label: "Rule".into(), value: "At least 1 GB and last activity older than 730 days".into(), weight: 1.0 }],
            safety: "These are candidates only. Important archives can be old and still essential.".into(),
        });
    }

    let temp = files_where(
        conn,
        "(name LIKE '%.tmp' OR name LIKE '%~$%' OR name LIKE '%.part' OR name LIKE '%.crdownload' OR name LIKE '%.download')",
        &[],
        200,
    )?;
    if !temp.is_empty() {
        let bytes = temp.iter().map(|f| f.size).sum();
        out.push(CleanupSuggestion {
            id: "temporary".into(),
            kind: "temporary".into(),
            title: "Temporary and partial downloads".into(),
            detail: format!(
                "{} temporary files or unfinished downloads were found.",
                temp.len()
            ),
            bytes,
            confidence: 0.65,
            files: temp,
            evidence: vec![Evidence {
                label: "Rule".into(),
                value: "Name ends with .tmp, .part, .crdownload or starts with ~$".into(),
                weight: 1.0,
            }],
            safety: "Quarantine first; restorable if an app still needs them.".into(),
        });
    }

    let empty = empty_folders(conn)?;
    if !empty.is_empty() {
        out.push(CleanupSuggestion {
            id: "empty-folders".into(),
            kind: "folders".into(),
            title: "Empty folders".into(),
            detail: format!("{} folders contain no indexed files. Some apps expect their folders to exist, so review before removing.", empty.len()),
            bytes: 0,
            confidence: 0.6,
            files: Vec::new(),
            evidence: vec![Evidence { label: "Rule".into(), value: "Scanned folder with zero files in its subtree".into(), weight: 1.0 }],
            safety: "Folder removal is permanent. Files are never affected.".into(),
        });
    }

    let (dup_bytes, dup_groups) = duplicate_summary(conn)?;
    if dup_groups > 0 {
        out.push(CleanupSuggestion {
            id: "duplicates".into(),
            kind: "duplicates".into(),
            title: "Confirmed byte-identical duplicates".into(),
            detail: format!("{dup_groups} groups are confirmed by full cryptographic hash. Keeping one copy recovers {}.", crate::format::bytes(dup_bytes)),
            bytes: dup_bytes,
            confidence: 0.99,
            files: Vec::new(),
            evidence: vec![Evidence { label: "Method".into(), value: "BLAKE3 full-content hash; only byte-identical files are grouped".into(), weight: 1.0 }],
            safety: "Always keep at least one copy. Review groups in Duplicates.".into(),
        });
    }

    out.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    Ok(out)
}

fn files_where(
    conn: &Connection,
    condition: &str,
    args: &[i64],
    limit: i64,
) -> AppResult<Vec<FileEntry>> {
    let columns = repo::FILE_COLUMNS
        .split(',')
        .map(|c| format!("f.{}", c.trim()))
        .collect::<Vec<_>>()
        .join(", ");
    let sql =
        format!("SELECT {columns} FROM files f WHERE {condition} ORDER BY f.size DESC LIMIT ?");
    let mut values: Vec<rusqlite::types::Value> = args
        .iter()
        .map(|v| rusqlite::types::Value::Integer(*v))
        .collect();
    values.push(rusqlite::types::Value::Integer(limit));
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(values.iter()), repo::row_to_file)?;
    Ok(rows.filter_map(Result::ok).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn seed(conn: &Connection) {
        conn.execute(
            "INSERT INTO roots(id,path,label,kind,created_at) VALUES (1,'C:/lib','lib','folder',0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO files(id,root_id,path,parent_path,name,size,mtime,atime,category,indexed_at,last_seen_at)
             VALUES (1,1,'C:/lib/a.mp4','C:/lib','a.mp4',1000,0,0,'video',0,0),
                    (2,1,'C:/lib/b.zip','C:/lib','b.zip',500,0,0,'archives',0,0)",
            [],
        )
        .unwrap();
    }

    #[test]
    fn categories_roll_up() {
        let conn = db::open_in_memory().unwrap();
        seed(&conn);
        let cats = category_breakdown(&conn).unwrap();
        assert_eq!(cats.len(), 2);
        assert_eq!(cats[0].bytes, 1000);
    }

    #[test]
    fn category_details_groups_extensions_folders_and_largest_files() {
        let conn = db::open_in_memory().unwrap();
        seed(&conn);
        conn.execute(
            "INSERT INTO files(id,root_id,path,parent_path,name,ext,size,mtime,atime,category,indexed_at,last_seen_at)
             VALUES (3,1,'C:/lib/c.dat','C:/lib','c.dat','dat',750,0,0,'other',0,0),
                    (4,1,'C:/lib/sub/d.dat','C:/lib/sub','d.dat','dat',250,0,0,'other',0,0)",
            [],
        )
        .unwrap();

        let details = category_details(&conn, "other", 10).unwrap();
        assert_eq!(details.file_count, 2);
        assert_eq!(details.total_bytes, 1000);
        assert_eq!(details.by_extension.len(), 1);
        assert_eq!(details.by_extension[0].ext, "dat");
        assert_eq!(details.by_extension[0].count, 2);
        assert_eq!(details.by_folder.len(), 2);
        assert_eq!(details.by_folder[0].path, "C:/lib");
        assert_eq!(details.largest_files.len(), 2);
        assert_eq!(details.largest_files[0].size, 750);
    }

    #[test]
    fn clutter_score_is_bounded() {
        let conn = db::open_in_memory().unwrap();
        seed(&conn);
        let score = clutter_score(&conn).unwrap();
        assert!((0..=100).contains(&score.score));
    }

    #[test]
    fn totals_are_cached_and_refresh_when_invalidated() {
        let conn = db::open_in_memory().unwrap();
        seed(&conn);
        let first = file_totals(&conn).unwrap();
        assert_eq!(first.file_count, 2);
        assert_eq!(first.total_bytes, 1500);

        // A new row is invisible while the cache is warm: that is the whole
        // point — the dashboard must not rescan millions of rows on every visit.
        conn.execute(
            "INSERT INTO files(id,root_id,path,parent_path,name,size,category,indexed_at,last_seen_at)
             VALUES (3,1,'C:/lib/c.mp4','C:/lib','c.mp4',250,'video',0,0)",
            [],
        )
        .unwrap();
        assert_eq!(file_totals(&conn).unwrap().file_count, 2);

        // An empty folder added while the cache is warm is also invisible.
        conn.execute(
            "INSERT INTO dirs(root_id,path,parent_path,depth,file_count,total_bytes)
             VALUES (1,'C:/lib/empty','C:/lib',1,0,0)",
            [],
        )
        .unwrap();
        assert_eq!(file_totals(&conn).unwrap().empty_folder_count, 0);

        // Anything that changes the index clears the cache.
        invalidate_totals(&conn);
        let refreshed = file_totals(&conn).unwrap();
        assert_eq!(refreshed.file_count, 3);
        assert_eq!(refreshed.total_bytes, 1750);
        assert_eq!(refreshed.by_category[0].category, "video");
        assert_eq!(refreshed.by_root[0].file_count, 3);
        assert_eq!(refreshed.empty_folder_count, 1);
    }

    #[test]
    fn forgotten_totals_match_the_max_test() {
        let conn = db::open_in_memory().unwrap();
        seed(&conn);
        // Both seed rows have atime and mtime 0, so they count as forgotten.
        let totals = file_totals(&conn).unwrap();
        let expected: i64 = conn
            .query_row(
                "SELECT count(*) FROM files WHERE MAX(atime, mtime) <= ?1",
                [fsutil::now_secs() - 180 * 86_400],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(totals.forgotten_count, expected);
        assert_eq!(totals.forgotten_bytes, 1500);
    }
}
