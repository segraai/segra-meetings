import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

/**
 * Transcribe-after-meeting mode: the recording runs without live transcription and
 * the saved audio is transcribed once the meeting stops (reusing the retranscription
 * job in Rust). Jobs are queued here at module level so they survive navigation;
 * Rust runs one retranscription at a time.
 */

export type PostMeetingTranscriptionStatus = 'queued' | 'running' | 'error';

export interface PostMeetingTranscriptionState {
  status: PostMeetingTranscriptionStatus;
  progress: number;
  message: string;
  error?: string;
}

interface Job {
  meetingId: string;
  folderPath: string;
}

interface ProgressPayload {
  meeting_id: string;
  progress_percentage: number;
  message: string;
}

const RETRY_WHEN_BUSY_MS = 5000;

const states = new Map<string, PostMeetingTranscriptionState>();
const jobs = new Map<string, Job>();
const queue: string[] = [];
const completed = new Set<string>();
const subscribers = new Set<() => void>();
let runningMeetingId: string | null = null;
let eventsReady: Promise<void> | null = null;

function notify() {
  subscribers.forEach(fn => fn());
}

function setState(meetingId: string, state: PostMeetingTranscriptionState | null) {
  if (state) {
    states.set(meetingId, state);
  } else {
    states.delete(meetingId);
  }
  notify();
}

function finishRunning() {
  runningMeetingId = null;
  void runNext();
}

function ensureEventListeners(): Promise<void> {
  eventsReady ??= (async () => {
    await listen<ProgressPayload>('retranscription-progress', ({ payload }) => {
      if (payload.meeting_id !== runningMeetingId) return;
      setState(payload.meeting_id, {
        status: 'running',
        progress: payload.progress_percentage,
        message: payload.message,
      });
    });
    await listen<{ meeting_id: string }>('retranscription-complete', ({ payload }) => {
      if (payload.meeting_id !== runningMeetingId) return;
      completed.add(payload.meeting_id);
      jobs.delete(payload.meeting_id);
      setState(payload.meeting_id, null);
      finishRunning();
    });
    await listen<{ meeting_id: string; error: string }>('retranscription-error', ({ payload }) => {
      if (payload.meeting_id !== runningMeetingId) return;
      setState(payload.meeting_id, {
        status: 'error',
        progress: 0,
        message: 'Transcription failed',
        error: payload.error,
      });
      finishRunning();
    });
  })();
  return eventsReady;
}

/** Uses the transcription model and language chosen in settings. */
async function transcriptionSettings() {
  const config = await invoke<{ provider: string; model: string } | null>('api_get_transcript_config')
    .catch(() => null);
  const isParakeet = config?.provider === 'parakeet';
  const isLocalWhisper = config?.provider === 'localWhisper';

  let language: string | null = null;
  try {
    language = localStorage.getItem('primaryLanguage');
  } catch {
    // Storage unavailable: let Whisper detect the language
  }

  return {
    provider: isParakeet ? 'parakeet' : 'localWhisper',
    // Cloud providers can't run on a saved file here: fall back to the local Whisper default
    model: isParakeet || isLocalWhisper ? config?.model ?? null : null,
    // Parakeet has no language option; 'auto' means Whisper detects it
    language: isParakeet || !language || language === 'auto' ? null : language,
  };
}

async function runNext() {
  if (runningMeetingId || queue.length === 0) return;

  const meetingId = queue.shift()!;
  const job = jobs.get(meetingId);
  if (!job) return void runNext();

  runningMeetingId = meetingId;
  setState(meetingId, { status: 'running', progress: 0, message: 'Starting transcription...' });

  try {
    const { provider, model, language } = await transcriptionSettings();
    await invoke('start_retranscription_command', {
      meetingId,
      meetingFolderPath: job.folderPath,
      language,
      model,
      provider,
    });
  } catch (error) {
    const message = typeof error === 'string' ? error : error instanceof Error ? error.message : String(error);
    runningMeetingId = null;

    if (message.includes('already in progress')) {
      // A manual retranscription is running: wait for it and try again
      queue.unshift(meetingId);
      setState(meetingId, { status: 'queued', progress: 0, message: 'Waiting for another transcription to finish...' });
      setTimeout(() => void runNext(), RETRY_WHEN_BUSY_MS);
      return;
    }

    setState(meetingId, { status: 'error', progress: 0, message: 'Transcription failed', error: message });
    void runNext();
  }
}

export async function isTranscribeAfterMeetingEnabled(): Promise<boolean> {
  try {
    const prefs = await invoke<{ transcribe_after_meeting?: boolean }>('get_recording_preferences');
    return prefs.transcribe_after_meeting ?? false;
  } catch {
    return false; // Matches the Rust default
  }
}

/** Queue the saved recording of a meeting for transcription. */
export async function queuePostMeetingTranscription(meetingId: string, folderPath: string) {
  await ensureEventListeners();
  jobs.set(meetingId, { meetingId, folderPath });
  if (!queue.includes(meetingId) && runningMeetingId !== meetingId) {
    queue.push(meetingId);
  }
  setState(meetingId, { status: 'queued', progress: 0, message: 'Waiting to transcribe...' });
  void runNext();
}

/** Retry a failed job with the current settings. */
export function retryPostMeetingTranscription(meetingId: string) {
  const job = jobs.get(meetingId);
  if (job) void queuePostMeetingTranscription(meetingId, job.folderPath);
}

export function getPostMeetingTranscription(meetingId: string): PostMeetingTranscriptionState | null {
  return states.get(meetingId) ?? null;
}

/** True once a meeting's post-meeting transcription has finished in this session. */
export function wasTranscribedAfterMeeting(meetingId: string): boolean {
  return completed.has(meetingId);
}

export function subscribePostMeetingTranscription(fn: () => void): () => void {
  subscribers.add(fn);
  return () => subscribers.delete(fn);
}
