'use client';

import { AlertCircle, AudioLines, RotateCcw, Sparkles } from 'lucide-react';

export type NotesProgressState =
  | { phase: 'transcribing'; progress: number; message: string }
  | { phase: 'transcription-error'; error?: string; onRetry: () => void }
  | { phase: 'summarizing'; onCancel?: () => void };

/**
 * Full-area progress shown in the Notes tab while a meeting's transcript and notes
 * are being produced. Transcription reports real progress; writing the notes doesn't,
 * so that step shows an indeterminate bar.
 */
export function NotesProgress({ state }: { state: NotesProgressState }) {
  if (state.phase === 'transcription-error') {
    return (
      <div className="flex h-full flex-col items-center justify-center p-8 text-center">
        <AlertCircle className="mb-4 h-12 w-12 text-red-400" />
        <h3 className="text-lg font-semibold text-gray-900">Couldn&apos;t transcribe the recording</h3>
        {state.error && (
          <p role="alert" className="mt-2 max-w-md rounded-md bg-red-50 px-3 py-2 text-sm text-red-700">
            {state.error}
          </p>
        )}
        <button
          type="button"
          onClick={state.onRetry}
          className="mt-6 inline-flex items-center gap-2 rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
        >
          <RotateCcw className="h-4 w-4" />
          Try again
        </button>
      </div>
    );
  }

  const transcribing = state.phase === 'transcribing';
  const Icon = transcribing ? AudioLines : Sparkles;

  return (
    <div className="flex h-full flex-col items-center justify-center p-8 text-center" role="status" aria-live="polite">
      <div className="mb-5 flex h-16 w-16 items-center justify-center rounded-full bg-blue-50">
        <Icon className="h-8 w-8 animate-pulse text-blue-600" />
      </div>
      <p className="text-xs font-semibold uppercase tracking-wide text-blue-700">
        Step {transcribing ? 1 : 2} of 2
      </p>
      <h3 className="mt-1 text-xl font-semibold text-gray-900">
        {transcribing ? 'Transcribing the recording' : 'Writing your notes'}
      </h3>
      <p className="mt-2 max-w-md text-sm text-gray-500">
        {transcribing
          ? 'Turning the meeting audio into text. Your notes are written right after.'
          : 'Summarizing the transcript and your own notes.'}
      </p>

      <div className="mt-8 w-full max-w-lg">
        <div className="h-2.5 overflow-hidden rounded-full bg-blue-100">
          {transcribing ? (
            <div
              className="h-full rounded-full bg-blue-600 transition-[width] duration-500"
              style={{ width: `${Math.max(state.progress, 2)}%` }}
            />
          ) : (
            <div className="animate-indeterminate h-full w-1/3 rounded-full bg-blue-600" />
          )}
        </div>
        {transcribing && (
          <div className="mt-2 flex justify-between gap-4 text-xs text-gray-500">
            <span className="truncate">{state.message}</span>
            <span className="shrink-0 tabular-nums">{Math.round(state.progress)}%</span>
          </div>
        )}
      </div>
      {state.phase === 'summarizing' && state.onCancel && (
        <button
          type="button"
          onClick={state.onCancel}
          className="mt-6 text-xs text-gray-500 underline-offset-2 hover:text-gray-800 hover:underline"
        >
          Stop generating
        </button>
      )}
    </div>
  );
}
