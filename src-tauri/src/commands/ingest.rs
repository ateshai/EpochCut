use crate::error::IngestionError;
use crate::models::ingestion::{IngestionConfig, IngestionProgress, IngestionResult};
use crate::services::ingestion::{extract_audio, extract_frames, probe_video};
use crate::services::{db, whisper};
use uuid::Uuid;
use tauri::{AppHandle, Emitter, Manager};
use std::path::Path;

/// Generate a unique job ID based on UUID
fn generate_job_id() -> String {
    Uuid::new_v4().to_string()
}

/// Create job directory structure
fn create_job_directory(app: &AppHandle, job_id: &str) -> Result<std::path::PathBuf, IngestionError> {
    let app_data_dir = app.path().app_data_dir().map_err(|_| {
        IngestionError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "App data directory not found",
        ))
    })?;
    
    let project_dir = app_data_dir.join("projects").join(job_id);
    
    std::fs::create_dir_all(&project_dir)
        .map_err(|e| IngestionError::Io(std::io::Error::new(e.kind(), format!("Failed to create project directory: {}", e))))?;
    
    // Create subdirectories
    let audio_dir = project_dir.join("audio");
    let frames_dir = project_dir.join("frames");
    
    std::fs::create_dir_all(&audio_dir)
        .map_err(|e| IngestionError::Io(std::io::Error::new(e.kind(), format!("Failed to create audio directory: {}", e))))?;
    
    std::fs::create_dir_all(&frames_dir)
        .map_err(|e| IngestionError::Io(std::io::Error::new(e.kind(), format!("Failed to create frames directory: {}", e))))?;
    
    Ok(project_dir)
}

/// Save result metadata to SQLite database
fn save_to_database(
    app: &AppHandle,
    job_id: &str,
    metadata: &crate::models::ingestion::VideoMetadata,
    _audio_path: &Path,
    frames: &[crate::models::ingestion::ExtractedFrame],
) -> Result<(), IngestionError> {
    // Create database path
    let app_data_dir = app.path().app_data_dir().map_err(|_| {
        IngestionError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "App data directory not found",
        ))
    })?;
    
    let project_dir = app_data_dir.join("projects").join(job_id);
    let db_path = project_dir.join("index.db");
    
    // Initialize database
    let conn = db::init_project_db(&db_path)?;
    
    // Save video metadata
    db::save_metadata(&conn, metadata)?;
    
    // Save frames to catalog
    db::save_frames(&conn, job_id, frames)?;
    
    Ok(())
}

/// Run transcription if audio file exists and is valid
fn run_transcription_if_audio_exists(
    app: &AppHandle,
    job_id: &str,
    audio_path: &Path,
) -> Result<(), IngestionError> {
    // Check if audio file exists and has valid content
    if !whisper::has_valid_audio(audio_path) {
        // No audio or empty audio - skip transcription gracefully
        let _ = app.emit("transcription-progress", 
            crate::models::transcription::TranscriptionProgress::SkippedSilent {
            reason: "No valid audio track found".to_string()
        });
        return Ok(());
    }
    
    // Run Whisper transcription
    let result = whisper::transcribe_audio(app, audio_path)
        .map_err(|e| IngestionError::FrameExtractionFailed(e.to_string()))?;
    
    if result.is_silent {
        let _ = app.emit("transcription-progress", 
            crate::models::transcription::TranscriptionProgress::SkippedSilent {
            reason: "No speech detected in audio".to_string()
        });
        return Ok(());
    }
    
    // Get database path
    let app_data_dir = app.path().app_data_dir()
        .map_err(|_| IngestionError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "App data directory not found",
        )))?;
    
    let project_dir = app_data_dir.join("projects").join(job_id);
    let db_path = project_dir.join("index.db");
    
    // Save segments to database
    let conn = db::init_project_db(&db_path)?;
    
    db::save_segments(&conn, job_id, &result.segments)?;
    
    Ok(())
}

