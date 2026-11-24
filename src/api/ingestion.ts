import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
  IngestionConfig,
  IngestionProgress,
  IngestionResult,
} from '../types/ingestion';

/**
 * Start video ingestion and return a promise that resolves with the result.
 * @param filePath - Path to the video file to ingest
 * @param config - Optional ingestion configuration
 * @returns Promise resolving to IngestionResult
 */
export async function ingestVideo(
  filePath: string,
  config?: IngestionConfig
): Promise<IngestionResult> {
  return await invoke<IngestionResult>('ingest_video', {
    filePath,
    config,
  });
}

/**
 * Listen to ingestion progress events.
 * @param callback - Function to call with each progress update
 * @returns Unlisten function to stop listening
 */
export function onIngestionProgress(
  callback: (progress: IngestionProgress) => void
): Promise<() => void> {
  return listen<IngestionProgress>('ingestion-progress', (event) => {
    callback(event.payload);
  });
}
