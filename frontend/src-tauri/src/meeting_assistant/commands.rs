use super::calendar_store::{CalendarEvent, CalendarEventsRepository};
use super::settings::{self, AssistantSettings};
use super::{google_calendar, popup, CALENDAR_UPDATED_EVENT};
use crate::state::AppState;
use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

#[derive(Serialize)]
pub struct CalendarStatus {
    pub connected: bool,
    pub account_email: Option<String>,
    pub has_credentials: bool,
}

#[tauri::command]
pub async fn meeting_assistant_get_settings<R: Runtime>(
    app: AppHandle<R>,
) -> Result<AssistantSettings, String> {
    Ok(settings::load_settings(&app))
}

#[tauri::command]
pub async fn meeting_assistant_save_settings<R: Runtime>(
    app: AppHandle<R>,
    settings: AssistantSettings,
) -> Result<(), String> {
    settings::save_settings(&app, &settings).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn calendar_get_status<R: Runtime>(app: AppHandle<R>) -> Result<CalendarStatus, String> {
    let config = settings::load_settings(&app);
    Ok(CalendarStatus {
        connected: google_calendar::is_connected(&app),
        account_email: google_calendar::account_email(&app),
        has_credentials: !config.google_client_id.trim().is_empty()
            && !config.google_client_secret.trim().is_empty(),
    })
}

/// Opens the Google sign-in page and waits (up to 5 minutes) for the user to finish,
/// then runs a first sync. Returns the connected account email.
#[tauri::command]
pub async fn calendar_connect<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let email = google_calendar::connect(&app)
        .await
        .map_err(|e| format!("{:#}", e))?;
    match google_calendar::sync(&app, state.db_manager.pool()).await {
        Ok(_) => {
            let _ = app.emit(CALENDAR_UPDATED_EVENT, ());
        }
        Err(e) => google_calendar::log_sync_error(&e),
    }
    Ok(email)
}

#[tauri::command]
pub async fn calendar_disconnect<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    google_calendar::disconnect(&app, state.db_manager.pool())
        .await
        .map_err(|e| e.to_string())?;
    let _ = app.emit(CALENDAR_UPDATED_EVENT, ());
    Ok(())
}

#[tauri::command]
pub async fn calendar_sync_now<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    let count = google_calendar::sync(&app, state.db_manager.pool())
        .await
        .map_err(|e| format!("{:#}", e))?;
    let _ = app.emit(CALENDAR_UPDATED_EVENT, ());
    Ok(count)
}

#[tauri::command]
pub async fn calendar_list_events(
    state: tauri::State<'_, AppState>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<CalendarEvent>, String> {
    CalendarEventsRepository::list_between(state.db_manager.pool(), from, to)
        .await
        .map_err(|e| e.to_string())
}

/// Handles a button press in the meeting popup window.
/// `action`: "record" | "join" (open meeting link and record) | "stop" | "dismiss"
#[tauri::command]
pub async fn meeting_popup_action<R: Runtime>(
    app: AppHandle<R>,
    action: String,
    title: Option<String>,
    meeting_url: Option<String>,
) -> Result<(), String> {
    popup::close(&app);
    match action.as_str() {
        "join" => {
            if let Some(url) = meeting_url.filter(|u| u.starts_with("https://") || u.starts_with("http://")) {
                crate::api::open_external_url(url).await?;
            }
            start_if_idle(&app, title.as_deref()).await;
        }
        "record" => start_if_idle(&app, title.as_deref()).await,
        "stop" => {
            if crate::audio::recording_commands::is_recording().await {
                crate::tray::stop_recording_handler(&app);
            }
        }
        "dismiss" => {}
        other => return Err(format!("Unknown popup action: {}", other)),
    }
    Ok(())
}

async fn start_if_idle<R: Runtime>(app: &AppHandle<R>, title: Option<&str>) {
    if !crate::audio::recording_commands::is_recording().await {
        crate::tray::request_start_recording(app, title);
    }
}
