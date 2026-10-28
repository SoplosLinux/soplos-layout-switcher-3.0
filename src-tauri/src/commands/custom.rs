// The layouts the user makes. The four of the program are data (see layouts.rs);
// a layout of the user is a picture of the desktop at the moment it is saved:
// the extensions that are on, the settings of all the extensions (what
// `dconf dump /org/gnome/shell/extensions/` says) and the window buttons. It is
// kept as one JSON file for each layout in
// `~/.config/soplos-layout-switcher/layouts/`, which is the user's own and is
// read back as the user, never as root.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::environment::find_tool;
use super::extensions::valid_uuid;
use super::jobs::{failed_job, start_task};
use super::report::Report;

pub const MAX_NAME_CHARS: usize = 40;
const MAX_DUMP_BYTES: usize = 1_000_000;
const MAX_FILE_BYTES: u64 = 2_000_000;
/// Where the settings of the extensions live in dconf.
pub const EXTENSIONS_PATH: &str = "/org/gnome/shell/extensions/";
pub const BUTTONS_SCHEMA: &str = "org.gnome.desktop.wm.preferences";
pub const BUTTONS_KEY: &str = "button-layout";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SavedLayout {
    pub name: String,
    pub extensions: Vec<String>,
    pub dconf: String,
    /// The window buttons as gsettings prints them, quotes included.
    pub button_layout: String,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct CustomLayout {
    pub id: String,
    pub name: String,
}

// ---------------------------------------------------------------------------
// Names and ids

/// A name is short and made of letters, digits, spaces and . , _ - only.
pub fn valid_name(name: &str) -> bool {
    let name = name.trim();
    let count = name.chars().count();
    (1..=MAX_NAME_CHARS).contains(&count) && !name.starts_with('-') && name.chars().all(|c| c.is_alphanumeric() || " .,_-".contains(c))
}

fn fnv32(text: &str) -> u32 {
    text.bytes().fold(0x811c_9dc5u32, |hash, byte| (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193))
}

/// The id a name has: `custom-<the name in plain letters>-<a number from the
/// whole name>`. The prefix keeps it apart from the four layouts of the
/// program; the number keeps names that only differ in letters other than
/// A to Z (a name in Russian) from sharing one. The same name, whatever its
/// case, gives the same id, so saving it again replaces it.
pub fn id_of(name: &str) -> String {
    let name = name.trim();
    let mut slug = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    let slug: String = slug.chars().take(24).collect();
    let slug = slug.trim_end_matches('-');
    let slug = if slug.is_empty() { "layout" } else { slug };
    format!("custom-{slug}-{:08x}", fnv32(&name.to_lowercase()))
}

/// An id of this program, as it is made: safe to be the name of a file.
pub fn valid_id(id: &str) -> bool {
    id.len() <= 64 && id.strip_prefix("custom-").map(|rest| !rest.is_empty() && !rest.starts_with('-') && rest.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')).unwrap_or(false)
}

/// The value of the window buttons, as gsettings prints a string: between
/// single quotes, with the words of the setting and nothing else.
pub fn valid_button_layout(value: &str) -> bool {
    let Some(inner) = value.strip_prefix('\'').and_then(|rest| rest.strip_suffix('\'')) else { return false };
    inner.len() <= 64 && inner.chars().all(|c| c.is_ascii_lowercase() || matches!(c, ':' | ',' | '-' | '_'))
}

// ---------------------------------------------------------------------------
// Reading the desktop

/// The extensions of `gsettings get org.gnome.shell enabled-extensions`, which
/// prints `['a@b', 'c@d']`, or `@as []` when there are none.
pub fn parse_enabled_extensions(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('\'') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('\'') else { break };
        let uuid = &after[..end];
        if valid_uuid(uuid) && !found.iter().any(|known| known == uuid) {
            found.push(uuid.to_string());
        }
        rest = &after[end + 1..];
    }
    found
}

/// The output of a program, as text; None when it fails.
fn output_of(program: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The desktop as it is now, as a layout with the name given.
pub fn capture(name: &str, report: &dyn Report) -> Option<SavedLayout> {
    let Some(gsettings) = find_tool("gsettings") else {
        report.step("missing gsettings");
        return None;
    };
    let Some(dconf) = find_tool("dconf") else {
        report.step("missing dconf");
        return None;
    };
    let enabled = output_of(&gsettings, &["get", "org.gnome.shell", "enabled-extensions"])?;
    let dump = output_of(&dconf, &["dump", EXTENSIONS_PATH])?;
    let buttons = output_of(&gsettings, &["get", BUTTONS_SCHEMA, BUTTONS_KEY]).map(|text| text.trim().to_string()).filter(|text| valid_button_layout(text)).unwrap_or_default();
    if dump.len() > MAX_DUMP_BYTES {
        report.step("tooBig");
        return None;
    }
    Some(SavedLayout { name: name.trim().to_string(), extensions: parse_enabled_extensions(&enabled), dconf: dump, button_layout: buttons })
}

/// Puts the settings of the extensions of a layout back: `dconf load` reads
/// them from its input and sets the ones the layout holds.
pub fn load_settings(dump: &str, report: &dyn Report) -> bool {
    if dump.trim().is_empty() {
        return true;
    }
    let Some(dconf) = find_tool("dconf") else {
        report.step("missing dconf");
        return false;
    };
    let Ok(mut child) = Command::new(dconf).args(["load", EXTENSIONS_PATH]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() else {
        return false;
    };
    if let Some(mut input) = child.stdin.take() {
        if input.write_all(dump.as_bytes()).is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return false;
        }
    }
    child.wait().map(|status| status.success()).unwrap_or(false)
}

// ---------------------------------------------------------------------------
// The files

fn layouts_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/soplos-layout-switcher/layouts"))
}

fn file_of(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.json"))
}

fn load_in(dir: &Path, id: &str) -> Option<SavedLayout> {
    if !valid_id(id) {
        return None;
    }
    let path = file_of(dir, id);
    let meta = std::fs::symlink_metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return None;
    }
    let saved: SavedLayout = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    // What was read is checked as what is new: a file can be edited by hand.
    if !valid_name(&saved.name) || saved.dconf.len() > MAX_DUMP_BYTES || (!saved.button_layout.is_empty() && !valid_button_layout(&saved.button_layout)) {
        return None;
    }
    Some(SavedLayout { extensions: saved.extensions.into_iter().filter(|uuid| valid_uuid(uuid)).collect(), ..saved })
}

