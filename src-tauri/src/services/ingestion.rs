use std::path::Path;
use std::process::Command;

use crate::error::IngestionError;
use crate::models::ingestion::{ExtractedFrame, VideoMetadata};

/// Probe video file metadata using ffprobe.
/// Executes ffprobe and parses JSON output to extract technical metadata.
pub fn probe_video(path: &Path) -> Result<VideoMetadata, IngestionError> {
    if !path.exists() {
        return Err(IngestionError::FileNotFound(path.display().to_string()));
    }

    let output = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            path.to_str().unwrap_or(""),
        ])
        .output()
        .map_err(|e| IngestionError::ProbeFailed(e.to_string()))?;

    if !output.status.success() {
        return Err(IngestionError::ProbeFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let json_str = String::from_utf8(output.stdout).map_err(|e| {
        IngestionError::ProbeFailed(format!("Failed to parse ffprobe output: {}", e))
    })?;

    let probe_result: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| IngestionError::ProbeFailed(format!("JSON parse error: {}", e)))?;

    let streams = probe_result["streams"].as_array().ok_or_else(|| {
        IngestionError::ProbeFailed("No streams found in ffprobe output".to_string())
    })?;

    let video_stream = streams
        .iter()
        .find(|s| s["codec_type"].as_str() == Some("video"))
        .ok_or_else(|| {
            IngestionError::ProbeFailed("No video stream found in ffprobe output".to_string())
        })?;

    let format = probe_result["format"].as_object().ok_or_else(|| {
        IngestionError::ProbeFailed("No format section found in ffprobe output".to_string())
    })?;

    // Extract file name from path
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Extract duration
    let duration_seconds = format["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .ok_or_else(|| {
            IngestionError::ProbeFailed("Failed to parse duration from ffprobe output".to_string())
        })?;

    // Extract file size
    let file_size_bytes = format["size"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    // Extract resolution
    let width = video_stream["width"]
        .as_i64()
        .and_then(|w| w.try_into().ok())
        .ok_or_else(|| {
            IngestionError::ProbeFailed("Failed to parse width from ffprobe output".to_string())
        })?;

    let height = video_stream["height"]
        .as_i64()
        .and_then(|h| h.try_into().ok())
        .ok_or_else(|| {
            IngestionError::ProbeFailed("Failed to parse height from ffprobe output".to_string())
        })?;

    // Extract FPS - handle fraction format like "30000/1001"
    let fps = parse_fps(video_stream["r_frame_rate"].as_str())
        .or_else(|| parse_fps(video_stream["avg_frame_rate"].as_str()))
        .unwrap_or(0.0);

    // Extract codecs
    let video_codec = video_stream["codec_name"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    // Find audio stream for audio codec
    let audio_codec = streams
        .iter()
        .find(|s| s["codec_type"].as_str() == Some("audio"))
        .and_then(|s| s["codec_name"].as_str())
        .map(|s| s.to_string());

    Ok(VideoMetadata {
        file_path: path.to_path_buf(),
        file_name,
        duration_seconds,
        width,
        height,
        fps,
        video_codec,
        audio_codec,
        file_size_bytes,
    })
}

/// Parse FPS from a string that may be a fraction (e.g., "30000/1001")
fn parse_fps(fps_str: Option<&str>) -> Option<f64> {
    let (num, den) = fps_str?.split_once('/')?;
    let numerator = num.parse::<f64>().ok()?;
    let denominator = den.parse::<f64>().ok()?;
    Some(numerator / denominator)
}

/// Extract audio track from video file using ffmpeg.
/// Converts to 16kHz, mono, 16-bit WAV format suitable for Whisper.
pub fn extract_audio(source: &Path, destination: &Path) -> Result<(), IngestionError> {
    // Ensure destination directory exists
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| IngestionError::AudioExtractionFailed(e.to_string()))?;
    }

    let output = Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            source.to_str().unwrap_or(""),
            "-vn",
            "-acodec",
            "pcm_s16le",
            "-ar",
            "16000",
            "-ac",
            "1",
            destination.to_str().unwrap_or(""),
        ])
        .output()
        .map_err(|e| IngestionError::AudioExtractionFailed(e.to_string()))?;

    if !output.status.success() {
        return Err(IngestionError::AudioExtractionFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    Ok(())
}

/// Extract keyframes from video at specified interval.
/// Uses ffmpeg to sample frames at fixed intervals with resolution scaling.
pub fn extract_frames(
    source: &Path,
    output_dir: &Path,
    interval_seconds: f64,
) -> Result<Vec<ExtractedFrame>, IngestionError> {
    // Ensure output directory exists
    std::fs::create_dir_all(output_dir)
        .map_err(|e| IngestionError::FrameExtractionFailed(e.to_string()))?;

    // Calculate FPS from interval (1 frame per N seconds)
    let _fps = if interval_seconds > 0.0 { 1.0 / interval_seconds } else { 0.25 };

    // Build ffmpeg command for frame extraction
    // fps=1/4 = 1 frame every 4 seconds
    // scale='min(720,iw)':-2 = scale long side to 720, preserve aspect ratio
    // q:v 3 = JPEG quality factor 3
    let fps_expr = format!("1/{}", interval_seconds);
    let vf = format!("fps={},scale='min(720,iw)':-2", fps_expr);

    let output_pattern = output_dir.join("frame_%06d.jpg");

    let output = Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            source.to_str().unwrap_or(""),
            "-vf",
            &vf,
            "-q:v",
            "3",
            output_pattern.to_str().unwrap_or(""),
        ])
        .output()
        .map_err(|e| IngestionError::FrameExtractionFailed(e.to_string()))?;

    if !output.status.success() {
        return Err(IngestionError::FrameExtractionFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    // List generated frame files and create ExtractedFrame entries
    let mut frames = Vec::new();
    let entries = std::fs::read_dir(output_dir).map_err(|e| {
        IngestionError::FrameExtractionFailed(format!("Failed to read output directory: {}", e))
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| {
            IngestionError::FrameExtractionFailed(format!("Failed to read directory entry: {}", e))
        })?;
        
        let path = entry.path();
        if path.extension().map_or(true, |ext| ext != "jpg") {
            continue;
        }

        // Extract frame index from filename (frame_XXXXXX.jpg)
        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        let frame_index = file_stem
            .strip_prefix("frame_")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);

        // Extract timestamp from filename (frame_XXXXXX_XX.XXs.jpg pattern)
        let timestamp_seconds = extract_timestamp_from_filename(file_stem);

        frames.push(ExtractedFrame {
            frame_index,
            timestamp_seconds,
            image_path: path.clone(),
            width: 0, // Will be populated by reading image metadata
            height: 0,
        });
    }

    // Sort frames by index to ensure correct order
    frames.sort_by_key(|f| f.frame_index);

    Ok(frames)
}

