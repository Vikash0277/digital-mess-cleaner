//! Media cleanup overview: screenshots, photos, video and audio.

use crate::error::AppResult;
use crate::models::*;
use crate::repo;
use chrono::{TimeZone, Utc};
use rusqlite::Connection;
use std::collections::BTreeMap;

fn columns() -> String {
    repo::FILE_COLUMNS
        .split(',')
        .map(|c| format!("f.{}", c.trim()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn category_totals(conn: &Connection, category: &str) -> AppResult<(i64, i64)> {
    Ok(conn.query_row(
        "SELECT count(*), COALESCE(sum(size),0) FROM files WHERE category=?1",
        [category],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?)
}

fn largest_in(conn: &Connection, category: &str, limit: i64) -> AppResult<Vec<FileEntry>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM files f WHERE f.category=?1 ORDER BY f.size DESC LIMIT ?2",
        columns()
    ))?;
    let rows = stmt.query_map(rusqlite::params![category, limit], repo::row_to_file)?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn overview(conn: &Connection) -> AppResult<MediaOverview> {
    let (image_count, image_bytes) = category_totals(conn, "images")?;
    let (video_count, video_bytes) = category_totals(conn, "video")?;
    let (audio_count, audio_bytes) = category_totals(conn, "audio")?;

    let screenshot_sql = format!(
        "SELECT {} FROM files f WHERE f.category='images'
         AND (lower(f.name) LIKE '%screenshot%' OR lower(f.name) LIKE '%screen shot%' OR lower(f.name) LIKE '%snip%')
         ORDER BY f.mtime DESC LIMIT 500",
        columns()
    );
    let mut stmt = conn.prepare(&screenshot_sql)?;
    let screenshots: Vec<FileEntry> = stmt
        .query_map([], repo::row_to_file)?
        .filter_map(Result::ok)
        .collect();
    let screenshot_bytes: i64 = screenshots.iter().map(|f| f.size).sum();

    let mut buckets: BTreeMap<String, Vec<FileEntry>> = BTreeMap::new();
    for file in screenshots {
        let key = Utc
            .timestamp_opt(file.mtime, 0)
            .single()
            .map(|d| d.format("%Y-%m").to_string())
            .unwrap_or_else(|| "Unknown".into());
        buckets.entry(key).or_default().push(file);
    }
    let screenshot_buckets: Vec<MediaBucket> = buckets
        .into_iter()
        .rev()
        .take(24)
        .map(|(key, files)| MediaBucket {
            key,
            count: files.len() as i64,
            bytes: files.iter().map(|f| f.size).sum(),
            files,
        })
        .collect();

    let mut notes = Vec::new();
    if image_count == 0 && video_count == 0 && audio_count == 0 {
        notes.push("No media indexed yet. Add a folder and run a scan.".into());
    } else {
        notes.push("Similar photos are compared by perceptual hash. They are shown as similar, never as exact.".into());
        notes.push("Video and audio duplicate detection in this build is byte-identical only; re-encoded copies need FFmpeg.".into());
    }

    Ok(MediaOverview {
        image_count,
        image_bytes,
        screenshot_count: screenshot_buckets.iter().map(|b| b.count).sum(),
        screenshot_bytes,
        video_count,
        video_bytes,
        audio_count,
        audio_bytes,
        screenshot_buckets,
        largest_videos: largest_in(conn, "video", 25)?,
        largest_audio: largest_in(conn, "audio", 25)?,
        largest_images: largest_in(conn, "images", 25)?,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn overview_totals() {
        let conn = db::open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO roots(id,path,label,kind,created_at) VALUES (1,'x','x','folder',0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO files(id,root_id,path,parent_path,name,size,mtime,category,indexed_at,last_seen_at)
             VALUES (1,1,'x/Screenshot 1.png','x','Screenshot 1.png',100,0,'images',0,0),
                    (2,1,'x/a.mp4','x','a.mp4',200,0,'video',0,0)",
            [],
        )
        .unwrap();
        let o = overview(&conn).unwrap();
        assert_eq!(o.image_count, 1);
        assert_eq!(o.screenshot_count, 1);
        assert_eq!(o.video_bytes, 200);
    }
}
