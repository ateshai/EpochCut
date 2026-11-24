mod commands;
mod error;
mod models {
    pub mod ingestion;
}
mod services {
    pub mod ingestion;
}

pub use error::IngestionError;
pub use models::ingestion::{
    ExtractedFrame, IngestionConfig, IngestionProgress, IngestionResult, VideoMetadata,
};
pub use services::ingestion::{extract_audio, extract_frames, probe_video};

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
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, ingest_video])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
