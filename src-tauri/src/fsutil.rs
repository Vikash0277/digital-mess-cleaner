//! Filesystem helpers: categories, hashing, perceptual hashes, text extraction,
//! safe names and protected locations.

use crate::error::{AppError, AppResult};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub fn ext_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

pub fn mime_of(path: &Path) -> String {
    mime_guess::from_path(path)
        .first_or_octet_stream()
        .essence_str()
        .to_string()
}

pub fn category_of(path: &Path) -> &'static str {
    match ext_of(path).as_str() {
        "pdf" | "doc" | "docx" | "txt" | "md" | "markdown" | "rtf" | "odt" | "pages" | "xls"
        | "xlsx" | "xlsm" | "csv" | "tsv" | "ods" | "ppt" | "pptx" | "key" | "epub" | "odp"
        | "vsd" | "vsdx" => "documents",
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "heic" | "heif"
        | "svg" | "avif" | "raw" | "cr2" | "nef" | "psd" => "images",
        "mp4" | "mov" | "mkv" | "avi" | "wmv" | "webm" | "m4v" | "flv" | "mpg" | "mpeg" | "3gp"
        | "mts" => "video",
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "wma" | "aiff" | "opus" => "audio",
        "zip" | "rar" | "7z" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "zst" | "cab" => "archives",
        "exe" | "msi" | "msix" | "pkg" | "dmg" | "deb" | "rpm" | "appimage" | "apk" | "iso" => {
            "installers"
        }
        _ => "other",
    }
}

pub fn category_label(category: &str) -> &'static str {
    match category {
        "documents" => "Documents",
        "images" => "Images",
        "video" => "Video",
        "audio" => "Audio",
        "archives" => "Archives",
        "installers" => "Installers",
        _ => "Other",
    }
}

pub fn is_image(path: &Path) -> bool {
    category_of(path) == "images" && ext_of(path) != "svg"
}

/// Full cryptographic hash of a file. Uses BLAKE3 (cryptographic, fast).
pub fn hash_file(path: &Path) -> AppResult<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::with_capacity(256 * 1024, file);
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let read = reader.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

/// Cheap hash over size + first/last 64 KiB. Used only to shortlist candidates;
/// exact duplicate claims always require the full hash.
pub fn partial_hash(path: &Path, size: u64) -> AppResult<String> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(&size.to_le_bytes());
    let mut buf = vec![0u8; 64 * 1024];
    let read = file.read(&mut buf)?;
    hasher.update(&buf[..read]);
    if size > 64 * 1024 {
        let start = size.saturating_sub(64 * 1024);
        file.seek(SeekFrom::Start(start))?;
        let read = file.read(&mut buf)?;
        hasher.update(&buf[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

/// 64-bit average perceptual hash (aHash) for similar-image grouping.
pub fn perceptual_hash(path: &Path) -> Option<(String, u32, u32)> {
    let img = image::open(path).ok()?;
    let width = img.width();
    let height = img.height();
    let small = img
        .resize_exact(8, 8, image::imageops::FilterType::Triangle)
        .to_luma8();
    let pixels: Vec<u8> = small.pixels().map(|p| p.0[0]).collect();
    if pixels.len() != 64 {
        return None;
    }
    let mean = pixels.iter().map(|v| *v as u32).sum::<u32>() / 64;
    let mut bits: u64 = 0;
    for (i, px) in pixels.iter().enumerate() {
        if (*px as u32) >= mean {
            bits |= 1u64 << i;
        }
    }
    Some((format!("{bits:016x}"), width, height))
}

pub fn hamming_hex(a: &str, b: &str) -> Option<u32> {
    let a = u64::from_str_radix(a, 16).ok()?;
    let b = u64::from_str_radix(b, 16).ok()?;
    Some((a ^ b).count_ones())
}

/// Text extraction for supported formats. Returns `None` when the format has no
/// text layer we can read locally (e.g. a scanned image without OCR).
pub fn extract_text(path: &Path, max_chars: usize) -> AppResult<Option<String>> {
    let ext = ext_of(path);
    let raw = match ext.as_str() {
        "txt" | "md" | "markdown" | "csv" | "tsv" | "json" | "log" | "xml" | "yaml" | "yml"
        | "toml" | "ini" | "cfg" | "rtf" => read_text_file(path, 2 * 1024 * 1024)?,
        "html" | "htm" => strip_tags(&read_text_file(path, 2 * 1024 * 1024)?),
        "pdf" => extract_pdf(path)?,
        "docx" => extract_zip_xml(path, "word/document.xml")?,
        "pptx" => extract_zip_xml_many(path, "ppt/slides/slide")?,
        "xlsx" | "xlsm" => extract_zip_xml_many(path, "xl/worksheets/sheet")?,
        "odt" | "ods" | "odp" => extract_zip_xml(path, "content.xml")?,
        _ => return Ok(None),
    };
    let normalized = normalize_ws(&raw);
    if normalized.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(truncate_chars(&normalized, max_chars)))
}

fn read_text_file(path: &Path, max_bytes: usize) -> AppResult<String> {
    let mut file = File::open(path)?;
    let mut buf = vec![0u8; max_bytes];
    let read = file.read(&mut buf)?;
    buf.truncate(read);
    Ok(String::from_utf8_lossy(&buf).to_string())
}

fn extract_pdf(path: &Path) -> AppResult<String> {
    let path = path.to_path_buf();
    // `pdf-extract` can panic on malformed PDFs; never let that kill a scan.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pdf_extract::extract_text(&path)
    }));
    match result {
        Ok(Ok(text)) => Ok(text),
        Ok(Err(e)) => Err(AppError::Io(format!("Could not read PDF: {e}"))),
        Err(_) => Err(AppError::Io("PDF could not be parsed safely".into())),
    }
}

