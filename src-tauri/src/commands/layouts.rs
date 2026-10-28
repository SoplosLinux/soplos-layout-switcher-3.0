// The four layouts of the desktop (Soplos, Dock, Tiling and GNOME) and what it
// takes to switch to one: install the extensions it needs, set the options of
// those extensions, turn off the extensions of the other layouts and turn on
// its own. The lists were checked against `gnome-extensions list --enabled`
// and `dconf dump` on a real Soplos Boro install, as in the 2.x program, and
// are kept as they were.
//
// Five extensions are the user's own taste and are active in all four
// layouts, so no layout touches them: Coverflow Alt-Tab, Clipboard Indicator,
// Desktop Cube, Removable Drive Menu and User Themes.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use super::custom;
use super::environment::find_tool;
use super::extensions;
use super::jobs::{failed_job, start_task};
use super::report::Report;
use super::settings::{read_layout, write_layout};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Tool {
    /// `gsettings set <target> <key> <value>`
    Gsettings,
    /// `dconf write <target> <value>`, for the extensions whose schema is not
    /// installed system-wide (those installed in the user's folder).
    Dconf,
}

struct Setting {
    tool: Tool,
    target: &'static str,
    key: &'static str,
    value: &'static str,
}

const fn gs(target: &'static str, key: &'static str, value: &'static str) -> Setting {
    Setting { tool: Tool::Gsettings, target, key, value }
}

const fn dc(target: &'static str, value: &'static str) -> Setting {
    Setting { tool: Tool::Dconf, target, key: "", value }
}

pub struct Layout {
    pub id: &'static str,
    pub required: &'static [&'static str],
    pub conflicting: &'static [&'static str],
    settings: &'static [Setting],
    /// Whether the window buttons are put as `:minimize,maximize,close`.
    button_layout: bool,
}

const DASH_TO_PANEL: &str = "org.gnome.shell.extensions.dash-to-panel";
const ARCMENU: &str = "org.gnome.shell.extensions.arcmenu";
const DASH_TO_DOCK: &str = "org.gnome.shell.extensions.dash-to-dock";
const BUTTONS: (&str, &str, &str) = ("org.gnome.desktop.wm.preferences", "button-layout", ":minimize,maximize,close");

const SOPLOS_SETTINGS: &[Setting] = &[
    gs(DASH_TO_PANEL, "panel-position", "BOTTOM"),
    gs(DASH_TO_PANEL, "panel-size", "48"),
    gs(DASH_TO_PANEL, "show-running-apps", "true"),
    gs(DASH_TO_PANEL, "dot-color-1", "'#e66100'"),
    gs(DASH_TO_PANEL, "dot-color-override", "true"),
    gs(ARCMENU, "activate-on-hover", "true"),
    gs(ARCMENU, "category-icon-type", "'Full_Color'"),
    gs(ARCMENU, "custom-menu-button-icon-size", "32.0"),
    gs(ARCMENU, "dash-to-panel-standalone", "false"),
    gs(ARCMENU, "hide-overview-on-startup", "true"),
    gs(ARCMENU, "menu-background-color", "'rgba(48,48,49,0.98)'"),
    gs(ARCMENU, "menu-border-color", "'rgb(230,97,0)'"),
    gs(ARCMENU, "menu-border-radius", "15"),
    gs(ARCMENU, "menu-border-width", "1"),
    gs(ARCMENU, "menu-button-appearance", "'Icon'"),
    gs(ARCMENU, "menu-button-icon", "'/usr/share/branding-soplos/menusoplos.png'"),
    gs(ARCMENU, "menu-item-active-bg-color", "'rgb(230,97,0)'"),
    gs(ARCMENU, "menu-item-hover-bg-color", "'rgb(230,97,0)'"),
    gs(ARCMENU, "menu-layout", "'enterprise'"),
    gs(ARCMENU, "multi-monitor", "true"),
];

