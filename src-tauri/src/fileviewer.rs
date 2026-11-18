//! Safe, read-only file viewing commands.
//!
//! The webview only sends an indexed file id. Every command looks the path up
//! in the local index, checks it still belongs to an enabled folder the user
//! chose, and then acts in Rust. No filesystem scope is granted to the webview,
//! no content leaves the device, and nothing is edited or logged.

use crate::error::{AppError, AppResult};
use crate::models::{FileDetails, FileEntry, FilePreview};
use crate::repo;
use base64::Engine as _;
use rusqlite::Connection;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_TEXT_BYTES: i64 = 1024 * 1024;
const MAX_IMAGE_BYTES: i64 = 8 * 1024 * 1024;
const MAX_PDF_BYTES: i64 = 12 * 1024 * 1024;

/// Normalise for containment checks. Windows paths are case-insensitive; the
/// separator is normalised too so a scan that stored `/` still works.
fn path_key(path: &Path) -> String {
    let separator = std::path::MAIN_SEPARATOR.to_string();
    let mut key = path
        .to_string_lossy()
        .replace(['\\', '/'], &separator)
        .to_lowercase();
    while key.len() > 1 && key.ends_with(&separator) {
        key.pop();
    }
    key
}

/// True when `child` is `root` or sits underneath it. Both sides must already
/// be canonical paths, so symlinks cannot escape the chosen folder.
fn is_within(child: &Path, root: &Path) -> bool {
    let child = path_key(child);
    let root = path_key(root);
    let separator = std::path::MAIN_SEPARATOR.to_string();
    child == root || child.starts_with(&format!("{root}{separator}"))
}

fn root_of(conn: &Connection, file: &FileEntry) -> AppResult<(PathBuf, String)> {
    conn.query_row(
        "SELECT path, label FROM roots WHERE id = ?1 AND enabled = 1",
        [file.root_id],
        |row| {
            Ok((
                PathBuf::from(row.get::<_, String>(0)?),
                row.get::<_, String>(1)?,
            ))
        },
    )
    .map_err(|_| {
        AppError::Permission(
            "This file belongs to a folder that is no longer enabled in Digital Mess Cleaner."
                .into(),
        )
    })
}

/// Load an indexed file and resolve it to a real file inside a chosen root.
fn resolve_indexed_file(conn: &Connection, file_id: i64) -> AppResult<(FileEntry, PathBuf)> {
    let file = repo::load_file(conn, file_id)?;
    let (root_path, _) = root_of(conn, &file)?;

    let root = dunce::canonicalize(&root_path).map_err(|_| {
        AppError::NotFound("The folder containing this file is no longer available.".into())
    })?;
    let path = dunce::canonicalize(PathBuf::from(&file.path))?;

    if !is_within(&path, &root) {
        return Err(AppError::Permission(
            "This file points outside the folders you chose, so it was not opened.".into(),
        ));
    }
    if !path.is_file() {
        return Err(AppError::Invalid(
            "This is no longer a file. Scan its folder again.".into(),
        ));
    }
    Ok((file, path))
}

pub fn open_file(conn: &Connection, file_id: i64) -> AppResult<()> {
    let (_, path) = resolve_indexed_file(conn, file_id)?;
    tauri_plugin_opener::open_path(&path, None::<&str>).map_err(|error| {
        AppError::Io(format!(
            "The default app could not open this file. ({error})"
        ))
    })
}

pub fn reveal_file(conn: &Connection, file_id: i64) -> AppResult<()> {
    let (_, path) = resolve_indexed_file(conn, file_id)?;
    tauri_plugin_opener::reveal_item_in_dir(&path).map_err(|error| {
        AppError::Io(format!(
            "File Explorer could not reveal this file. ({error})"
        ))
    })
}

pub fn file_details(conn: &Connection, file_id: i64) -> AppResult<FileDetails> {
    let file = repo::load_file(conn, file_id)?;
    let (root_path, root_label) = root_of(conn, &file)?;
    let metadata = fs::metadata(PathBuf::from(&file.path)).ok();
    let exists = metadata.as_ref().map(|m| m.is_file()).unwrap_or(false);
    let current_size = metadata.as_ref().map(|m| m.len() as i64).unwrap_or(0);
    let current_mtime = metadata
        .and_then(|m| m.modified().ok())
        .and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs() as i64)
        })
        .unwrap_or(0);

    Ok(FileDetails {
        file,
        exists,
        current_size,
        current_mtime,
        root_label,
        root_path: root_path.to_string_lossy().to_string(),
    })
}

fn preview_limit(file: &FileEntry) -> (&'static str, i64) {
    let ext = file.ext.to_ascii_lowercase();
    match ext.as_str() {
        "txt" | "text" | "md" | "markdown" | "log" | "csv" | "tsv" | "json" | "xml" | "yaml"
        | "yml" | "ini" | "toml" | "rtf" | "js" | "jsx" | "ts" | "tsx" | "rs" | "py" | "java"
        | "c" | "h" | "cpp" | "hpp" | "go" | "rb" | "php" | "sh" | "sql" => {
            ("text", MAX_TEXT_BYTES)
        }
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "avif" => ("image", MAX_IMAGE_BYTES),
        "pdf" => ("pdf", MAX_PDF_BYTES),
        _ if file.mime.starts_with("text/") => ("text", MAX_TEXT_BYTES),
        _ if file.mime.starts_with("image/") && file.mime != "image/svg+xml" => {
            ("image", MAX_IMAGE_BYTES)
        }
        _ => ("unsupported", 0),
    }
}

