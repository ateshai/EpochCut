use crate::error::AiPipelineError;
use crate::models::transcription::{SearchHit, TranscriptionResult, TranscriptionSegment, TranscriptionProgress};
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager};

/// Generate a unique job ID based on UUID
fn generate_job_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
