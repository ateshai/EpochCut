use serde::{Deserialize, Serialize};

/// Represents a single transcription segment with timestamps and confidence
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionSegment {
    pub id: Option<i64>,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub confidence: f32,
}

/// Result of a complete transcription run
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionResult {
    pub job_id: String,
    pub language: String,
    pub duration_ms: u64,
    pub segments: Vec<TranscriptionSegment>,
    pub is_silent: bool,
}

/// Progress updates during transcription
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

/// Search result from FTS5 full-text search
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub segment_id: i64,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub matched_frame_path: Option<String>,
}
