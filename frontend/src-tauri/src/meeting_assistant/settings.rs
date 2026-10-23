use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "meeting_assistant.json";
const SETTINGS_KEY: &str = "settings";
const GOOGLE_TOKENS_KEY: &str = "google_tokens";

/// What to do when a Zoom/Meet/Teams call is detected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoRecordMode {
    /// Ignore detected calls
    Off,
    /// Show a popup asking whether to record
    Ask,
    /// Start recording immediately and notify
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AssistantSettings {
    /// OAuth client ID of a Google Cloud "Desktop app" credential
    pub google_client_id: String,
    /// Client secret of the same credential (Google requires it even for desktop apps)
    pub google_client_secret: String,
    pub auto_record_mode: AutoRecordMode,
    pub meeting_reminders_enabled: bool,
    /// How long before an event starts to show the "meeting starting" popup
    pub reminder_minutes_before: u32,
    /// Stop recording once the meeting app releases the microphone
    pub auto_stop_when_call_ends: bool,
}

impl Default for AssistantSettings {
    fn default() -> Self {
        Self {
            google_client_id: String::new(),
            google_client_secret: String::new(),
            auto_record_mode: AutoRecordMode::Ask,
            meeting_reminders_enabled: true,
            reminder_minutes_before: 1,
            auto_stop_when_call_ends: true,
        }
    }
}

/// The Google OAuth client built into this build (see build.rs), if any
pub fn builtin_google_client() -> Option<(&'static str, &'static str)> {
    match (option_env!("SEGRA_GOOGLE_CLIENT_ID"), option_env!("SEGRA_GOOGLE_CLIENT_SECRET")) {
        (Some(id), Some(secret)) if !id.is_empty() && !secret.is_empty() => Some((id, secret)),
        _ => None,
    }
}

/// The OAuth client to sign in with: the user's own when they entered one,
/// otherwise the built-in one.
pub fn google_client(config: &AssistantSettings) -> Option<(String, String)> {
    let (id, secret) = (config.google_client_id.trim(), config.google_client_secret.trim());
    if !id.is_empty() && !secret.is_empty() {
        return Some((id.to_string(), secret.to_string()));
    }
    builtin_google_client().map(|(id, secret)| (id.to_string(), secret.to_string()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleTokens {
    pub access_token: String,
    pub refresh_token: String,
    /// Unix seconds
    pub expires_at: i64,
    pub account_email: Option<String>,
    /// Google rejected the refresh token (expired or revoked): the user must sign in again
    #[serde(default)]
    pub needs_reconnect: bool,
}

pub fn load_settings<R: Runtime>(app: &AppHandle<R>) -> AssistantSettings {
    app.store(STORE_FILE)
        .ok()
        .and_then(|store| store.get(SETTINGS_KEY))
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

pub fn save_settings<R: Runtime>(app: &AppHandle<R>, settings: &AssistantSettings) -> Result<()> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| anyhow!("Failed to access store: {}", e))?;
    store.set(SETTINGS_KEY, serde_json::to_value(settings)?);
    store
        .save()
        .map_err(|e| anyhow!("Failed to save store to disk: {}", e))
}

pub fn load_google_tokens<R: Runtime>(app: &AppHandle<R>) -> Option<GoogleTokens> {
    app.store(STORE_FILE)
        .ok()
        .and_then(|store| store.get(GOOGLE_TOKENS_KEY))
        .and_then(|value| serde_json::from_value(value).ok())
}

pub fn save_google_tokens<R: Runtime>(
    app: &AppHandle<R>,
    tokens: Option<&GoogleTokens>,
) -> Result<()> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| anyhow!("Failed to access store: {}", e))?;
    match tokens {
        Some(tokens) => store.set(GOOGLE_TOKENS_KEY, serde_json::to_value(tokens)?),
        None => {
            store.delete(GOOGLE_TOKENS_KEY);
        }
    }
    store
        .save()
        .map_err(|e| anyhow!("Failed to save store to disk: {}", e))
}
