//! Small always-on-top window (Granola-style) used for "meeting starting" and
//! "call detected" prompts. Native notifications can't reliably carry buttons on Linux.

use log::{error, info};
use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};

pub const POPUP_LABEL: &str = "meeting-popup";
const WIDTH: f64 = 380.0;
const HEIGHT: f64 = 150.0;
const MARGIN: f64 = 16.0;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PopupKind {
    /// A calendar event is about to start
    Upcoming,
    /// A call was detected and the user is asked whether to record
    Detected,
    /// Recording was started automatically
    Recording,
}

pub struct PopupContent<'a> {
    pub kind: PopupKind,
    pub title: &'a str,
    pub subtitle: &'a str,
    pub meeting_url: Option<&'a str>,
}

pub fn show<R: Runtime>(app: &AppHandle<R>, content: PopupContent<'_>) {
    let kind = serde_json::to_value(content.kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    query
        .append_pair("kind", &kind)
        .append_pair("title", content.title)
        .append_pair("subtitle", content.subtitle);
    if let Some(meeting_url) = content.meeting_url {
        query.append_pair("meetingUrl", meeting_url);
    }
    let path = format!("popup?{}", query.finish());

    // Replace any prompt that is still open
    close(app);

    let mut builder = WebviewWindowBuilder::new(app, POPUP_LABEL, WebviewUrl::App(path.into()))
        .title(crate::APP_NAME)
        .inner_size(WIDTH, HEIGHT)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false);

    if let Ok(Some(monitor)) = app.primary_monitor() {
        let scale = monitor.scale_factor();
        let size = monitor.size().to_logical::<f64>(scale);
        let origin = monitor.position().to_logical::<f64>(scale);
        builder = builder.position(origin.x + size.width - WIDTH - MARGIN, origin.y + MARGIN * 3.0);
    }

    match builder.build() {
        Ok(_) => info!("Showing meeting popup: {} ({})", content.title, kind),
        Err(e) => error!("Failed to open meeting popup: {}", e),
    }
}

pub fn close<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(POPUP_LABEL) {
        let _ = window.destroy();
    }
}
