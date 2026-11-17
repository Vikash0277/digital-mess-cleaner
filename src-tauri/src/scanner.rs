//! Incremental folder scanner. Metadata-only; content analysis is a separate,
//! cancellable pass so a first scan of a large drive stays fast.
//!
//! The walk deliberately never touches the database for more than one small
//! batch at a time. Commands call [`prepare_session`], then [`walk_root`] (which
//! hands back batches through a callback), then [`finish_root`]. That keeps the
//! shared connection free for the UI, keeps the SQLite WAL bounded, and means a
//! cancel or a crash cannot leave one enormous uncommitted transaction behind.

use crate::error::{AppError, AppResult};
use crate::fsutil;
use crate::models::{ScanProgress, ScanReport};
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Rows applied per transaction. Small enough that a lock is held for only a
/// few milliseconds, large enough that we do not pay commit overhead per file.
const BATCH_FILES: usize = 512;
const BATCH_DIRS: usize = 512;
/// Also flush on a timer so an awkward folder (or a slow network share) cannot
/// stall progress and leave a long-lived transaction open.
const FLUSH_EVERY: Duration = Duration::from_millis(250);
/// Progress events are throttled to this rate.
const PROGRESS_EVERY: Duration = Duration::from_millis(220);

pub struct ScanOptions {
    pub scan_id: String,
    pub max_files: Option<u64>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ScanCounters {
    pub seen: i64,
    pub indexed: i64,
    pub skipped: i64,
    pub bytes: i64,
    pub errors: i64,
}

pub struct ScannedFile {
    pub root_id: i64,
    pub path: String,
    pub parent: String,
    pub name: String,
    pub ext: String,
    pub size: i64,
    pub mtime: i64,
    pub atime: i64,
    pub ctime: i64,
    pub category: String,
    pub mime: String,
}

pub struct ScannedDir {
    pub root_id: i64,
    pub path: String,
    pub parent: String,
    pub depth: i64,
}

/// One unit of work handed back to the caller to persist.
pub struct ScanBatch {
    pub files: Vec<ScannedFile>,
    pub dirs: Vec<ScannedDir>,
    pub seen: Vec<String>,
}

struct Batcher {
    files: Vec<ScannedFile>,
    dirs: Vec<ScannedDir>,
    seen: Vec<String>,
}

impl Batcher {
    fn new() -> Self {
        Self {
            files: Vec::with_capacity(BATCH_FILES),
            dirs: Vec::with_capacity(BATCH_DIRS),
            seen: Vec::with_capacity(BATCH_FILES),
        }
    }

    fn is_full(&self) -> bool {
        self.files.len() >= BATCH_FILES || self.dirs.len() >= BATCH_DIRS
    }

    fn is_empty(&self) -> bool {
        self.files.is_empty() && self.dirs.is_empty()
    }

    fn take(&mut self) -> ScanBatch {
        ScanBatch {
            files: std::mem::take(&mut self.files),
            dirs: std::mem::take(&mut self.dirs),
            seen: std::mem::take(&mut self.seen),
        }
    }
}

/// Create the temporary "seen this scan" table. Safe to call once per scan run.
pub fn prepare_session(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS scan_seen (path TEXT PRIMARY KEY); DELETE FROM scan_seen;",
    )?;
    Ok(())
}

