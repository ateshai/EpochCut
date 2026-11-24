# SPEC-002: Local Speech Transcription & Semantic Vector Indexing

* **Status:** Draft / Ready for Implementation
* **Scope:** Backend (Tauri v2 / Rust Core), Audio AI Worker (Whisper), SQLite + sqlite-vec Storage
* **Dependencies:** SPEC-001 (Artifact paths & Job UUID)
* **Target Hardware:** Apple Silicon (Metal/NEON) & Windows (NVIDIA CUDA / RTX 4070 12GB)

---

## 1. System Overview & Context

Once SPEC-001 completes and outputs `audio/track_16k_mono.wav` (if an audio stream exists), this module performs:
1. **Local Audio Transcription:** Converting 16kHz WAV audio into timestamped text segments (`start_ms`, `end_ms`, `text`) using `whisper-large-v3-turbo` with hardware acceleration.
2. **Persistence & Full-Text/Vector Indexing:** Storing metadata, transcription segments, and extracted frames into an embedded SQLite database (`index.db`) within the project folder.
3. **Graceful Silence Tolerance:** If no audio was extracted in SPEC-001 (e.g., screen recording without microphone), transcription is skipped with a status flag (`transcription_status = "skipped_silent"`), leaving the pipeline open for Vision indexing.

```text
[SPEC-001 Output: track_16k_mono.wav]
                   │
                   ▼
       [Audio Present Check]
       ├── (No Audio) ──► Mark skipped & proceed to Frame Catalog
       └── (Audio Exists)
             │
             ▼
       [Whisper-large-v3-turbo Engine]
             │
             ▼
       [Timestamped Segments]
             │
             ▼
    [SQLite Persistence: index.db]
    ├── table: video_metadata
    ├── table: transcription_segments (FTS5 enabled)
    └── table: frame_catalog
```

---

## 2. Whisper Model Orchestration Strategy

To balance inference speed, memory safety, and cross-platform reliability on consumer hardware (12GB VRAM cap / Apple Silicon Unified Memory):

- **Model:** `whisper-large-v3-turbo` (or `distil-whisper-large-v3` as fallback)
- **Engine Execution:**
  - **macOS:** `whisper-rs` (built on `whisper.cpp` with Apple Metal framework)
  - **Windows:** `whisper.cpp` (CUDA backend) or local sidecar binary wrapping `faster-whisper` (CTranslate2).
- **Execution Mode:** Isolated asynchronous task that yields progress events to avoid blocking Tauri UI thread.

---

## 3. Database Schema (`<PROJECT_DIR>/index.db`)

Each project contains an isolated SQLite database using `rusqlite` with optional `sqlite-vec` extension.

```sql
-- 1. Video Metadata Table
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

-- 2. Transcription Segments Table
CREATE TABLE IF NOT EXISTS transcription_segments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id TEXT NOT NULL,
    start_ms INTEGER NOT NULL,
    end_ms INTEGER NOT NULL,
    text TEXT NOT NULL,
    confidence REAL DEFAULT 1.0,
    FOREIGN KEY(job_id) REFERENCES project_meta(job_id)
);

-- Full-Text Search Virtual Table for instant keyword matching
CREATE VIRTUAL TABLE IF NOT EXISTS transcription_fts USING fts5(
    text,
    content='transcription_segments',
    content_rowid='id'
);

-- 3. Frame Catalog Table
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
```

---

## 4. Core Domain Models (Rust & TypeScript)

### 4.1 Rust Models (`src-tauri/src/models/transcription.rs`)

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionSegment {
    pub id: Option<i64>,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionResult {
    pub job_id: String,
    pub language: String,
    pub duration_ms: u64,
    pub segments: Vec<TranscriptionSegment>,
    pub is_silent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "stage", content = "payload", rename_all = "camelCase")]
pub enum TranscriptionProgress {
    Starting { job_id: String },
    Transcribing { progress: f32, current_time_ms: u64, total_time_ms: u64 },
    IndexingDatabase { total_records: usize },
    Complete { result: TranscriptionResult },
    SkippedSilent { reason: String },
    Failed { error_message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub segment_id: i64,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub matched_frame_path: Option<String>,
}
```

### 4.2 TypeScript Types (`src/types/transcription.ts`)

```typescript
export interface TranscriptionSegment {
  id?: number;
  startMs: number;
  endMs: number;
  text: string;
  confidence: number;
}

export interface TranscriptionResult {
  jobId: string;
  language: string;
  durationMs: number;
  segments: TranscriptionSegment[];
  isSilent: boolean;
}

export type TranscriptionProgress =
  | { stage: "starting"; payload: { jobId: string } }
  | { stage: "transcribing"; payload: { progress: number; currentTimeMs: number; totalTimeMs: number } }
  | { stage: "indexingDatabase"; payload: { totalRecords: number } }
  | { stage: "complete"; payload: { result: TranscriptionResult } }
  | { stage: "skippedSilent"; payload: { reason: string } }
  | { stage: "failed"; payload: { errorMessage: string } };

export interface SearchHit {
  segmentId: number;
  startMs: number;
  endMs: number;
  text: string;
  matchedFramePath?: string;
}
```

---

## 5. Tauri IPC Command Interface

### 5.1 Rust Commands (`src-tauri/src/commands/transcription.rs`)

```rust
#[tauri::command]
pub async fn transcribe_project(
    app: tauri::AppHandle,
    job_id: String,
) -> Result<TranscriptionResult, String>;

#[tauri::command]
pub async fn search_transcription(
    app: tauri::AppHandle,
    job_id: String,
    query: String,
) -> Result<Vec<SearchHit>, String>;
```

---

## 6. Error Handling (`src-tauri/src/error.rs`)

Extend `IngestionError` or define `AiPipelineError`:

```rust
#[derive(thiserror::Error, Debug)]
pub enum AiPipelineError {
    #[error("Audio file missing for job: {0}")]
    AudioNotFound(String),
    #[error("Model weights not found at path: {0}")]
    ModelNotFound(String),
    #[error("Transcription inference failed: {0}")]
    InferenceFailed(String),
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

---

## 7. Acceptance Criteria

1. **Silent Fallback:** Processing a job without an audio track must immediately resolve `is_silent: true` without errors, preserving database records for frames.
2. **FTS Search:** Querying `search_transcription` with any keyword must return timestamped hits in $< 10\text{ ms}$ on local SQLite.
3. **Strict Resource Cleanup:** Whisper model context must unload or stay bounded to avoid memory leaks after transcription finishes.
4. **Build & Validation:** `cargo check` and `yarn tsc --noEmit` pass with 0 errors.