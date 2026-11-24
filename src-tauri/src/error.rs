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

#[derive(Error, Debug)]
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

impl From<IngestionError> for AiPipelineError {
    fn from(err: IngestionError) -> Self {
        match err {
            IngestionError::Io(e) => AiPipelineError::Io(e),
            IngestionError::Json(e) => AiPipelineError::Database(rusqlite::Error::ToSqlConversionFailure(e.into())),
            _ => AiPipelineError::InferenceFailed(err.to_string()),
        }
    }
}