fn save_in(dir: &Path, layout: &SavedLayout) -> Result<String, String> {
    if !valid_name(&layout.name) {
        return Err("Not a valid name".to_string());
    }
    let id = id_of(&layout.name);
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(layout).map_err(|e| e.to_string())?;
    // Written beside the file and renamed over it, so a failure leaves the old one.
    let temporary = dir.join(format!(".{id}.json.new"));
    std::fs::write(&temporary, json).map_err(|e| e.to_string())?;
    std::fs::rename(&temporary, file_of(dir, &id)).map_err(|e| {
        let _ = std::fs::remove_file(&temporary);
        e.to_string()
    })?;
    Ok(id)
}

fn list_in(dir: &Path) -> Vec<CustomLayout> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut layouts: Vec<CustomLayout> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let id = name.strip_suffix(".json")?;
            load_in(dir, id).map(|saved| CustomLayout { id: id.to_string(), name: saved.name })
        })
        .collect();
    layouts.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    layouts
}

fn delete_in(dir: &Path, id: &str) -> bool {
    valid_id(id) && std::fs::remove_file(file_of(dir, id)).is_ok()
}

pub fn load(id: &str) -> Option<SavedLayout> {
    load_in(&layouts_dir()?, id)
}

pub fn exists(id: &str) -> bool {
    load(id).is_some()
}

// ---------------------------------------------------------------------------
// The commands

#[tauri::command]
pub async fn list_custom_layouts() -> Vec<CustomLayout> {
    tauri::async_runtime::spawn_blocking(|| layouts_dir().map(|dir| list_in(&dir)).unwrap_or_default()).await.unwrap_or_default()
}

/// Saves the desktop as it is now with the name given, as a job.
#[tauri::command]
pub fn save_current_layout(name: String) -> u64 {
    if !valid_name(&name) {
        return failed_job("Not a valid name".to_string());
    }
    start_task(move |log| {
        let Some(dir) = layouts_dir() else {
            log.line("There is no folder to keep the layout in");
            return false;
        };
        let Some(layout) = capture(&name, &log) else { return false };
        match save_in(&dir, &layout) {
            Ok(_) => true,
            Err(e) => {
                log.line(&e);
                false
            }
        }
    })
}

