-- Calendar events synced from Google Calendar (read-only mirror)
CREATE TABLE IF NOT EXISTS calendar_events (
    id TEXT PRIMARY KEY NOT NULL,          -- "<calendar_id>:<event_id>"
    calendar_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    start_time TEXT NOT NULL,              -- RFC 3339, UTC
    end_time TEXT NOT NULL,                -- RFC 3339, UTC
    is_all_day INTEGER NOT NULL DEFAULT 0,
    meeting_url TEXT,                      -- Meet/Zoom/Teams link if found
    html_link TEXT,                        -- Link to the event in Google Calendar
    attendees_json TEXT,                   -- JSON array of {email, name}
    reminded_at TEXT,                      -- When the "meeting starting" popup was shown
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_calendar_events_start ON calendar_events(start_time);
