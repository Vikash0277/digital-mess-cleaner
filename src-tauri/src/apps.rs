//! Installed application inventory.
//!
//! Windows reads the three Uninstall registry hives. macOS reads app bundles in
//! the standard Application folders. Nothing is executed and nothing is changed:
//! this is a read-only view that never deletes or moves an app.
//!
//! An app's size comes from the local file index when an install folder is
//! recorded (measured, not guessed). When the installer left no folder, the
//! installer's own estimate is shown and labelled as such.

use crate::error::AppResult;
use crate::models::{InstalledApp, InstalledApps};
use rusqlite::{Connection, OpenFlags};
use std::collections::HashSet;
use std::path::Path;

/// Opens a second, read-only connection. The inventory can take a while on a
/// cold multi-gigabyte index; keeping it off the app's single connection means
/// a slow size lookup can never block the dashboard or search.
fn read_only_connection(db_path: &Path) -> Option<Connection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(db_path, flags).ok()?;
    conn.busy_timeout(std::time::Duration::from_secs(30)).ok()?;
    Some(conn)
}

pub fn installed_apps(db_path: &Path) -> AppResult<InstalledApps> {
    #[cfg(target_os = "windows")]
    let apps = windows_apps(db_path)?;
    #[cfg(target_os = "macos")]
    let apps = macos_apps(db_path)?;
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let apps: Vec<InstalledApp> = Vec::new();

    let mut apps = dedup(apps);
    apps.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)));

    let system_apps: Vec<InstalledApp> = apps.iter().filter(|a| a.system).cloned().collect();
    let user_apps: Vec<InstalledApp> = apps.iter().filter(|a| !a.system).cloned().collect();
    let system_bytes = system_apps.iter().map(|a| a.bytes).sum();
    let user_bytes = user_apps.iter().map(|a| a.bytes).sum();
    let estimated_count = apps
        .iter()
        .filter(|a| a.size_source == "installer estimate")
        .count() as i64;

    let platform = std::env::consts::OS.to_string();
    let notes = platform_notes(&platform, estimated_count);

    Ok(InstalledApps {
        system_count: system_apps.len() as i64,
        user_count: user_apps.len() as i64,
        system_bytes,
        user_bytes,
        system_apps,
        user_apps,
        estimated_count,
        platform,
        notes,
    })
}

fn platform_notes(platform: &str, estimated_count: i64) -> Vec<String> {
    let mut notes = Vec::new();
    match platform {
        "windows" => {
            notes.push(
                "System apps are marked by Windows as system components or installed inside Windows/ProgramData. Everything else is listed as an app you installed."
                    .to_string(),
            );
            if estimated_count > 0 {
                notes.push(format!(
                    "{estimated_count} {} no install folder in Windows, so the installer's own size estimate is shown instead of a measured size.",
                    if estimated_count == 1 { "app has" } else { "apps have" }
                ));
            }
        }
        "macos" => {
            notes.push(
                "Apps in /System are bundled with macOS; apps in /Applications are shown as installed by you."
                    .to_string(),
            );
        }
        _ => notes.push("Installed app detection is not available on this platform.".to_string()),
    }
    notes
}

fn dedup(apps: Vec<InstalledApp>) -> Vec<InstalledApp> {
    let mut seen = HashSet::new();
    apps.into_iter()
        .filter(|app| {
            let key = format!(
                "{}|{}",
                app.name.to_ascii_lowercase(),
                app.install_location.to_ascii_lowercase()
            );
            seen.insert(key)
        })
        .collect()
}

/// Trim quotes and trailing separators so the value can be used as a path prefix.
fn normalize_location(raw: &str) -> String {
    let mut value = raw.trim().trim_matches('"').trim().to_string();
    while value.ends_with('\\') || value.ends_with('/') {
        value.pop();
    }
    value
}