fn open_zip(path: &Path) -> AppResult<zip::ZipArchive<File>> {
    let file = File::open(path)?;
    zip::ZipArchive::new(file).map_err(|e| AppError::Io(format!("Damaged archive: {e}")))
}

fn extract_zip_xml(path: &Path, member: &str) -> AppResult<String> {
    let mut archive = open_zip(path)?;
    let mut file = archive
        .by_name(member)
        .map_err(|e| AppError::Io(format!("Archive member missing: {e}")))?;
    let mut xml = String::new();
    file.read_to_string(&mut xml)?;
    Ok(strip_tags(&xml))
}

fn extract_zip_xml_many(path: &Path, prefix: &str) -> AppResult<String> {
    let mut archive = open_zip(path)?;
    let names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();
    let mut out = String::new();
    for name in names {
        if name.starts_with(prefix) && name.ends_with(".xml") {
            if let Ok(mut f) = archive.by_name(&name) {
                let mut xml = String::new();
                let _ = f.read_to_string(&mut xml);
                out.push_str(&strip_tags(&xml));
                out.push(' ');
            }
        }
    }
    Ok(out)
}

fn tag_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?is)<[^>]+>").expect("valid tag regex"))
}

fn strip_tags(input: &str) -> String {
    tag_regex().replace_all(input, " ").to_string()
}

pub fn normalize_ws(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_space = false;
    for ch in input.chars() {
        let c = if ch == '\u{00a0}' { ' ' } else { ch };
        if c.is_whitespace() {
            if !last_space {
                out.push(' ');
                last_space = true;
            }
        } else if !c.is_control() {
            out.push(c);
            last_space = false;
        }
    }
    out.trim().to_string()
}

pub fn truncate_chars(input: &str, max: usize) -> String {
    if input.chars().count() <= max {
        return input.to_string();
    }
    input.chars().take(max).collect()
}

pub fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Human-safe filename for the current OS. Keeps the extension, strips characters
/// Windows rejects, and avoids reserved device names.
pub fn sanitize_filename(name: &str, fallback: &str) -> String {
    let mut stem_ext: Vec<String> = Vec::new();
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if e.len() <= 12 && !s.is_empty() => (s.to_string(), e.to_ascii_lowercase()),
        _ => (name.to_string(), String::new()),
    };
    let mut clean = String::new();
    for ch in stem.chars() {
        if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
            clean.push('-');
        } else {
            clean.push(ch);
        }
    }
    let clean = clean.trim().trim_matches('.').trim().to_string();
    let mut clean = if clean.is_empty() {
        fallback.to_string()
    } else {
        clean
    };
    if clean.chars().count() > 120 {
        clean = clean.chars().take(120).collect();
    }
    let upper = clean.to_ascii_uppercase();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if reserved.contains(&upper.as_str()) {
        clean = format!("_{clean}");
    }
    if ext.is_empty() {
        stem_ext.push(clean);
    } else {
        stem_ext.push(clean);
        stem_ext.push(ext);
    }
    stem_ext.join(".")
}

