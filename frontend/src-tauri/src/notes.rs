//! The user's own notes for a meeting (typed during or after recording).

use crate::state::AppState;
use chrono::Utc;
use sqlx::SqlitePool;

pub async fn load(pool: &SqlitePool, meeting_id: &str) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT notes_markdown FROM meeting_notes WHERE meeting_id = ?")
            .bind(meeting_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.and_then(|(notes,)| notes).filter(|n| !n.trim().is_empty()))
}

/// Combines the user's notes with any custom prompt, so note generation builds on them
pub fn with_user_notes(custom_prompt: &str, notes: Option<&str>) -> String {
    match notes.map(str::trim).filter(|n| !n.is_empty()) {
        None => custom_prompt.to_string(),
        Some(notes) => {
            let mut prompt = format!(
                "The user took these notes during the meeting. Treat them as the most important \
                 points: keep everything they wrote, expand on it with details from the transcript, \
                 and correct obvious typos.\n<user_notes>\n{}\n</user_notes>",
                notes
            );
            if !custom_prompt.trim().is_empty() {
                prompt.push_str("\n\n");
                prompt.push_str(custom_prompt);
            }
            prompt
        }
    }
}

#[tauri::command]
pub async fn api_get_meeting_notes(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<Option<String>, String> {
    load(state.db_manager.pool(), &meeting_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn api_save_meeting_notes(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    notes_markdown: String,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO meeting_notes (meeting_id, notes_markdown, created_at, updated_at)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(meeting_id) DO UPDATE SET
            notes_markdown = excluded.notes_markdown,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(&meeting_id)
    .bind(&notes_markdown)
    .bind(&now)
    .bind(&now)
    .execute(state.db_manager.pool())
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_are_prepended_to_custom_prompt() {
        assert_eq!(with_user_notes("Be brief", None), "Be brief");
        assert_eq!(with_user_notes("", Some("   ")), "");
        let prompt = with_user_notes("Be brief", Some("- ship Friday"));
        assert!(prompt.contains("<user_notes>\n- ship Friday\n</user_notes>"));
        assert!(prompt.ends_with("Be brief"));
    }
}