/// Main ingestion command that orchestrates the entire pipeline
#[tauri::command]
pub fn ingest_video(
    app: AppHandle,
    file_path: String,
    config: Option<IngestionConfig>,
) -> Result<IngestionResult, String> {
    let config = config.unwrap_or_default();
    
    let source_path = std::path::Path::new(&file_path);
    
    // Validate source file exists
    if !source_path.exists() {
        return Err(format!("Video file does not exist: {}", file_path));
    }
    
    // Generate job ID and create directory structure
    let job_id = generate_job_id();
    let job_dir = create_job_directory(&app, &job_id)
        .map_err(|e| format!("Failed to create job directory: {}", e))?;
    
    let audio_dir = job_dir.join("audio");
    let frames_dir = job_dir.join("frames");
    
    // Emit initial probing event
    let _ = app.emit("ingestion-progress", IngestionProgress::Probing { progress: 0.0 });
    
    // Step 1: Probe video metadata
    let metadata = match probe_video(source_path) {
        Ok(m) => {
            let _ = app.emit("ingestion-progress", IngestionProgress::Probing { progress: 1.0 });
            m
        }
        Err(e) => {
            let _ = app.emit("ingestion-progress", IngestionProgress::Failed {
                error_message: format!("Failed to probe video: {}", e),
            });
            return Err(format!("Failed to probe video: {}", e));
        }
    };
    
    // Step 2: Extract audio
    let audio_path = audio_dir.join("track_16k_mono.wav");
    let _ = app.emit("ingestion-progress", IngestionProgress::ExtractingAudio { progress: 0.0 });
    
    let has_audio = match extract_audio(source_path, &audio_path) {
        Ok(has_audio) => {
            let _ = app.emit("ingestion-progress", IngestionProgress::ExtractingAudio { progress: 1.0 });
            if !has_audio {
                let _ = app.emit("ingestion-progress", IngestionProgress::Info {
                    message: "No audio track found in video. Generated silent audio track.".to_string(),
                });
            }
            has_audio
        }
        Err(e) => {
            let _ = app.emit("ingestion-progress", IngestionProgress::Failed {
                error_message: format!("Failed to extract audio: {}", e),
            });
            return Err(format!("Failed to extract audio: {}", e));
        }
    };
    
    // Step 3: Extract frames
    let total_frames = (metadata.duration_seconds / config.interval_seconds).ceil() as usize;
    
    let _ = app.emit("ingestion-progress", IngestionProgress::ExtractingFrames {
        progress: 0.0,
        current_frame: 0,
        total_frames,
    });
    
    let frames = match extract_frames(source_path, &frames_dir, config.interval_seconds) {
        Ok(f) => {
            let _ = app.emit("ingestion-progress", IngestionProgress::ExtractingFrames {
                progress: 1.0,
                current_frame: f.len(),
                total_frames,
            });
            f
        }
        Err(e) => {
            let _ = app.emit("ingestion-progress", IngestionProgress::Failed {
                error_message: format!("Failed to extract frames: {}", e),
            });
            return Err(format!("Failed to extract frames: {}", e));
        }
    };
    
    // Step 4: Save to database
    let _ = app.emit("ingestion-progress", IngestionProgress::Info {
        message: "Saving metadata and frame catalog to database...".to_string(),
    });
    
    save_to_database(&app, &job_id, &metadata, &audio_path, &frames)
        .map_err(|e| format!("Failed to save to database: {}", e))?;
    
    // Step 5: Run transcription if audio exists
    if has_audio {
        let _ = app.emit("ingestion-progress", IngestionProgress::Info {
            message: "Starting audio transcription...".to_string(),
        });
        
        if let Err(e) = run_transcription_if_audio_exists(&app, &job_id, &audio_path) {
            let _ = app.emit("ingestion-progress", IngestionProgress::Info {
                message: format!("Transcription skipped due to error: {}", e),
            });
        }
    } else {
        let _ = app.emit("ingestion-progress", IngestionProgress::Info {
            message: "Skipping transcription - no audio track found.".to_string(),
        });
    }
    
    // Step 6: Build and return result
    let result = IngestionResult {
        job_id: job_id.clone(),
        metadata,
        audio_track_path: audio_path,
        frames,
    };
    
    // Emit complete event
    let _ = app.emit("ingestion-progress", IngestionProgress::Complete { result: result.clone() });
    
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_job_id_uniqueness() {
        let id1 = generate_job_id();
        let id2 = generate_job_id();
        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 36); // UUID format length
    }
}