/// Walk `root`, calling `on_batch` with small batches to persist and
/// `on_progress` at most ~4 times a second. No database connection is held by
/// this function, so the caller decides how long each batch may lock.
///
/// `on_batch` returns how many rows were actually inserted or changed, which is
/// used to report `files_indexed` without keeping every existing path in memory.
pub fn walk_root(
    root_id: i64,
    root: &Path,
    options: &ScanOptions,
    cancelled: &AtomicBool,
    on_batch: &mut dyn FnMut(ScanBatch) -> AppResult<u64>,
    on_progress: &mut dyn FnMut(ScanProgress),
) -> AppResult<ScanCounters> {
    if !root.exists() {
        return Err(AppError::NotFound(format!(
            "{} is not available right now. Reconnect the drive or remove it from your library.",
            root.display()
        )));
    }

    let root_depth = root.components().count() as i64;
    let mut counters = ScanCounters::default();
    let mut batcher = Batcher::new();
    let mut last_flush = Instant::now();
    let mut last_progress = Instant::now();
    let mut current_path = String::new();

    let mut flush = |batcher: &mut Batcher, counters: &mut ScanCounters| -> AppResult<()> {
        if batcher.is_empty() {
            return Ok(());
        }
        let changed = on_batch(batcher.take())?;
        counters.indexed += changed as i64;
        counters.skipped = counters.seen - counters.indexed;
        Ok(())
    };

    let walker = walkdir::WalkDir::new(root)
        .follow_links(false)
        .max_open(64)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.eq_ignore_ascii_case(".git") || e.depth() == 0
        });

    let mut stopped = false;
    for entry in walker {
        if cancelled.load(Ordering::Relaxed) {
            stopped = true;
            break;
        }
        let entry = match entry {
            Ok(e) => e,
            Err(_) => {
                counters.errors += 1;
                continue;
            }
        };
        let path = entry.path();
        if entry.file_type().is_dir() {
            let parent = path.parent().unwrap_or(root);
            let depth = path.components().count() as i64 - root_depth;
            batcher.dirs.push(ScannedDir {
                root_id,
                path: path.to_string_lossy().to_string(),
                parent: parent.to_string_lossy().to_string(),
                depth: depth.max(0),
            });
        } else if entry.file_type().is_file() {
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => {
                    counters.errors += 1;
                    continue;
                }
            };
            counters.seen += 1;
            counters.bytes += meta.len() as i64;
            current_path = path.to_string_lossy().to_string();
            batcher.seen.push(current_path.clone());

            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file")
                .to_string();
            let parent = path.parent().unwrap_or(root).to_string_lossy().to_string();
            batcher.files.push(ScannedFile {
                root_id,
                path: current_path.clone(),
                parent,
                name,
                ext: fsutil::ext_of(path),
                size: meta.len() as i64,
                mtime: unix_secs(meta.modified().ok()),
                atime: unix_secs(meta.accessed().ok()),
                ctime: unix_secs(meta.created().ok()),
                category: fsutil::category_of(path).to_string(),
                mime: fsutil::mime_of(path),
            });
        } else {
            continue;
        }

        if batcher.is_full() || last_flush.elapsed() >= FLUSH_EVERY {
            flush(&mut batcher, &mut counters)?;
            last_flush = Instant::now();
        }
        if last_progress.elapsed() >= PROGRESS_EVERY {
            last_progress = Instant::now();
            on_progress(ScanProgress {
                scan_id: options.scan_id.clone(),
                root_id,
                phase: "scanning".into(),
                files_seen: counters.seen,
                files_indexed: counters.indexed,
                files_skipped: counters.skipped,
                bytes_seen: counters.bytes,
                current_path: current_path.clone(),
                errors: counters.errors,
            });
        }
        if let Some(max) = options.max_files {
            if counters.seen >= max as i64 {
                stopped = true;
                break;
            }
        }
    }

    flush(&mut batcher, &mut counters)?;
    on_progress(ScanProgress {
        scan_id: options.scan_id.clone(),
        root_id,
        phase: if stopped {
            "finishing".into()
        } else {
            "saving".into()
        },
        files_seen: counters.seen,
        files_indexed: counters.indexed,
        files_skipped: counters.skipped,
        bytes_seen: counters.bytes,
        current_path,
        errors: counters.errors,
    });
    Ok(counters)
}

fn unix_secs(time: Option<std::time::SystemTime>) -> i64 {
    time.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

const UPSERT_FILE: &str =
    "INSERT INTO files(root_id, path, parent_path, name, ext, size, mtime, atime, ctime,
                           category, mime, indexed_at, last_seen_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)
     ON CONFLICT(path) DO UPDATE SET
       root_id = excluded.root_id,
       parent_path = excluded.parent_path,
       name = excluded.name,
       ext = excluded.ext,
       size = excluded.size,
       mtime = excluded.mtime,
       atime = excluded.atime,
       ctime = excluded.ctime,
       category = excluded.category,
       mime = excluded.mime,
       content_text = NULL,
       content_status = 'none',
       hash = NULL,
       partial_hash = NULL,
       phash = NULL,
       width = NULL,
       height = NULL,
       tags = '[]',
       doc_type = NULL,
       vendor = NULL,
       amount = NULL,
       invoice_no = NULL,
       doc_date = NULL,
       due_date = NULL,
       confidence = 0,
       sensitive = 0,
       sensitive_kind = NULL,
       evidence = '[]',
       summary = NULL,
       analyzed_at = NULL,
       indexed_at = excluded.indexed_at,
       last_seen_at = excluded.last_seen_at
     WHERE files.size <> excluded.size OR files.mtime <> excluded.mtime";

