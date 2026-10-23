//! Segra: private models in TEEs, paid from the user's Segra account.
//!
//! The user signs in once (device sign-in, see `commands.rs`) and gets a Segra API key. The key can
//! only start short-lived sessions with a model provider: for each request we trade it for
//! `{endpoint, token}` and send the transcript straight to that endpoint, so Segra's server never
//! sees meeting content. Afterwards the token counts are reported for billing.

pub mod commands;

use std::collections::HashMap;
use std::time::Duration;

use log::{info, warn};
use once_cell::sync::Lazy;
use serde::Deserialize;
use sqlx::SqlitePool;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::summary::llm_client::{build_openai_compat_chat_body, LLMProvider, LlmCompletion};

/// The model new Segra users start with.
pub const DEFAULT_MODEL: &str = "0GM-1.0-35B-A3B";

/// Lets the server pick its default model.
const DEFAULT_MODEL_ALIAS: &str = "default";

/// Same limit as the other providers' summary requests.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

/// Segra's app API. Release builds use the deployed server and debug builds (`tauri dev`) the local
/// dev server (`npm run dev` in segra-web). `SEGRA_BASE_URL` overrides it at run time or build time.
const PRODUCTION_BASE_URL: &str = "https://web.segra.ai/api/app/v1";
const DEV_BASE_URL: &str = "http://localhost:3000/api/app/v1";

pub fn base_url() -> String {
    let default = if cfg!(debug_assertions) { DEV_BASE_URL } else { PRODUCTION_BASE_URL };
    std::env::var("SEGRA_BASE_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| option_env!("SEGRA_BASE_URL").map(str::to_string))
        .unwrap_or_else(|| default.to_string())
        .trim_end_matches('/')
        .to_string()
}

fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("SegraMeetings/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// Error text from a Segra API response (`{"error": "..."}`), or the raw body.
async fn error_text(res: reqwest::Response) -> String {
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
        .unwrap_or_else(|| format!("HTTP {}: {}", status, body.chars().take(200).collect::<String>()))
}

// ===== Account storage (settings row) =====

pub async fn api_key(pool: &SqlitePool) -> Result<Option<String>, sqlx::Error> {
    let key: Option<Option<String>> = sqlx::query_scalar("SELECT segraApiKey FROM settings WHERE id = '1' LIMIT 1")
        .fetch_optional(pool)
        .await?;
    Ok(key.flatten().filter(|k| !k.is_empty()))
}

pub async fn user_id(pool: &SqlitePool) -> Result<Option<String>, sqlx::Error> {
    let id: Option<Option<String>> = sqlx::query_scalar("SELECT segraUserId FROM settings WHERE id = '1' LIMIT 1")
        .fetch_optional(pool)
        .await?;
    Ok(id.flatten())
}

pub async fn save_account(pool: &SqlitePool, key: Option<&str>, user_id: Option<&str>) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO settings (id, provider, model, whisperModel, segraApiKey, segraUserId)
        VALUES ('1', 'segra', $3, 'large-v3', $1, $2)
        ON CONFLICT(id) DO UPDATE SET segraApiKey = $1, segraUserId = $2
        "#,
    )
    .bind(key)
    .bind(user_id)
    .bind(DEFAULT_MODEL)
    .execute(pool)
    .await?;
    SESSIONS.lock().await.clear();
    Ok(())
}

