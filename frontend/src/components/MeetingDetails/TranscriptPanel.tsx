"use client";

import { TranscriptSegmentData } from '@/types';
import { VirtualizedTranscriptView } from '@/components/VirtualizedTranscriptView';
import { TranscriptButtonGroup } from './TranscriptButtonGroup';
import { FileText, Loader2 } from 'lucide-react';

interface TranscriptPanelProps {
  segments: TranscriptSegmentData[];
  onCopyTranscript: () => void;
  hasMore?: boolean;
  isLoadingMore?: boolean;
  totalCount?: number;
  loadedCount?: number;
  onLoadMore?: () => void;
  meetingId: string;
  meetingFolderPath?: string | null;
  onRefetchTranscripts?: () => Promise<void>;
  /** Progress (0-100) while the recording is still being transcribed */
  transcriptionProgress?: number | null;
}

/** The Transcript tab of a saved meeting */
export function TranscriptPanel({
  segments,
  onCopyTranscript,
  hasMore,
  isLoadingMore,
  totalCount,
  loadedCount,
  onLoadMore,
  meetingId,
  meetingFolderPath,
  onRefetchTranscripts,
  transcriptionProgress = null,
}: TranscriptPanelProps) {
  const count = totalCount ?? segments.length;

  if (count === 0) {
    const transcribing = transcriptionProgress !== null;
    return (
      <div className="flex h-full flex-col items-center justify-center p-8 text-center">
        {transcribing ? (
          <Loader2 className="mb-4 h-10 w-10 animate-spin text-blue-500" />
        ) : (
          <FileText className="mb-4 h-12 w-12 text-gray-300" />
        )}
        <h3 className="text-lg font-semibold text-gray-900">
          {transcribing ? 'Creating the transcript' : 'No transcript'}
        </h3>
        <p className="mt-1 max-w-md text-sm text-gray-500">
          {transcribing
            ? `The recording is being transcribed (${Math.round(transcriptionProgress)}%).`
            : 'No speech was transcribed for this meeting.'}
        </p>
      </div>
    );
  }

  return (
    <div className="flex h-full min-w-0 w-full flex-col bg-white @container">
      <div className="flex shrink-0 items-center gap-3 border-b border-gray-200 px-6 py-2">
        <span className="text-xs text-gray-500">{count} segments</span>
        <div className="ml-auto">
          <TranscriptButtonGroup
            transcriptCount={count}
            onCopyTranscript={onCopyTranscript}
            meetingId={meetingId}
            meetingFolderPath={meetingFolderPath}
            onRefetchTranscripts={onRefetchTranscripts}
          />
        </div>
      </div>

      <div className="flex-1 overflow-hidden pb-4">
        <VirtualizedTranscriptView
          segments={segments}
          isRecording={false}
          isPaused={false}
          isProcessing={false}
          isStopping={false}
          enableStreaming={false}
          showConfidence={true}
          disableAutoScroll={true}
          hasMore={hasMore}
          isLoadingMore={isLoadingMore}
          totalCount={totalCount}
          loadedCount={loadedCount}
          onLoadMore={onLoadMore}
        />
      </div>
    </div>
  );
}
