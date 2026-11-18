//! Thin `#[tauri::command]` entry points. All filesystem work happens in the
//! engine modules; these functions only lock state, call the engine and return.

use crate::error::{AppError, AppResult};
use crate::intelligence::AnalysisOutcome;
use crate::models::*;
use crate::{
    apps, automation, cleanup, duplicates, fileviewer, health, intelligence, media, organizer,
    repo, scanner, search, storage, AppState,
};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, State};

fn lock<'a>(state: &'a AppState) -> AppResult<std::sync::MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| {
        AppError::Internal("The local database lock was interrupted. Restart the app.".into())
    })
}

async fn with_db<T, F>(app: AppHandle, f: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce(&mut Connection, &Path) -> AppResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut conn = lock(&state)?;
        let data_dir = state.data_dir.clone();
        f(&mut conn, &data_dir)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Background task failed: {e}")))?
}

/// Same as [`with_db`] for commands that change the index. The cached dashboard
/// totals are dropped whether the command succeeds or fails, so a half-applied
/// change can never leave stale numbers on screen.
async fn with_db_write<T, F>(app: AppHandle, f: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce(&mut Connection, &Path) -> AppResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut conn = lock(&state)?;
        let data_dir = state.data_dir.clone();
        let result = f(&mut conn, &data_dir);
        storage::invalidate_totals(&conn);
        result
    })
    .await
    .map_err(|e| AppError::Internal(format!("Background task failed: {e}")))?
}

// ---------------------------------------------------------------------------
// App + library roots
// ---------------------------------------------------------------------------

#[tauri::command(async)]
pub fn app_info(state: State<'_, AppState>) -> AppResult<AppInfo> {
    let conn = lock(&state)?;
    health::app_info(&conn, &state.data_dir)
}

#[tauri::command(async)]
pub fn roots_list(state: State<'_, AppState>) -> AppResult<Vec<RootInfo>> {
    let conn = lock(&state)?;
    storage::roots_with_totals(&conn)
}

#[tauri::command(async)]
pub fn roots_add(
    state: State<'_, AppState>,
    path: String,
    kind: Option<String>,
) -> AppResult<Vec<RootInfo>> {
    let conn = lock(&state)?;
    let raw = PathBuf::from(&path);
    if !raw.is_dir() {
        return Err(AppError::Invalid(
            "Choose an existing folder or drive.".into(),
        ));
    }
    let canonical = dunce::canonicalize(&raw).unwrap_or(raw);
    let path_string = canonical.to_string_lossy().to_string();
    if let Some(reason) = crate::fsutil::protected_reason(&canonical) {
        return Err(AppError::Permission(reason));
    }
    let label = canonical
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| path_string.clone());
    let kind = kind.unwrap_or_else(|| {
        if canonical.parent().is_none() {
            "drive".into()
        } else {
            "folder".into()
        }
    });
    conn.execute(
        "INSERT OR IGNORE INTO roots(path, label, kind, enabled, created_at) VALUES (?1, ?2, ?3, 1, ?4)",
        rusqlite::params![path_string, label, kind, crate::fsutil::now_secs()],
    )?;
    storage::invalidate_totals(&conn);
    storage::roots_with_totals(&conn)
}

#[tauri::command(async)]
pub fn roots_remove(state: State<'_, AppState>, id: i64) -> AppResult<Vec<RootInfo>> {
    let conn = lock(&state)?;
    conn.execute("DELETE FROM roots WHERE id=?1", [id])?;
    storage::invalidate_totals(&conn);
    storage::roots_with_totals(&conn)
}

#[tauri::command(async)]
pub fn index_clear(state: State<'_, AppState>, confirmation: String) -> AppResult<()> {
    if confirmation.trim() != "CLEAR INDEX" {
        return Err(AppError::Invalid("Type “CLEAR INDEX” to confirm.".into()));
    }
    let conn = lock(&state)?;
    conn.execute("DELETE FROM dirs", [])?;
    conn.execute("DELETE FROM files", [])?;
    conn.execute("DELETE FROM roots", [])?;
    conn.execute("DELETE FROM relations", [])?;
    conn.execute("DELETE FROM reminders", [])?;
    conn.execute("DELETE FROM storage_snapshots", [])?;
    storage::invalidate_totals(&conn);
    Ok(())
}

