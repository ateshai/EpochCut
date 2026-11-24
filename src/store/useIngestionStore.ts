import { create } from 'zustand';
import type { IngestionConfig, IngestionProgress, IngestionResult } from '../types/ingestion';
import { ingestVideo, onIngestionProgress } from '../api/ingestion';

interface IngestionState {
  isIngesting: boolean;
  currentStage: string | null;
  progressPercentage: number;
  result: IngestionResult | null;
  error: string | null;
  unlistenFn?: () => void;
}

interface IngestionActions {
  startIngestion: (filePath: string, config?: IngestionConfig) => Promise<void>;
  reset: () => void;
}

export const useIngestionStore = create<IngestionState & IngestionActions>((set, get) => ({
  isIngesting: false,
  currentStage: null,
  progressPercentage: 0,
  result: null,
  error: null,

  startIngestion: async (filePath: string, config?: IngestionConfig) => {
    console.log('[STORE] Starting ingestion for:', filePath);
    console.log('[STORE] Config:', config);
    
    set({ isIngesting: true, error: null, result: null, currentStage: 'probing', progressPercentage: 0 });

    // Set up event listener first
    const unlisten = await onIngestionProgress((progress: IngestionProgress) => {
      console.log('[STORE] Received progress:', progress);
      switch (progress.stage) {
        case 'probing':
          set({ currentStage: 'probing', progressPercentage: progress.payload.progress * 100 });
          break;
        case 'extractingAudio':
          set({ currentStage: 'extractingAudio', progressPercentage: progress.payload.progress * 100 });
          break;
        case 'extractingFrames':
          set({ 
            currentStage: 'extractingFrames', 
            progressPercentage: progress.payload.progress * 100 
          });
          break;
        case 'info':
          console.log('[STORE] Info:', progress.payload.message);
          // Just log info messages, don't change state
          break;
        case 'complete':
          set({ 
            isIngesting: false, 
            result: progress.payload.result,
            currentStage: 'complete',
            progressPercentage: 100,
            error: null 
          });
          break;
        case 'failed':
          set({ 
            isIngesting: false, 
            error: progress.payload.errorMessage,
            currentStage: 'failed',
            progressPercentage: 0 
          });
          break;
      }
    });

    // Store the unlisten function for cleanup
    set({ unlistenFn: unlisten });

    try {
      // Trigger the ingestion command
      console.log('[STORE] Invoking ingest_video with:', filePath, 'config:', config);
      const res = await ingestVideo(filePath, config);
      console.log('[STORE] Ingestion succeeded:', res);
    } catch (err: any) {
      console.error('[STORE] Ingestion invoke failed:', err);
      set({ 
        isIngesting: false, 
        error: err?.message || 'Ingestion failed',
        currentStage: null,
        progressPercentage: 0 
      });
    }
  },

  reset: () => {
    const { unlistenFn } = get();
    if (unlistenFn) {
      unlistenFn();
    }
    set({
      isIngesting: false,
      currentStage: null,
      progressPercentage: 0,
      result: null,
      error: null,
      unlistenFn: undefined
    });
  },
}));