/// Sum the indexed files under an install folder. A `LIKE` prefix with a bound
/// parameter scans the whole table on SQLite; an explicit range uses the unique
/// index on `files.path` and stays fast on a multi-million-row index.
fn indexed_size(conn: &Connection, location: &str, separator: char) -> AppResult<(i64, i64)> {
    let normalized = normalize_location(location);
    if normalized.is_empty() {
        return Ok((0, 0));
    }
    let mut prefix = normalized;
    prefix.push(separator);
    let high = format!("{prefix}\u{10ffff}");

    let (bytes, count): (i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(size),0), COUNT(*) FROM files WHERE path >= ?1 AND path < ?2",
        rusqlite::params![prefix, high],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok((bytes, count))
}

#[cfg(target_os = "windows")]
struct RawApp {
    name: String,
    publisher: String,
    version: String,
    location: String,
    system_component: bool,
    estimated_kb: i64,
}

#[cfg(target_os = "windows")]
fn reg_string(key: &winreg::RegKey, name: &str) -> Option<String> {
    key.get_value::<String, _>(name)
        .ok()
        .map(|v| v.trim().trim_matches('"').trim().to_string())
        .filter(|v| !v.is_empty())
}

#[cfg(target_os = "windows")]
fn reg_dword(key: &winreg::RegKey, name: &str) -> Option<u32> {
    key.get_value::<u32, _>(name).ok()
}

#[cfg(target_os = "windows")]
fn env_folder(key: &str, fallback: &str) -> String {
    let mut value = std::env::var(key)
        .unwrap_or_else(|_| fallback.to_string())
        .to_ascii_lowercase();
    while value.ends_with('\\') {
        value.pop();
    }
    value.push('\\');
    value
}

#[cfg(target_os = "windows")]
fn classify_system(raw: &RawApp, location: &str) -> (bool, String, f64) {
    let lower = location.to_ascii_lowercase();
    let windows = env_folder("WINDIR", r"C:\Windows");
    let program_data = env_folder("PROGRAMDATA", r"C:\ProgramData");

    if raw.system_component {
        return (
            true,
            "Windows marks this as a system component".into(),
            0.99,
        );
    }
    if !lower.is_empty() && lower.starts_with(&windows) {
        return (
            true,
            format!("Installed under {location} (Windows folder)"),
            0.97,
        );
    }
    if lower.contains("\\windowsapps") {
        return (
            true,
            format!("Installed under {location} (Windows app)"),
            0.95,
        );
    }
    if !lower.is_empty() && lower.starts_with(&program_data) {
        return (
            true,
            format!("Installed under {location} (shared system data)"),
            0.9,
        );
    }
    let microsoft = raw.publisher.to_ascii_lowercase().contains("microsoft");
    let microsoft_program_files = lower.starts_with(r"c:\program files\microsoft")
        || lower.starts_with(r"c:\program files (x86)\microsoft");
    if microsoft && microsoft_program_files {
        return (
            true,
            format!("Microsoft app installed under {location}"),
            0.75,
        );
    }
    if location.is_empty() {
        (false, "No install folder recorded in Windows".into(), 0.6)
    } else {
        (false, format!("Installed under {location}"), 0.9)
    }
}