// ---------------------------------------------------------------------------
// Dashboard, storage and insights
// ---------------------------------------------------------------------------

#[tauri::command(async)]
pub fn dashboard(state: State<'_, AppState>) -> AppResult<Dashboard> {
    let conn = lock(&state)?;
    storage::dashboard(&conn)
}

#[tauri::command(async)]
pub fn storage_largest_files(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> AppResult<Vec<FileEntry>> {
    let conn = lock(&state)?;
    storage::largest_files(&conn, limit.unwrap_or(100))
}

#[tauri::command(async)]
pub fn storage_largest_folders(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> AppResult<Vec<FolderStat>> {
    let conn = lock(&state)?;
    storage::largest_folders(&conn, limit.unwrap_or(50))
}

#[tauri::command(async)]
pub fn storage_child_folders(
    state: State<'_, AppState>,
    path: String,
) -> AppResult<Vec<FolderStat>> {
    let conn = lock(&state)?;
    storage::child_folders(&conn, &path)
}

#[tauri::command(async)]
pub fn storage_forgotten(
    state: State<'_, AppState>,
    days: Option<i64>,
    min_size: Option<i64>,
    limit: Option<i64>,
) -> AppResult<Vec<FileEntry>> {
    let conn = lock(&state)?;
    storage::forgotten_files(
        &conn,
        days.unwrap_or(365),
        min_size.unwrap_or(0),
        limit.unwrap_or(200),
    )
}

#[tauri::command(async)]
pub fn storage_empty_folders(state: State<'_, AppState>) -> AppResult<Vec<FolderStat>> {
    let conn = lock(&state)?;
    storage::empty_folders(&conn)
}

#[tauri::command(async)]
pub fn storage_suggestions(state: State<'_, AppState>) -> AppResult<Vec<CleanupSuggestion>> {
    let conn = lock(&state)?;
    storage::cleanup_suggestions(&conn)
}

#[tauri::command(async)]
pub fn storage_trends(state: State<'_, AppState>, days: Option<i64>) -> AppResult<Vec<Snapshot>> {
    let conn = lock(&state)?;
    storage::trends(&conn, days.unwrap_or(90))
}

#[tauri::command(async)]
pub fn storage_snapshot(state: State<'_, AppState>) -> AppResult<Snapshot> {
    let conn = lock(&state)?;
    storage::take_snapshot(&conn)
}

#[tauri::command(async)]
pub fn storage_growth(state: State<'_, AppState>, days: Option<i64>) -> AppResult<StorageGrowth> {
    let conn = lock(&state)?;
    health::storage_growth(&conn, days.unwrap_or(90))
}

/// Explicit drill-down: never part of dashboard load because it groups millions
/// of rows. Runs off the UI thread so the app stays responsive while it works.
#[tauri::command]
pub async fn storage_category_details(
    app: AppHandle,
    category: String,
    limit: Option<i64>,
) -> AppResult<CategoryDetails> {
    with_db(app, move |conn, _| {
        storage::category_details(conn, &category, limit.unwrap_or(20))
    })
    .await
}

#[tauri::command]
pub async fn installed_apps(state: State<'_, AppState>) -> AppResult<InstalledApps> {
    let db_path = state.data_dir.join("digital-mess-cleaner.db");
    tauri::async_runtime::spawn_blocking(move || apps::installed_apps(&db_path))
        .await
        .map_err(|e| AppError::Internal(format!("Background task failed: {e}")))?
}

// ---------------------------------------------------------------------------
// Read-only file viewing
// ---------------------------------------------------------------------------

/// All four commands validate the indexed id and path in Rust. They never write
/// to the operation log: opening or inspecting a file is not a file operation.
#[tauri::command]
pub async fn file_open(app: AppHandle, file_id: i64) -> AppResult<()> {
    with_db(app, move |conn, _| fileviewer::open_file(conn, file_id)).await
}

#[tauri::command]
pub async fn file_reveal(app: AppHandle, file_id: i64) -> AppResult<()> {
    with_db(app, move |conn, _| fileviewer::reveal_file(conn, file_id)).await
}

#[tauri::command]
pub async fn file_details(app: AppHandle, file_id: i64) -> AppResult<FileDetails> {
    with_db(app, move |conn, _| fileviewer::file_details(conn, file_id)).await
}

#[tauri::command]
pub async fn file_preview(app: AppHandle, file_id: i64) -> AppResult<FilePreview> {
    with_db(app, move |conn, _| fileviewer::file_preview(conn, file_id)).await
}

// ---------------------------------------------------------------------------
// Scanning + analysis
// ---------------------------------------------------------------------------

#[tauri::command(async)]
pub fn scan_start(
    app: AppHandle,
    root_ids: Option<Vec<i64>>,
    max_files: Option<u64>,
) -> AppResult<String> {
    let scan_id = uuid::Uuid::new_v4().to_string();
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let state = app.state::<AppState>();
        let mut registry = state
            .cancels
            .lock()
            .map_err(|_| AppError::Internal("Cancellation lock was interrupted.".into()))?;
        if !registry.is_empty() {
            return Err(AppError::Invalid(
                "A scan or analysis is already running. Wait for it to finish or cancel it.".into(),
            ));
        }
        registry.insert(scan_id.clone(), cancel.clone());
    }

    let targets: Vec<(i64, PathBuf)> = {
        let state = app.state::<AppState>();
        let conn = lock(&state)?;
        let roots = repo::root_paths(&conn)?;
        roots
            .into_iter()
            .filter(|(id, _)| {
                root_ids
                    .as_ref()
                    .map(|ids| ids.contains(id))
                    .unwrap_or(true)
            })
            .collect()
    };
    if targets.is_empty() {
        let state = app.state::<AppState>();
        if let Ok(mut registry) = state.cancels.lock() {
            registry.remove(&scan_id);
        }
        return Err(AppError::Invalid(
            "Add a folder to your library before scanning.".into(),
        ));
    }

    let app2 = app.clone();
    let id2 = scan_id.clone();
    let cancel2 = cancel.clone();
    std::thread::spawn(move || {
        let state = app2.state::<AppState>();

        // A panic in the walker must never take the whole app down or leave the
        // UI waiting forever for a "done" event.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_scan(&app2, &state, &id2, &cancel2, &targets, max_files)
        }));

        {
            let conn = lock(&state);
            if let Ok(conn) = conn {
                // The walk just changed the index: drop the cached dashboard
                // numbers, then rebuild them once (the snapshot needs them too)
                // so the next dashboard visit is a single row read.
                storage::invalidate_totals(&conn);
                // Rebuild once, here, so the dashboard's automatic reload after
                // a scan finds the numbers already computed instead of scanning
                // millions of rows while the window waits.
                let _ = storage::file_totals(&conn);
                let _ = storage::record_snapshot_if_due(&conn);
                crate::db::checkpoint(&conn);
            }
        }
        if let Ok(mut registry) = state.cancels.lock() {
            registry.remove(&id2);
        }

        let (reports, errors) = match outcome {
            Ok(Ok(value)) => value,
            Ok(Err(err)) => (Vec::new(), vec![err.to_string()]),
            Err(_) => (
                Vec::new(),
                vec!["Scanning stopped unexpectedly. Your index is safe; try again.".to_string()],
            ),
        };
        let _ = app2.emit(
            "scan://done",
            serde_json::json!({
                "scanId": id2,
                "reports": reports,
                "errors": errors,
                "cancelled": cancel2.load(Ordering::Relaxed),
            }),
        );
    });

    Ok(scan_id)
}