/// Returns `None` when the file is larger than the preview cap. The cap also
/// bounds how many bytes are ever read from disk.
fn read_bounded(path: &Path, limit: i64) -> AppResult<Option<Vec<u8>>> {
    let file = File::open(path)?;
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() as i64 > limit {
        Ok(None)
    } else {
        Ok(Some(bytes))
    }
}

fn empty_preview(kind: &str, reason: &str, limit: i64) -> FilePreview {
    FilePreview {
        kind: kind.into(),
        mime: String::new(),
        text: None,
        data: None,
        reason: reason.into(),
        truncated: false,
        limit_bytes: limit,
    }
}

pub fn file_preview(conn: &Connection, file_id: i64) -> AppResult<FilePreview> {
    let (file, path) = resolve_indexed_file(conn, file_id)?;
    let (kind, limit) = preview_limit(&file);
    if kind == "unsupported" {
        return Ok(empty_preview(
            "unsupported",
            "There is no safe in-app preview for this file type. Open it with its default app instead.",
            limit,
        ));
    }

    let bytes = match read_bounded(&path, limit)? {
        Some(bytes) => bytes,
        None => {
            let reason = format!(
                "This file is too large to preview inside Digital Mess Cleaner (limit {}). Open it with its default app instead.",
                crate::format::bytes(limit)
            );
            return Ok(empty_preview("too_large", &reason, limit));
        }
    };

    match kind {
        "text" => Ok(FilePreview {
            kind: kind.into(),
            mime: file.mime.clone(),
            text: Some(String::from_utf8_lossy(&bytes).into_owned()),
            data: None,
            reason: String::new(),
            truncated: false,
            limit_bytes: limit,
        }),
        "image" | "pdf" => {
            let mime = if file.mime.is_empty() {
                crate::fsutil::mime_of(&path)
            } else {
                file.mime.clone()
            };
            Ok(FilePreview {
                kind: kind.into(),
                mime,
                text: None,
                data: Some(base64::engine::general_purpose::STANDARD.encode(bytes)),
                reason: String::new(),
                truncated: false,
                limit_bytes: limit,
            })
        }
        _ => Ok(empty_preview(
            "unsupported",
            "There is no safe in-app preview for this file type.",
            limit,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_containment_respects_separators_and_case() {
        let root = Path::new(r"C:\Users\Alex\Photos");
        let child = Path::new(r"C:\USERS\ALEX\PHOTOS\holiday\one.jpg");
        let sibling = Path::new(r"C:\Users\Alex\Photos 2\one.jpg");
        assert!(is_within(child, root));
        assert!(!is_within(sibling, root));
    }

    #[test]
    fn bounded_read_reports_oversized_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("small.txt");
        fs::write(&path, b"hello").unwrap();
        assert_eq!(read_bounded(&path, 10).unwrap(), Some(b"hello".to_vec()));

        let big = dir.path().join("big.txt");
        fs::write(&big, b"1234567890").unwrap();
        assert_eq!(read_bounded(&big, 9).unwrap(), None);
    }

    #[test]
    fn details_and_preview_use_only_the_index_and_a_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_string_lossy().to_string();
        let file_path = dir.path().join("note.txt");
        fs::write(&file_path, "local note").unwrap();

        let conn = crate::db::open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO roots(id,path,label,kind,created_at) VALUES (1,?1,'Notes','folder',0)",
            [&root],
        )
        .unwrap();
        let path = file_path.to_string_lossy().to_string();
        conn.execute(
            "INSERT INTO files(id,root_id,path,parent_path,name,ext,size,mtime,atime,category,mime,indexed_at,last_seen_at)
             VALUES (1,1,?1,?2,'note.txt','txt',10,0,0,'documents','text/plain',0,0)",
            rusqlite::params![path, root],
        )
        .unwrap();

        let details = file_details(&conn, 1).unwrap();
        assert!(details.exists);
        assert_eq!(details.current_size, 10);
        assert_eq!(details.root_label, "Notes");

        let preview = file_preview(&conn, 1).unwrap();
        assert_eq!(preview.kind, "text");
        assert_eq!(preview.text.as_deref(), Some("local note"));

        let binary = dir.path().join("image.bin");
        fs::write(&binary, b"binary").unwrap();
        let binary_path = binary.to_string_lossy().to_string();
        conn.execute(
            "INSERT INTO files(id,root_id,path,parent_path,name,ext,size,category,mime,indexed_at,last_seen_at)
             VALUES (2,1,?1,?2,'image.bin','bin',6,'other','application/octet-stream',0,0)",
            rusqlite::params![binary_path, root],
        )
        .unwrap();
        assert_eq!(file_preview(&conn, 2).unwrap().kind, "unsupported");
    }
}
