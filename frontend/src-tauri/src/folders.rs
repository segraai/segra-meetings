//! User folders for organizing meetings in the sidebar.

use crate::state::AppState;
use chrono::Utc;
use serde::Serialize;
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct MeetingFolder {
    pub id: String,
    pub name: String,
    pub sort_order: i64,
}

fn clean_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Folder name can't be empty".to_string());
    }
    Ok(name.chars().take(80).collect())
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<MeetingFolder>, sqlx::Error> {
    sqlx::query_as::<_, MeetingFolder>(
        "SELECT id, name, sort_order FROM meeting_folders ORDER BY sort_order, name COLLATE NOCASE",
    )
    .fetch_all(pool)
    .await
}

#[tauri::command]
pub async fn api_list_folders(state: tauri::State<'_, AppState>) -> Result<Vec<MeetingFolder>, String> {
    list(state.db_manager.pool()).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn api_create_folder(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<MeetingFolder, String> {
    let pool = state.db_manager.pool();
    let name = clean_name(&name)?;
    let next_order: (i64,) =
        sqlx::query_as("SELECT COALESCE(MAX(sort_order), -1) + 1 FROM meeting_folders")
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;
    let folder = MeetingFolder {
        id: format!("folder-{}", uuid::Uuid::new_v4()),
        name,
        sort_order: next_order.0,
    };
    sqlx::query("INSERT INTO meeting_folders (id, name, sort_order, created_at) VALUES (?, ?, ?, ?)")
        .bind(&folder.id)
        .bind(&folder.name)
        .bind(folder.sort_order)
        .bind(Utc::now())
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(folder)
}

#[tauri::command]
pub async fn api_rename_folder(
    state: tauri::State<'_, AppState>,
    folder_id: String,
    name: String,
) -> Result<(), String> {
    let name = clean_name(&name)?;
    sqlx::query("UPDATE meeting_folders SET name = ? WHERE id = ?")
        .bind(name)
        .bind(folder_id)
        .execute(state.db_manager.pool())
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Deletes a folder; its meetings stay and go back to "All meetings" only.
#[tauri::command]
pub async fn api_delete_folder(
    state: tauri::State<'_, AppState>,
    folder_id: String,
) -> Result<(), String> {
    let mut tx = state.db_manager.pool().begin().await.map_err(|e| e.to_string())?;
    // Foreign keys may not be enforced, so detach meetings explicitly
    sqlx::query("UPDATE meetings SET folder_id = NULL WHERE folder_id = ?")
        .bind(&folder_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM meeting_folders WHERE id = ?")
        .bind(&folder_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())
}

/// Moves a meeting into a folder, or out of any folder when `folder_id` is None.
#[tauri::command]
pub async fn api_move_meeting_to_folder(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    folder_id: Option<String>,
) -> Result<(), String> {
    let pool = state.db_manager.pool();
    if let Some(id) = &folder_id {
        let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM meeting_folders WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
        if exists.is_none() {
            return Err("Folder not found".to_string());
        }
    }
    sqlx::query("UPDATE meetings SET folder_id = ? WHERE id = ?")
        .bind(folder_id)
        .bind(meeting_id)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}