/// Walk every target one after another. The database is only locked for the
/// split second it takes to persist a batch, so the rest of the UI keeps working
/// while a drive is being scanned.
fn run_scan(
    app: &AppHandle,
    state: &AppState,
    scan_id: &str,
    cancel: &Arc<AtomicBool>,
    targets: &[(i64, PathBuf)],
    max_files: Option<u64>,
) -> AppResult<(Vec<ScanReport>, Vec<String>)> {
    let mut reports: Vec<ScanReport> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    for (root_id, path) in targets {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let options = scanner::ScanOptions {
            scan_id: scan_id.to_string(),
            max_files,
        };
        match scan_one_root(app, state, *root_id, path, &options, cancel) {
            Ok(report) => reports.push(report),
            Err(e) => errors.push(e.to_string()),
        }
    }
    Ok((reports, errors))
}

fn scan_one_root(
    app: &AppHandle,
    state: &AppState,
    root_id: i64,
    path: &Path,
    options: &scanner::ScanOptions,
    cancel: &AtomicBool,
) -> AppResult<ScanReport> {
    let started = Instant::now();
    {
        let conn = lock(state)?;
        scanner::prepare_session(&conn)?;
    }

    let counters = scanner::walk_root(
        root_id,
        path,
        options,
        cancel,
        &mut |batch| {
            let mut conn = lock(state)?;
            scanner::apply_batch(&mut conn, batch)
        },
        &mut |progress| {
            let _ = app.emit("scan://progress", progress);
        },
    )?;

    let was_cancelled = cancel.load(Ordering::Relaxed);
    let files_removed = {
        let conn = lock(state)?;
        scanner::finish_root(&conn, root_id, was_cancelled)?
    };

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

#[tauri::command(async)]
pub fn scan_cancel(app: AppHandle, scan_id: String) -> AppResult<()> {
    let state = app.state::<AppState>();
    let registry = state
        .cancels
        .lock()
        .map_err(|_| AppError::Internal("Cancellation lock was interrupted.".into()))?;
    if let Some(flag) = registry.get(&scan_id) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command(async)]
pub fn analyze_start(app: AppHandle, limit: Option<i64>) -> AppResult<String> {
    let job_id = uuid::Uuid::new_v4().to_string();
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let state = app.state::<AppState>();
        let mut registry = state
            .cancels
            .lock()
            .map_err(|_| AppError::Internal("Cancellation lock was interrupted.".into()))?;
        if !registry.is_empty() {
            return Err(AppError::Invalid(
                "Another scan or analysis is already running.".into(),
            ));
        }
        registry.insert(job_id.clone(), cancel.clone());
    }
    let app2 = app.clone();
    let id2 = job_id.clone();
    let cancel2 = cancel.clone();
    std::thread::spawn(move || {
        let state = app2.state::<AppState>();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_analysis(&app2, &state, &cancel2, limit.unwrap_or(2000))
        }));
        if let Ok(mut registry) = state.cancels.lock() {
            registry.remove(&id2);
        }
        match outcome {
            Ok(Ok(report)) => {
                let _ = app2.emit("analyze://done", serde_json::json!({ "report": report }));
            }
            Ok(Err(e)) => {
                let _ = app2.emit(
                    "analyze://done",
                    serde_json::json!({ "error": e.to_string() }),
                );
            }
            Err(_) => {
                let _ = app2.emit(
                    "analyze://done",
                    serde_json::json!({
                        "error": "Analysis stopped unexpectedly. Your files were not changed."
                    }),
                );
            }
        }
    });
    Ok(job_id)
}

