// The Settings page: the two switches of the 2.x program (the system tray and
// the desktop icons, which are two GNOME Shell extensions), the buttons that
// open other tools, and the file that remembers the layout chosen last. The
// file is the one of the 2.x program, `~/.config/soplos-layout-switcher.conf`,
// with the layout under `[SETTING]`, so a layout chosen with that version is
// still the one shown.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::environment::find_tool;
use super::jobs::{failed_job, start_task};
use super::report::Report;

const TRAY: &str = "appindicatorsupport@rgcjonas.gmail.com";
const DESKTOP_ICONS: &str = "ding@rastersoft.com";

const EXTENSION_MANAGER: &str = "com.mattjakeman.ExtensionManager";
const GDM_SETTINGS: &str = "io.github.realmazharhussain.GdmSettings";
const ADD_WATER: &str = "dev.qwery.AddWater";
const REFINE: &str = "page.tesk.Refine";

// ---------------------------------------------------------------------------
// The file of the layout

fn conf_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/soplos-layout-switcher.conf"))
}

/// The value of `layout` under `[SETTING]`.
pub fn parse_layout(text: &str) -> Option<String> {
    let mut in_section = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_section = line[1..line.len() - 1].eq_ignore_ascii_case("setting");
        } else if in_section {
            if let Some((key, value)) = line.split_once(['=', ':']) {
                if key.trim().eq_ignore_ascii_case("layout") {
                    let value = value.trim();
                    return (!value.is_empty()).then(|| value.to_string());
                }
            }
        }
    }
    None
}

/// The text of the file with the layout set, keeping everything else in it.
pub fn with_layout(text: &str, id: &str) -> String {
    let mut out = Vec::new();
    let mut in_section = false;
    let mut has_section = false;
    let mut done = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            // Leaving the section without having found the key: it goes at its end.
            if in_section && !done {
                out.push(format!("layout = {id}"));
                done = true;
            }
            in_section = trimmed[1..trimmed.len() - 1].eq_ignore_ascii_case("setting");
            has_section |= in_section;
            out.push(line.to_string());
            continue;
        }
        if in_section && !done {
            if let Some((key, _)) = trimmed.split_once(['=', ':']) {
                if key.trim().eq_ignore_ascii_case("layout") {
                    out.push(format!("layout = {id}"));
                    done = true;
                    continue;
                }
            }
        }
        out.push(line.to_string());
    }
    if !done {
        if !has_section {
            if !out.is_empty() && !out.last().map(|l| l.trim().is_empty()).unwrap_or(true) {
                out.push(String::new());
            }
            out.push("[SETTING]".to_string());
        }
        out.push(format!("layout = {id}"));
    }
    let mut result = out.join("\n");
    result.push('\n');
    result
}

pub fn read_layout() -> Option<String> {
    parse_layout(&std::fs::read_to_string(conf_path()?).ok()?)
}

pub fn write_layout(id: &str) -> bool {
    let Some(path) = conf_path() else { return false };
    let current = std::fs::read_to_string(&path).unwrap_or_default();
    if let Some(folder) = path.parent() {
        let _ = std::fs::create_dir_all(folder);
    }
    std::fs::write(&path, with_layout(&current, id)).is_ok()
}

// ---------------------------------------------------------------------------
// The switches

/// The extensions that are on, as `gnome-extensions list --enabled` says it,
/// which does not depend on the language, as the text of `info` does.
fn enabled_extensions() -> Vec<String> {
    let Some(tool) = find_tool("gnome-extensions") else { return Vec::new() };
    let output = Command::new(tool).args(["list", "--enabled"]).stdin(Stdio::null()).stderr(Stdio::null()).output();
    output.map(|o| String::from_utf8_lossy(&o.stdout).lines().map(|line| line.trim().to_string()).filter(|line| !line.is_empty()).collect()).unwrap_or_default()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Switches {
    pub tray: bool,
    pub desktop_icons: bool,
}

#[tauri::command]
pub async fn get_switches() -> Switches {
    tauri::async_runtime::spawn_blocking(|| {
        let on = enabled_extensions();
        Switches { tray: on.iter().any(|uuid| uuid == TRAY), desktop_icons: on.iter().any(|uuid| uuid == DESKTOP_ICONS) }
    })
    .await
    .unwrap_or(Switches { tray: false, desktop_icons: false })
}

fn switch_extension(kind: &str) -> Option<&'static str> {
    match kind {
        "tray" => Some(TRAY),
        "desktopIcons" => Some(DESKTOP_ICONS),
        _ => None,
    }
}

