// summary/subscription_cli.rs
//
// "Use your subscription" summary providers: summaries run through the user's own
// command-line AI tool, signed in with their consumer plan instead of an API key.
//
//   claude-cli  Claude Code (`claude`)  Claude Pro/Max
//   codex-cli   Codex CLI (`codex`)     ChatGPT plans
//   gemini-cli  Gemini CLI (`gemini`)   Google account / Google AI plans
//
// Also backs the Settings setup guide: install check, sign-in, model list, test.
// Every run is a plain prompt-in, text-out call from an empty working folder, with
// tools off or read-only and no session history where the CLI allows it.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use base64::Engine;
use log::{info, warn};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

const STATUS_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CliKind {
    Claude,
    Codex,
    Gemini,
}

impl CliKind {
    pub fn from_provider(provider: &str) -> Option<Self> {
        match provider {
            "claude-cli" => Some(Self::Claude),
            "codex-cli" => Some(Self::Codex),
            "gemini-cli" => Some(Self::Gemini),
            _ => None,
        }
    }

    fn binary_name(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
            Self::Gemini => "Gemini CLI",
        }
    }

    fn install_docs_url(self) -> &'static str {
        match self {
            Self::Claude => "https://docs.claude.com/en/docs/claude-code/setup",
            Self::Codex => "https://github.com/openai/codex",
            Self::Gemini => "https://github.com/google-gemini/gemini-cli",
        }
    }
}

// ---------------------------------------------------------------------------
// Finding and running the CLI
// ---------------------------------------------------------------------------

/// Finds the CLI binary. Apps launched from a desktop menu often don't have the
/// user's shell PATH (nvm, npm prefixes, ~/.local/bin), so the usual install
/// locations are checked as well.
pub fn find_binary(kind: CliKind) -> Option<PathBuf> {
    let name = kind.binary_name();
    if let Ok(path) = which::which(name) {
        return Some(path);
    }
    let home = dirs::home_dir()?;
    let mut dirs_to_check: Vec<PathBuf> = vec![
        home.join(".local/bin"),
        home.join(".claude/local"),
        home.join(".npm-global/bin"),
        home.join(".bun/bin"),
        home.join(".volta/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/opt/homebrew/bin"),
        home.join("AppData/Roaming/npm"),
    ];
    // nvm / fnm Node versions, newest first
    for versions in [home.join(".nvm/versions/node"), home.join(".local/share/fnm/node-versions")] {
        if let Ok(entries) = std::fs::read_dir(&versions) {
            let mut found: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            found.sort();
            for version in found.into_iter().rev() {
                dirs_to_check.push(version.join("bin"));
                dirs_to_check.push(version.join("installation/bin"));
            }
        }
    }
    let names = [name.to_string(), format!("{}.cmd", name), format!("{}.exe", name)];
    dirs_to_check
        .iter()
        .flat_map(|dir| names.iter().map(move |n| dir.join(n)))
        .find(|p| p.is_file())
}

/// A command for the CLI. Its own folder is put first on PATH: npm-installed CLIs
/// are Node scripts that need the `node` installed next to them.
fn cli_command(kind: CliKind, binary: &Path) -> Command {
    let mut command = Command::new(binary);
    if let Some(dir) = binary.parent() {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![dir.to_path_buf()];
        paths.extend(std::env::split_paths(&path));
        if let Ok(joined) = std::env::join_paths(paths) {
            command.env("PATH", joined);
        }
    }
    command.env("NO_COLOR", "1");
    if kind == CliKind::Gemini {
        // Sign in with a Google account (not an API key or Vertex AI)
        command.env("GOOGLE_GENAI_USE_GCA", "true");
    }
    command.kill_on_drop(true);
    command
}

