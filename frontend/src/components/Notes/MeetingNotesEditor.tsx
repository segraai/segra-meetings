'use client';

import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Check, ChevronDown, ChevronUp, Loader2, PenLine } from 'lucide-react';

const SAVE_DELAY_MS = 600;

/** The user's own notes for a saved meeting, editable with autosave */
export function MeetingNotesEditor({ meetingId }: { meetingId: string }) {
  const [notes, setNotes] = useState('');
  const [isLoaded, setIsLoaded] = useState(false);
  const [isOpen, setIsOpen] = useState(false);
  const [saveState, setSaveState] = useState<'idle' | 'saving' | 'saved'>('idle');
  const saveTimer = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    setIsLoaded(false);
    setSaveState('idle');
    invoke<string | null>('api_get_meeting_notes', { meetingId })
      .then(saved => {
        setNotes(saved ?? '');
        setIsOpen(!!saved?.trim());
      })
      .catch(error => console.error('Failed to load notes:', error))
      .finally(() => setIsLoaded(true));
    return () => clearTimeout(saveTimer.current);
  }, [meetingId]);

  const onChange = (value: string) => {
    setNotes(value);
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

  return (
    <div className="shrink-0 border-b border-gray-200 bg-white">
      <button
        onClick={() => setIsOpen(open => !open)}
        className="flex w-full items-center gap-2 px-4 py-2 text-left text-sm font-semibold text-gray-800 hover:bg-gray-50"
      >
        <PenLine className="h-4 w-4 text-blue-600" />
        My notes
        {!isOpen && !notes.trim() && <span className="text-xs font-normal text-gray-400">add your own notes</span>}
        <span className="ml-auto flex items-center gap-2 text-xs font-normal text-gray-400">
          {saveState === 'saving' && <Loader2 className="h-3 w-3 animate-spin" />}
          {saveState === 'saved' && (
            <>
              <Check className="h-3 w-3" /> Saved
            </>
          )}
          {isOpen ? <ChevronUp className="h-4 w-4" /> : <ChevronDown className="h-4 w-4" />}
        </span>
      </button>
      {isOpen && (
        <textarea
          value={notes}
          onChange={e => onChange(e.target.value)}
          rows={Math.min(14, Math.max(4, notes.split('\n').length + 1))}
          placeholder="Your notes about this meeting. Regenerate the summary to fold them into the AI notes."
          className="block w-full resize-y px-4 pb-3 text-sm leading-relaxed text-gray-900 outline-none placeholder:text-gray-400"
        />
      )}
    </div>
  );
}
