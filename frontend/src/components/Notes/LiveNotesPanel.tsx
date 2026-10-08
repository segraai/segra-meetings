'use client';

import { useEffect, useState } from 'react';
import { ChevronDown, ChevronUp, PenLine } from 'lucide-react';
import { readLiveNotes, writeLiveNotes } from '@/lib/live-notes';

/** Free-form notes typed while recording; saved with the meeting when it stops */
export function LiveNotesPanel({ collapsed, onToggle }: { collapsed: boolean; onToggle: () => void }) {
  const [notes, setNotes] = useState('');

  useEffect(() => {
    setNotes(readLiveNotes());
  }, []);

  return (
    <div className={`flex flex-col border-b border-gray-200 bg-white ${collapsed ? '' : 'min-h-0 flex-[2]'}`}>
      <button
        onClick={onToggle}
        className="flex shrink-0 items-center gap-2 px-6 py-2 text-left text-sm font-semibold text-gray-800 hover:bg-gray-50"
      >
        <PenLine className="h-4 w-4 text-blue-600" />
        My notes
        <span className="text-xs font-normal text-gray-400">
          {collapsed ? (notes.trim() ? 'hidden' : 'click to take notes') : 'saved with the meeting, used for AI notes'}
        </span>
        <span className="ml-auto text-gray-400">
          {collapsed ? <ChevronDown className="h-4 w-4" /> : <ChevronUp className="h-4 w-4" />}
        </span>
      </button>
      {!collapsed && (
        <textarea
          autoFocus
          value={notes}
          onChange={e => {
            setNotes(e.target.value);
            writeLiveNotes(e.target.value);
          }}
          placeholder={'Type anything: key points, decisions, follow-ups…\n\nThe AI will expand on these with details from the transcript.'}
          className="mx-auto w-full max-w-[750px] flex-1 resize-none px-6 pb-4 text-[15px] leading-relaxed text-gray-900 outline-none placeholder:text-gray-400"
        />
      )}
    </div>
  );
}
