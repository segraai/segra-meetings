"use client";
import { MeetingNotesEditor } from '@/components/Notes/MeetingNotesEditor';
import { RecordingPlayer } from '@/components/MeetingDetails/RecordingPlayer';
import type { NotesProgressState } from '@/components/MeetingDetails/NotesProgress';
import { useState, useEffect, useRef } from 'react';
import { motion } from 'framer-motion';
import { MeetingSummary, SummaryProcessResponse } from '@/types';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import Analytics from '@/lib/analytics';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import { TranscriptPanel } from '@/components/MeetingDetails/TranscriptPanel';
import { SummaryPanel } from '@/components/MeetingDetails/SummaryPanel';
import { MeetingDetailsTabs, type MeetingDetailsTab } from '@/components/MeetingDetails/MeetingDetailsTabs';
import { ModelConfig } from '@/components/ModelSettingsModal';

// Custom hooks
import { useMeetingData } from '@/hooks/meeting-details/useMeetingData';
import { useSummaryGeneration } from '@/hooks/meeting-details/useSummaryGeneration';
import { useTemplates } from '@/hooks/meeting-details/useTemplates';
import { useCopyOperations } from '@/hooks/meeting-details/useCopyOperations';
import { useMeetingOperations } from '@/hooks/meeting-details/useMeetingOperations';
import { useConfig } from '@/contexts/ConfigContext';
import { listen } from '@tauri-apps/api/event';
import { usePostMeetingTranscription } from '@/hooks/usePostMeetingTranscription';
import { retryPostMeetingTranscription, wasTranscribedAfterMeeting } from '@/lib/post-meeting-transcription';

