//! The poebo Tauri v2 shell. It owns the overlay window, plugins, and IPC, and delegates all
//! campaign logic to `poebo-core`.

mod commands;
mod hotkeys;
mod logfile;
mod menu;
mod overlay;
mod runtime;
mod settings;
mod state;

use tauri::Manager;

pub use state::AppState;

/// Build and run the application.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let data = runtime::load_data_pack()?;
            let saved = settings::load(app.handle()).unwrap_or_default();
            let route = runtime::load_route(&data, saved.league_start);
            app.manage(AppState::new(
                data,
                route,
                saved.route_edge,
                saved.route_enabled,
            ));
            overlay::restore_anchor(app.handle(), &saved);

            if let Some(log) = logfile::find_client_log(saved.last_log_path.as_deref()) {
                let path = log.to_string_lossy().into_owned();
                if let Err(e) = runtime::start_live(app.handle().clone(), path) {
                    eprintln!("could not watch {}: {e}", log.display());
                }
            }

            overlay::create_tray(app.handle())?;
            let grabbed = !cfg!(windows);
            overlay::set_grab(app.handle(), grabbed);
            overlay::watch_game_focus(app.handle().clone());
            menu::watch_esc_hold(app.handle().clone());

            for e in hotkeys::register_all(app.handle()) {
                eprintln!("hotkey {} ({}): {}", e.action, e.shortcut, e.error);
            }

            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::Moved(_) => overlay::on_moved(window),
            tauri::WindowEvent::Focused(false) if window.label() == menu::MENU_LABEL => {
                let _ = window.hide();
            }
            // Settings is created once at startup; closing it only hides it.
            tauri::WindowEvent::CloseRequested { api, .. }
                if window.label() == menu::SETTINGS_LABEL =>
            {
                api.prevent_close();
                let _ = window.hide();
            }
            _ => {}
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
            commands::import_build_file,
            commands::import_build_code,
            commands::clear_build,
            commands::get_build,
            commands::get_guidance,
            commands::get_route,
            commands::get_source,
            commands::get_overlay_state,
            commands::snap_overlay_bottom_center,
            commands::fit_overlay,
            commands::open_settings,
            commands::start_move,
            commands::close_menu,
            commands::quit_app,
            commands::get_route_enabled,
            commands::set_route_enabled,
            commands::get_hotkeys,
            commands::set_hotkeys,
            commands::pause_hotkeys,
            commands::resume_hotkeys,
            commands::route_next,
            commands::route_prev,
            commands::route_reset,
            commands::set_click_through,
            commands::get_settings,
            commands::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running poebo");
}
