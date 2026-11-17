//! Digital Mess Cleaner — app setup, shared state and command registration.

pub mod automation;
pub mod cleanup;
pub mod commands;
pub mod db;
pub mod duplicates;
pub mod error;
pub mod format;
pub mod fsutil;
pub mod health;
pub mod intelligence;
pub mod media;
pub mod models;
pub mod organizer;
pub mod repo;
pub mod scanner;
pub mod search;
pub mod storage;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::Manager;

/// Append panic details to `logs/crash.log` so an unexpected stop can be
/// diagnosed later, and still let the default hook print to stderr.
fn install_panic_logger(log_dir: PathBuf) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        use std::io::Write;
        let path = log_dir.join("crash.log");
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let location = info
                .location()
                .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
                .unwrap_or_else(|| "unknown location".to_string());
            let message = if let Some(text) = info.payload().downcast_ref::<&str>() {
                (*text).to_string()
            } else if let Some(text) = info.payload().downcast_ref::<String>() {
                text.clone()
            } else {
                "unknown panic".to_string()
            };
            let _ = writeln!(
                file,
                "[{}] {} — {}",
                chrono::Utc::now().to_rfc3339(),
                location,
                message
            );
        }
        previous(info);
    }));
}

/// State shared by every command. The database uses a single connection behind a
/// mutex; long jobs run on their own thread and take the lock in short bursts.
pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    pub data_dir: PathBuf,
    pub cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            std::fs::create_dir_all(data_dir.join("quarantine"))?;
            std::fs::create_dir_all(data_dir.join("logs"))?;
            install_panic_logger(data_dir.join("logs"));
            let conn = db::open(&data_dir.join("digital-mess-cleaner.db"))
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            organizer::seed_default_rules(&conn).ok();

            app.manage(AppState {
                db: Mutex::new(conn),
                data_dir: data_dir.clone(),
                cancels: Mutex::new(HashMap::new()),
            });

            // Conservative background worker: storage snapshots and explicitly
            // enabled auto-apply automations. It never deletes anything.
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(120));
                let state = handle.state::<AppState>();
                let messages = {
                    let mut conn = match state.db.lock() {
                        Ok(c) => c,
                        Err(_) => continue,
                    };
                    automation::tick(&mut conn).unwrap_or_default()
                };
                for message in messages {
                    let _ = tauri::Emitter::emit(&handle, "automation://applied", message);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::roots_list,
            commands::roots_add,
            commands::roots_remove,
            commands::index_clear,
            commands::dashboard,
            commands::storage_largest_files,
            commands::storage_largest_folders,
            commands::storage_child_folders,
            commands::storage_forgotten,
            commands::storage_empty_folders,
            commands::storage_suggestions,
            commands::storage_trends,
            commands::storage_snapshot,
            commands::storage_growth,
            commands::scan_start,
            commands::scan_cancel,
            commands::analyze_start,
            commands::analyze_file,
            commands::analyze_folder,
            commands::search_files,
            commands::list_files,
            commands::ask_files,
            commands::duplicates_analyze,
            commands::organize_plan,
            commands::organize_apply,
            commands::rules_list,
            commands::rules_save,
            commands::rules_delete,
            commands::rule_run,
            commands::documents_list,
            commands::document_detail,
            commands::versions_list,
            commands::reminders_list,
            commands::reminder_dismiss,
            commands::relations_rebuild,
            commands::related_files,
            commands::privacy_scan,
            commands::timeline,
            commands::quarantine_files,
            commands::quarantine_paths,
            commands::quarantine_list,
            commands::quarantine_restore,
            commands::quarantine_permanent_delete,
            commands::quarantine_purge,
            commands::operations_list,
            commands::operation_undo,
            commands::empty_folders_remove,
            commands::media_overview,
            commands::health_check,
            commands::app_leftovers,
            commands::temp_candidates,
            commands::migration_checklist,
            commands::migration_export,
            commands::automations_list,
            commands::automation_save,
            commands::automation_delete,
            commands::automation_run,
            commands::automation_runs,
            commands::notifications,
            commands::notifications_read,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Digital Mess Cleaner");
}
