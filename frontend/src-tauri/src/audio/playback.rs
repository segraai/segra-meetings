// audio/playback.rs
//
// Playback of a saved meeting's recording in the meeting page's player.
//
// The webview can't reliably stream media from the asset protocol on Linux
// (WebKitGTK), so the file is handed over as raw bytes and played from a Blob.
// Files are looked up from the database by meeting id, so the frontend can't
// read arbitrary paths this way.

use std::path::{Path, PathBuf};

use log::info;
use tauri::{ipc::Response, AppHandle, Manager, Runtime};

use crate::state::AppState;

/// Path of a meeting's recorded audio, or None when it has no saved recording.
async fn meeting_audio_file<R: Runtime>(app: &AppHandle<R>, meeting_id: &str) -> Result<Option<PathBuf>, String> {
    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| "App state not available".to_string())?;

    let folder_path: Option<String> =
        sqlx::query_scalar("SELECT folder_path FROM meetings WHERE id = ?")
            .bind(meeting_id)
            .fetch_optional(state.db_manager.pool())
            .await
            .map_err(|e| format!("Failed to look up meeting: {}", e))?
            .flatten();

    let Some(folder_path) = folder_path.filter(|p| !p.trim().is_empty()) else {
        return Ok(None);
    };

    Ok(super::retranscription::find_audio_file(Path::new(&folder_path)).ok())
}

/// Path of a meeting's recorded audio, for display. Ok(None) means no saved recording.
#[tauri::command]
pub async fn get_meeting_audio_path<R: Runtime>(
    app: AppHandle<R>,
    meeting_id: String,
) -> Result<Option<String>, String> {
    Ok(meeting_audio_file(&app, &meeting_id)
        .await?
        .map(|path| path.to_string_lossy().to_string()))
}

/// The bytes of a meeting's recorded audio, sent as a raw (non-JSON) response.
#[tauri::command]
pub async fn read_meeting_audio<R: Runtime>(
    app: AppHandle<R>,
    meeting_id: String,
) -> Result<Response, String> {
    let path = meeting_audio_file(&app, &meeting_id)
        .await?
        .ok_or_else(|| "This meeting has no saved recording".to_string())?;

    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    info!("Loaded recording for meeting {} ({} bytes)", meeting_id, bytes.len());
    Ok(Response::new(bytes))
}
