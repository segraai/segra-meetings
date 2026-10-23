//! Google Calendar integration: OAuth 2.0 installed-app flow (PKCE + loopback redirect)
//! and a read-only sync of the primary calendar into `calendar_events`.

use super::calendar_store::{CalendarEvent, CalendarEventsRepository};
use super::settings::{self, GoogleTokens};
use anyhow::{anyhow, Context, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use log::{info, warn};
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tauri::{AppHandle, Runtime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";
const CALENDAR_API: &str = "https://www.googleapis.com/calendar/v3";
const SCOPE: &str = "https://www.googleapis.com/auth/calendar.readonly";
const PRIMARY_CALENDAR: &str = "primary";
const OAUTH_TIMEOUT_SECS: u64 = 300;

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("Meetily/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
    refresh_token: Option<String>,
}

/// Runs the browser sign-in flow and stores the resulting tokens. Returns the account email.
pub async fn connect<R: Runtime>(app: &AppHandle<R>) -> Result<String> {
    let config = settings::load_settings(app);
    let (client_id, client_secret) = settings::google_client(&config).ok_or_else(|| {
        anyhow!("Add your Google OAuth client ID and secret in Settings before connecting")
    })?;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("Failed to open local port for Google sign-in")?;
    let redirect_uri = format!("http://127.0.0.1:{}", listener.local_addr()?.port());

    let verifier: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(24)
        .map(char::from)
        .collect();

    let auth_url = url::Url::parse_with_params(
        AUTH_URL,
        &[
            ("client_id", client_id.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("response_type", "code"),
            ("scope", SCOPE),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("state", state.as_str()),
            ("access_type", "offline"),
            ("prompt", "consent"),
        ],
    )?;

    crate::api::open_external_url(auth_url.to_string())
        .await
        .map_err(|e| anyhow!(e))?;

    let code = tokio::time::timeout(
        std::time::Duration::from_secs(OAUTH_TIMEOUT_SECS),
        wait_for_auth_code(&listener, &state),
    )
    .await
    .map_err(|_| anyhow!("Timed out waiting for Google sign-in"))??;

    let response = http_client()
        .post(TOKEN_URL)
        .form(&[
            ("code", code.as_str()),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
            ("code_verifier", verifier.as_str()),
        ])
        .send()
        .await?;
    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("Google token exchange failed: {}", body));
    }
    let token: TokenResponse = response.json().await?;
    let refresh_token = token
        .refresh_token
        .ok_or_else(|| anyhow!("Google did not return a refresh token"))?;

    let mut tokens = GoogleTokens {
        access_token: token.access_token,
        refresh_token,
        expires_at: Utc::now().timestamp() + token.expires_in,
        account_email: None,
        needs_reconnect: false,
    };
    // The primary calendar's id is the account's email address
    tokens.account_email = fetch_primary_calendar_id(&tokens.access_token).await.ok();
    settings::save_google_tokens(app, Some(&tokens))?;

    info!("Google Calendar connected for {:?}", tokens.account_email);
    Ok(tokens.account_email.unwrap_or_else(|| "Google account".to_string()))
}

/// Accepts loopback connections until one carries the OAuth redirect, then returns its code.
async fn wait_for_auth_code(listener: &TcpListener, expected_state: &str) -> Result<String> {
    loop {
        let (mut stream, _) = listener.accept().await?;
        let mut buf = vec![0u8; 8192];
        let n = stream.read(&mut buf).await.unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..n]);
        let path = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/");

        let url = match url::Url::parse(&format!("http://127.0.0.1{}", path)) {
            Ok(url) => url,
            Err(_) => continue,
        };
        let param = |name: &str| {
            url.query_pairs()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.into_owned())
        };

        let (outcome, message) = match (param("code"), param("error")) {
            (Some(code), _) if param("state").as_deref() == Some(expected_state) => {
                (Some(Ok(code)), "Meetily is now connected to Google Calendar. You can close this tab.")
            }
            (Some(_), _) => (
                Some(Err(anyhow!("Google sign-in returned an unexpected state"))),
                "Sign-in failed: unexpected state. Please try again from Meetily.",
            ),
            (None, Some(error)) => (
                Some(Err(anyhow!("Google sign-in was not completed: {}", error))),
                "Sign-in was cancelled. You can close this tab.",
            ),
            // e.g. /favicon.ico
            (None, None) => (None, ""),
        };

        let body = format!(
            "<html><body style=\"font-family:sans-serif;padding:40px\"><h2>{}</h2></body></html>",
            message
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.shutdown().await;

        if let Some(outcome) = outcome {
            return outcome;
        }
    }
}