/// Choose a non-existing destination in `dir`, adding " (2)", " (3)" when needed.
pub fn unique_destination(dir: &Path, desired_name: &str) -> PathBuf {
    let sanitized = sanitize_filename(desired_name, "file");
    let mut candidate = dir.join(&sanitized);
    if !candidate.exists() {
        return candidate;
    }
    let path = Path::new(&sanitized);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|s| s.to_str());
    for n in 2..10_000 {
        let name = match ext {
            Some(e) => format!("{stem} ({n}).{e}"),
            None => format!("{stem} ({n})"),
        };
        candidate = dir.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!(
        "{stem}-{}.{}",
        uuid::Uuid::new_v4(),
        ext.unwrap_or("bin")
    ))
}

pub fn protected_reason(path: &Path) -> Option<String> {
    let lower = normalize_for_compare(path).to_ascii_lowercase();
    let protected_prefixes = [
        "c:\\windows",
        "c:\\program files",
        "c:\\program files (x86)",
        "c:\\programdata\\microsoft",
        "c:\\$recycle.bin",
        "c:\\system volume information",
        "/system",
        "/usr",
        "/bin",
        "/sbin",
        "/private/system",
        "/library/apple",
        "/etc",
        "/var/db",
    ];
    for prefix in protected_prefixes {
        if lower == prefix
            || lower.starts_with(&format!("{prefix}\\"))
            || lower.starts_with(&format!("{prefix}/"))
        {
            return Some(format!(
                "This is a protected system location ({prefix}). Digital Mess Cleaner will not move or delete it."
            ));
        }
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(
        name.as_str(),
        "pagefile.sys"
            | "hiberfil.sys"
            | "swapfile.sys"
            | "bootmgr"
            | "ntldr"
            | "ntuser.dat"
            | "boot.ini"
    ) {
        return Some("This is a critical operating-system file.".to_string());
    }
    None
}

pub fn is_within(path: &Path, root: &Path) -> bool {
    let path = normalize_for_compare(path);
    let root = normalize_for_compare(root);
    path == root || path.starts_with(&format!("{root}{}", std::path::MAIN_SEPARATOR))
}

fn normalize_for_compare(path: &Path) -> String {
    let s = dunce::simplified(path)
        .to_string_lossy()
        .replace('/', std::path::MAIN_SEPARATOR_STR);
    let s = s.trim_end_matches(std::path::MAIN_SEPARATOR).to_string();
    if cfg!(any(target_os = "windows", target_os = "macos")) {
        s.to_lowercase()
    } else {
        s
    }
}

pub fn dir_size(path: &Path) -> (u64, u64) {
    let mut bytes = 0u64;
    let mut files = 0u64;
    let walker = walkdir::WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok);
    for entry in walker {
        if entry.file_type().is_file() {
            if let Ok(md) = entry.metadata() {
                bytes += md.len();
                files += 1;
            }
        }
    }
    (bytes, files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_are_stable() {
        assert_eq!(category_of(Path::new("a.PDF")), "documents");
        assert_eq!(category_of(Path::new("a.MOV")), "video");
        assert_eq!(category_of(Path::new("a.exe")), "installers");
        assert_eq!(category_of(Path::new("a.unknown")), "other");
    }

    #[test]
    fn hash_is_deterministic_and_partial_differs_for_different_content() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        std::fs::write(&a, b"hello world").unwrap();
        std::fs::write(&b, b"hello worlds").unwrap();
        assert_eq!(hash_file(&a).unwrap(), hash_file(&a).unwrap());
        assert_ne!(hash_file(&a).unwrap(), hash_file(&b).unwrap());
        let pa = partial_hash(&a, 11).unwrap();
        let pb = partial_hash(&b, 12).unwrap();
        assert_ne!(pa, pb);
    }

    #[test]
    fn sanitizes_windows_hostile_names() {
        assert_eq!(sanitize_filename("in:voice?.pdf", "file"), "in-voice-.pdf");
        assert_eq!(sanitize_filename("CON", "file"), "_CON");
        assert_eq!(sanitize_filename("", "fallback"), "fallback");
    }

    #[test]
    fn unique_destination_avoids_conflicts() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"x").unwrap();
        let dest = unique_destination(dir.path(), "a.txt");
        assert_eq!(dest.file_name().unwrap().to_string_lossy(), "a (2).txt");
    }

    #[test]
    fn protects_system_paths() {
        assert!(protected_reason(Path::new("C:/Windows/System32/drivers/etc/hosts")).is_some());
        assert!(protected_reason(Path::new("C:/Users/me/file.txt")).is_none());
    }

    #[test]
    fn hamming_distance_works() {
        assert_eq!(hamming_hex("0000000000000000", "0000000000000001"), Some(1));
        assert_eq!(
            hamming_hex("0000000000000000", "ffffffffffffffff"),
            Some(64)
        );
    }
}
