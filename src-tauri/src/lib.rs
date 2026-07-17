//! The poebo Tauri v2 shell. It owns the overlay window, plugins, and IPC, and delegates all
//! campaign logic to `poebo-core`.

mod commands;
mod overlay;
mod runtime;
mod settings;
mod state;

use tauri::Manager;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub use state::AppState;

/// Build and run the application.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let zones = runtime::load_zone_table()?;
            app.manage(AppState::new(zones));

            let handle = app.handle();
            overlay::set_click_through_default(handle);

            // Ctrl+Alt+Space toggles the overlay between click-through and interactive.
            let toggle = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::Space);
            app.global_shortcut()
                .on_shortcut(toggle, move |app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        overlay::toggle_grab(app);
                    }
                })?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::pick_log_file,
            commands::start_live,
            commands::stop_source,
            commands::start_replay,
            commands::replay_pause,
            commands::replay_resume,
            commands::replay_step,
            commands::replay_set_speed,
            commands::get_state,
            commands::set_click_through,
            commands::get_settings,
            commands::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running poebo");
}