/// Persist one batch in a short transaction. Returns how many files were new or
/// changed; unchanged files are recognised by SQLite itself, so a large existing
/// index is never loaded into memory.
pub fn apply_batch(conn: &mut Connection, batch: ScanBatch) -> AppResult<u64> {
    let now = fsutil::now_secs();
    let tx = conn.transaction()?;
    let mut changed = 0u64;
    {
        let mut upsert = tx.prepare_cached(UPSERT_FILE)?;
        let mut mark_seen =
            tx.prepare_cached("INSERT OR IGNORE INTO scan_seen(path) VALUES (?1)")?;
        let mut upsert_dir = tx.prepare_cached(
            "INSERT INTO dirs(root_id, path, parent_path, depth) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(root_id, path) DO UPDATE SET parent_path = excluded.parent_path, depth = excluded.depth",
        )?;
        for path in &batch.seen {
            mark_seen.execute([path])?;
        }
        for file in &batch.files {
            changed += upsert.execute(params![
                file.root_id,
                file.path,
                file.parent,
                file.name,
                file.ext,
                file.size,
                file.mtime,
                file.atime,
                file.ctime,
                file.category,
                file.mime,
                now,
            ])? as u64;
        }
        for dir in &batch.dirs {
            upsert_dir.execute(params![dir.root_id, dir.path, dir.parent, dir.depth])?;
        }
    }
    tx.commit()?;
    Ok(changed)
}

/// Remove rows for files that no longer exist (unless the scan was cancelled),
/// roll directory totals up and stamp the scan time.
pub fn finish_root(conn: &Connection, root_id: i64, cancelled: bool) -> AppResult<i64> {
    // A cancelled walk must not delete anything it did not get to see.
    let files_removed = if cancelled {
        0
    } else {
        conn.execute(
            "DELETE FROM files WHERE root_id = ?1 AND path NOT IN (SELECT path FROM scan_seen)",
            [root_id],
        )? as i64
    };
    recompute_dir_totals(conn, root_id)?;
    let ended = fsutil::now_secs();
    let _ = conn.execute(
        "UPDATE roots SET last_scan_at = ?1 WHERE id = ?2",
        params![ended, root_id],
    );
    let _ = crate::db::set_setting(conn, "last_scan_at", &ended.to_string());
    Ok(files_removed)
}

