// What the window needs to know about the session it runs in: the desktop
// (this program is for GNOME Shell only), the display protocol (X11 or
// Wayland) and the version of the shell. Nothing here changes anything.

use serde::Serialize;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentInfo {
    pub desktop_environment: String,
    pub display_protocol: String,
    /// The major version of GNOME Shell, "46" for 46.0; empty when it cannot be told.
    pub shell_version: String,
}

/// The path of a program, looked for in PATH.
pub fn find_tool(name: &str) -> Option<PathBuf> {
    let dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|path| std::env::split_paths(&path).collect()).unwrap_or_default();
    dirs.into_iter().map(|dir| dir.join(name)).find(|candidate| candidate.is_file())
}

fn desktop_from(xdg_current_desktop: &str, gnome_session: bool) -> &'static str {
    if xdg_current_desktop.to_lowercase().contains("gnome") || gnome_session {
        "gnome"
    } else {
        "unknown"
    }
}

fn protocol_from(session_type: &str, wayland_display: bool, x_display: bool) -> &'static str {
    let session = session_type.to_lowercase();
    if session == "wayland" || wayland_display {
        "wayland"
    } else if session == "x11" || x_display {
        "x11"
    } else {
        "unknown"
    }
}

/// "GNOME Shell 46.0" gives "46".
pub fn major_version(text: &str) -> String {
    text.split_whitespace().last().and_then(|version| version.split('.').next()).filter(|major| !major.is_empty() && major.chars().all(|c| c.is_ascii_digit())).unwrap_or("").to_string()
}

pub fn shell_version() -> String {
    let Some(shell) = find_tool("gnome-shell") else { return String::new() };
    let output = Command::new(shell).arg("--version").stdin(Stdio::null()).stderr(Stdio::null()).output();
    output.ok().map(|out| major_version(&String::from_utf8_lossy(&out.stdout))).unwrap_or_default()
}

#[tauri::command]
pub async fn get_environment_info() -> EnvironmentInfo {
    tauri::async_runtime::spawn_blocking(|| EnvironmentInfo {
        desktop_environment: desktop_from(&std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(), std::env::var_os("GNOME_DESKTOP_SESSION_ID").is_some()).to_string(),
        display_protocol: protocol_from(
            &std::env::var("XDG_SESSION_TYPE").unwrap_or_default(),
            std::env::var_os("WAYLAND_DISPLAY").is_some(),
            std::env::var_os("DISPLAY").is_some(),
        )
        .to_string(),
        shell_version: shell_version(),
    })
    .await
    .unwrap_or(EnvironmentInfo { desktop_environment: "unknown".to_string(), display_protocol: "unknown".to_string(), shell_version: String::new() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_desktop_is_gnome_or_it_is_not_known() {
        assert_eq!(desktop_from("GNOME", false), "gnome");
        assert_eq!(desktop_from("ubuntu:GNOME", false), "gnome");
        assert_eq!(desktop_from("", true), "gnome");
        assert_eq!(desktop_from("XFCE", false), "unknown");
    }

    #[test]
    fn wayland_wins_over_a_display_variable_that_xwayland_also_sets() {
        assert_eq!(protocol_from("wayland", true, true), "wayland");
        assert_eq!(protocol_from("", true, true), "wayland");
        assert_eq!(protocol_from("x11", false, true), "x11");
        assert_eq!(protocol_from("", false, false), "unknown");
    }

    #[test]
    fn the_major_version_of_the_shell_comes_from_its_version_line() {
        assert_eq!(major_version("GNOME Shell 46.0\n"), "46");
        assert_eq!(major_version("GNOME Shell 47.2"), "47");
        assert_eq!(major_version(""), "");
        assert_eq!(major_version("nonsense"), "");
    }
}