/// A clean working directory for runs, so no project instructions or settings
/// are picked up.
fn work_dir(app_data_dir: Option<&PathBuf>) -> PathBuf {
    let dir = app_data_dir
        .cloned()
        .unwrap_or_else(std::env::temp_dir)
        .join("subscription-cli");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn strip_ansi(text: &str) -> String {
    static ANSI: OnceLock<Regex> = OnceLock::new();
    let re = ANSI.get_or_init(|| {
        Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(\x07|\x1b\\)").unwrap()
    });
    re.replace_all(text, "").to_string()
}

/// The JSON object in a CLI's stdout (some print notices before it).
fn json_object(stdout: &str) -> Option<serde_json::Value> {
    let start = stdout.find('{')?;
    let end = stdout.rfind('}')?;
    serde_json::from_str(&stdout[start..=end]).ok()
}

/// Turns CLI failures into something a user can act on.
fn friendly_error(kind: CliKind, message: &str) -> String {
    let lower = message.to_lowercase();
    let message = message.trim();
    if lower.contains("not logged in") || lower.contains("/login") || lower.contains("logged in")
        || lower.contains("401") || lower.contains("unauthorized") || lower.contains("auth method")
        || lower.contains("authenticat") || lower.contains("oauth") || lower.contains("invalid api key")
    {
        format!("{} isn't signed in. Open Settings → Summary and sign in.", kind.label())
    } else if lower.contains("rate limit") || lower.contains("usage limit") || lower.contains("limit reached")
        || lower.contains("quota") || lower.contains("429")
    {
        format!("Your plan's usage limit was reached. Try again later. ({})", message)
    } else {
        format!("{} failed: {}", kind.label(), message)
    }
}

/// Runs the CLI with `prompt` on stdin and returns its stdout, honoring cancellation.
async fn run_with_stdin(
    mut command: Command,
    stdin_text: String,
    cancellation_token: Option<&CancellationToken>,
) -> Result<std::process::Output, String> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Couldn't start the CLI: {}", e))?;

    let mut stdin = child.stdin.take().ok_or("Couldn't write to the CLI")?;
    let writer = tokio::spawn(async move {
        let _ = stdin.write_all(stdin_text.as_bytes()).await;
        // Dropping stdin closes it, which tells the CLI the input is complete
    });

    let output = match cancellation_token {
        Some(token) => tokio::select! {
            output = child.wait_with_output() => output,
            _ = token.cancelled() => return Err("Summary generation was cancelled".to_string()),
        },
        None => child.wait_with_output().await,
    }
    .map_err(|e| format!("The CLI didn't finish: {}", e))?;
    let _ = writer.await;
    Ok(output)
}

fn stderr_text(output: &std::process::Output) -> String {
    strip_ansi(&String::from_utf8_lossy(&output.stderr))
}

