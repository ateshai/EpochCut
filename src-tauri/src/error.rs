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