/// The key for a summary request, or a message telling the user to sign in.
pub async fn require_api_key(pool: &SqlitePool) -> Result<String, String> {
    api_key(pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Sign in to Segra in Settings → Summary to use Segra models.".to_string())
}

// ===== Sessions =====

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Session {
    provider: String,
    /// OpenAI-compatible base URL of the provider (`…/v1/proxy`)
    endpoint: String,
    model: String,
    token: String,
    /// Unix ms
    expires_at: f64,
}

impl Session {
    fn fresh(&self) -> bool {
        let now = chrono::Utc::now().timestamp_millis() as f64;
        self.expires_at - now > 60_000.0
    }
}

/// One session per requested model, reused until a minute before it expires.
static SESSIONS: Lazy<Mutex<HashMap<String, Session>>> = Lazy::new(|| Mutex::new(HashMap::new()));

async fn session(api_key: &str, model: &str) -> Result<Session, String> {
    if let Some(s) = SESSIONS.lock().await.get(model).filter(|s| s.fresh()) {
        return Ok(s.clone());
    }
    let res = http()
        .post(format!("{}/session", base_url()))
        .bearer_auth(api_key)
        .json(&serde_json::json!({ "model": model }))
        .send()
        .await
        .map_err(|e| format!("Could not reach Segra at {}: {}", base_url(), e))?;
    match res.status().as_u16() {
        200 => {}
        401 => return Err("Your Segra sign-in has expired or was revoked. Sign in again in Settings → Summary.".into()),
        402 => return Err("Your Segra balance is used up. Top up your Segra account to keep generating notes.".into()),
        // "Setting up your account…", "<model> is available after a top-up…": already written for the user.
        403 | 409 => return Err(error_text(res).await),
        _ => return Err(format!("Segra could not start a session: {}", error_text(res).await)),
    }
    let s: Session = res
        .json()
        .await
        .map_err(|_| format!("{} is not a Segra server (unexpected session response).", base_url()))?;
    SESSIONS.lock().await.insert(model.to_string(), s.clone());
    Ok(s)
}

fn report_usage(api_key: String, s: &Session, input_tokens: u64, output_tokens: u64) {
    let body = serde_json::json!({
        "provider": s.provider,
        "model": s.model,
        "chatId": null,
        "inputTokens": input_tokens,
        "outputTokens": output_tokens,
        "verification": "unavailable",
    });
    tokio::spawn(async move {
        let res = http().post(format!("{}/usage", base_url())).bearer_auth(api_key).json(&body).send().await;
        match res {
            Ok(r) if r.status().is_success() => {}
            Ok(r) => warn!("Segra usage report failed: {}", error_text(r).await),
            Err(e) => warn!("Segra usage report failed: {}", e),
        }
    });
}

// ===== Completions =====

#[derive(Deserialize)]
struct Completion {
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Deserialize)]
struct Message {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    reasoning: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
}

enum Attempt {
    Done(LlmCompletion),
    /// The provider rejected the token (expired early or revoked); mint a new one.
    Unauthorized,
}

async fn attempt(
    client: &reqwest::Client,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<Attempt, String> {
    let s = session(api_key, model).await?;
    // Segra models think before answering; leave max_tokens unset so the answer isn't cut off.
    let body = build_openai_compat_chat_body(&LLMProvider::CustomOpenAI, &s.model, system_prompt, user_prompt, None, None, None);
    let res = client
        .post(format!("{}/chat/completions", s.endpoint.trim_end_matches('/')))
        .bearer_auth(&s.token)
        .json(&body)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                format!("The Segra model timed out after {} seconds", REQUEST_TIMEOUT.as_secs())
            } else {
                format!("Could not reach the Segra model: {}", e)
            }
        })?;
    let status = res.status();
    if status.as_u16() == 401 {
        return Ok(Attempt::Unauthorized);
    }
    if !status.is_success() {
        let text = error_text(res).await;
        if text.contains("insufficient balance") {
            return Err("This Segra model is temporarily unavailable. Try again later, or pick another Segra model in Settings.".into());
        }
        return Err(format!("The Segra model returned an error ({}): {}", status, text));
    }
    let completion: Completion = res.json().await.map_err(|e| format!("Unexpected response from the Segra model: {}", e))?;
    if let Some(u) = &completion.usage {
        report_usage(api_key.to_string(), &s, u.prompt_tokens, u.completion_tokens);
    }
    let message = match completion.choices.into_iter().next() {
        Some(choice) => choice.message,
        None => return Err("No content in the Segra model's response".into()),
    };
    let reasoning_stripped = [&message.reasoning, &message.reasoning_content]
        .into_iter()
        .flatten()
        .any(|r| !r.trim().is_empty());
    let content = message.content.unwrap_or_default().trim().to_string();
    Ok(Attempt::Done(LlmCompletion { content, reasoning_stripped }))
}

