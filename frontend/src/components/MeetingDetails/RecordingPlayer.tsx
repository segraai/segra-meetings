'use client';

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FolderOpen, Loader2, Music } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface RecordingPlayerProps {
  meetingId: string;
  /** The Recording tab is showing: the audio is only loaded once it is */
  active: boolean;
  onOpenFolder: () => Promise<void>;
}

type PlayerState =
  | { status: 'loading' }
  | { status: 'missing' }
  | { status: 'error'; message: string }
  | { status: 'found'; path: string }
  | { status: 'ready'; path: string; url: string };

function mimeTypeFor(path: string): string {
  const ext = path.split('.').pop()?.toLowerCase();
  switch (ext) {
    case 'wav': return 'audio/wav';
    case 'mp3': return 'audio/mpeg';
    case 'ogg': return 'audio/ogg';
    case 'webm': return 'audio/webm';
    case 'flac': return 'audio/flac';
    default: return 'audio/mp4';
  }
}

/** Plays back a meeting's saved recording */
export function RecordingPlayer({ meetingId, active, onOpenFolder }: RecordingPlayerProps) {
  const [state, setState] = useState<PlayerState>({ status: 'loading' });

  // Find the recording
  useEffect(() => {
    let cancelled = false;
    setState({ status: 'loading' });
    invoke<string | null>('get_meeting_audio_path', { meetingId })
      .then(path => {
        if (!cancelled) setState(path ? { status: 'found', path } : { status: 'missing' });
      })
      .catch(error => {
        if (!cancelled) setState({ status: 'error', message: String(error) });
      });
    return () => { cancelled = true; };
  }, [meetingId]);

  // Load its audio the first time the tab is shown. The webview can't stream it
  // from disk on Linux, so it's played from an in-memory Blob.
  const pathToLoad = active && state.status === 'found' ? state.path : null;
  useEffect(() => {
    if (!pathToLoad) return;
    let cancelled = false;
    invoke<ArrayBuffer | number[]>('read_meeting_audio', { meetingId })
      .then(bytes => {
        if (cancelled) return;
        // Raw bytes arrive as an ArrayBuffer over the IPC protocol, but as a plain
        // number array if Tauri had to fall back to its postMessage channel
        const data = bytes instanceof ArrayBuffer ? bytes : new Uint8Array(bytes);
        const url = URL.createObjectURL(new Blob([data], { type: mimeTypeFor(pathToLoad) }));
        setState({ status: 'ready', path: pathToLoad, url });
      })
      .catch(error => {
        if (!cancelled) setState({ status: 'error', message: String(error) });
      });
    return () => { cancelled = true; };
  }, [pathToLoad, meetingId]);

  const blobUrl = state.status === 'ready' ? state.url : null;
  useEffect(() => () => {
    if (blobUrl) URL.revokeObjectURL(blobUrl);
  }, [blobUrl]);

  if (state.status === 'missing' || state.status === 'error') {
    return (
      <div className="flex h-full flex-col items-center justify-center p-8 text-center">
        <Music className="mb-4 h-12 w-12 text-gray-300" />
        <h3 className="text-lg font-semibold text-gray-900">
          {state.status === 'error' ? "Couldn't load the recording" : 'No recording saved'}
        </h3>
        <p className="mt-1 max-w-md text-sm text-gray-500">
          {state.status === 'error'
            ? state.message
            : 'This meeting has no saved audio. Turn on "Save Audio Recordings" in Settings to keep recordings.'}
        </p>
      </div>
    );
  }

  if (state.status === 'loading') return null;

  const fileName = state.path.split(/[\\/]/).pop();

  return (
    <div className="flex h-full flex-col items-center justify-center p-8">
      <div className="w-full max-w-2xl rounded-xl border border-gray-200 bg-white p-6 shadow-sm">
        <div className="mb-4 flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-full bg-blue-50">
            <Music className="h-5 w-5 text-blue-600" />
          </div>
          <div className="min-w-0 flex-1">
            <p className="text-sm font-semibold text-gray-900">Meeting recording</p>
            <p className="truncate text-xs text-gray-500" title={state.path}>{fileName}</p>
          </div>
          <Button variant="outline" size="sm" onClick={() => void onOpenFolder()}>
            <FolderOpen />
            Open folder
          </Button>
        </div>
        {state.status === 'ready' ? (
          <audio
            key={state.url}
            controls
            preload="metadata"
            src={state.url}
            className="w-full"
            onError={() => setState({ status: 'error', message: 'The audio file could not be played.' })}
          />
        ) : (
          <div className="flex h-12 items-center justify-center gap-2 text-sm text-gray-500">
            <Loader2 className="h-4 w-4 animate-spin" />
            Loading recording...
          </div>
        )}
      </div>
    </div>
  );
}