const DOCK_SETTINGS: &[Setting] = &[
    gs(DASH_TO_DOCK, "apply-custom-theme", "true"),
    gs(DASH_TO_DOCK, "background-opacity", "0.8"),
    gs(DASH_TO_DOCK, "custom-theme-shrink", "true"),
    gs(DASH_TO_DOCK, "dash-max-icon-size", "48"),
    gs(DASH_TO_DOCK, "dock-position", "'BOTTOM'"),
    gs(DASH_TO_DOCK, "dock-fixed", "false"),
    gs(DASH_TO_DOCK, "extend-height", "false"),
    gs(DASH_TO_DOCK, "height-fraction", "0.9"),
];

const TILING_SETTINGS: &[Setting] = &[
    gs(DASH_TO_DOCK, "apply-custom-theme", "false"),
    gs(DASH_TO_DOCK, "background-opacity", "0.8"),
    gs(DASH_TO_DOCK, "click-action", "'focus-minimize-or-previews'"),
    gs(DASH_TO_DOCK, "custom-theme-shrink", "true"),
    gs(DASH_TO_DOCK, "dash-max-icon-size", "32"),
    gs(DASH_TO_DOCK, "dock-fixed", "false"),
    gs(DASH_TO_DOCK, "dock-position", "'LEFT'"),
    gs(DASH_TO_DOCK, "extend-height", "false"),
    gs(DASH_TO_DOCK, "height-fraction", "0.9"),
    gs(DASH_TO_DOCK, "multi-monitor", "true"),
    gs(DASH_TO_DOCK, "scroll-action", "'switch-workspace'"),
    dc("/org/gnome/shell/extensions/openbar/bartype", "'Islands'"),
    dc("/org/gnome/shell/extensions/openbar/bcolor", "['0.902', '0.380', '0.000']"),
    dc("/org/gnome/shell/extensions/openbar/bg-change", "true"),
    dc("/org/gnome/shell/extensions/openbar/bguri", "'file:///usr/share/backgrounds/soplos/006.jpg'"),
    dc("/org/gnome/shell/extensions/openbar/hcolor", "['0.902', '0.380', '0.000']"),
    dc("/org/gnome/shell/extensions/openbar/height", "32.0"),
    dc("/org/gnome/shell/extensions/openbar/mhcolor", "['0.902', '0.380', '0.000']"),
    dc("/org/gnome/shell/extensions/openbar/mscolor", "['0.902', '0.380', '0.000']"),
    dc("/org/gnome/shell/extensions/blur-my-shell/dash-to-dock/blur", "true"),
    dc("/org/gnome/shell/extensions/blur-my-shell/dash-to-dock/pipeline", "'pipeline_default_rounded'"),
    dc("/org/gnome/shell/extensions/blur-my-shell/dash-to-dock/static-blur", "false"),
    dc("/org/gnome/shell/extensions/blur-my-shell/panel/pipeline", "'pipeline_default'"),
    dc("/org/gnome/shell/extensions/blur-my-shell/panel/static-blur", "false"),
];

