//! Meeting assistant: Google Calendar sync, "meeting starting" reminders, and
//! detection of Zoom/Meet/Teams calls to prompt for (or start) a recording and stop it
//! when the call ends.

pub mod calendar_store;
pub mod commands;
pub mod detector;
pub mod google_calendar;
pub mod popup;
pub mod settings;

use crate::state::AppState;
use calendar_store::CalendarEventsRepository;
use chrono::Utc;
use detector::CallApp;
use log::{info, warn};
use popup::{PopupContent, PopupKind};
use settings::{AssistantSettings, AutoRecordMode};
use sqlx::SqlitePool;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_notification::NotificationExt;

const CALENDAR_TICK: Duration = Duration::from_secs(30);
const CALENDAR_SYNC_INTERVAL: Duration = Duration::from_secs(5 * 60);
const DETECTION_TICK: Duration = Duration::from_secs(5);
/// How long the meeting app must be off the mic before the call counts as ended
const CALL_END_GRACE: Duration = Duration::from_secs(20);

pub const CALENDAR_UPDATED_EVENT: &str = "calendar-events-updated";

/// Starts the background calendar and call-detection loops. Call after the database is ready.
pub fn start<R: Runtime>(app: &AppHandle<R>) {
    tauri::async_runtime::spawn(calendar_loop(app.clone()));
    tauri::async_runtime::spawn(call_detection_loop(app.clone()));
}

fn pool<R: Runtime>(app: &AppHandle<R>) -> Option<SqlitePool> {
    app.try_state::<AppState>()
        .map(|state| state.db_manager.pool().clone())
}

fn notify<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) {
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        warn!("Failed to show notification '{}': {}", title, e);
    }
}

async fn calendar_loop<R: Runtime>(app: AppHandle<R>) {
    let mut last_sync: Option<Instant> = None;
    loop {
        if let Some(pool) = pool(&app) {
            if google_calendar::is_connected(&app)
                && !google_calendar::needs_reconnect(&app)
                && last_sync.map_or(true, |t| t.elapsed() >= CALENDAR_SYNC_INTERVAL)
            {
                last_sync = Some(Instant::now());
                match google_calendar::sync(&app, &pool).await {
                    Ok(count) => {
                        info!("Synced {} calendar events", count);
                        let _ = app.emit(CALENDAR_UPDATED_EVENT, ());
                    }
                    Err(e) => {
                        google_calendar::log_sync_error(&e);
                        if google_calendar::needs_reconnect(&app) {
                            notify(
                                &app,
                                "Google Calendar disconnected",
                                "Your Google sign-in expired. Reconnect in Settings → Calendar & Meetings.",
                            );
                            let _ = app.emit(CALENDAR_UPDATED_EVENT, ());
                        }
                    }
                }
            }

            let settings = settings::load_settings(&app);
            if settings.meeting_reminders_enabled {
                remind_due_events(&app, &pool, &settings).await;
            }
        }
        tokio::time::sleep(CALENDAR_TICK).await;
    }
}

async fn remind_due_events<R: Runtime>(
    app: &AppHandle<R>,
    pool: &SqlitePool,
    settings: &AssistantSettings,
) {
    let now = Utc::now();
    let lead = chrono::Duration::minutes(settings.reminder_minutes_before as i64);
    let due = match CalendarEventsRepository::due_for_reminder(pool, now, lead).await {
        Ok(due) => due,
        Err(e) => {
            warn!("Failed to query due calendar events: {}", e);
            return;
        }
    };
    let recording = crate::audio::recording_commands::is_recording().await;

    for event in due {
        if let Err(e) = CalendarEventsRepository::mark_reminded(pool, &event.id, now).await {
            warn!("Failed to mark event reminded: {}", e);
            continue;
        }
        // Already recording: the popup would only get in the way
        if recording {
            continue;
        }

        let minutes = (event.start_time - now).num_minutes();
        let when = if minutes >= 1 {
            format!("Starts in {} min", minutes)
        } else {
            "Starting now".to_string()
        };
        notify(app, &event.title, &when);
        popup::show(
            app,
            PopupContent {
                kind: PopupKind::Upcoming,
                title: &event.title,
                subtitle: &when,
                meeting_url: event.meeting_url.as_deref(),
            },
        );
    }
}

async fn call_detection_loop<R: Runtime>(app: AppHandle<R>) {
    let mut active_call: Option<CallApp> = None;
    let mut prompted = false;
    let mut absent_since: Option<Instant> = None;
    let mut seen_while_recording = false;

    loop {
        tokio::time::sleep(DETECTION_TICK).await;

        let settings = settings::load_settings(&app);
        if settings.auto_record_mode == AutoRecordMode::Off && !settings.auto_stop_when_call_ends {
            active_call = None;
            continue;
        }

        let detected = detector::detect_active_call().await;
        let recording = crate::audio::recording_commands::is_recording().await;
        if !recording {
            seen_while_recording = false;
        }

        match detected {
            Some(call) => {
                absent_since = None;
                if active_call.is_none() {
                    info!("Call detected: {}", call.display_name());
                    prompted = false;
                }
                active_call = Some(call);
                if recording {
                    seen_while_recording = true;
                } else if !prompted && settings.auto_record_mode != AutoRecordMode::Off {
                    prompted = true;
                    on_call_started(&app, call, settings.auto_record_mode).await;
                }
            }
            None if active_call.is_some() => {
                let since = *absent_since.get_or_insert_with(Instant::now);
                if since.elapsed() < CALL_END_GRACE {
                    continue;
                }
                info!("Call ended");
                active_call = None;
                absent_since = None;
                if prompted && !recording {
                    // Drop the "record this call?" prompt if it was never answered
                    popup::close(&app);
                }
                if recording && seen_while_recording && settings.auto_stop_when_call_ends {
                    info!("Stopping recording because the call ended");
                    notify(&app, "Call ended", "Recording stopped. Generating notes…");
                    crate::tray::stop_recording_handler(&app);
                }
                seen_while_recording = false;
            }
            None => {}
        }
    }
}

async fn on_call_started<R: Runtime>(app: &AppHandle<R>, call: CallApp, mode: AutoRecordMode) {
    let event = match pool(app) {
        Some(pool) => CalendarEventsRepository::current_event(&pool, Utc::now())
            .await
            .ok()
            .flatten(),
        None => None,
    };
    let title = event
        .as_ref()
        .map(|e| e.title.clone())
        .unwrap_or_else(|| format!("{} call", call.display_name()));
    let meeting_url = event.as_ref().and_then(|e| e.meeting_url.clone());

    // A browser on the mic is only a meeting for sure when the calendar says so;
    // otherwise ask instead of recording on our own.
    let confident = call != CallApp::Browser || meeting_url.is_some();

    if mode == AutoRecordMode::Auto && confident {
        crate::tray::request_start_recording(app, Some(&title));
        notify(app, "Recording started", &title);
        popup::show(
            app,
            PopupContent {
                kind: PopupKind::Recording,
                title: &title,
                subtitle: &format!("{} detected · recording", call.display_name()),
                meeting_url: None,
            },
        );
    } else {
        popup::show(
            app,
            PopupContent {
                kind: PopupKind::Detected,
                title: &title,
                subtitle: &format!("{} detected · record this meeting?", call.display_name()),
                meeting_url: None,
            },
        );
    }
}