/// Generates text with the user's CLI. The meeting content goes in on stdin
/// (transcripts are too large for a command-line argument).
pub async fn generate(
    kind: CliKind,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    app_data_dir: Option<&PathBuf>,
    cancellation_token: Option<&CancellationToken>,
) -> Result<String, String> {
    let binary = find_binary(kind).ok_or_else(|| {
        format!("{} isn't installed. Open Settings → Summary to set it up.", kind.label())
    })?;
    let dir = work_dir(app_data_dir);
    let model = model.trim();

    // Fail fast when signed out: Codex would retry for a while, and Gemini would
    // ask on stdin whether to open a browser
    if kind != CliKind::Claude && !status_for(kind, &binary).await.logged_in {
        return Err(friendly_error(kind, "not logged in"));
    }

    info!("Generating with {} (model {})", kind.label(), if model.is_empty() { "default" } else { model });

    match kind {
        CliKind::Claude => {
            let mut command = cli_command(kind, &binary);
            command
                .args(["-p", "--model", if model.is_empty() { "sonnet" } else { model }])
                .args(["--system-prompt", system_prompt])
                .args(["--tools", "", "--strict-mcp-config", "--setting-sources", "project"])
                .args(["--no-session-persistence", "--output-format", "json"])
                .current_dir(&dir);
            let output = run_with_stdin(command, user_prompt.to_string(), cancellation_token).await?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            match json_object(&stdout) {
                Some(json) => {
                    let text = json.get("result").and_then(|v| v.as_str()).unwrap_or_default();
                    let is_error = json.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false)
                        || json.get("subtype").and_then(|v| v.as_str()).is_some_and(|s| s != "success");
                    if is_error {
                        Err(friendly_error(kind, if text.is_empty() { "unknown error" } else { text }))
                    } else if text.is_empty() {
                        Err("Claude Code returned no text".to_string())
                    } else {
                        Ok(text.to_string())
                    }
                }
                None => Err(friendly_error(kind, &or_stdout(stderr_text(&output), &stdout))),
            }
        }
        CliKind::Codex => {
            let last_message = dir.join(format!("last-message-{}.txt", uuid::Uuid::new_v4()));
            let mut command = cli_command(kind, &binary);
            command
                .arg("exec")
                .args(["--sandbox", "read-only", "--skip-git-repo-check", "--ephemeral"])
                .args(["--ignore-user-config", "--ignore-rules", "--color", "never"])
                .arg("-C")
                .arg(&dir)
                .arg("-o")
                .arg(&last_message);
            if !model.is_empty() && model != "default" {
                command.args(["-m", model]);
            }
            // The instructions are the prompt; the meeting content on stdin is
            // appended to it as a <stdin> block
            command.arg(system_prompt).current_dir(&dir);
            let output = run_with_stdin(command, user_prompt.to_string(), cancellation_token).await;
            let text = tokio::fs::read_to_string(&last_message).await.ok();
            let _ = tokio::fs::remove_file(&last_message).await;
            let output = output?;
            match text.filter(|t| !t.trim().is_empty()) {
                Some(text) if output.status.success() => Ok(text.trim().to_string()),
                _ => {
                    let stderr = stderr_text(&output);
                    let detail = stderr
                        .lines()
                        .rev()
                        .find(|l| l.contains("ERROR") || l.to_lowercase().contains("error"))
                        .unwrap_or(stderr.trim())
                        .to_string();
                    Err(friendly_error(kind, &or_stdout(detail, &String::from_utf8_lossy(&output.stdout))))
                }
            }
        }
        CliKind::Gemini => {
            let mut command = cli_command(kind, &binary);
            command.args(["--skip-trust", "--approval-mode", "plan", "-o", "json"]);
            if !model.is_empty() && model != "default" {
                command.args(["-m", model]);
            }
            // -p is appended after the stdin content: meeting content, then instructions
            command.args(["-p", system_prompt]).current_dir(&dir);
            let output = run_with_stdin(command, user_prompt.to_string(), cancellation_token).await?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            match json_object(&stdout) {
                Some(json) => {
                    if let Some(message) = json.pointer("/error/message").and_then(|v| v.as_str()) {
                        Err(friendly_error(kind, message))
                    } else {
                        json.get("response")
                            .and_then(|v| v.as_str())
                            .filter(|t| !t.trim().is_empty())
                            .map(|t| t.trim().to_string())
                            .ok_or_else(|| "Gemini returned no text".to_string())
                    }
                }
                None => Err(friendly_error(kind, &or_stdout(stderr_text(&output), &stdout))),
            }
        }
    }
}

fn or_stdout(primary: String, stdout: &str) -> String {
    if primary.trim().is_empty() { stdout.trim().to_string() } else { primary }
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CliStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub logged_in: bool,
    /// Signed in with a consumer plan (not API-key / per-use billing)
    pub uses_subscription: bool,
    /// How it's signed in, for display (e.g. "Claude account", "API key")
    pub auth_label: Option<String>,
    /// e.g. "pro", "plus"
    pub plan: Option<String>,
    pub email: Option<String>,
    pub install_docs_url: String,
    pub error: Option<String>,
}

async fn run_quick(kind: CliKind, binary: &Path, args: &[&str]) -> Result<std::process::Output, String> {
    let mut command = cli_command(kind, binary);
    command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    tokio::time::timeout(STATUS_TIMEOUT, command.output())
        .await
        .map_err(|_| format!("{} didn't respond", kind.label()))?
        .map_err(|e| e.to_string())
}

/// Claims inside a JWT payload (no signature check: display only).
fn jwt_claims(token: &str) -> Option<serde_json::Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

