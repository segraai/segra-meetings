'use client';

import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Check, Loader2, PenLine, Sparkles } from 'lucide-react';

const SAVE_DELAY_MS = 600;

interface MeetingNotesEditorProps {
  meetingId: string;
  /** Regenerates the AI notes so they include these notes; hidden when not possible yet */
  onRegenerate?: () => void;
}

/** The user's own notes for a saved meeting, editable with autosave */
export function MeetingNotesEditor({ meetingId, onRegenerate }: MeetingNotesEditorProps) {
  const [notes, setNotes] = useState('');
  const [isLoaded, setIsLoaded] = useState(false);
  const [saveState, setSaveState] = useState<'idle' | 'saving' | 'saved'>('idle');
  const [changedSinceLoad, setChangedSinceLoad] = useState(false);
  const saveTimer = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    setIsLoaded(false);
    setSaveState('idle');
    setChangedSinceLoad(false);
    invoke<string | null>('api_get_meeting_notes', { meetingId })
      .then(saved => setNotes(saved ?? ''))
      .catch(error => console.error('Failed to load notes:', error))
      .finally(() => setIsLoaded(true));
    return () => clearTimeout(saveTimer.current);
  }, [meetingId]);

  const onChange = (value: string) => {
    setNotes(value);
    setChangedSinceLoad(true);
    setSaveState('saving');
    clearTimeout(saveTimer.current);
    saveTimer.current = setTimeout(async () => {
      try {
        await invoke('api_save_meeting_notes', { meetingId, notesMarkdown: value });
        setSaveState('saved');
      } catch (error) {
        console.error('Failed to save notes:', error);
        setSaveState('idle');
      }
    }, SAVE_DELAY_MS);
  };

  if (!isLoaded) return null;

  const isEmpty = !notes.trim();

  return (
    <div className="flex h-full min-h-0 flex-col bg-white">
      <div className="flex shrink-0 items-center gap-3 border-b border-gray-200 px-6 py-3">
        <PenLine className="h-4 w-4 text-blue-600" />
        <span className="text-sm font-semibold text-gray-800">Your notes</span>
        <span className="flex items-center gap-1 text-xs text-gray-400">
          {saveState === 'saving' && <><Loader2 className="h-3 w-3 animate-spin" /> Saving...</>}
          {saveState === 'saved' && <><Check className="h-3 w-3" /> Saved</>}
        </span>
        {onRegenerate && changedSinceLoad && !isEmpty && (
          <button
            type="button"
            onClick={onRegenerate}
            className="ml-auto inline-flex items-center gap-1.5 rounded-md border border-blue-200 bg-blue-50 px-3 py-1.5 text-xs font-medium text-blue-700 hover:bg-blue-100"
          >
            <Sparkles className="h-3.5 w-3.5" />
            Regenerate notes with these
          </button>
        )}
      </div>

      {isEmpty && (
        <div className="shrink-0 px-6 pt-5">
          <p className="text-sm font-medium text-gray-700">No notes taken.</p>
          <p className="text-sm text-gray-500">
            Add some below to give the AI more context, then regenerate the summary.
          </p>
        </div>
      )}

      <textarea
        value={notes}
        onChange={e => onChange(e.target.value)}
        placeholder="Key points, decisions, names, follow-ups..."
        className="min-h-0 flex-1 resize-none px-6 py-4 text-sm leading-relaxed text-gray-900 outline-none placeholder:text-gray-400"
      />
    </div>
  );
}
