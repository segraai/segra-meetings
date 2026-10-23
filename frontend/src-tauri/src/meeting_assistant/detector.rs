//! Detects an active call by looking at which apps are capturing the microphone.
//!
//! Linux: PulseAudio/PipeWire source outputs (`pactl list source-outputs`), which name the
//! process that owns each capture stream. Zoom only opens one while in a meeting; browsers
//! open one for Google Meet (and other web calls).
//! macOS/Windows: Zoom's in-meeting helper process (`CptHost`).

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CallApp {
    Zoom,
    Teams,
    Slack,
    /// A web browser capturing the mic: most likely Google Meet
    Browser,
}

impl CallApp {
    pub fn display_name(&self) -> &'static str {
        match self {
            CallApp::Zoom => "Zoom",
            CallApp::Teams => "Microsoft Teams",
            CallApp::Slack => "Slack huddle",
            CallApp::Browser => "Browser call",
        }
    }
}

/// Classifies an app capturing the microphone. Returns None for apps that aren't call apps.
pub fn classify_app(name: &str) -> Option<CallApp> {
    let name = name.to_lowercase();
    if name.contains("meetily") || name.contains("segra_meetings") {
        return None;
    }
    if name.contains("zoom") {
        Some(CallApp::Zoom)
    } else if name.contains("teams") {
        Some(CallApp::Teams)
    } else if name.contains("slack") {
        Some(CallApp::Slack)
    } else if ["chrome", "chromium", "firefox", "brave", "msedge", "vivaldi", "opera", "zen"]
        .iter()
        .any(|browser| name.contains(browser))
    {
        Some(CallApp::Browser)
    } else {
        None
    }
}

/// Returns the call app currently capturing the microphone, preferring dedicated apps
/// over browsers.
pub async fn detect_active_call() -> Option<CallApp> {
    let apps = capturing_apps().await;
    let mut found: Vec<CallApp> = apps.iter().filter_map(|a| classify_app(a)).collect();
    found.sort_by_key(|app| *app == CallApp::Browser);
    found.into_iter().next()
}

#[cfg(target_os = "linux")]
async fn capturing_apps() -> Vec<String> {
    let output = tokio::process::Command::new("pactl")
        .args(["list", "source-outputs"])
        .env("LC_ALL", "C")
        .output()
        .await;
    match output {
        Ok(output) if output.status.success() => {
            parse_source_outputs(&String::from_utf8_lossy(&output.stdout), std::process::id())
        }
        _ => Vec::new(),
    }
}

#[cfg(not(target_os = "linux"))]
async fn capturing_apps() -> Vec<String> {
    tokio::task::spawn_blocking(|| {
        use sysinfo::{ProcessRefreshKind, RefreshKind, System};
        let system = System::new_with_specifics(
            RefreshKind::new().with_processes(ProcessRefreshKind::new()),
        );
        let in_zoom_meeting = system
            .processes()
            .values()
            .any(|p| p.name().to_string_lossy().to_lowercase().starts_with("cpthost"));
        if in_zoom_meeting {
            vec!["zoom".to_string()]
        } else {
            Vec::new()
        }
    })
    .await
    .unwrap_or_default()
}

/// Extracts the owning app of each capture stream from `pactl list source-outputs`,
/// skipping streams that belong to this process and monitor (system audio) captures.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn parse_source_outputs(output: &str, own_pid: u32) -> Vec<String> {
    let mut apps = Vec::new();
    for block in output.split("Source Output #").skip(1) {
        let property = |key: &str| {
            block.lines().find_map(|line| {
                let (k, v) = line.trim().split_once(" = ")?;
                (k == key).then(|| v.trim_matches('"').to_string())
            })
        };
        if property("application.process.id").and_then(|pid| pid.parse::<u32>().ok())
            == Some(own_pid)
        {
            continue;
        }
        let is_monitor = block
            .lines()
            .any(|line| line.trim().starts_with("Source:") && line.contains(".monitor"));
        if is_monitor {
            continue;
        }
        if let Some(app) = property("application.process.binary").or_else(|| property("application.name")) {
            apps.push(app);
        }
    }
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"Source Output #41
	Driver: PipeWire
	Owner Module: n/a
	Client: 40
	Source: 52
	Properties:
		application.name = "ZOOM VoiceEngine"
		application.process.id = "3121"
		application.process.binary = "zoom"
Source Output #57
	Driver: PipeWire
	Source: 52
	Properties:
		application.name = "meetily"
		application.process.id = "999"
		application.process.binary = "meetily"
Source Output #60
	Source: alsa_output.pci-0000_00_1f.3.analog-stereo.monitor
	Properties:
		application.process.binary = "obs"
"#;

    #[test]
    fn parses_capture_streams_skipping_own_and_monitors() {
        assert_eq!(parse_source_outputs(SAMPLE, 999), vec!["zoom".to_string()]);
    }

    #[test]
    fn classifies_call_apps() {
        assert_eq!(classify_app("zoom"), Some(CallApp::Zoom));
        assert_eq!(classify_app("google-chrome"), Some(CallApp::Browser));
        assert_eq!(classify_app("firefox-bin"), Some(CallApp::Browser));
        assert_eq!(classify_app("meetily"), None);
        assert_eq!(classify_app("segra_meetings"), None);
        assert_eq!(classify_app("obs"), None);
    }
}
