mod commands;
mod error;
mod models {
    pub mod ingestion;
    pub mod transcription;
}
mod services {
    pub mod db;
    pub mod ingestion;
    pub mod whisper;
}

pub use error::{AiPipelineError, IngestionError};
pub use models::ingestion::{
    ExtractedFrame, IngestionConfig, IngestionProgress, IngestionResult, VideoMetadata,
};
pub use models::transcription::{
    SearchHit, TranscriptionProgress, TranscriptionResult, TranscriptionSegment,
};
pub use services::db::{
    init_project_db, save_frames, save_metadata, save_segments, search_text,
};
pub use services::ingestion::{extract_audio, extract_frames, probe_video};
pub use services::whisper::{decode_wav_to_samples, transcribe_audio, has_valid_audio};

// Re-export commands for Tauri
pub use commands::ingest::ingest_video;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, ingest_video])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