/// Roll direct sizes up the tree so "largest folders" reflects whole subtrees.
fn recompute_dir_totals(conn: &Connection, root_id: i64) -> AppResult<()> {
    let mut direct: HashMap<String, (String, i64, i64)> = HashMap::new();
    {
        // One pass over files, grouped by parent, instead of a correlated
        // subquery per directory (there can be hundreds of thousands of them).
        let mut stmt = conn.prepare(
            "SELECT d.path, d.parent_path, COALESCE(f.c, 0), COALESCE(f.b, 0)
             FROM dirs d
             LEFT JOIN (
               SELECT parent_path, count(*) AS c, COALESCE(sum(size), 0) AS b
               FROM files WHERE root_id = ?1 GROUP BY parent_path
             ) f ON f.parent_path = d.path
             WHERE d.root_id = ?1",
        )?;
        let rows = stmt.query_map([root_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?;
        for row in rows.flatten() {
            direct.insert(row.0, (row.1, row.2, row.3));
        }
    }
    let mut paths: Vec<String> = direct.keys().cloned().collect();
    paths.sort_by_key(|p| std::cmp::Reverse(p.matches(std::path::MAIN_SEPARATOR).count()));
    let mut totals: HashMap<String, (i64, i64)> = HashMap::new();
    for path in &paths {
        let (_, count, bytes) = direct.get(path).cloned().unwrap_or_default();
        let agg = totals.entry(path.clone()).or_insert((0, 0));
        agg.0 += count;
        agg.1 += bytes;
        let (sub_count, sub_bytes) = *agg;
        if let Some((parent, _, _)) = direct.get(path) {
            if parent != path && !parent.is_empty() {
                let p = totals.entry(parent.clone()).or_insert((0, 0));
                p.0 += sub_count;
                p.1 += sub_bytes;
            }
        }
    }
    // Write the totals back in chunks. One giant transaction over hundreds of
    // thousands of folders would grow the WAL by hundreds of megabytes and hold
    // the database lock for seconds; small chunks keep both bounded.
    const UPDATE_CHUNK: usize = 1000;
    let rows: Vec<(&String, &(i64, i64))> = totals.iter().collect();
    for chunk in rows.chunks(UPDATE_CHUNK) {
        let tx = conn.unchecked_transaction()?;
        {
            let mut update = tx.prepare_cached(
                "UPDATE dirs SET file_count = ?1, total_bytes = ?2 WHERE root_id = ?3 AND path = ?4",
            )?;
            for (path, (count, bytes)) in chunk {
                update.execute(params![count, bytes, root_id, path])?;
            }
        }
        tx.commit()?;
    }
    Ok(())
}

/// Convenience wrapper used by tests: scan one root synchronously.
pub fn scan_root(
    conn: &mut Connection,
    root_id: i64,
    root: &Path,
    options: &ScanOptions,
    cancelled: &AtomicBool,
) -> AppResult<ScanReport> {
    let started = Instant::now();
    prepare_session(conn)?;
    let counters = walk_root(
        root_id,
        root,
        options,
        cancelled,
        &mut |batch| apply_batch(conn, batch),
        &mut |_| {},
    )?;
    let was_cancelled = cancelled.load(Ordering::Relaxed);
    let files_removed = finish_root(conn, root_id, was_cancelled)?;
    Ok(ScanReport {
        scan_id: options.scan_id.clone(),
        files_seen: counters.seen,
        files_indexed: counters.indexed,
        files_skipped: counters.skipped,
        files_removed,
        bytes_seen: counters.bytes,
        errors: counters.errors,
        cancelled: was_cancelled,
        took_ms: started.elapsed().as_millis() as i64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn scan(conn: &mut Connection, root: &Path) -> ScanReport {
        let cancel = AtomicBool::new(false);
        let opts = ScanOptions {
            scan_id: "t".into(),
            max_files: None,
        };
        scan_root(conn, 1, root, &opts, &cancel).unwrap()
    }

    #[test]
    fn scans_and_skips_unchanged_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"hello").unwrap();
        std::fs::create_dir_all(dir.path().join("nested/empty")).unwrap();
        std::fs::write(dir.path().join("nested/b.pdf"), b"%PDF-1.4 test").unwrap();

        let mut conn = db::open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO roots(id, path, label, kind, created_at) VALUES (1, ?1, 'test', 'folder', 0)",
            [dir.path().to_string_lossy().to_string()],
        )
        .unwrap();
        let first = scan(&mut conn, dir.path());
        assert_eq!(first.files_seen, 2);
        assert_eq!(first.files_indexed, 2);
        let second = scan(&mut conn, dir.path());
        assert_eq!(second.files_indexed, 0);
        assert_eq!(second.files_skipped, 2);
        let empty: i64 = conn
            .query_row(
                "SELECT count(*) FROM dirs WHERE path LIKE '%empty' AND file_count = 0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(empty, 1);
        let totals: i64 = conn
            .query_row(
                "SELECT total_bytes FROM dirs WHERE path = ?1",
                [dir.path().to_string_lossy().to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(totals, 18);
    }

    #[test]
    fn removes_deleted_files() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("gone.txt");
        std::fs::write(&f, b"x").unwrap();
        let mut conn = db::open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO roots(id, path, label, kind, created_at) VALUES (1, ?1, 'test', 'folder', 0)",
            [dir.path().to_string_lossy().to_string()],
        )
        .unwrap();
        scan(&mut conn, dir.path());
        std::fs::remove_file(&f).unwrap();
        let again = scan(&mut conn, dir.path());
        assert_eq!(again.files_removed, 1);
    }

    #[test]
    fn changed_file_is_reindexed_and_content_reset() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("note.txt");
        std::fs::write(&f, b"one").unwrap();
        let mut conn = db::open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO roots(id, path, label, kind, created_at) VALUES (1, ?1, 'test', 'folder', 0)",
            [dir.path().to_string_lossy().to_string()],
        )
        .unwrap();
        scan(&mut conn, dir.path());
        conn.execute("UPDATE files SET content_text = 'stale'", [])
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        std::fs::write(&f, b"two-two").unwrap();
        let again = scan(&mut conn, dir.path());
        assert_eq!(again.files_indexed, 1);
        let (size, content): (i64, Option<String>) = conn
            .query_row(
                "SELECT size, content_text FROM files WHERE name='note.txt'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(size, 7);
        assert!(content.is_none());
    }

    #[test]
    fn cancelled_walk_keeps_existing_rows() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("keep.txt"), b"x").unwrap();
        let mut conn = db::open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO roots(id, path, label, kind, created_at) VALUES (1, ?1, 'test', 'folder', 0)",
            [dir.path().to_string_lossy().to_string()],
        )
        .unwrap();
        scan(&mut conn, dir.path());
        let cancel = AtomicBool::new(true);
        let opts = ScanOptions {
            scan_id: "t".into(),
            max_files: None,
        };
        let report = scan_root(&mut conn, 1, dir.path(), &opts, &cancel).unwrap();
        assert!(report.cancelled);
        assert_eq!(report.files_removed, 0);
        let count: i64 = conn
            .query_row("SELECT count(*) FROM files", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    /// Manual smoke test: tens of thousands of files, a concurrent reader, and a
    /// check that the write-ahead log stays small. Run with:
    /// `cargo test large_scan -- --ignored --nocapture`
    #[test]
    #[ignore = "creates 20k files; run manually for performance checks"]
    fn large_scan_keeps_locks_short_and_wal_small() {
        use std::sync::atomic::AtomicI64;
        use std::sync::Mutex;

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        for bucket in 0..40 {
            let bucket_dir = root.join(format!("dir{bucket}"));
            std::fs::create_dir_all(&bucket_dir).unwrap();
            for item in 0..500 {
                std::fs::write(bucket_dir.join(format!("file{item}.txt")), b"x").unwrap();
            }
        }

        let db_path = dir.path().join("index.db");
        let conn = db::open(&db_path).unwrap();
        conn.execute(
            "INSERT INTO roots(id, path, label, kind, created_at) VALUES (1, ?1, 'test', 'folder', 0)",
            [root.to_string_lossy().to_string()],
        )
        .unwrap();
        prepare_session(&conn).unwrap();

        let shared = std::sync::Arc::new(Mutex::new(conn));
        let done = std::sync::Arc::new(AtomicBool::new(false));
        let worst_wait = std::sync::Arc::new(AtomicI64::new(0));
        let worst_wal = std::sync::Arc::new(AtomicI64::new(0));

        let reader = {
            let shared = shared.clone();
            let done = done.clone();
            let worst_wait = worst_wait.clone();
            let worst_wal = worst_wal.clone();
            let wal = db_path.with_extension("db-wal");
            std::thread::spawn(move || {
                while !done.load(Ordering::Relaxed) {
                    let started = Instant::now();
                    let guard = shared.lock().unwrap();
                    worst_wait.fetch_max(started.elapsed().as_millis() as i64, Ordering::Relaxed);
                    let _: i64 = guard
                        .query_row("SELECT count(*) FROM files", [], |r| r.get(0))
                        .unwrap_or(0);
                    drop(guard);
                    if let Ok(meta) = std::fs::metadata(&wal) {
                        worst_wal.fetch_max(meta.len() as i64, Ordering::Relaxed);
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
            })
        };

        let cancel = AtomicBool::new(false);
        let options = ScanOptions {
            scan_id: "big".into(),
            max_files: None,
        };
        let counters = walk_root(
            1,
            &root,
            &options,
            &cancel,
            &mut |batch| {
                let mut guard = shared.lock().unwrap();
                apply_batch(&mut guard, batch)
            },
            &mut |_| {},
        )
        .unwrap();
        {
            let guard = shared.lock().unwrap();
            finish_root(&guard, 1, false).unwrap();
        }
        done.store(true, Ordering::Relaxed);
        reader.join().unwrap();

        assert_eq!(counters.seen, 20_000);
        assert_eq!(counters.indexed, 20_000);
        let wait = worst_wait.load(Ordering::Relaxed);
        let wal = worst_wal.load(Ordering::Relaxed);
        println!("worst lock wait: {wait}ms, largest WAL: {wal} bytes");
        assert!(wait < 500, "a reader waited {wait}ms for the database");
        assert!(
            wal < 32 * 1024 * 1024,
            "the write-ahead log grew to {wal} bytes"
        );
    }
}
