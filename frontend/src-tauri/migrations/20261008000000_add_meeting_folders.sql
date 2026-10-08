-- User-created folders for organizing meetings. A meeting is in at most one folder;
-- meetings without a folder appear only under "All meetings".
CREATE TABLE IF NOT EXISTS meeting_folders (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

ALTER TABLE meetings ADD COLUMN folder_id TEXT REFERENCES meeting_folders(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_meetings_folder_id ON meetings(folder_id);
