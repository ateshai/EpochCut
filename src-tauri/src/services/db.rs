use std::path::Path;

use crate::error::AiPipelineError;
use crate::models::ingestion::{ExtractedFrame, VideoMetadata};
use crate::models::transcription::{SearchHit, TranscriptionSegment};
use rusqlite::{Connection, params};

/// Initialize project database with all required tables
/// Creates:
/// - project_meta: Video metadata storage
/// - transcription_segments: Timestamped transcription segments
/// - transcription_fts: FTS5 virtual table for full-text search
/// - frame_catalog: Extracted frames catalog
/// - Associated indexes for time-based queries
pub fn init_project_db(db_path: &Path) -> Result<Connection, AiPipelineError> {
    let conn = Connection::open(db_path)?;

    // Create project_meta table
    conn.execute_batch(include_str!("../sql/init_project_db.sql"))?;

    Ok(conn)
}

/// Save video metadata to the database
pub fn save_metadata(conn: &Connection, meta: &VideoMetadata) -> Result<(), AiPipelineError> {
    let file_path = meta.file_path.to_str()
        .ok_or_else(|| AiPipelineError::Database(rusqlite::Error::ToSqlConversionFailure(
            "Invalid file path".into()
        )))?
        .to_string();
    let file_name = meta.file_name.clone();
    let duration_seconds = meta.duration_seconds;
    let width = meta.width as i64;
    let height = meta.height as i64;
    let fps = meta.fps;
    let video_codec = meta.video_codec.clone();
    let audio_codec = meta.audio_codec.clone();

    conn.execute(
        "INSERT INTO project_meta (job_id, file_path, file_name, duration_seconds, width, height, fps, video_codec, audio_codec)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            meta.file_name.clone(), // job_id is set to file_name for now
            file_path,
            file_name,
            duration_seconds,
            width,
            height,
            fps,
            video_codec,
            audio_codec,
        ],
    )?;

    Ok(())
}

/// Save extracted frames to the frame catalog
pub fn save_frames(
    conn: &Connection,
    job_id: &str,
    frames: &[ExtractedFrame],
) -> Result<(), AiPipelineError> {
    for frame in frames {
        let image_rel_path = frame.image_path.to_str()
            .ok_or_else(|| AiPipelineError::Database(rusqlite::Error::ToSqlConversionFailure(
                "Invalid image path".into()
            )))?
            .to_string();

        conn.execute(
            "INSERT INTO frame_catalog (job_id, frame_index, timestamp_seconds, image_rel_path, width, height)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                job_id,
                frame.frame_index as i64,
                frame.timestamp_seconds,
                image_rel_path,
                frame.width as i64,
                frame.height as i64,
            ],
        )?;
    }

    Ok(())
}

/// Save transcription segments to the database
/// Also maintains FTS5 index by inserting into transcription_fts
pub fn save_segments(
    conn: &Connection,
    job_id: &str,
    segments: &[TranscriptionSegment],
) -> Result<(), AiPipelineError> {
    for segment in segments {
        conn.execute(
            "INSERT INTO transcription_segments (job_id, start_ms, end_ms, text, confidence)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                job_id,
                segment.start_ms as i64,
                segment.end_ms as i64,
                segment.text,
                segment.confidence,
            ],
        )?;

        let rowid = conn.last_insert_rowid();

        // Insert into FTS5 virtual table
        conn.execute(
            "INSERT INTO transcription_fts (rowid, text) VALUES (?1, ?2)",
            params![rowid, segment.text],
        )?;
    }

    Ok(())
}