pub const LAYOUTS: &[Layout] = &[
    Layout {
        id: "soplos",
        required: &["dash-to-panel@jderose9.github.com", "arcmenu@arcmenu.com", "ding@rastersoft.com", "quick-settings-tweaks@qwreey"],
        conflicting: &[
            "dash-to-dock@micxgx.gmail.com",
            "forge@jmmaranan.com",
            "openbar@neuromorph",
            "blur-my-shell@aunetx",
            "workspace-indicator@gnome-shell-extensions.gcampax.github.com",
            "appindicatorsupport@rgcjonas.gmail.com",
            "gtk4-ding@smedius.gitlab.com",
            "space-bar@luchrioh",
        ],
        settings: SOPLOS_SETTINGS,
        button_layout: true,
    },
    Layout {
        id: "dock",
        required: &["dash-to-dock@micxgx.gmail.com"],
        conflicting: &[
            "arcmenu@arcmenu.com",
            "dash-to-panel@jderose9.github.com",
            "ding@rastersoft.com",
            "gtk4-ding@smedius.gitlab.com",
            "quick-settings-tweaks@qwreey",
            "forge@jmmaranan.com",
            "openbar@neuromorph",
            "blur-my-shell@aunetx",
            "workspace-indicator@gnome-shell-extensions.gcampax.github.com",
            "appindicatorsupport@rgcjonas.gmail.com",
            "space-bar@luchrioh",
        ],
        settings: DOCK_SETTINGS,
        button_layout: false,
    },
    Layout {
        id: "tiling",
        required: &[
            "appindicatorsupport@rgcjonas.gmail.com",
            "dash-to-dock@micxgx.gmail.com",
            "openbar@neuromorph",
            "forge@jmmaranan.com",
            "blur-my-shell@aunetx",
            "workspace-indicator@gnome-shell-extensions.gcampax.github.com",
        ],
        conflicting: &[
            "dash-to-panel@jderose9.github.com",
            "arcmenu@arcmenu.com",
            "ding@rastersoft.com",
            "gtk4-ding@smedius.gitlab.com",
            "quick-settings-tweaks@qwreey",
            "material-shell@papyelgringo",
            "pop-shell@system76.com",
            "space-bar@luchrioh",
        ],
        settings: TILING_SETTINGS,
        button_layout: true,
    },
    Layout {
        id: "gnome",
        required: &[],
        conflicting: &[
            "dash-to-dock@micxgx.gmail.com",
            "arcmenu@arcmenu.com",
            "ding@rastersoft.com",
            "gtk4-ding@smedius.gitlab.com",
            "quick-settings-tweaks@qwreey",
            "dash-to-panel@jderose9.github.com",
            "forge@jmmaranan.com",
            "openbar@neuromorph",
            "blur-my-shell@aunetx",
            "workspace-indicator@gnome-shell-extensions.gcampax.github.com",
            "appindicatorsupport@rgcjonas.gmail.com",
            "space-bar@luchrioh",
        ],
        settings: &[],
        button_layout: true,
    },
];

pub fn find(id: &str) -> Option<&'static Layout> {
    LAYOUTS.iter().find(|layout| layout.id == id)
}

