export interface TranscriptionSegment {
  id?: number;
  startMs: number;
  endMs: number;
  text: string;
  confidence: number;
}

export interface TranscriptionResult {
  jobId: string;
  language: string;
  durationMs: number;
  segments: TranscriptionSegment[];
  isSilent: boolean;
}

export type TranscriptionProgress =
  | { stage: "starting"; payload: { jobId: string } }
  | { stage: "transcribing"; payload: { progress: number; currentTimeMs: number; totalTimeMs: number } }
  | { stage: "indexingDatabase"; payload: { totalRecords: number } }
  | { stage: "complete"; payload: { result: TranscriptionResult } }
  | { stage: "skippedSilent"; payload: { reason: string } }
  | { stage: "failed"; payload: { errorMessage: string } };

export interface SearchHit {
  segmentId: number;
  startMs: number;
  endMs: number;
  text: string;
  matchedFramePath?: string;
}