async fn status_for(kind: CliKind, binary: &Path) -> CliStatus {
    let mut status = CliStatus {
        installed: true,
        path: Some(binary.to_string_lossy().to_string()),
        install_docs_url: kind.install_docs_url().to_string(),
        ..Default::default()
    };
    let home = dirs::home_dir().unwrap_or_default();

    match kind {
        CliKind::Claude => match run_quick(kind, binary, &["auth", "status", "--json"]).await {
            Ok(output) => match json_object(&String::from_utf8_lossy(&output.stdout)) {
                Some(json) => {
                    let text = |key: &str| json.get(key).and_then(|v| v.as_str()).map(str::to_string);
                    status.logged_in = json.get("loggedIn").and_then(|v| v.as_bool()).unwrap_or(false);
                    status.uses_subscription = status.logged_in && text("authMethod").as_deref() == Some("claude.ai");
                    status.auth_label = text("authMethod").map(|m| {
                        if m == "claude.ai" { "Claude account".to_string() } else { "Anthropic Console (per-use billing)".to_string() }
                    });
                    status.plan = text("subscriptionType");
                    status.email = text("email");
                }
                None => status.error = Some("Couldn't read the sign-in status. Update Claude Code and try again.".to_string()),
            },
            Err(e) => status.error = Some(e),
        },
        CliKind::Codex => match run_quick(kind, binary, &["login", "status"]).await {
            Ok(output) => {
                let text = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
                status.logged_in = output.status.success() && !text.to_lowercase().contains("not logged in");
                let chatgpt = text.contains("ChatGPT");
                status.uses_subscription = status.logged_in && chatgpt;
                if status.logged_in {
                    status.auth_label = Some(if chatgpt { "ChatGPT account".to_string() } else { "OpenAI API key (per-use billing)".to_string() });
                }
                // Email and plan from the saved ChatGPT sign-in
                let codex_home = std::env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".codex"));
                if let Some(claims) = std::fs::read_to_string(codex_home.join("auth.json"))
                    .ok()
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                    .and_then(|auth| auth.pointer("/tokens/id_token").and_then(|t| t.as_str()).and_then(jwt_claims))
                {
                    status.email = claims.get("email").and_then(|v| v.as_str()).map(str::to_string);
                    status.plan = claims
                        .pointer("/https:~1~1api.openai.com~1auth/chatgpt_plan_type")
                        .and_then(|v| v.as_str())
                        .map(str::to_string);
                }
            }
            Err(e) => status.error = Some(e),
        },
        CliKind::Gemini => {
            let gemini_home = home.join(".gemini");
            status.logged_in = gemini_home.join("oauth_creds.json").is_file();
            status.uses_subscription = status.logged_in;
            if status.logged_in {
                status.auth_label = Some("Google account".to_string());
            }
            status.email = std::fs::read_to_string(gemini_home.join("google_accounts.json"))
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .and_then(|v| v.get("active").and_then(|a| a.as_str()).map(str::to_string));
        }
    }

    status.version = run_quick(kind, binary, &["--version"])
        .await
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        // "2.1.300 (Claude Code)" / "codex-cli 0.162.1" / "0.64.0" -> the version number
        .and_then(|v| v.split_whitespace().find(|w| w.chars().next().is_some_and(|c| c.is_ascii_digit())).map(str::to_string));
    status
}

/// Whether the CLI is installed and signed in, for the setup guide.
#[tauri::command]
pub async fn subscription_cli_status(cli: CliKind) -> CliStatus {
    match find_binary(cli) {
        Some(binary) => status_for(cli, &binary).await,
        None => CliStatus {
            install_docs_url: cli.install_docs_url().to_string(),
            ..Default::default()
        },
    }
}

/// Models to offer in Settings.
#[tauri::command]
pub async fn subscription_cli_models(cli: CliKind) -> Vec<String> {
    match cli {
        CliKind::Claude => ["sonnet", "opus", "haiku"].map(String::from).to_vec(),
        CliKind::Gemini => ["auto", "pro", "flash", "flash-lite"].map(String::from).to_vec(),
        CliKind::Codex => {
            // Codex's catalog changes often: read it from the installed CLI
            let mut models = vec!["default".to_string()];
            if let Some(binary) = find_binary(cli) {
                if let Ok(output) = run_quick(cli, &binary, &["debug", "models"]).await {
                    if let Some(list) = json_object(&String::from_utf8_lossy(&output.stdout))
                        .and_then(|json| json.get("models").and_then(|m| m.as_array()).cloned())
                    {
                        let mut listed: Vec<(i64, String)> = list
                            .iter()
                            .filter(|m| m.get("visibility").and_then(|v| v.as_str()) == Some("list"))
                            .filter_map(|m| {
                                let slug = m.get("slug").and_then(|s| s.as_str())?;
                                let priority = m.get("priority").and_then(|p| p.as_i64()).unwrap_or(i64::MAX);
                                Some((priority, slug.to_string()))
                            })
                            .collect();
                        listed.sort();
                        models.extend(listed.into_iter().map(|(_, slug)| slug));
                    }
                }
            }
            models
        }
    }
}

