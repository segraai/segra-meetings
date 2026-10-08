import { invoke } from '@tauri-apps/api/core';

export type AutoRecordMode = 'off' | 'ask' | 'auto';

export interface AssistantSettings {
  google_client_id: string;
  google_client_secret: string;
  auto_record_mode: AutoRecordMode;
  meeting_reminders_enabled: boolean;
  reminder_minutes_before: number;
  auto_stop_when_call_ends: boolean;
}

export interface CalendarStatus {
  connected: boolean;
  account_email: string | null;
  has_credentials: boolean;
  needs_reconnect: boolean;
}

export interface CalendarEvent {
  id: string;
  title: string;
  description: string | null;
  start_time: string;
  end_time: string;
  is_all_day: boolean;
  meeting_url: string | null;
  html_link: string | null;
  attendees_json: string | null;
}

export interface MeetingOverview {
  id: string;
  title: string;
  created_at: string;
  duration_seconds: number | null;
  summary_preview: string | null;
}

export interface MeetingSearchHit {
  id: string;
  title: string;
  created_at: string;
  matched_in: 'title' | 'notes' | 'summary' | 'transcript';
  snippet: string;
}

export interface ChatTurn {
  role: 'user' | 'assistant';
  content: string;
}

export interface ChatAnswer {
  answer: string;
  sources: { id: string; title: string; created_at: string }[];
}

export const CALENDAR_UPDATED_EVENT = 'calendar-events-updated';

export const meetingAssistantService = {
  getSettings: () => invoke<AssistantSettings>('meeting_assistant_get_settings'),
  saveSettings: (settings: AssistantSettings) =>
    invoke<void>('meeting_assistant_save_settings', { settings }),

  getCalendarStatus: () => invoke<CalendarStatus>('calendar_get_status'),
  connectCalendar: () => invoke<string>('calendar_connect'),
  disconnectCalendar: () => invoke<void>('calendar_disconnect'),
  syncCalendar: () => invoke<number>('calendar_sync_now'),
  listEvents: (from: Date, to: Date) =>
    invoke<CalendarEvent[]>('calendar_list_events', {
      from: from.toISOString(),
      to: to.toISOString(),
    }),

  listMeetings: () => invoke<MeetingOverview[]>('api_list_meetings_overview'),
  searchMeetings: (query: string) => invoke<MeetingSearchHit[]>('api_search_meetings', { query }),
  askMeetings: (question: string, history: ChatTurn[]) =>
    invoke<ChatAnswer>('api_ask_meetings', { question, history }),

  openExternal: (url: string) => invoke<void>('open_external_url', { url }),

  popupAction: (action: 'record' | 'join' | 'stop' | 'dismiss', title?: string, meetingUrl?: string | null) =>
    invoke<void>('meeting_popup_action', { action, title, meetingUrl }),
};