export default function PageContent({
  meeting,
  summaryData,
  initialSummary,
  shouldAutoGenerate = false,
  isAwaitingAutoSummary = false,
  onAutoGenerateComplete,
  onMeetingUpdated,
  onRefetchTranscripts,
  // Pagination props for efficient transcript loading
  segments,
  hasMore,
  isLoadingMore,
  totalCount,
  loadedCount,
  onLoadMore,
}: {
  meeting: any;
  summaryData: MeetingSummary | null;
  initialSummary: SummaryProcessResponse | null;
  shouldAutoGenerate?: boolean;
  /** A just-recorded meeting whose summary will be generated automatically */
  isAwaitingAutoSummary?: boolean;
  onAutoGenerateComplete?: () => void;
  onMeetingUpdated?: () => Promise<void>;
  onRefetchTranscripts?: () => Promise<void>;
  // Pagination props
  segments?: any[];
  hasMore?: boolean;
  isLoadingMore?: boolean;
  totalCount?: number;
  loadedCount?: number;
  onLoadMore?: () => void;
}) {
  console.log('📄 PAGE CONTENT: Initializing with data:', {
    meetingId: meeting.id,
    summaryDataKeys: summaryData ? Object.keys(summaryData) : null,
    transcriptsCount: meeting.transcripts?.length
  });

  // Extra context for the summary now comes from the Raw notes tab
  const customPrompt = '';
  const [activeTab, setActiveTab] = useState<MeetingDetailsTab>('notes');

  // Ref to store the modal open function from SummaryGeneratorButtonGroup
  const openModelSettingsRef = useRef<(() => void) | null>(null);
  const autoGenerationStartedMeetingIdRef = useRef<string | null>(null);

  // Sidebar context
  const { serverAddress } = useSidebar();

  // Get model config from ConfigContext
  const { modelConfig, setModelConfig, isModelConfigLoading } = useConfig();

  // Custom hooks
  const meetingData = useMeetingData({ meeting, summaryData, onMeetingUpdated });
  const templates = useTemplates();

  // Callback to register the modal open function
  const handleRegisterModalOpen = (openFn: () => void) => {
    console.log('📝 Registering modal open function in PageContent');
    openModelSettingsRef.current = openFn;
  };

  // Callback to trigger modal open (called from error handler)
  const handleOpenModelSettings = () => {
    console.log('🔔 Opening model settings from PageContent');
    if (openModelSettingsRef.current) {
      openModelSettingsRef.current();
    } else {
      console.warn('⚠️ Modal open function not yet registered');
    }
  };

  // Save model config to backend database and sync via event
  const handleSaveModelConfig = async (config?: ModelConfig) => {
    if (!config) return;
    try {
      await invoke('api_save_model_config', {
        provider: config.provider,
        model: config.model,
        whisperModel: config.whisperModel,
        apiKey: config.apiKey ?? null,
        ollamaEndpoint: config.ollamaEndpoint ?? null,
      });

      // Emit event so ConfigContext and other listeners stay in sync
      const { emit } = await import('@tauri-apps/api/event');
      await emit('model-config-updated', config);

    } catch (error) {
      console.error('Failed to save model config:', error);
      toast.error('Failed to save model settings');
    }
  };

  const summaryGeneration = useSummaryGeneration({
    initialSummary,
    meeting,
    transcripts: meetingData.transcripts,
    modelConfig: modelConfig,
    isModelConfigLoading,
    selectedTemplate: templates.selectedTemplate,
    onMeetingUpdated,
    updateMeetingTitle: meetingData.updateMeetingTitle,
    setAiSummary: meetingData.setAiSummary,
    onOpenModelSettings: handleOpenModelSettings,
  });

  const copyOperations = useCopyOperations({
    meeting,
    transcripts: meetingData.transcripts,
    meetingTitle: meetingData.meetingTitle,
    aiSummary: meetingData.aiSummary,
    blockNoteSummaryRef: meetingData.blockNoteSummaryRef,
  });

  const meetingOperations = useMeetingOperations({
    meeting,
  });

  // Track page view
  useEffect(() => {
    Analytics.trackPageView('meeting_details');
  }, []);

  // Transcribe-after-meeting: show the job's progress and load the transcript when it
  // finishes (the summary auto-generates once transcripts are present).
  const postMeetingTranscription = usePostMeetingTranscription(meeting.id);
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listen<{ meeting_id: string }>('retranscription-complete', ({ payload }) => {
      if (payload.meeting_id === meeting.id) void onRefetchTranscripts?.();
    }).then(fn => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [meeting.id, onRefetchTranscripts]);

  // Each meeting opens on its Notes tab
  useEffect(() => {
    setActiveTab('notes');
  }, [meeting.id]);

  // What the Notes tab shows while the transcript or the summary is still being made
  const hasTranscripts = meetingData.transcripts.length > 0;
  const notesProgress: NotesProgressState | null = postMeetingTranscription
    ? postMeetingTranscription.status === 'error'
      ? {
          phase: 'transcription-error',
          error: postMeetingTranscription.error,
          onRetry: () => retryPostMeetingTranscription(meeting.id),
        }
      : {
          phase: 'transcribing',
          progress: postMeetingTranscription.progress,
          message: postMeetingTranscription.message,
        }
    : !meetingData.aiSummary
      && summaryGeneration.summaryStatus === 'idle'
      && isAwaitingAutoSummary
      // Transcript just finished: it's being reloaded before the summary starts
      && (hasTranscripts || wasTranscribedAfterMeeting(meeting.id))
      ? { phase: 'summarizing' }
      : null;

  // After editing raw notes: regenerate so the AI notes include them
  const regenerateWithNotes = hasTranscripts
    ? () => {
        setActiveTab('notes');
        void (meetingData.aiSummary
          ? summaryGeneration.handleRegenerateSummary()
          : summaryGeneration.handleGenerateSummary(''));
      }
    : undefined;

  // Auto-generate only after the model configuration has settled.
  useEffect(() => {
    if (
      !shouldAutoGenerate
      || summaryGeneration.summaryStatus !== 'idle'
      || isModelConfigLoading
      || meetingData.transcripts.length === 0
      || autoGenerationStartedMeetingIdRef.current === meeting.id
    ) {
      return;
    }

    autoGenerationStartedMeetingIdRef.current = meeting.id;
    console.log(`🤖 Auto-generating summary with ${modelConfig.provider}/${modelConfig.model}...`);
    onAutoGenerateComplete?.();
    void summaryGeneration.handleGenerateSummary('');
  }, [
    shouldAutoGenerate,
    meeting.id,
    meetingData.transcripts.length,
    isModelConfigLoading,
    modelConfig.provider,
    modelConfig.model,
    summaryGeneration.handleGenerateSummary,
    summaryGeneration.summaryStatus,
    onAutoGenerateComplete,
  ]);

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.3, ease: 'easeOut' }}
      className="flex flex-col h-screen min-w-0 bg-white"
    >
      <MeetingDetailsTabs
        activeTab={activeTab}
        onTabChange={setActiveTab}
        content={{
          notes: (
            <SummaryPanel
              meeting={meeting}
              meetingTitle={meetingData.meetingTitle}
              summaryRef={meetingData.blockNoteSummaryRef}
              isSummaryDirty={meetingData.isSummaryDirty}
              onCopySummary={copyOperations.handleCopySummary}
              aiSummary={meetingData.aiSummary}
              summaryStatus={summaryGeneration.summaryStatus}
              transcripts={meetingData.transcripts}
              modelConfig={modelConfig}
              setModelConfig={setModelConfig}
              onSaveModelConfig={handleSaveModelConfig}
              onGenerateSummary={summaryGeneration.handleGenerateSummary}
              onStopGeneration={summaryGeneration.handleStopGeneration}
              customPrompt={customPrompt}
              onSaveSummary={meetingData.handleSaveSummary}
              onSummaryChange={meetingData.handleSummaryChange}
              onDirtyChange={meetingData.setIsSummaryDirty}
              summaryError={summaryGeneration.summaryError}
              onRegenerateSummary={summaryGeneration.handleRegenerateSummary}
              getSummaryStatusMessage={summaryGeneration.getSummaryStatusMessage}
              availableTemplates={templates.availableTemplates}
              selectedTemplate={templates.selectedTemplate}
              onTemplateSelect={templates.handleTemplateSelection}
              isModelConfigLoading={isModelConfigLoading}
              onOpenModelSettings={handleRegisterModalOpen}
              progress={notesProgress}
            />
          ),
          transcript: (
            <TranscriptPanel
              segments={segments ?? []}
              onCopyTranscript={copyOperations.handleCopyTranscript}
              hasMore={hasMore}
              isLoadingMore={isLoadingMore}
              totalCount={totalCount}
              loadedCount={loadedCount}
              onLoadMore={onLoadMore}
              meetingId={meeting.id}
              meetingFolderPath={meeting.folder_path}
              onRefetchTranscripts={onRefetchTranscripts}
              transcriptionProgress={postMeetingTranscription?.status === 'running' || postMeetingTranscription?.status === 'queued'
                ? postMeetingTranscription.progress
                : null}
            />
          ),
          rawNotes: <MeetingNotesEditor meetingId={meeting.id} onRegenerate={regenerateWithNotes} />,
          recording: (
            <RecordingPlayer
              meetingId={meeting.id}
              active={activeTab === 'recording'}
              onOpenFolder={meetingOperations.handleOpenMeetingFolder}
            />
          ),
        }}
      />
    </motion.div>
  );
}
