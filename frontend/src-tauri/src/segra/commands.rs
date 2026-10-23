//! Tauri commands for the Segra account: device sign-in (RFC 8628), sign-out, models and balance.
//!
//! Sign-in: `segra_start_sign_in` gets a code and opens the approval page in the browser;
//! `segra_finish_sign_in` then waits for the user to approve it and stores the API key. The key
//! never goes to the webview.

use std::time::{Duration, Instant};

use log::{info, warn};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use super::{base_url, error_text, http};
use crate::state::AppState;

const CLIENT_APP: &str = "Segra Meetings";

struct PendingSignIn {
    device_code: String,
    interval: Duration,
    expires_at: Instant,
}

static PENDING: Lazy<Mutex<Option<PendingSignIn>>> = Lazy::new(|| Mutex::new(None));

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SegraAccount {
    pub signed_in: bool,
    pub user_id: Option<String>,
    pub server: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SegraSignInStart {
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: u64,
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    verification_uri_complete: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Deserialize)]
struct DeviceTokenResponse {
    api_key: Option<String>,
    user_id: Option<String>,
    error: Option<String>,
}

/// Shown on the approval page and in the key's name ("Segra Meetings (laptop)").
fn host_name() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .ok()
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty())
        .or_else(|| {
            std::process::Command::new("hostname")
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|h| !h.is_empty())
        })
        .unwrap_or_default()
}

#[tauri::command]
pub async fn segra_get_account(state: tauri::State<'_, AppState>) -> Result<SegraAccount, String> {
    let pool = state.db_manager.pool();
    let signed_in = super::api_key(pool).await.map_err(|e| e.to_string())?.is_some();
    let user_id = if signed_in { super::user_id(pool).await.map_err(|e| e.to_string())? } else { None };
    Ok(SegraAccount { signed_in, user_id, server: base_url() })
}

/// Step 1: ask Segra for a sign-in code and open the approval page in the browser.
#[tauri::command]
pub async fn segra_start_sign_in() -> Result<SegraSignInStart, String> {
    let res = http()
        .post(format!("{}/device/code", base_url()))
        .json(&serde_json::json!({ "client_name": host_name(), "client_app": CLIENT_APP }))
        .send()
        .await
        .map_err(|e| format!("Could not reach Segra at {}: {}", base_url(), e))?;
    if !res.status().is_success() {
        return Err(format!("Segra sign-in failed: {}", error_text(res).await));
    }
    let code: DeviceCodeResponse = res
        .json()
        .await
        .map_err(|_| format!("{} is not a Segra server.", base_url()))?;
    *PENDING.lock().await = Some(PendingSignIn {
        device_code: code.device_code,
        interval: Duration::from_secs(code.interval.max(1)),
        expires_at: Instant::now() + Duration::from_secs(code.expires_in),
    });
    if let Err(e) = crate::api::api::open_external_url(code.verification_uri_complete.clone()).await {
        warn!("Could not open the Segra sign-in page: {}", e);
    }
    Ok(SegraSignInStart {
        user_code: code.user_code,
        verification_uri: code.verification_uri,
        verification_uri_complete: code.verification_uri_complete,
        expires_in: code.expires_in,
    })
}

/// Step 2: wait until the user approves (or denies) in the browser, then store the key.
#[tauri::command]
pub async fn segra_finish_sign_in(state: tauri::State<'_, AppState>) -> Result<SegraAccount, String> {
    loop {
        let (device_code, interval, expires_at) = match PENDING.lock().await.as_ref() {
            Some(p) => (p.device_code.clone(), p.interval, p.expires_at),
            None => return Err("Sign-in was cancelled.".into()),
        };
        if Instant::now() >= expires_at {
            *PENDING.lock().await = None;
            return Err("The sign-in code expired. Start again.".into());
        }
        tokio::time::sleep(interval).await;

        let res = http()
            .post(format!("{}/device/token", base_url()))
            .json(&serde_json::json!({ "device_code": device_code }))
            .send()
            .await;
        let body: DeviceTokenResponse = match res {
            Ok(r) => match r.json().await {
                Ok(b) => b,
                Err(_) => continue,
            },
            // Network blip: keep polling until the code expires.
            Err(_) => continue,
        };
        if let Some(key) = body.api_key {
            // Ignore the result if the user cancelled while this poll was in flight.
            let still_pending = PENDING.lock().await.take().is_some_and(|p| p.device_code == device_code);
            if !still_pending {
                return Err("Sign-in was cancelled.".into());
            }
            let pool = state.db_manager.pool();
            super::save_account(pool, Some(&key), body.user_id.as_deref())
                .await
                .map_err(|e| format!("Signed in, but could not save the account: {}", e))?;
            info!("Signed in to Segra as {:?}", body.user_id);
            return Ok(SegraAccount { signed_in: true, user_id: body.user_id, server: base_url() });
        }
        match body.error.as_deref() {
            Some("authorization_pending") | None => {}
            Some("access_denied") => {
                *PENDING.lock().await = None;
                return Err("Sign-in was denied in the browser.".into());
            }
            Some("expired_token") => {
                *PENDING.lock().await = None;
                return Err("The sign-in code expired. Start again.".into());
            }
            Some(other) => {
                *PENDING.lock().await = None;
                return Err(format!("Sign-in failed ({}). Start again.", other));
            }
        }
    }
}

#[tauri::command]
pub async fn segra_cancel_sign_in() -> Result<(), String> {
    *PENDING.lock().await = None;
    Ok(())
}

/// Forgets the key on this computer. (It stays listed under API keys on the Segra website, where
/// it can be revoked.)
#[tauri::command]
pub async fn segra_sign_out(state: tauri::State<'_, AppState>) -> Result<(), String> {
    super::save_account(state.db_manager.pool(), None, None).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn segra_list_models() -> Result<Vec<String>, String> {
    super::list_models().await
}

/// The account's balance as Segra reports it. Every account has its own wallet and a balance in credits.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SegraBalance {
    pub credits: Option<f64>,
    /// "ok", "setting_up" or "empty".
    pub status: Option<String>,
}

/// `None` when signed out.
#[tauri::command]
pub async fn segra_get_balance(state: tauri::State<'_, AppState>) -> Result<Option<SegraBalance>, String> {
    let Some(key) = super::api_key(state.db_manager.pool()).await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let res = http()
        .get(format!("{}/me", base_url()))
        .bearer_auth(key)
        .send()
        .await
        .map_err(|e| format!("Could not reach Segra: {}", e))?;
    if res.status().as_u16() == 401 {
        return Err("Your Segra sign-in has expired or was revoked. Sign in again.".into());
    }
    if !res.status().is_success() {
        return Err(error_text(res).await);
    }
    let me: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    Ok(Some(SegraBalance {
        credits: me.get("credits").and_then(|v| v.as_f64()),
        status: me.get("status").and_then(|v| v.as_str()).map(str::to_string),
    }))
}