pub async fn disconnect<R: Runtime>(app: &AppHandle<R>, pool: &SqlitePool) -> Result<()> {
    if let Some(tokens) = settings::load_google_tokens(app) {
        // Best effort: revoking fails harmlessly if the token is already invalid
        let _ = http_client()
            .post(REVOKE_URL)
            .form(&[("token", tokens.refresh_token.as_str())])
            .send()
            .await;
    }
    settings::save_google_tokens(app, None)?;
    CalendarEventsRepository::clear(pool).await?;
    Ok(())
}

pub const RECONNECT_ERROR: &str = "Google sign-in expired. Reconnect Google Calendar in Settings";

fn mark_needs_reconnect<R: Runtime>(app: &AppHandle<R>) {
    if let Some(mut tokens) = settings::load_google_tokens(app) {
        if !tokens.needs_reconnect {
            warn!("Google rejected the stored sign-in; reconnect required");
            tokens.needs_reconnect = true;
            let _ = settings::save_google_tokens(app, Some(&tokens));
        }
    }
}

pub fn needs_reconnect<R: Runtime>(app: &AppHandle<R>) -> bool {
    settings::load_google_tokens(app).map_or(false, |t| t.needs_reconnect)
}

/// Returns a valid access token, refreshing it when it is about to expire.
async fn access_token<R: Runtime>(app: &AppHandle<R>) -> Result<String> {
    let mut tokens =
        settings::load_google_tokens(app).ok_or_else(|| anyhow!("Google Calendar is not connected"))?;
    if tokens.needs_reconnect {
        return Err(anyhow!(RECONNECT_ERROR));
    }
    if tokens.expires_at > Utc::now().timestamp() + 60 {
        return Ok(tokens.access_token);
    }

    let (client_id, client_secret) = settings::google_client(&settings::load_settings(app))
        .ok_or_else(|| anyhow!(RECONNECT_ERROR))?;
    let response = http_client()
        .post(TOKEN_URL)
        .form(&[
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("refresh_token", tokens.refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        // invalid_grant: refresh token expired (7-day limit in Testing mode) or access revoked
        if status == reqwest::StatusCode::BAD_REQUEST || status == reqwest::StatusCode::UNAUTHORIZED {
            mark_needs_reconnect(app);
            return Err(anyhow!("{}: {}", RECONNECT_ERROR, body));
        }
        return Err(anyhow!("Failed to refresh Google token: {}", body));
    }
    let token: TokenResponse = response.json().await?;
    tokens.access_token = token.access_token;
    tokens.expires_at = Utc::now().timestamp() + token.expires_in;
    if let Some(refresh_token) = token.refresh_token {
        tokens.refresh_token = refresh_token;
    }
    settings::save_google_tokens(app, Some(&tokens))?;
    Ok(tokens.access_token)
}

async fn fetch_primary_calendar_id(access_token: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct Calendar {
        id: String,
    }
    let calendar: Calendar = http_client()
        .get(format!("{}/calendars/{}", CALENDAR_API, PRIMARY_CALENDAR))
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(calendar.id)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventsPage {
    #[serde(default)]
    items: Vec<ApiEvent>,
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiEvent {
    id: String,
    status: Option<String>,
    summary: Option<String>,
    description: Option<String>,
    location: Option<String>,
    start: Option<ApiTime>,
    end: Option<ApiTime>,
    hangout_link: Option<String>,
    html_link: Option<String>,
    conference_data: Option<ConferenceData>,
    #[serde(default)]
    attendees: Vec<Attendee>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiTime {
    date_time: Option<DateTime<chrono::FixedOffset>>,
    date: Option<NaiveDate>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConferenceData {
    #[serde(default)]
    entry_points: Vec<EntryPoint>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntryPoint {
    entry_point_type: Option<String>,
    uri: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Attendee {
    email: Option<String>,
    display_name: Option<String>,
    #[serde(rename = "self", default)]
    is_self: bool,
    response_status: Option<String>,
}

/// Syncs events from yesterday through the next 7 days. Returns how many events were stored.
pub async fn sync<R: Runtime>(app: &AppHandle<R>, pool: &SqlitePool) -> Result<usize> {
    let token = access_token(app).await?;
    let now = Utc::now();
    let from = (now - Duration::days(1))
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .map(|t| Utc.from_utc_datetime(&t))
        .unwrap_or(now);
    let to = now + Duration::days(7);

    let mut events = Vec::new();
    let mut page_token: Option<String> = None;
    loop {
        let mut request = http_client()
            .get(format!("{}/calendars/{}/events", CALENDAR_API, PRIMARY_CALENDAR))
            .bearer_auth(&token)
            .query(&[
                ("timeMin", from.to_rfc3339()),
                ("timeMax", to.to_rfc3339()),
                ("singleEvents", "true".to_string()),
                ("orderBy", "startTime".to_string()),
                ("maxResults", "250".to_string()),
            ]);
        if let Some(page) = &page_token {
            request = request.query(&[("pageToken", page)]);
        }
        let response = request.send().await?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            mark_needs_reconnect(app);
            return Err(anyhow!(RECONNECT_ERROR));
        }
        let page: EventsPage = response.error_for_status()?.json().await?;
        events.extend(page.items.into_iter().filter_map(|e| convert_event(e, now)));
        page_token = page.next_page_token;
        if page_token.is_none() {
            break;
        }
    }

    CalendarEventsRepository::replace_range(pool, PRIMARY_CALENDAR, from, to, &events).await?;
    Ok(events.len())
}

fn convert_event(event: ApiEvent, now: DateTime<Utc>) -> Option<CalendarEvent> {
    if event.status.as_deref() == Some("cancelled") {
        return None;
    }
    let declined = event
        .attendees
        .iter()
        .any(|a| a.is_self && a.response_status.as_deref() == Some("declined"));
    if declined {
        return None;
    }

    let (start, start_all_day) = parse_time(event.start.as_ref()?)?;
    let (end, _) = parse_time(event.end.as_ref()?)?;

    let meeting_url = event
        .hangout_link
        .clone()
        .or_else(|| {
            event.conference_data.as_ref().and_then(|c| {
                c.entry_points
                    .iter()
                    .find(|p| p.entry_point_type.as_deref() == Some("video"))
                    .and_then(|p| p.uri.clone())
            })
        })
        .or_else(|| find_meeting_url(event.location.as_deref().unwrap_or("")))
        .or_else(|| find_meeting_url(event.description.as_deref().unwrap_or("")));

    let attendees: Vec<serde_json::Value> = event
        .attendees
        .iter()
        .filter(|a| !a.is_self)
        .map(|a| serde_json::json!({ "email": a.email, "name": a.display_name }))
        .collect();
    let attendees_json = if attendees.is_empty() {
        None
    } else {
        serde_json::to_string(&attendees).ok()
    };

    Some(CalendarEvent {
        id: format!("{}:{}", PRIMARY_CALENDAR, event.id),
        calendar_id: PRIMARY_CALENDAR.to_string(),
        title: event
            .summary
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "(No title)".to_string()),
        description: event.description,
        start_time: start,
        end_time: end,
        is_all_day: start_all_day,
        meeting_url,
        html_link: event.html_link,
        attendees_json,
        reminded_at: None,
        updated_at: now,
    })
}

fn parse_time(time: &ApiTime) -> Option<(DateTime<Utc>, bool)> {
    if let Some(dt) = time.date_time {
        return Some((dt.with_timezone(&Utc), false));
    }
    // All-day events: interpret the date in local time
    let date = time.date?;
    let local = chrono::Local
        .from_local_datetime(&date.and_hms_opt(0, 0, 0)?)
        .earliest()?;
    Some((local.with_timezone(&Utc), true))
}

/// Finds the first Zoom/Meet/Teams/Webex link in free text
pub(crate) fn find_meeting_url(text: &str) -> Option<String> {
    lazy_static::lazy_static! {
        static ref MEETING_URL: regex::Regex = regex::Regex::new(
            r#"https://(?:[\w-]+\.)?(?:zoom\.us/(?:j|my|w)/|meet\.google\.com/|teams\.microsoft\.com/l/meetup-join/|teams\.live\.com/meet/|[\w-]+\.webex\.com/)[^\s"'<>)]+"#
        ).unwrap();
    }
    MEETING_URL.find(text).map(|m| m.as_str().to_string())
}

pub fn is_connected<R: Runtime>(app: &AppHandle<R>) -> bool {
    settings::load_google_tokens(app).is_some()
}

pub fn account_email<R: Runtime>(app: &AppHandle<R>) -> Option<String> {
    settings::load_google_tokens(app).and_then(|t| t.account_email)
}

pub fn log_sync_error(error: &anyhow::Error) {
    warn!("Google Calendar sync failed: {:#}", error);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_meeting_urls_in_text() {
        assert_eq!(
            find_meeting_url("Join: https://us02web.zoom.us/j/123456?pwd=abc please"),
            Some("https://us02web.zoom.us/j/123456?pwd=abc".to_string())
        );
        assert_eq!(
            find_meeting_url("<a href=\"https://meet.google.com/abc-defg-hij\">x</a>"),
            Some("https://meet.google.com/abc-defg-hij".to_string())
        );
        assert_eq!(find_meeting_url("no link here https://example.com"), None);
    }

    #[test]
    fn skips_declined_and_cancelled_events() {
        let json = r#"{"id":"1","summary":"Sync","start":{"dateTime":"2026-10-07T10:00:00-07:00"},
            "end":{"dateTime":"2026-10-07T10:30:00-07:00"},
            "attendees":[{"email":"me@x.com","self":true,"responseStatus":"declined"}]}"#;
        let event: ApiEvent = serde_json::from_str(json).unwrap();
        assert!(convert_event(event, Utc::now()).is_none());

        let json = r#"{"id":"2","status":"cancelled"}"#;
        let event: ApiEvent = serde_json::from_str(json).unwrap();
        assert!(convert_event(event, Utc::now()).is_none());
    }

    #[test]
    fn converts_meet_event() {
        let json = r#"{"id":"3","summary":"Standup","hangoutLink":"https://meet.google.com/aaa-bbbb-ccc",
            "start":{"dateTime":"2026-10-07T10:00:00-07:00"},"end":{"dateTime":"2026-10-07T10:15:00-07:00"},
            "attendees":[{"email":"me@x.com","self":true},{"email":"bo@x.com","displayName":"Bo"}]}"#;
        let event: ApiEvent = serde_json::from_str(json).unwrap();
        let converted = convert_event(event, Utc::now()).unwrap();
        assert_eq!(converted.title, "Standup");
        assert_eq!(converted.start_time.to_rfc3339(), "2026-10-07T17:00:00+00:00");
        assert_eq!(converted.meeting_url.as_deref(), Some("https://meet.google.com/aaa-bbbb-ccc"));
        assert_eq!(
            converted.attendees_json.as_deref(),
            Some(r#"[{"email":"bo@x.com","name":"Bo"}]"#)
        );
    }
}