/// One summary/chat completion through Segra: mints (or reuses) a session for `model`, sends the
/// prompt straight to the provider, and reports token counts. Retries once with a new session if
/// the provider rejects the token.
pub(crate) async fn generate(
    client: &reqwest::Client,
    model: &str,
    api_key: &str,
    system_prompt: &str,
    user_prompt: &str,
    cancellation_token: Option<&CancellationToken>,
) -> Result<LlmCompletion, String> {
    let model = if model.trim().is_empty() { DEFAULT_MODEL_ALIAS } else { model.trim() };
    info!("Segra request: model={}", model);
    for _ in 0..2 {
        let run = attempt(client, api_key, model, system_prompt, user_prompt);
        let result = match cancellation_token {
            Some(token) => tokio::select! {
                r = run => r,
                _ = token.cancelled() => return Err("Summary generation was cancelled".to_string()),
            },
            None => run.await,
        }?;
        match result {
            Attempt::Done(completion) => return Ok(completion),
            Attempt::Unauthorized => {
                SESSIONS.lock().await.remove(model);
            }
        }
    }
    Err("The Segra model rejected the session twice. Try again in a few minutes.".into())
}

/// Model IDs with a funded provider right now (public; no sign-in needed).
pub async fn list_models() -> Result<Vec<String>, String> {
    let res = http()
        .get(format!("{}/models", base_url()))
        .send()
        .await
        .map_err(|e| format!("Could not reach Segra at {}: {}", base_url(), e))?;
    if !res.status().is_success() {
        return Err(error_text(res).await);
    }
    let body: serde_json::Value = res
        .json()
        .await
        .map_err(|_| format!("{} is not a Segra server.", base_url()))?;
    Ok(body
        .get("data")
        .and_then(|d| d.as_array())
        .map(|models| models.iter().filter_map(|m| m.get("id")?.as_str().map(str::to_string)).collect())
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_url_strips_trailing_slash() {
        std::env::set_var("SEGRA_BASE_URL", "https://example.test/api/app/v1/");
        assert_eq!(base_url(), "https://example.test/api/app/v1");
        std::env::remove_var("SEGRA_BASE_URL");
    }

    /// Live test against a Segra server: `SEGRA_TEST_KEY=sk-segra-… cargo test segra::tests::live -- --ignored`
    /// (uses `SEGRA_BASE_URL`, default the local dev server in test builds). Spends a little of the key's credits.
    #[tokio::test]
    #[ignore]
    async fn live_summary_through_segra() {
        let key = std::env::var("SEGRA_TEST_KEY").expect("set SEGRA_TEST_KEY");
        let models = list_models().await.expect("list models");
        assert!(!models.is_empty(), "no funded Segra models");
        let completion = generate(
            &reqwest::Client::new(),
            DEFAULT_MODEL,
            &key,
            "You summarize meeting transcripts into markdown with sections: Summary, Decisions, Action items.",
            "Alice: Let's ship the beta Friday. Bob: I need until Monday for the billing fix. Alice: OK, Monday then. Bob will send release notes.",
            None,
        )
        .await
        .expect("summary");
        println!("models: {:?}\nreasoning stripped: {}\n---\n{}", models, completion.reasoning_stripped, completion.content);
        assert!(completion.content.contains("Monday"));
        // The session is cached: a second call reuses it.
        assert!(SESSIONS.lock().await.contains_key(DEFAULT_MODEL));
        // Let the usage report finish before the runtime shuts down.
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