/// Analyse one file per lock acquisition, emitting progress between files.
fn run_analysis(
    app: &AppHandle,
    state: &AppState,
    cancel: &Arc<AtomicBool>,
    limit: i64,
) -> AppResult<crate::models::AnalyzeReport> {
    let started = Instant::now();
    let targets = {
        let conn = lock(state)?;
        intelligence::pending_targets(&conn, limit)?
    };
    let total = targets.len() as i64;
    let mut analyzed = 0i64;
    let mut failed = 0i64;
    let mut cancelled = false;

    for (index, (file_id, file_path)) in targets.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }
        let _ = app.emit(
            "analyze://progress",
            serde_json::json!({ "done": index, "total": total, "currentPath": file_path }),
        );
        let result = {
            let conn = lock(state)?;
            intelligence::analyze_file(&conn, *file_id)
        };
        match result {
            Ok(_) => analyzed += 1,
            Err(_) => failed += 1,
        }
    }

    {
        let conn = lock(state)?;
        if !cancelled {
            let _ = intelligence::rebuild_relations(&conn);
        }
        // Analysis rewrites content_status and sensitive flags, which the
        // dashboard totals count.
        crate::storage::invalidate_totals(&conn);
    }

    Ok(crate::models::AnalyzeReport {
        analyzed,
        failed,
        cancelled,
        took_ms: started.elapsed().as_millis() as i64,
    })
}

