'use client';

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FolderOpen, Loader2, Music } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface RecordingPlayerProps {
  meetingId: string;
  /** The Transcript tab is showing: the audio is only loaded once it is */
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

/** Plays back a meeting's saved recording: a bar at the top of the Transcript tab */
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

  // No saved audio: nothing to play, so the transcript gets the whole tab
  if (state.status === 'loading' || state.status === 'missing') return null;

  if (state.status === 'error') {
    return (
      <div className="flex shrink-0 items-center gap-2 border-b border-gray-200 px-6 py-2 text-xs text-red-600">
        <Music className="h-4 w-4 shrink-0" />
        <span className="truncate">Couldn&apos;t load the recording: {state.message}</span>
      </div>
    );
  }

  const fileName = state.path.split(/[\\/]/).pop();

  return (
    <div className="flex shrink-0 items-center gap-3 border-b border-gray-200 px-6 py-2">
      <Music className="h-4 w-4 shrink-0 text-blue-600" aria-hidden />
      {state.status === 'ready' ? (
        <audio
          key={state.url}
          controls
          preload="metadata"
          src={state.url}
          title={fileName}
          className="h-9 min-w-0 flex-1"
          onError={() => setState({ status: 'error', message: 'The audio file could not be played.' })}
        />
      ) : (
        <div className="flex h-9 flex-1 items-center gap-2 text-sm text-gray-500">
          <Loader2 className="h-4 w-4 animate-spin" />
          Loading recording...
        </div>
      )}
      <Button variant="outline" size="sm" onClick={() => void onOpenFolder()} title={state.path}>
        <FolderOpen />
        Open folder
      </Button>
    </div>
  );
}
