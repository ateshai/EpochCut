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
    set({ isIngesting: true, error: null, result: null, currentStage: 'probing', progressPercentage: 0 });

    // Set up event listener first
    const unlisten = await onIngestionProgress((progress: IngestionProgress) => {
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
      await ingestVideo(filePath, config);
    } catch (err: any) {
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