/// Turns one of the two extensions on or off. True when it worked.
#[tauri::command]
pub async fn set_switch(kind: String, enabled: bool) -> bool {
    tauri::async_runtime::spawn_blocking(move || {
        let (Some(uuid), Some(tool)) = (switch_extension(&kind), find_tool("gnome-extensions")) else { return false };
        Command::new(tool).args([if enabled { "enable" } else { "disable" }, uuid]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
    })
    .await
    .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// The tools

fn run_ok(program: &Path, args: &[&str]) -> bool {
    Command::new(program).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

/// Starts a program that stays open after this job ends.
fn launch(program: &Path, args: &[&str]) -> bool {
    match Command::new(program).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
        Ok(mut child) => {
            // Waited for on its own thread, so it does not stay a zombie.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            true
        }
        Err(_) => false,
    }
}

fn flatpak_installed(flatpak: &Path, app: &str) -> bool {
    run_ok(flatpak, &["info", app])
}

/// Opens a Flatpak app, saying so when it is not installed.
fn open_flatpak(app: &str, report: &dyn Report) -> bool {
    let Some(flatpak) = find_tool("flatpak") else {
        report.step("missing flatpak");
        return false;
    };
    if !flatpak_installed(&flatpak, app) {
        report.step(&format!("notInstalled {app}"));
        return false;
    }
    launch(&flatpak, &["run", app])
}

fn open_program(name: &str, report: &dyn Report) -> bool {
    match find_tool(name) {
        Some(program) => launch(&program, &[]),
        None => {
            report.step(&format!("missing {name}"));
            false
        }
    }
}

/// The tools of the 2.x program, by the name the window gives them.
pub fn open_tool(tool: &str, report: &dyn Report) -> bool {
    match tool {
        "tweaks" => open_program("gnome-tweaks", report),
        // Extension Manager browses and searches extensions.gnome.org; the
        // stock Extensions app only lists what is installed. Debian packages it
        // as a program; otherwise its Flatpak is used.
        "extensions" => {
            if let Some(program) = find_tool("extension-manager") {
                launch(&program, &[])
            } else {
                open_flatpak(EXTENSION_MANAGER, report)
            }
        }
        // The accent color is a feature of GNOME Settings itself now.
        "accent" => open_program("gnome-control-center", report),
        // GDM Settings has no Debian package; it is a Flatpak.
        "gdm" => open_flatpak(GDM_SETTINGS, report),
        // Refine, the alternative to GNOME Tweaks, and Add Water, which themes
        // Firefox, have no Debian package either: they are Flatpaks, installed
        // for the user, which needs no administrator, the first time they are
        // used.
        "refine" => open_flatpak_installing(REFINE, "Refine", report),
        "firefox" => open_flatpak_installing(ADD_WATER, "Add Water", report),
        _ => false,
    }
}

/// Opens a Flatpak app, installing it from Flathub for the user first when it
/// is not there.
fn open_flatpak_installing(app: &str, name: &str, report: &dyn Report) -> bool {
    let Some(flatpak) = find_tool("flatpak") else {
        report.step("missing flatpak");
        return false;
    };
    if !flatpak_installed(&flatpak, app) {
        report.step(&format!("installingFlatpak {name}"));
        if !run_ok(&flatpak, &["install", "-y", "--user", "flathub", app]) {
            report.step(&format!("flatpakFailed {name}"));
            return false;
        }
    }
    launch(&flatpak, &["run", app])
}

pub const TOOLS: &[&str] = &["tweaks", "refine", "extensions", "accent", "gdm", "firefox"];

#[tauri::command]
pub fn open_tool_job(tool: String) -> u64 {
    if !TOOLS.contains(&tool.as_str()) {
        return failed_job(format!("Unknown tool: {tool}"));
    }
    start_task(move |log| open_tool(&tool, &log))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layout_is_read_from_the_setting_section() {
        assert_eq!(parse_layout("[SETTING]\nlayout = tiling\n"), Some("tiling".to_string()));
        assert_eq!(parse_layout("[SETTING]\nlayout=dock"), Some("dock".to_string()));
        assert_eq!(parse_layout("[OTHER]\nlayout = gnome\n[SETTING]\nlayout = soplos\n"), Some("soplos".to_string()));
        assert_eq!(parse_layout("[OTHER]\nlayout = gnome\n"), None);
        assert_eq!(parse_layout("[SETTING]\nlayout =\n"), None);
        assert_eq!(parse_layout(""), None);
    }

    #[test]
    fn the_layout_is_written_keeping_the_rest_of_the_file() {
        assert_eq!(with_layout("", "dock"), "[SETTING]\nlayout = dock\n");
        assert_eq!(with_layout("[SETTING]\nlayout = soplos\n", "tiling"), "[SETTING]\nlayout = tiling\n");
        assert_eq!(with_layout("[SETTING]\nother = 1\n", "gnome"), "[SETTING]\nother = 1\nlayout = gnome\n");
        assert_eq!(with_layout("[TEST]\nfirst = True\n", "dock"), "[TEST]\nfirst = True\n\n[SETTING]\nlayout = dock\n");
        let both = with_layout("[SETTING]\nlayout = soplos\n\n[SIZE]\nx = 5\n", "dock");
        assert_eq!(parse_layout(&both), Some("dock".to_string()));
        assert!(both.contains("[SIZE]\nx = 5"));
        // Writing it twice gives the same file.
        assert_eq!(with_layout(&both, "dock"), both);
    }

    #[test]
    fn a_switch_names_one_of_two_extensions() {
        assert_eq!(switch_extension("tray"), Some(TRAY));
        assert_eq!(switch_extension("desktopIcons"), Some(DESKTOP_ICONS));
        assert_eq!(switch_extension("forge@jmmaranan.com"), None);
        assert_eq!(switch_extension(""), None);
    }

    #[test]
    fn the_tools_are_a_closed_list() {
        for tool in ["tweaks", "refine", "extensions", "accent", "gdm", "firefox"] {
            assert!(TOOLS.contains(&tool));
        }
        assert!(!TOOLS.contains(&"rm"));
    }
}