#[tauri::command]
pub async fn delete_custom_layout(id: String) -> bool {
    tauri::async_runtime::spawn_blocking(move || layouts_dir().map(|dir| delete_in(&dir, &id)).unwrap_or(false)).await.unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("soplos-layouts-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn layout(name: &str) -> SavedLayout {
        SavedLayout {
            name: name.to_string(),
            extensions: vec!["dash-to-dock@micxgx.gmail.com".to_string(), "forge@jmmaranan.com".to_string()],
            dconf: "[dash-to-dock]\nbackground-opacity=0.8\n".to_string(),
            button_layout: "':minimize,maximize,close'".to_string(),
        }
    }

    #[test]
    fn a_name_is_short_and_plain() {
        for name in ["My layout", "Trabajo 2", "Diseño nuevo", "Мой макет", "a.b,c_d-e", " padded "] {
            assert!(valid_name(name), "{name}");
        }
        for name in ["", "   ", "-x", "a/b", "a;b", "a\"b", "a$b", &"x".repeat(41)] {
            assert!(!valid_name(name), "{name}");
        }
    }

    #[test]
    fn an_id_is_safe_for_a_file_and_apart_from_the_four_layouts() {
        for name in ["My layout", "Diseño nuevo", "Мой макет", "../../etc", "a"] {
            let id = id_of(name);
            assert!(valid_id(&id), "{id}");
            assert!(id.starts_with("custom-"));
        }
        // The same name, whatever its case or its spaces, is the same layout.
        assert_eq!(id_of("My Layout"), id_of(" my layout "));
        assert_ne!(id_of("My layout"), id_of("My layout 2"));
        // Names with no letters of A to Z still differ.
        assert_ne!(id_of("Мой макет"), id_of("Другой макет"));
        for id in ["soplos", "dock", "custom-", "custom--x", "custom-a/b", "custom-A", "custom-a b", "../custom-a"] {
            assert!(!valid_id(id), "{id}");
        }
    }

    #[test]
    fn the_window_buttons_are_a_quoted_word_list() {
        for value in ["':minimize,maximize,close'", "'appmenu:close'", "''", "'close'"] {
            assert!(valid_button_layout(value), "{value}");
        }
        for value in ["", "close", "'close", "'clo;se'", "\"close\"", "'a b'", &format!("'{}'", "a".repeat(70))] {
            assert!(!valid_button_layout(value), "{value}");
        }
    }

    #[test]
    fn the_enabled_extensions_are_read_from_what_gsettings_prints() {
        assert_eq!(parse_enabled_extensions("['dash-to-dock@micxgx.gmail.com', 'forge@jmmaranan.com']\n"), vec!["dash-to-dock@micxgx.gmail.com", "forge@jmmaranan.com"]);
        assert_eq!(parse_enabled_extensions("@as []"), Vec::<String>::new());
        assert_eq!(parse_enabled_extensions("['a@b', 'a@b', 'not valid', 'c@d']"), vec!["a@b", "c@d"]);
        assert!(parse_enabled_extensions("").is_empty());
        assert!(parse_enabled_extensions("['unclosed").is_empty());
    }

    #[test]
    fn a_layout_is_saved_listed_loaded_and_deleted() {
        let dir = scratch("files");
        let saved = layout("My layout");
        let id = save_in(&dir, &saved).unwrap();
        assert_eq!(id, id_of("My layout"));
        assert_eq!(load_in(&dir, &id), Some(saved.clone()));
        assert_eq!(list_in(&dir), vec![CustomLayout { id: id.clone(), name: "My layout".to_string() }]);
        // Saving it again replaces it.
        let mut again = saved.clone();
        again.extensions = vec!["ding@rastersoft.com".to_string()];
        assert_eq!(save_in(&dir, &again).unwrap(), id);
        assert_eq!(list_in(&dir).len(), 1);
        assert_eq!(load_in(&dir, &id).unwrap().extensions, vec!["ding@rastersoft.com"]);
        // A second layout is listed after, by name.
        save_in(&dir, &layout("Another one")).unwrap();
        let names: Vec<String> = list_in(&dir).into_iter().map(|l| l.name).collect();
        assert_eq!(names, vec!["Another one", "My layout"]);
        assert!(delete_in(&dir, &id));
        assert!(!delete_in(&dir, &id));
        assert_eq!(list_in(&dir).len(), 1);
        assert!(save_in(&dir, &layout("bad;name")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_edited_by_hand_is_checked_when_it_is_read() {
        let dir = scratch("edited");
        std::fs::create_dir_all(&dir).unwrap();
        let id = id_of("Edited");
        let hostile = r#"{"name":"Edited","extensions":["../x@y","ok@fine.com"],"dconf":"","buttonLayout":"'a'; rm -rf ~"}"#;
        std::fs::write(file_of(&dir, &id), hostile).unwrap();
        assert!(load_in(&dir, &id).is_none());
        let fixed = r#"{"name":"Edited","extensions":["../x@y","ok@fine.com"],"dconf":"","buttonLayout":"'close'"}"#;
        std::fs::write(file_of(&dir, &id), fixed).unwrap();
        assert_eq!(load_in(&dir, &id).unwrap().extensions, vec!["ok@fine.com"]);
        // Only ids of the program are looked for, whatever the name of the file says.
        assert!(load_in(&dir, "../edited").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