/// Search transcription text using FTS5
/// Returns segments matched with nearest frame timestamp
pub fn search_text(conn: &Connection, query: &str) -> Result<Vec<SearchHit>, AiPipelineError> {
    // First, get matched segments from FTS5
    let mut stmt = conn.prepare(
        "SELECT ts.id, ts.start_ms, ts.end_ms, ts.text
         FROM transcription_segments ts
         INNER JOIN transcription_fts fts ON ts.id = fts.rowid
         WHERE fts.text MATCH ?1
         ORDER BY ts.start_ms"
    )?;

    let mut rows = stmt.query(params![query])?;

    let mut hits = Vec::new();

    while let Some(row) = rows.next()? {
        let segment_id: i64 = row.get(0)?;
        let start_ms: u64 = row.get(1)?;
        let end_ms: u64 = row.get(2)?;
        let text: String = row.get(3)?;

        // Find nearest frame for this segment
        let matched_frame_path: Option<String> = conn.query_row(
            "SELECT image_rel_path
             FROM frame_catalog
             WHERE job_id = (
                 SELECT job_id FROM transcription_segments WHERE id = ?1
             )
             AND timestamp_seconds * 1000 <= ?2
             ORDER BY timestamp_seconds DESC
             LIMIT 1",
            params![segment_id, start_ms as f64],
            |row| row.get(0),
        ).ok();

        hits.push(SearchHit {
            segment_id,
            start_ms,
            end_ms,
            text,
            matched_frame_path,
        });
    }

    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ingestion::VideoMetadata;
    use std::path::PathBuf;

    #[test]
    fn test_init_project_db_in_memory() {
        let conn = Connection::open_in_memory().expect("Failed to create in-memory database");

        // Execute DDL statements directly
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS project_meta (
                job_id TEXT PRIMARY KEY,
                file_path TEXT NOT NULL,
                file_name TEXT NOT NULL,
                duration_seconds REAL NOT NULL,
                width INTEGER NOT NULL,
                height INTEGER NOT NULL,
                fps REAL NOT NULL,
                video_codec TEXT NOT NULL,
                audio_codec TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS transcription_segments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                job_id TEXT NOT NULL,
                start_ms INTEGER NOT NULL,
                end_ms INTEGER NOT NULL,
                text TEXT NOT NULL,
                confidence REAL DEFAULT 1.0,
                FOREIGN KEY(job_id) REFERENCES project_meta(job_id)
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS transcription_fts USING fts5(
                text,
                content='transcription_segments',
                content_rowid='id'
            );

            CREATE TABLE IF NOT EXISTS frame_catalog (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                job_id TEXT NOT NULL,
                frame_index INTEGER NOT NULL,
                timestamp_seconds REAL NOT NULL,
                image_rel_path TEXT NOT NULL,
                width INTEGER NOT NULL,
                height INTEGER NOT NULL,
                FOREIGN KEY(job_id) REFERENCES project_meta(job_id)
            );

            CREATE INDEX IF NOT EXISTS idx_segments_time ON transcription_segments(start_ms, end_ms);
            CREATE INDEX IF NOT EXISTS idx_frames_time ON frame_catalog(timestamp_seconds);"
        ).expect("Failed to execute DDL");

        // Verify tables exist
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('project_meta', 'transcription_segments', 'transcription_fts', 'frame_catalog')",
            [], 
            |row| row.get(0)
        ).expect("Failed to count tables");

        assert_eq!(count, 4);
    }

    #[test]
    fn test_save_metadata() {
        let conn = Connection::open_in_memory().expect("Failed to create in-memory database");

        // Create table first
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS project_meta (
                job_id TEXT PRIMARY KEY,
                file_path TEXT NOT NULL,
                file_name TEXT NOT NULL,
                duration_seconds REAL NOT NULL,
                width INTEGER NOT NULL,
                height INTEGER NOT NULL,
                fps REAL NOT NULL,
                video_codec TEXT NOT NULL,
                audio_codec TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )"
        ).expect("Failed to create table");

        let metadata = VideoMetadata {
            file_path: PathBuf::from("/test/video.mp4"),
            file_name: "test_video.mp4".to_string(),
            duration_seconds: 120.5,
            width: 1920,
            height: 1080,
            fps: 30.0,
            video_codec: "h264".to_string(),
            audio_codec: Some("aac".to_string()),
            file_size_bytes: 1024000,
        };

        save_metadata(&conn, &metadata).expect("Failed to save metadata");

        let saved: String = conn.query_row(
            "SELECT file_name FROM project_meta WHERE job_id = ?1",
            params!["test_video.mp4"],
            |row| row.get(0)
        ).expect("Failed to query metadata");

        assert_eq!(saved, "test_video.mp4");
    }

    #[test]
    fn test_save_segments_and_fts_search() {
        let conn = Connection::open_in_memory().expect("Failed to create in-memory database");

        // Create tables
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS project_meta (
                job_id TEXT PRIMARY KEY,
                file_path TEXT NOT NULL,
                file_name TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS transcription_segments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                job_id TEXT NOT NULL,
                start_ms INTEGER NOT NULL,
                end_ms INTEGER NOT NULL,
                text TEXT NOT NULL,
                confidence REAL DEFAULT 1.0
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS transcription_fts USING fts5(
                text,
                content='transcription_segments',
                content_rowid='id'
            )"
        ).expect("Failed to create tables");

        // Insert project_meta first
        conn.execute(
            "INSERT INTO project_meta (job_id, file_path, file_name) VALUES (?1, ?2, ?3)",
            params!["test_job", "/test/video.mp4", "test_video.mp4"]
        ).expect("Failed to insert project_meta");

        let segments = vec![
            TranscriptionSegment {
                id: None,
                start_ms: 0,
                end_ms: 5000,
                text: "Hello world this is a test".to_string(),
                confidence: 0.95,
            },
            TranscriptionSegment {
                id: None,
                start_ms: 5000,
                end_ms: 10000,
                text: "Another segment with different words".to_string(),
                confidence: 0.92,
            },
        ];

        save_segments(&conn, "test_job", &segments).expect("Failed to save segments");

        // Verify segments were saved
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM transcription_segments WHERE job_id = ?1",
            params!["test_job"],
            |row| row.get(0)
        ).expect("Failed to count segments");

        assert_eq!(count, 2);

        // Test FTS search
        let hits = search_text(&conn, "Hello").expect("Failed to search");
        assert_eq!(hits.len(), 1);
        assert!(hits[0].text.contains("Hello"));

        let hits = search_text(&conn, "different").expect("Failed to search");
        assert_eq!(hits.len(), 1);
        assert!(hits[0].text.contains("different"));

        let hits = search_text(&conn, "this is").expect("Failed to search");
        assert_eq!(hits.len(), 1);
        assert!(hits[0].text.contains("this is"));

        let hits = search_text(&conn, "world").expect("Failed to search");
        assert_eq!(hits.len(), 1);
        assert!(hits[0].text.contains("world"));
    }

    #[test]
    fn test_save_frames() {
        let conn = Connection::open_in_memory().expect("Failed to create in-memory database");

        // Create tables
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS project_meta (
                job_id TEXT PRIMARY KEY,
                file_path TEXT NOT NULL,
                file_name TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS frame_catalog (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                job_id TEXT NOT NULL,
                frame_index INTEGER NOT NULL,
                timestamp_seconds REAL NOT NULL,
                image_rel_path TEXT NOT NULL,
                width INTEGER NOT NULL,
                height INTEGER NOT NULL,
                FOREIGN KEY(job_id) REFERENCES project_meta(job_id)
            )"
        ).expect("Failed to create tables");

        // Insert project_meta first
        conn.execute(
            "INSERT INTO project_meta (job_id, file_path, file_name) VALUES (?1, ?2, ?3)",
            params!["test_job", "/test/video.mp4", "test_video.mp4"]
        ).expect("Failed to insert project_meta");

        let frames = vec![
            ExtractedFrame {
                frame_index: 0,
                timestamp_seconds: 0.0,
                image_path: PathBuf::from("/test/frames/frame_000000.jpg"),
                width: 1920,
                height: 1080,
            },
            ExtractedFrame {
                frame_index: 1,
                timestamp_seconds: 4.0,
                image_path: PathBuf::from("/test/frames/frame_000001.jpg"),
                width: 1920,
                height: 1080,
            },
        ];

        save_frames(&conn, "test_job", &frames).expect("Failed to save frames");

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM frame_catalog WHERE job_id = ?1",
            params!["test_job"],
            |row| row.get(0)
        ).expect("Failed to count frames");

        assert_eq!(count, 2);
    }
}
