# SPEC-001: Core Domain Models, Tauri IPC Contracts & ffmpeg Ingestion Pipeline

- **Status:** Approved
- **Scope:** Backend (Tauri v2 / Rust Core), IPC Schema (TypeScript Types), ffmpeg Integration
- **Target OS:** Windows 11 (PowerShell/WSL2) & macOS (Darwin arm64)

---

## 1. System Context & Responsibilities

The primary responsibility of this module is to accept a local video file (`.mp4`, `.mkv`, `.mov`), extract technical metadata, demux a lightweight mono audio track for Whisper, and extract representative keyframes (fixed-interval fallbacks) for local Vision processing—without saturating disk I/O or VRAM.

```text
[Local Video File]
       │
       ▼
[Tauri Command: ingest_video]
       │
       ├─► 1. Probe Metadata via ffprobe (Duration, Resolution, FPS, Codec)
       ├─► 2. Demux Audio via ffmpeg (16kHz, mono, 16-bit WAV) -> storage/audio/
       └─► 3. Extract Keyframes via ffmpeg (1 frame per N seconds) -> storage/frames/
```

---

## 2. Core Domain Models (Rust & TypeScript)

### 2.1 Rust Structs (`src-tauri/src/models/ingestion.rs`)

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VideoMetadata {
    pub file_path: PathBuf,
    pub file_name: String,
    pub duration_seconds: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub video_codec: String,
    pub audio_codec: Option<String>,
    pub file_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedFrame {
    pub frame_index: usize,
    pub timestamp_seconds: f64,
    pub image_path: PathBuf,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IngestionResult {
    pub job_id: String,
    pub metadata: VideoMetadata,
    pub audio_track_path: PathBuf,
    pub frames: Vec<ExtractedFrame>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "stage", content = "payload", rename_all = "camelCase")]
pub enum IngestionProgress {
    Probing { progress: f32 },
    ExtractingAudio { progress: f32 },
    ExtractingFrames { progress: f32, current_frame: usize, total_frames: usize },
    Complete { result: IngestionResult },
    Failed { error_message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IngestionConfig {
    pub interval_seconds: f64,
    pub max_frame_resolution: u32,
}

impl Default for IngestionConfig {
    fn default() -> Self {
        Self {
            interval_seconds: 4.0,
            max_frame_resolution: 720,
        }
    }
}
```

### 2.2 TypeScript IPC Definitions (`src/types/ingestion.ts`)

```typescript
export interface VideoMetadata {
  filePath: string;
  fileName: string;
  durationSeconds: number;
  width: number;
  height: number;
  fps: number;
  videoCodec: string;
  audioCodec?: string;
  fileSizeBytes: number;
}

export interface ExtractedFrame {
  frameIndex: number;
  timestampSeconds: number;
  imagePath: string;
  width: number;
  height: number;
}

export interface IngestionResult {
  jobId: string;
  metadata: VideoMetadata;
  audioTrackPath: string;
  frames: ExtractedFrame[];
}

export type IngestionProgress =
  | { stage: "probing"; payload: { progress: number } }
  | { stage: "extractingAudio"; payload: { progress: number } }
  | { stage: "extractingFrames"; payload: { progress: number; currentFrame: number; totalFrames: number } }
  | { stage: "complete"; payload: { result: IngestionResult } }
  | { stage: "failed"; payload: { errorMessage: string } };

export interface IngestionConfig {
  intervalSeconds: number; // Default: 4.0
  maxFrameResolution: number; // Default: 720
}
```

---

## 3. Storage & Directory Hierarchy

Ingestion artifacts must be isolated per video job:

```text
<APP_DATA_DIR>/epochcut/
  └── projects/
      └── <JOB_UUID>/
          ├── metadata.json
          ├── audio/
          │   └── track_16k_mono.wav
          └── frames/
              ├── frame_000000_00.00s.jpg
              ├── frame_000001_04.00s.jpg
              └── ...
```

---

## 4. Subprocess Execution Contract

### 4.1 Metadata Extraction (`ffprobe`)
- Command:
  ```bash
  ffprobe -v quiet -print_format json -show_format -show_streams "<VIDEO_PATH>"
  ```
- Expected behavior: Parse duration, resolution, frame rate fraction (e.g. `30000/1001`), and codec names.

### 4.2 Audio Demuxing (`ffmpeg`)
- Specification: WAV, 16-bit PCM, Mono, 16,000 Hz (Whisper native requirement)
- Command:
  ```bash
  ffmpeg -y -i "<VIDEO_PATH>" -vn -acodec pcm_s16le -ar 16000 -ac 1 "<DEST_AUDIO_PATH>"
  ```

### 4.3 Frame Extraction (`ffmpeg`)
- Strategy: Fixed time-interval sampling (1 frame per N seconds, default N=4.0)
- Resolution: Max long side scaled to 720px, aspect ratio preserved (`scale='min(720,iw)':-2`)
- Compression: JPEG quality factor 3
- Command:
  ```bash
  ffmpeg -y -i "<VIDEO_PATH>" -vf "fps=1/4,scale='min(720,iw)':-2" -q:v 3 "<FRAMES_DIR>/frame_%06d.jpg"
  ```

---

## 5. Error Handling Contract (`src-tauri/src/error.rs`)

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum IngestionError {
    #[error("Video file does not exist: {0}")]
    FileNotFound(String),
    #[error("ffprobe failed to read file metadata: {0}")]
    ProbeFailed(String),
    #[error("ffmpeg audio demux failed: {0}")]
    AudioExtractionFailed(String),
    #[error("ffmpeg frame extraction failed: {0}")]
    FrameExtractionFailed(String),
    #[error("IO error occurred: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),
}
```

---

## 6. Acceptance Criteria

1. **Serialization:** All Rust models pass unit tests asserting JSON field casing matches TypeScript camelCase equivalents.
2. **Error Safety:** Supplying a non-existent path returns `IngestionError::FileNotFound` without panicking.
3. **Build:** `cargo check` and `yarn tsc` execute with 0 errors and 0 warnings.