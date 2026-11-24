pub mod db;
pub mod ingestion;
pub mod whisper;

// Re-export commonly used functions
pub use db::{init_project_db, save_frames, save_metadata, save_segments, search_text};
pub use ingestion::{extract_audio, extract_frames, probe_video};
pub use whisper::{decode_wav_to_samples, has_valid_audio, transcribe_audio};
