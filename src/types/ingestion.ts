export interface VideoMetadata {
  filePath: string;
  fileName: string;
  durationSeconds: number;
  width: number;
  height: number;
  fps: number;
  videoCodec: string;
  audioCodec?: string;
  fileSizeBytes: number;
}

export interface ExtractedFrame {
  frameIndex: number;
  timestampSeconds: number;
  imagePath: string;
  width: number;
  height: number;
}

export interface IngestionResult {
  jobId: string;
  metadata: VideoMetadata;
  audioTrackPath: string;
  frames: ExtractedFrame[];
}

export type IngestionProgress =
  | { stage: "probing"; payload: { progress: number } }
  | { stage: "extractingAudio"; payload: { progress: number } }
  | { stage: "extractingFrames"; payload: { progress: number; currentFrame: number; totalFrames: number } }
  | { stage: "complete"; payload: { result: IngestionResult } }
  | { stage: "failed"; payload: { errorMessage: string } };

export interface IngestionConfig {
  intervalSeconds: number;
  maxFrameResolution: number;
}
