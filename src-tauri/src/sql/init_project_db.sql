-- Initialize project database schema
-- Creates project_meta, transcription_segments, transcription_fts, and frame_catalog tables

CREATE TABLE IF NOT EXISTS project_meta (
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
CREATE INDEX IF NOT EXISTS idx_frames_time ON frame_catalog(timestamp_seconds);