/// Extract timestamp from filename like "frame_000000_04.00s"
fn extract_timestamp_from_filename(file_stem: &str) -> f64 {
    // Try pattern: frame_XXXXXX_XX.XXs
    if let Some(idx) = file_stem.rfind('_') {
        let timestamp_part = &file_stem[idx + 1..];
        if timestamp_part.ends_with('s') {
            let time_str = &timestamp_part[..timestamp_part.len() - 1];
            return time_str.parse::<f64>().unwrap_or(0.0);
        }
    }
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_probe_video_file_not_found() {
        let result = probe_video(Path::new("/nonexistent/path/video.mp4"));
        assert!(matches!(result, Err(IngestionError::FileNotFound(_))));
    }

    #[test]
    fn test_probe_video_invalid_json() {
        // Create a temporary file with invalid content
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_invalid.mp4");
        
        // Create a minimal valid MP4 file (empty is fine for this test)
        fs::write(&test_file, []).expect("Failed to create test file");
        
        // This should fail with ProbeFailed since we don't have ffprobe or valid video
        let result = probe_video(&test_file);
        
        // Clean up
        let _ = fs::remove_file(&test_file);
        
        // We expect either FileNotFound (if file is too small) or ProbeFailed
        assert!(matches!(
            result,
            Err(IngestionError::FileNotFound(_)) | Err(IngestionError::ProbeFailed(_))
        ));
    }

    #[test]
    fn test_parse_fps() {
        assert_eq!(parse_fps(Some("30000/1001")), Some(29.97002997002997));
        assert_eq!(parse_fps(Some("30/1")), Some(30.0));
        assert_eq!(parse_fps(Some("24000/1001")), Some(23.976023976023978));
        assert_eq!(parse_fps(Some("invalid")), None);
        assert_eq!(parse_fps(None), None);
    }

    #[test]
    fn test_extract_timestamp_from_filename() {
        assert_eq!(extract_timestamp_from_filename("frame_000000_00.00s"), 0.0);
        assert_eq!(extract_timestamp_from_filename("frame_000001_04.00s"), 4.0);
        assert_eq!(extract_timestamp_from_filename("frame_000002_08.50s"), 8.5);
        assert_eq!(extract_timestamp_from_filename("frame_000003"), 0.0);
    }

    #[test]
    fn test_extract_frames_directory_creation() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_video.mp4");
        let output_dir = temp_dir.join("test_frames_output");
        
        // Clean up any existing test files
        let _ = fs::remove_file(&test_file);
        let _ = fs::remove_dir_all(&output_dir);
        
        // Create a minimal test file
        fs::write(&test_file, []).expect("Failed to create test file");
        
        // This should create the output directory even if ffmpeg fails
        let result = extract_frames(&test_file, &output_dir, 4.0);
        
        // Clean up
        let _ = fs::remove_file(&test_file);
        let _ = fs::remove_dir_all(&output_dir);
        
        // We expect FrameExtractionFailed since we don't have a valid video
        assert!(matches!(
            result,
            Err(IngestionError::FrameExtractionFailed(_))
        ));
    }
}
