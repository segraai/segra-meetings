import { useCallback, useEffect, useState, useSyncExternalStore } from 'react';
import {
  getPostMeetingTranscription,
  isTranscribeAfterMeetingEnabled,
  subscribePostMeetingTranscription,
  type PostMeetingTranscriptionState,
} from '@/lib/post-meeting-transcription';

/** Live state of a meeting's transcribe-after-meeting job, or null when there is none. */
export function usePostMeetingTranscription(meetingId: string): PostMeetingTranscriptionState | null {
  const getSnapshot = useCallback(() => getPostMeetingTranscription(meetingId), [meetingId]);
  return useSyncExternalStore(subscribePostMeetingTranscription, getSnapshot, () => null);
}

/** Whether the current recording is in transcribe-after-meeting mode (no live transcript). */
export function useTranscribeAfterMeetingMode(isRecording: boolean): boolean {
  const [enabled, setEnabled] = useState(false);

  useEffect(() => {
    if (!isRecording) return;
    let cancelled = false;
    isTranscribeAfterMeetingEnabled().then(value => {
      if (!cancelled) setEnabled(value);
    });
    return () => { cancelled = true; };
  }, [isRecording]);

  return enabled;
}
