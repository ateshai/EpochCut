use crate::error::IngestionError;
use crate::models::ingestion::{IngestionConfig, IngestionProgress, IngestionResult};
use crate::services::ingestion::{extract_audio, extract_frames, probe_video};
use uuid::Uuid;
use tauri::{AppHandle, Emitter, Manager};

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
    
    match extract_audio(source_path, &audio_path) {
        Ok(has_audio) => {
            let _ = app.emit("ingestion-progress", IngestionProgress::ExtractingAudio { progress: 1.0 });
            if !has_audio {
                let _ = app.emit("ingestion-progress", IngestionProgress::Info {
                    message: "No audio track found in video. Generated silent audio track.".to_string(),
                });
            }
        }
        Err(e) => {
            let _ = app.emit("ingestion-progress", IngestionProgress::Failed {
                error_message: format!("Failed to extract audio: {}", e),
            });
            return Err(format!("Failed to extract audio: {}", e));
        }
    }
    
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
    
    // Step 4: Build and return result
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
