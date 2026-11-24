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