// ---------------------------------------------------------------------------
// Sign-in
// ---------------------------------------------------------------------------

struct LoginProcess {
    child: Child,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
}

/// The running sign-in, so a pasted code can be sent to it.
static LOGIN: Mutex<Option<LoginProcess>> = Mutex::const_new(None);

fn login_args(cli: CliKind) -> Vec<&'static str> {
    match cli {
        CliKind::Claude => vec!["auth", "login", "--claudeai"],
        CliKind::Codex => vec!["login"],
        // Gemini signs in on its first run: a tiny headless prompt triggers it
        CliKind::Gemini => vec!["--skip-trust", "-o", "json", "-p", "Reply with exactly: OK"],
    }
}

/// Starts the CLI's sign-in. Its output (sign-in link, prompts) is forwarded as
/// `subscription-cli-login-output` events and `subscription-cli-login-finished`
/// reports the result. A "[Y/n]" confirmation (Gemini's "open the browser?") is
/// answered yes.
#[tauri::command]
pub async fn subscription_cli_login_start<R: Runtime>(app: AppHandle<R>, cli: CliKind) -> Result<(), String> {
    let binary = find_binary(cli).ok_or_else(|| format!("{} isn't installed", cli.label()))?;
    let mut guard = LOGIN.lock().await;
    if let Some(mut old) = guard.take() {
        let _ = old.child.kill().await;
    }

    let app_data_dir = app.path().app_data_dir().ok();
    let mut command = cli_command(cli, &binary);
    let mut child = command
        .args(login_args(cli))
        .current_dir(work_dir(app_data_dir.as_ref()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Couldn't start sign-in: {}", e))?;

    let stdin = Arc::new(Mutex::new(child.stdin.take()));
    let streams: Vec<Box<dyn tokio::io::AsyncRead + Unpin + Send>> = [
        child.stdout.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>),
        child.stderr.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>),
    ]
    .into_iter()
    .flatten()
    .collect();

    for mut stream in streams {
        let app = app.clone();
        let stdin = stdin.clone();
        tokio::spawn(async move {
            let mut buffer = [0u8; 4096];
            let mut pending = String::new();
            while let Ok(n) = stream.read(&mut buffer).await {
                if n == 0 {
                    break;
                }
                pending.push_str(&String::from_utf8_lossy(&buffer[..n]));
                // Prompts like "[Y/n]: " wait without a newline, so check before splitting
                if pending.contains("[Y/n]") {
                    if let Some(input) = stdin.lock().await.as_mut() {
                        let _ = input.write_all(b"Y\n").await;
                        let _ = input.flush().await;
                    }
                    pending = pending.replace("[Y/n]", "");
                }
                while let Some(newline) = pending.find('\n') {
                    let line = strip_ansi(&pending[..newline]);
                    pending.drain(..=newline);
                    if !line.trim().is_empty() {
                        let _ = app.emit("subscription-cli-login-output", line);
                    }
                }
            }
            let rest = strip_ansi(&pending);
            if !rest.trim().is_empty() {
                let _ = app.emit("subscription-cli-login-output", rest);
            }
        });
    }

    *guard = Some(LoginProcess { child, stdin });
    drop(guard);

    // Report when it ends
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let mut guard = LOGIN.lock().await;
            let Some(process) = guard.as_mut() else { return };
            if let Ok(Some(exit)) = process.child.try_wait() {
                *guard = None;
                drop(guard);
                let status = subscription_cli_status(cli).await;
                let _ = app.emit(
                    "subscription-cli-login-finished",
                    serde_json::json!({ "cli": cli, "success": exit.success() && status.logged_in }),
                );
                return;
            }
        }
    });
    Ok(())
}

