// Notes typed during a recording live here until the meeting is saved, so they
// survive reloads and crashes. Cleared once they are stored with the meeting.
const LIVE_NOTES_KEY = 'liveMeetingNotes';

export function readLiveNotes(): string {
  try {
    return localStorage.getItem(LIVE_NOTES_KEY) ?? '';
  } catch {
    return '';
  }
}

export function writeLiveNotes(notes: string) {
  try {
    localStorage.setItem(LIVE_NOTES_KEY, notes);
  } catch {
    // Storage unavailable: notes stay in memory only
  }
}

export function clearLiveNotes() {
  try {
    localStorage.removeItem(LIVE_NOTES_KEY);
  } catch {
    // ignore
  }
}
