import { useState, useRef, useEffect } from 'react';
import { useIngestionStore } from '../store/useIngestionStore';
import type { IngestionProgress } from '../types/ingestion';
import { onIngestionProgress } from '../api/ingestion';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { getCurrentWebview } from '@tauri-apps/api/webview';

const ALLOWED_EXTENSIONS = ['.mp4', '.mkv', '.mov'];

const formatDuration = (seconds: number): string => {
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, '0')}`;
};

const VideoDropzone = () => {
  const [isDragging, setIsDragging] = useState(false);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [fileName, setFileName] = useState<string | null>(null);
  const [dialogError, setDialogError] = useState<string | null>(null);
  const unlistenFn = useRef<UnlistenFn | null>(null);
  const dropUnlistenFn = useRef<UnlistenFn | null>(null);
  
  const { 
    isIngesting, 
    currentStage, 
    progressPercentage, 
    result, 
    error,
    startIngestion,
    reset: resetStore
  } = useIngestionStore();

  // Cleanup event listener on unmount
  useEffect(() => {
    return () => {
      if (unlistenFn.current) {
        unlistenFn.current();
      }
      if (dropUnlistenFn.current) {
        dropUnlistenFn.current();
      }
    };
  }, []);

  // Listen to ingestion progress events
  useEffect(() => {
    if (isIngesting && !unlistenFn.current) {
      onIngestionProgress((progress: IngestionProgress) => {
        console.log('Ingestion progress:', progress);
        switch (progress.stage) {
          case 'probing':
            resetStore();
            break;
          case 'extractingAudio':
            // Update progress for audio extraction
            break;
          case 'extractingFrames':
            // Update progress for frame extraction
            break;
          case 'complete':
            // Result is already set by the promise
            break;
          case 'failed':
            // Error is already set by the promise
            break;
        }
      }).then((unlisten) => {
        unlistenFn.current = unlisten;
      });
    }
  }, [isIngesting, resetStore]);

  // Handle native drag-and-drop events from Tauri
  useEffect(() => {
    const setupDragDropHandler = async () => {
      try {
        const webview = getCurrentWebview();
        
        dropUnlistenFn.current = await webview.onDragDropEvent((event) => {
          console.log('Drag drop event received:', event);
          
          if (event.payload.type === 'drop') {
            console.log('Files dropped, paths:', event.payload.paths);
            
            const droppedPath = event.payload.paths[0];
            if (droppedPath) {
              console.log('Processing dropped file path:', droppedPath);
              handleFileSelect(droppedPath);
            }
          } else if (event.payload.type === 'enter') {
            setIsDragging(true);
          } else if (event.payload.type === 'leave' || event.payload.type === 'over') {
            setIsDragging(false);
          }
        });
        
        console.log('Drag drop event listener attached to webview');
      } catch (err) {
        console.error('Failed to set up drag drop handler:', err);
      }
    };
    
    setupDragDropHandler();
    
    return () => {
      if (dropUnlistenFn.current) {
        dropUnlistenFn.current();
        console.log('Drag drop event listener removed');
      }
    };
  }, []);

  const handleFileSelect = async (filePath?: string) => {
    try {
      setDialogError(null);
      
      let selectedPath = filePath;
      
      if (!selectedPath) {
        console.log('Opening file dialog...');
        
        const result = await open({
          multiple: false,
          filters: [{
            name: 'Video Files',
            extensions: ['mp4', 'mkv', 'mov']
          }]
        });
        
        console.log('Dialog result:', result);
        
        if (result === null) {
          console.log('No file selected');
          return;
        }
        
        selectedPath = result;
        console.log('Selected file path:', selectedPath);
      }
      
      // Validate file extension
      const ext = selectedPath.split('.').pop()?.toLowerCase();
      if (!ext || !ALLOWED_EXTENSIONS.includes('.' + ext)) {
        const errorMsg = `Invalid file type. Allowed: ${ALLOWED_EXTENSIONS.join(', ')}`;
        console.error(errorMsg);
        setDialogError(errorMsg);
        return;
      }
      
      // Extract file name from path
      const fileNameFromPath = selectedPath.split('/').pop() || 'unknown';
      console.log('Selected file name:', fileNameFromPath);
      
      // Store path and filename directly - no blob needed
      setSelectedPath(selectedPath);
      setFileName(fileNameFromPath);
      setDialogError(null);
    } catch (err: any) {
      console.error('Error selecting file:', err);
      setDialogError(err instanceof Error ? err.message : 'Failed to select file');
    }
  };

  const handleStartIngestion = async () => {
    if (!selectedPath || !fileName) {
      console.error('Cannot start ingestion: selectedPath or fileName is missing');
      return;
    }
    
    console.log('[DROPZONE] Starting ingestion for:', selectedPath, 'fileName:', fileName);
    
    try {
      await startIngestion(selectedPath);
      console.log('[DROPZONE] Ingestion completed successfully');
      setDialogError(null);
    } catch (err: any) {
      const errorMsg = err instanceof Error ? err.message : 'Ingestion failed';
      console.error('[DROPZONE] Ingestion error:', errorMsg, 'Error object:', err);
      setDialogError(errorMsg);
    }
  };

  const getStageLabel = () => {
    if (!currentStage) return 'Idle';
    switch (currentStage) {
      case 'probing': return 'Probing Video Metadata';
      case 'extractingAudio': return 'Extracting Audio Track';
      case 'extractingFrames': return 'Extracting Keyframes';
      case 'complete': return 'Ingestion Complete';
      case 'failed': return 'Ingestion Failed';
      default: return currentStage;
    }
  };

  const getStageColor = () => {
    switch (currentStage) {
      case 'probing': return 'text-blue-500';
      case 'extractingAudio': return 'text-purple-500';
      case 'extractingFrames': return 'text-pink-500';
      case 'complete': return 'text-green-500';
      case 'failed': return 'text-red-500';
      default: return 'text-gray-500';
    }
  };

  return (
    <div className="max-w-4xl mx-auto p-6 space-y-8">
      {/* Header */}
      <div className="text-center">
        <h1 className="text-3xl font-bold text-gray-900 mb-2">
          Video Ingestion Pipeline
        </h1>
        <p className="text-gray-600">
          Drag & drop video files or select from your system
        </p>
      </div>

      {/* Dropzone */}
      <div
        className={`
          relative group cursor-pointer
          border-3 border-dashed rounded-2xl p-12 text-center transition-all duration-300
          ${isDragging 
            ? 'border-blue-500 bg-blue-50 scale-105' 
            : 'border-gray-300 hover:border-blue-400 hover:bg-gray-50'
          }
        `}
      >
         {selectedPath ? (
           <div className="space-y-4">
             <div className="flex items-center justify-center space-x-4">
               <div className="w-24 h-16 bg-gray-800 rounded flex items-center justify-center">
                 <svg className="w-12 h-12 text-zinc-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                   <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                   <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                 </svg>
               </div>
               <div className="text-left">
                 <p className="font-semibold text-gray-900">{fileName}</p>
                 <p className="text-sm text-gray-500">
                   {selectedPath}
                 </p>
               </div>
             </div>
             <button
               onClick={handleStartIngestion}
               disabled={isIngesting}
               className={`
                 px-8 py-3 rounded-lg font-medium transition-colors
                 ${isIngesting
                   ? 'bg-gray-400 cursor-not-allowed'
                   : 'bg-blue-600 text-white hover:bg-blue-700'
                 }
               `}
             >
               {isIngesting ? 'Processing...' : 'Start Ingestion'}
             </button>
           </div>
         ) : (
          <div className="space-y-4">
            <div className="mx-auto w-16 h-16 bg-gray-200 rounded-full flex items-center justify-center">
              <svg className="w-8 h-8 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
              </svg>
            </div>
            <div>
              <p className="text-lg font-medium text-gray-700">
                Drag & drop video files here
              </p>
              <p className="text-sm text-gray-500 mt-1">
                or click to browse (.mp4, .mkv, .mov)
              </p>
            </div>
            <p className="text-xs text-gray-400">
              Max file size: 4GB
            </p>
          </div>
        )}
      </div>

      {/* Error Display */}
      {(error || dialogError) && (
        <div className="bg-red-50 border border-red-200 rounded-lg p-4 flex items-center">
          <svg className="w-5 h-5 text-red-500 mr-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <p className="text-red-700">{error || dialogError}</p>
        </div>
      )}

      {/* Progress Display */}
      {(isIngesting || result) && (
        <div className="bg-white rounded-xl shadow-lg p-6">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center">
              <div className={`w-3 h-3 rounded-full mr-3 ${isIngesting || error ? 'animate-pulse' : 'bg-green-500'}`}></div>
              <span className={`text-lg font-semibold ${getStageColor()}`}>
                {getStageLabel()}
              </span>
            </div>
            <span className="text-sm font-mono text-gray-500">
              {Math.round(progressPercentage)}%
            </span>
          </div>
          
          <div className="w-full bg-gray-200 rounded-full h-4 overflow-hidden">
            <div
              className="bg-gradient-to-r from-blue-500 to-purple-500 h-4 rounded-full transition-all duration-300 ease-out"
              style={{ width: `${progressPercentage}%` }}
            >
              {isIngesting && (
                <div className="h-full w-full animate-[shimmer_2s_infinite] bg-gradient-to-r from-transparent via-white/30 to-transparent" />
              )}
            </div>
          </div>
          
          {/* Stage-specific details */}
          {isIngesting && currentStage === 'extractingFrames' && (
            <p className="mt-2 text-sm text-gray-500">
              Extracting keyframes at 4-second intervals...
            </p>
          )}
          {result && (
            <div className="mt-4 grid grid-cols-2 gap-4 text-sm">
              <div>
                <span className="text-gray-500">Duration:</span>
                <span className="ml-2 font-medium">{formatDuration(result.metadata.durationSeconds)}</span>
              </div>
              <div>
                <span className="text-gray-500">Resolution:</span>
                <span className="ml-2 font-medium">
                  {result.metadata.width}×{result.metadata.height}
                </span>
              </div>
              <div>
                <span className="text-gray-500">FPS:</span>
                <span className="ml-2 font-medium">{result.metadata.fps.toFixed(2)}</span>
              </div>
              <div>
                <span className="text-gray-500">Codec:</span>
                <span className="ml-2 font-medium">{result.metadata.videoCodec}</span>
              </div>
            </div>
          )}
        </div>
      )}

      {/* Extracted Frames Grid */}
      {result && result.frames.length > 0 && (
        <div className="space-y-4">
          <h3 className="text-lg font-semibold text-gray-900">
            Extracted Keyframes ({result.frames.length} total)
          </h3>
          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6 gap-4">
            {result.frames.slice(0, 6).map((frame) => (
              <div key={frame.frameIndex} className="bg-white rounded-lg shadow-md overflow-hidden">
                <div className="aspect-video bg-gray-900 flex items-center justify-center">
                  <svg className="w-12 h-12 text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
                  </svg>
                </div>
                <div className="p-3">
                  <p className="text-xs text-gray-500 font-mono">
                    frame_{frame.frameIndex.toString().padStart(6, '0')}
                  </p>
                  <p className="text-xs text-gray-400">
                    {frame.timestampSeconds.toFixed(2)}s
                  </p>
                </div>
              </div>
            ))}
          </div>
          {result.frames.length > 6 && (
            <p className="text-center text-sm text-gray-500">
              Showing first 6 of {result.frames.length} frames
            </p>
          )}
        </div>
      )}
    </div>
  );
};

export default VideoDropzone;