/// Sends a code pasted from the browser to the running sign-in.
#[tauri::command]
pub async fn subscription_cli_login_submit_code(code: String) -> Result<(), String> {
    let stdin = LOGIN
        .lock()
        .await
        .as_ref()
        .map(|p| p.stdin.clone())
        .ok_or("Sign-in isn't running")?;
    let mut guard = stdin.lock().await;
    let input = guard.as_mut().ok_or("Sign-in isn't accepting input")?;
    input.write_all(format!("{}\n", code.trim()).as_bytes()).await.map_err(|e| e.to_string())?;
    input.flush().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn subscription_cli_login_cancel() {
    if let Some(mut process) = LOGIN.lock().await.take() {
        let _ = process.child.kill().await;
    }
}

/// Runs a tiny prompt to confirm summaries will work.
#[tauri::command]
pub async fn subscription_cli_test<R: Runtime>(app: AppHandle<R>, cli: CliKind, model: String) -> Result<String, String> {
    let app_data_dir = app.path().app_data_dir().ok();
    let reply = generate(
        cli,
        &model,
        "You are a connection test. Reply with exactly: OK",
        "Connection test",
        app_data_dir.as_ref(),
        None,
    )
    .await?;
    if reply.trim().is_empty() {
        warn!("{} test returned no text", cli.label());
    }
    Ok(reply)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_providers() {
        assert_eq!(CliKind::from_provider("claude-cli"), Some(CliKind::Claude));
        assert_eq!(CliKind::from_provider("codex-cli"), Some(CliKind::Codex));
        assert_eq!(CliKind::from_provider("gemini-cli"), Some(CliKind::Gemini));
        assert_eq!(CliKind::from_provider("claude"), None);
    }

    #[test]
    fn strips_terminal_colors_and_links() {
        assert_eq!(strip_ansi("\x1b[1mOpen\x1b[0m this"), "Open this");
        assert_eq!(strip_ansi("\x1b]8;;https://x\x07link\x1b]8;;\x07"), "link");
    }

    #[test]
    fn finds_json_after_notices() {
        let json = json_object("Loaded cached credentials.\n{\n  \"response\": \"OK\"\n}\n").unwrap();
        assert_eq!(json["response"], "OK");
    }

    #[test]
    fn explains_failures() {
        assert!(friendly_error(CliKind::Claude, "Invalid API key · Please run /login").contains("isn't signed in"));
        assert!(friendly_error(CliKind::Codex, "unexpected status 401 Unauthorized").contains("isn't signed in"));
        assert!(friendly_error(CliKind::Gemini, "Please set an Auth method").contains("isn't signed in"));
        assert!(friendly_error(CliKind::Gemini, "Quota exceeded").contains("usage limit"));
        assert!(friendly_error(CliKind::Codex, "boom").starts_with("Codex failed"));
    }

    #[test]
    fn reads_jwt_claims() {
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"email":"a@b.c"}"#);
        assert_eq!(jwt_claims(&format!("x.{}.y", payload)).unwrap()["email"], "a@b.c");
    }

    // Real CLI runs (use the signed-in plan): cargo test subscription_cli -- --ignored
    #[tokio::test]
    #[ignore]
    async fn real_status_of_each_cli() {
        for cli in [CliKind::Claude, CliKind::Codex, CliKind::Gemini] {
            println!("{:?}: {:?}", cli, subscription_cli_status(cli).await);
        }
    }

    #[tokio::test]
    #[ignore]
    async fn real_claude_generation() {
        let text = generate(CliKind::Claude, "haiku", "Reply with exactly: OK", "Connection test", None, None)
            .await
            .unwrap();
        assert!(text.contains("OK"), "{}", text);
    }

    #[tokio::test]
    #[ignore]
    async fn real_codex_model_list() {
        let models = subscription_cli_models(CliKind::Codex).await;
        println!("{:?}", models);
        assert!(models.len() > 1);
    }
}
