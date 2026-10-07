// Groups transcript segments into readable paragraphs. A paragraph (and its
// timestamp) starts at most once per TIMESTAMP_INTERVAL_SECONDS, or after a
// pause of at least PARAGRAPH_GAP_SECONDS.

export const TIMESTAMP_INTERVAL_SECONDS = 60;
export const PARAGRAPH_GAP_SECONDS = 8;

interface TimedSegment {
  id: string;
  start?: number;
  end?: number;
}

export function getParagraphStarts(segments: TimedSegment[]): Set<string> {
  const starts = new Set<string>();
  let lastShown: number | undefined;
  let prevEnd: number | undefined;

  for (const segment of segments) {
    const { start } = segment;
    const isFirst = starts.size === 0;
    const intervalElapsed =
      start !== undefined && lastShown !== undefined && start - lastShown >= TIMESTAMP_INTERVAL_SECONDS;
    const longPause =
      start !== undefined && prevEnd !== undefined && start - prevEnd >= PARAGRAPH_GAP_SECONDS;

    if (isFirst || intervalElapsed || longPause) {
      starts.add(segment.id);
      lastShown = start;
    }
    prevEnd = segment.end ?? start;
  }
  return starts;
}

export function formatRecordingTime(seconds: number | undefined): string {
  if (seconds === undefined) return '[--:--]';
  const totalSeconds = Math.floor(seconds);
  const minutes = Math.floor(totalSeconds / 60);
  const secs = totalSeconds % 60;
  return `[${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}]`;
}