#[tauri::command(async)]
pub fn analyze_file(state: State<'_, AppState>, file_id: i64) -> AppResult<AnalysisOutcome> {
    let conn = lock(&state)?;
    let outcome = intelligence::analyze_file(&conn, file_id)?;
    storage::invalidate_totals(&conn);
    Ok(outcome)
}

#[tauri::command(async)]
pub fn analyze_folder(
    state: State<'_, AppState>,
    folder: String,
    limit: Option<i64>,
) -> AppResult<i64> {
    let conn = lock(&state)?;
    let analyzed = organizer::analyze_folder(&conn, &folder, limit.unwrap_or(300))?;
    storage::invalidate_totals(&conn);
    Ok(analyzed)
}

// ---------------------------------------------------------------------------
// Search + Ask
// ---------------------------------------------------------------------------

#[tauri::command(async)]
pub fn search_files(
    state: State<'_, AppState>,
    query: String,
    filters: Option<SearchFilters>,
) -> AppResult<SearchResponse> {
    let conn = lock(&state)?;
    search::search(&conn, &query, filters.unwrap_or_default())
}

#[tauri::command(async)]
pub fn list_files(
    state: State<'_, AppState>,
    filters: Option<SearchFilters>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> AppResult<Page<FileEntry>> {
    let conn = lock(&state)?;
    search::list_files(
        &conn,
        filters.unwrap_or_default(),
        limit.unwrap_or(200).clamp(1, 2000),
        offset.unwrap_or(0),
    )
}

#[tauri::command(async)]
pub fn ask_files(state: State<'_, AppState>, question: String) -> AppResult<AskAnswer> {
    let conn = lock(&state)?;
    search::ask(&conn, &question)
}

// ---------------------------------------------------------------------------
// Duplicates
// ---------------------------------------------------------------------------

#[tauri::command(async)]
pub async fn duplicates_analyze(
    app: AppHandle,
    mode: String,
    threshold: Option<u32>,
) -> AppResult<DuplicateReport> {
    with_db_write(app, move |conn, _| {
        duplicates::analyze(conn, &mode, threshold.unwrap_or(10))
    })
    .await
}

// ---------------------------------------------------------------------------
// Organisation
// ---------------------------------------------------------------------------

#[tauri::command(async)]
pub async fn organize_plan(
    app: AppHandle,
    folder: String,
    limit: Option<i64>,
) -> AppResult<OrganizePlan> {
    with_db(app, move |conn, _| {
        organizer::plan(conn, &folder, limit.unwrap_or(400))
    })
    .await
}

#[tauri::command(async)]
pub async fn organize_apply(
    app: AppHandle,
    items: Vec<OrganizeRequestItem>,
) -> AppResult<ApplyResult> {
    with_db_write(app, move |conn, _| organizer::apply(conn, &items)).await
}

#[tauri::command(async)]
pub fn rules_list(state: State<'_, AppState>) -> AppResult<Vec<OrganizerRule>> {
    let conn = lock(&state)?;
    organizer::rules(&conn)
}

#[tauri::command(async)]
pub async fn rules_save(app: AppHandle, rule: OrganizerRule) -> AppResult<Vec<OrganizerRule>> {
    with_db(app, move |conn, _| organizer::save_rule(conn, rule)).await
}

#[tauri::command(async)]
pub fn rules_delete(state: State<'_, AppState>, id: String) -> AppResult<Vec<OrganizerRule>> {
    let conn = lock(&state)?;
    organizer::delete_rule(&conn, &id)
}

#[tauri::command(async)]
pub async fn rule_run(app: AppHandle, id: String) -> AppResult<ApplyResult> {
    with_db_write(app, move |conn, _| {
        let rule = organizer::rules(conn)?.into_iter().find(|r| r.id == id);
        match rule {
            Some(rule) => organizer::run_rule(conn, &rule),
            None => Err(AppError::NotFound("Rule not found.".into())),
        }
    })
    .await
}

// ---------------------------------------------------------------------------
// Documents, versions, privacy
// ---------------------------------------------------------------------------

#[tauri::command(async)]
pub fn documents_list(
    state: State<'_, AppState>,
    doc_type: Option<String>,
    limit: Option<i64>,
) -> AppResult<Vec<DocumentRecord>> {
    let conn = lock(&state)?;
    intelligence::list_documents(&conn, doc_type.as_deref(), limit.unwrap_or(150))
}

#[tauri::command(async)]
pub fn document_detail(state: State<'_, AppState>, file_id: i64) -> AppResult<DocumentRecord> {
    let conn = lock(&state)?;
    intelligence::document_detail(&conn, file_id)
}

#[tauri::command(async)]
pub fn versions_list(state: State<'_, AppState>) -> AppResult<Vec<VersionGroup>> {
    let conn = lock(&state)?;
    intelligence::list_versions(&conn)
}

#[tauri::command(async)]
pub fn reminders_list(
    state: State<'_, AppState>,
    include_past: Option<bool>,
) -> AppResult<Vec<Reminder>> {
    let conn = lock(&state)?;
    intelligence::list_reminders(&conn, include_past.unwrap_or(false))
}

#[tauri::command(async)]
pub fn reminder_dismiss(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let conn = lock(&state)?;
    conn.execute("UPDATE reminders SET dismissed=1 WHERE id=?1", [id])?;
    Ok(())
}

#[tauri::command(async)]
pub fn relations_rebuild(state: State<'_, AppState>) -> AppResult<usize> {
    let conn = lock(&state)?;
    intelligence::rebuild_relations(&conn)
}

#[tauri::command(async)]
pub fn related_files(state: State<'_, AppState>, file_id: i64) -> AppResult<Vec<FileEntry>> {
    let conn = lock(&state)?;
    intelligence::related_files(&conn, file_id)
}

#[tauri::command(async)]
pub async fn privacy_scan(app: AppHandle, limit: Option<i64>) -> AppResult<Vec<SensitiveFinding>> {
    with_db(app, move |conn, _| {
        intelligence::sensitive_findings(conn, limit.unwrap_or(300))
    })
    .await
}

#[tauri::command(async)]
pub fn timeline(
    state: State<'_, AppState>,
    file_id: Option<i64>,
    limit: Option<i64>,
) -> AppResult<Vec<TimelineEvent>> {
    let conn = lock(&state)?;
    health::timeline(&conn, file_id, limit.unwrap_or(120))
}

// ---------------------------------------------------------------------------
// Cleanup + quarantine
// ---------------------------------------------------------------------------

#[tauri::command(async)]
pub async fn quarantine_files(app: AppHandle, file_ids: Vec<i64>) -> AppResult<QuarantineResult> {
    with_db_write(app, move |conn, data_dir| {
        cleanup::quarantine(conn, data_dir, &file_ids)
    })
    .await
}

#[tauri::command(async)]
pub async fn quarantine_paths(
    app: AppHandle,
    paths: Vec<String>,
    label: Option<String>,
) -> AppResult<QuarantineResult> {
    with_db_write(app, move |conn, data_dir| {
        cleanup::quarantine_paths(
            conn,
            data_dir,
            &paths,
            label.as_deref().unwrap_or("reviewed item"),
        )
    })
    .await
}

#[tauri::command(async)]
pub fn quarantine_list(
    state: State<'_, AppState>,
    include_restored: Option<bool>,
) -> AppResult<Vec<QuarantineEntry>> {
    let conn = lock(&state)?;
    cleanup::list_quarantine(&conn, include_restored.unwrap_or(false))
}

#[tauri::command(async)]
pub async fn quarantine_restore(app: AppHandle, ids: Vec<String>) -> AppResult<QuarantineResult> {
    with_db_write(app, move |conn, _| cleanup::restore(conn, &ids)).await
}

#[tauri::command(async)]
pub async fn quarantine_permanent_delete(
    app: AppHandle,
    ids: Vec<String>,
    confirmation: String,
) -> AppResult<QuarantineResult> {
    with_db_write(app, move |conn, data_dir| {
        cleanup::permanent_delete(conn, data_dir, &ids, &confirmation)
    })
    .await
}

#[tauri::command(async)]
pub async fn quarantine_purge(app: AppHandle) -> AppResult<i64> {
    with_db_write(app, move |conn, data_dir| {
        cleanup::purge_quarantine(conn, data_dir)
    })
    .await
}

#[tauri::command(async)]
pub fn operations_list(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> AppResult<Vec<OperationEntry>> {
    let conn = lock(&state)?;
    cleanup::operations(&conn, limit.unwrap_or(100))
}

#[tauri::command(async)]
pub async fn operation_undo(app: AppHandle, id: String) -> AppResult<OperationEntry> {
    with_db_write(app, move |conn, data_dir| {
        cleanup::undo_operation(conn, &id, data_dir)
    })
    .await
}

#[tauri::command(async)]
pub async fn empty_folders_remove(app: AppHandle, folders: Vec<String>) -> AppResult<i64> {
    with_db_write(app, move |conn, _| {
        cleanup::remove_empty_folders(conn, &folders)
    })
    .await
}

// ---------------------------------------------------------------------------
// Media, health, leftovers, migration
// ---------------------------------------------------------------------------

#[tauri::command(async)]
pub async fn media_overview(app: AppHandle) -> AppResult<MediaOverview> {
    with_db(app, move |conn, _| media::overview(conn)).await
}

#[tauri::command(async)]
pub async fn health_check(app: AppHandle) -> AppResult<Vec<HealthIssue>> {
    with_db(app, move |conn, _| health::health_check(conn)).await
}

#[tauri::command(async)]
pub async fn app_leftovers(app: AppHandle, limit: Option<i64>) -> AppResult<Vec<Leftover>> {
    with_db(app, move |conn, _| {
        let _ = conn;
        health::app_leftovers(limit.unwrap_or(40))
    })
    .await
}

#[tauri::command(async)]
pub async fn temp_candidates(app: AppHandle, limit: Option<i64>) -> AppResult<Vec<Leftover>> {
    with_db(app, move |conn, _| {
        let _ = conn;
        health::temp_candidates(limit.unwrap_or(40))
    })
    .await
}

#[tauri::command(async)]
pub fn migration_checklist(state: State<'_, AppState>) -> AppResult<MigrationChecklist> {
    let conn = lock(&state)?;
    health::migration_checklist(&conn)
}

#[tauri::command(async)]
pub async fn migration_export(app: AppHandle, path: String) -> AppResult<()> {
    with_db(app, move |conn, _| {
        let checklist = health::migration_checklist(conn)?;
        health::write_migration_checklist(Path::new(&path), &checklist)
    })
    .await
}

// ---------------------------------------------------------------------------
// Automation
// ---------------------------------------------------------------------------

#[tauri::command(async)]
pub fn automations_list(state: State<'_, AppState>) -> AppResult<Vec<Automation>> {
    let conn = lock(&state)?;
    automation::list(&conn)
}

#[tauri::command(async)]
pub fn automation_save(
    state: State<'_, AppState>,
    automation: Automation,
) -> AppResult<Vec<Automation>> {
    let conn = lock(&state)?;
    automation::save(&conn, automation)
}

#[tauri::command(async)]
pub fn automation_delete(state: State<'_, AppState>, id: String) -> AppResult<Vec<Automation>> {
    let conn = lock(&state)?;
    automation::delete(&conn, &id)
}

#[tauri::command(async)]
pub async fn automation_run(
    app: AppHandle,
    id: String,
    apply_now: Option<bool>,
) -> AppResult<AutomationRun> {
    with_db_write(app, move |conn, _| {
        automation::run(conn, &id, apply_now.unwrap_or(false))
    })
    .await
}

#[tauri::command(async)]
pub fn automation_runs(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> AppResult<Vec<AutomationRun>> {
    let conn = lock(&state)?;
    automation::runs(&conn, limit.unwrap_or(50))
}

#[tauri::command(async)]
pub fn notifications(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> AppResult<Vec<Notification>> {
    let conn = lock(&state)?;
    automation::notifications(&conn, limit.unwrap_or(50))
}

#[tauri::command(async)]
pub fn notifications_read(state: State<'_, AppState>) -> AppResult<()> {
    let conn = lock(&state)?;
    automation::mark_notifications_read(&conn)
}