fn run_quiet(program: &PathBuf, args: &[&str]) -> bool {
    Command::new(program).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

fn apply_setting(setting: &Setting, gsettings: &PathBuf, dconf: Option<&PathBuf>) -> bool {
    match setting.tool {
        Tool::Gsettings => run_quiet(gsettings, &["set", setting.target, setting.key, setting.value]),
        Tool::Dconf => dconf.map(|dconf| run_quiet(dconf, &["write", setting.target, setting.value])).unwrap_or(false),
    }
}

fn pause(milliseconds: u64) {
    std::thread::sleep(Duration::from_millis(milliseconds));
}

/// Whether the id is one of the four layouts of the program or one the user
/// saved.
pub fn exists(id: &str) -> bool {
    find(id).is_some() || custom::exists(id)
}

/// Turns off the extensions to be turned off, waits, and turns on the ones to
/// be turned on, as the 2.x program did: the pauses are there so the shell is
/// done with the old layout before the new one starts. An extension that cannot
/// be turned on yet (a new one the shell has not seen) is reported.
fn change_extensions(gnome_extensions: &PathBuf, disable: &[&str], enable: &[&str], report: &dyn Report) {
    for uuid in disable {
        run_quiet(gnome_extensions, &["disable", uuid]);
        report.step(&format!("disabled {uuid}"));
        pause(500);
    }
    pause(1000);
    for uuid in enable {
        if run_quiet(gnome_extensions, &["enable", uuid]) {
            report.step(&format!("enabled {uuid}"));
        } else {
            report.step(&format!("enableFailed {uuid}"));
        }
        pause(500);
    }
}

/// Switches the desktop to a layout of the user: the extensions it had are
/// the ones that are on, with the settings of all of them and the window
/// buttons it had.
fn apply_custom(id: &str, report: &dyn Report) -> bool {
    let Some(saved) = custom::load(id) else {
        report.line(&format!("Unknown layout: {id}"));
        return false;
    };
    let (Some(gsettings), Some(gnome_extensions)) = (find_tool("gsettings"), find_tool("gnome-extensions")) else {
        report.step("missing gnome-extensions");
        return false;
    };

    report.step("ensuring");
    let wanted: Vec<&str> = saved.extensions.iter().map(String::as_str).collect();
    extensions::ensure(&wanted, report);

    report.step(&format!("applying {}", saved.name));
    if !custom::load_settings(&saved.dconf, report) {
        report.step("settingFailed extensions");
    }
    if !saved.button_layout.is_empty() {
        run_quiet(&gsettings, &["set", custom::BUTTONS_SCHEMA, custom::BUTTONS_KEY, &saved.button_layout]);
    }

    let enabled = Command::new(&gsettings).args(["get", "org.gnome.shell", "enabled-extensions"]).stdin(Stdio::null()).stderr(Stdio::null()).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let current = custom::parse_enabled_extensions(&enabled);
    let to_disable: Vec<&str> = current.iter().map(String::as_str).filter(|uuid| !wanted.contains(uuid)).collect();
    let to_enable: Vec<&str> = wanted.iter().copied().filter(|uuid| !current.iter().any(|on| on == uuid)).collect();
    change_extensions(&gnome_extensions, &to_disable, &to_enable, report);

    if !write_layout(id) {
        report.step("saveFailed");
    }
    run_quiet(&gsettings, &["set", "org.gnome.shell", "disable-user-extensions", "false"]);
    report.step(&format!("applied {}", saved.name));
    true
}

/// Switches the desktop to a layout. True when it was done; extensions that
/// could not be turned on yet (a new one the shell has not seen) are reported
/// and do not make it fail: the desktop has to be reloaded for them.
pub fn apply(id: &str, report: &dyn Report) -> bool {
    let Some(layout) = find(id) else {
        return if custom::valid_id(id) {
            apply_custom(id, report)
        } else {
            report.line(&format!("Unknown layout: {id}"));
            false
        };
    };
    let (Some(gsettings), Some(gnome_extensions)) = (find_tool("gsettings"), find_tool("gnome-extensions")) else {
        report.step("missing gnome-extensions");
        return false;
    };
    let dconf = find_tool("dconf");

    report.step("ensuring");
    extensions::ensure(layout.required, report);

    report.step(&format!("applying {id}"));
    for setting in layout.settings {
        if !apply_setting(setting, &gsettings, dconf.as_ref()) {
            report.step(&format!("settingFailed {}", if setting.key.is_empty() { setting.target } else { setting.key }));
        }
    }
    if layout.button_layout {
        run_quiet(&gsettings, &["set", BUTTONS.0, BUTTONS.1, BUTTONS.2]);
    }

    let enabled = Command::new(&gsettings).args(["get", "org.gnome.shell", "enabled-extensions"]).stdin(Stdio::null()).stderr(Stdio::null()).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let to_disable: Vec<&str> = layout.conflicting.iter().copied().filter(|uuid| enabled.contains(uuid)).collect();
    let to_enable: Vec<&str> = layout.required.iter().copied().filter(|uuid| !enabled.contains(uuid)).collect();
    change_extensions(&gnome_extensions, &to_disable, &to_enable, report);

    if !write_layout(id) {
        report.step("saveFailed");
    }
    run_quiet(&gsettings, &["set", "org.gnome.shell", "disable-user-extensions", "false"]);
    report.step(&format!("applied {id}"));
    true
}

/// Reloads the desktop: a Wayland session cannot restart the shell, so the
/// session is closed; an X11 one restarts it.
pub fn reload(report: &dyn Report) -> bool {
    let wayland = Command::new("pgrep").arg("Xwayland").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
    if wayland {
        let Some(quit) = find_tool("gnome-session-quit") else {
            report.step("missing gnome-session-quit");
            return false;
        };
        report.step("loggingOut");
        return run_quiet(&quit, &["--logout"]);
    }
    let Some(busctl) = find_tool("busctl") else {
        report.step("missing busctl");
        return false;
    };
    report.step("restartingShell");
    run_quiet(&busctl, &["--user", "call", "org.gnome.Shell", "/org/gnome/Shell", "org.gnome.Shell", "Eval", "s", "Meta.restart(\"Restarting GNOME...\")"])
}

/// The layout the user chose last, if any.
#[tauri::command]
pub async fn get_layout() -> Option<String> {
    tauri::async_runtime::spawn_blocking(|| read_layout().filter(|id| exists(id))).await.ok().flatten()
}

#[tauri::command]
pub fn apply_layout(id: String) -> u64 {
    if !exists(&id) {
        return failed_job(format!("Unknown layout: {id}"));
    }
    start_task(move |log| apply(&id, &log))
}

#[tauri::command]
pub fn reload_desktop() -> u64 {
    start_task(|log| reload(&log))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct Recorder(RefCell<Vec<String>>);

    impl Report for Recorder {
        fn line(&self, text: &str) {
            self.0.borrow_mut().push(text.to_string());
        }
        fn step(&self, text: &str) {
            self.0.borrow_mut().push(format!("@@step {text}"));
        }
    }

    #[test]
    fn there_are_four_layouts_with_their_own_names() {
        let ids: Vec<&str> = LAYOUTS.iter().map(|layout| layout.id).collect();
        assert_eq!(ids, vec!["soplos", "dock", "tiling", "gnome"]);
        assert!(find("dock").is_some());
        assert!(find("eval(1)").is_none());
    }

    #[test]
    fn every_extension_of_a_layout_has_a_valid_id() {
        for layout in LAYOUTS {
            for uuid in layout.required.iter().chain(layout.conflicting.iter()) {
                assert!(extensions::valid_uuid(uuid), "{} {uuid}", layout.id);
            }
        }
    }

    #[test]
    fn no_layout_asks_for_an_extension_it_also_turns_off() {
        for layout in LAYOUTS {
            for uuid in layout.required {
                assert!(!layout.conflicting.contains(uuid), "{} {uuid}", layout.id);
            }
        }
    }

    #[test]
    fn what_a_layout_needs_is_turned_off_by_the_others() {
        // Switching away from a layout must not leave its extensions on.
        for layout in LAYOUTS {
            for other in LAYOUTS.iter().filter(|other| other.id != layout.id) {
                for uuid in other.required {
                    if layout.required.contains(uuid) {
                        continue;
                    }
                    assert!(layout.conflicting.contains(uuid), "{} does not turn off {uuid} of {}", layout.id, other.id);
                }
            }
        }
    }

    #[test]
    fn the_settings_of_a_layout_name_what_they_set() {
        for layout in LAYOUTS {
            for setting in layout.settings {
                assert!(!setting.target.is_empty() && !setting.value.is_empty());
                match setting.tool {
                    Tool::Gsettings => assert!(!setting.key.is_empty() && setting.target.starts_with("org.gnome."), "{}", setting.target),
                    Tool::Dconf => assert!(setting.key.is_empty() && setting.target.starts_with("/org/gnome/shell/extensions/"), "{}", setting.target),
                }
            }
        }
        assert!(find("dock").unwrap().settings.iter().all(|s| s.tool == Tool::Gsettings));
        assert!(find("tiling").unwrap().settings.iter().any(|s| s.tool == Tool::Dconf));
    }

    #[test]
    fn an_unknown_layout_is_refused_before_anything_is_touched() {
        let recorder = Recorder(RefCell::new(Vec::new()));
        assert!(!apply("nope", &recorder));
        assert_eq!(recorder.0.borrow().len(), 1);
    }
}