#[cfg(target_os = "windows")]
fn windows_apps(db_path: &Path) -> AppResult<Vec<InstalledApp>> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::{RegKey, HKEY};

    let hives: &[(HKEY, &str)] = &[
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            HKEY_CURRENT_USER,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
    ];

    let mut raw_apps = Vec::new();
    for (hive, path) in hives {
        let base = RegKey::predef(*hive);
        let uninstall = match base.open_subkey_with_flags(path, KEY_READ) {
            Ok(key) => key,
            Err(_) => continue,
        };
        for subkey in uninstall.enum_keys().flatten() {
            let item = match uninstall.open_subkey(&subkey) {
                Ok(key) => key,
                Err(_) => continue,
            };
            let name = match reg_string(&item, "DisplayName") {
                Some(value) => value,
                None => continue,
            };
            raw_apps.push(RawApp {
                name,
                publisher: reg_string(&item, "Publisher").unwrap_or_default(),
                version: reg_string(&item, "DisplayVersion").unwrap_or_default(),
                location: reg_string(&item, "InstallLocation").unwrap_or_default(),
                system_component: reg_dword(&item, "SystemComponent").unwrap_or(0) == 1,
                estimated_kb: reg_dword(&item, "EstimatedSize").unwrap_or(0) as i64,
            });
        }
    }

    let conn = read_only_connection(db_path);
    let mut apps = Vec::with_capacity(raw_apps.len());
    for raw in raw_apps {
        let location = normalize_location(&raw.location);
        let (indexed_bytes, file_count) = match (conn.as_ref(), location.is_empty()) {
            (Some(conn), false) => indexed_size(conn, &location, '\\').unwrap_or((0, 0)),
            _ => (0, 0),
        };
        let (bytes, size_source) = if indexed_bytes > 0 {
            (indexed_bytes, "local index")
        } else if raw.estimated_kb > 0 {
            (raw.estimated_kb.saturating_mul(1024), "installer estimate")
        } else {
            (0, "unknown")
        };
        let (system, evidence, confidence) = classify_system(&raw, &location);
        apps.push(InstalledApp {
            name: raw.name,
            publisher: raw.publisher,
            version: raw.version,
            install_location: location,
            bytes,
            file_count,
            system,
            size_source: size_source.into(),
            evidence,
            confidence,
        });
    }
    Ok(apps)
}

#[cfg(target_os = "macos")]
fn macos_apps(db_path: &Path) -> AppResult<Vec<InstalledApp>> {
    let conn = read_only_connection(db_path);
    let locations: &[(&str, bool, &str)] = &[
        ("/Applications", false, "Installed in /Applications"),
        ("/System/Applications", true, "Bundled with macOS"),
        ("/System/Library/CoreServices", true, "macOS system service"),
    ];

    let mut apps = Vec::new();
    for (directory, system, evidence) in locations {
        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("app") {
                continue;
            }
            let location = path.to_string_lossy().to_string();
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| location.clone());
            let (bytes, file_count) = conn
                .as_ref()
                .map(|conn| indexed_size(conn, &location, '/').unwrap_or((0, 0)))
                .unwrap_or((0, 0));
            apps.push(InstalledApp {
                name,
                publisher: String::new(),
                version: String::new(),
                install_location: location,
                bytes,
                file_count,
                system: *system,
                size_source: if bytes > 0 {
                    "local index".into()
                } else {
                    "unknown".into()
                },
                evidence: (*evidence).into(),
                confidence: 0.9,
            });
        }
    }
    Ok(apps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_quirky_registry_paths() {
        assert_eq!(
            normalize_location(r#" "C:\Program Files\Foo\" "#),
            r"C:\Program Files\Foo"
        );
        assert_eq!(
            normalize_location("/Applications/Foo.app/"),
            "/Applications/Foo.app"
        );
        assert_eq!(normalize_location("  "), "");
    }

    #[test]
    fn indexed_size_uses_an_indexed_range_and_does_not_leak_to_sibling_folders() {
        let conn = crate::db::open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO roots(id,path,label,kind,created_at) VALUES (1,'C:\\apps','apps','folder',0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO files(id,root_id,path,parent_path,name,ext,size,category,indexed_at,last_seen_at)
             VALUES (1,1,'C:\\apps\\Foo\\a.dll','C:\\apps\\Foo','a.dll','dll',100,'other',0,0),
                    (2,1,'C:\\apps\\FooBar\\b.dll','C:\\apps\\FooBar','b.dll','dll',500,'other',0,0)",
            [],
        )
        .unwrap();

        let (bytes, count) = indexed_size(&conn, r"C:\apps\Foo", '\\').unwrap();
        assert_eq!(bytes, 100);
        assert_eq!(count, 1);
    }
}
