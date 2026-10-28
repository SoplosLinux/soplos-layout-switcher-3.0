mod commands;
mod system_theme;

pub use commands::privileged::run_ops_file;

/// The command line of the 2.x program, `soplos-layout-switcher-cli`, for
/// scripts and installers that need a layout without a window: apply-soplos,
/// apply-dock, apply-gnome, apply-tiling and reload-gnome-shell. Returns the
/// exit code.
pub fn run_cli(args: &[String]) -> i32 {
    use commands::{layouts, report::Console};
    let ok = match args.first().map(String::as_str) {
        Some("reload-gnome-shell") => layouts::reload(&Console),
        Some(command) => match command.strip_prefix("apply-") {
            Some(id) if layouts::exists(id) => layouts::apply(id, &Console),
            _ => {
                eprintln!("Unknown command: {command}");
                return 2;
            }
        },
        None => {
            eprintln!("Usage: soplos-layout-switcher-cli COMMAND");
            eprintln!("Commands: apply-soplos, apply-dock, apply-gnome, apply-tiling, reload-gnome-shell");
            return 1;
        }
    };
    if ok {
        0
    } else {
        1
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be the first plugin registered, same as the other Soplos
        // Tauri apps: it needs to intercept and exit a second launch before
        // anything else runs.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            system_theme::get_system_theme,
            commands::environment::get_environment_info,
            commands::jobs::poll_job,
            commands::layouts::get_layout,
            commands::layouts::apply_layout,
            commands::layouts::reload_desktop,
            commands::custom::list_custom_layouts,
            commands::custom::save_current_layout,
            commands::custom::delete_custom_layout,
            commands::settings::get_switches,
            commands::settings::set_switch,
            commands::settings::open_tool_job,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Soplos Layout Switcher");
}
