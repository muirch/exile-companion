//! The poebo Tauri v2 shell. It owns the overlay window, plugins, and IPC, and delegates all
//! campaign logic to `poebo-core`.

mod commands;
mod logfile;
mod menu;
mod overlay;
mod runtime;
mod settings;
mod state;

use runtime::RouteMove;
use tauri::Manager;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

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

            // The overlay starts interactive so its controls are reachable without a hotkey.
            // Ctrl+Alt+Space toggles click-through on X11; on Wayland global shortcuts do not fire.
            let toggle = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::Space);
            app.global_shortcut()
                .on_shortcut(toggle, move |app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        overlay::toggle_grab(app);
                    }
                })?;

            overlay::create_tray(app.handle())?;
            let grabbed = !cfg!(windows);
            overlay::set_grab(app.handle(), grabbed);
            overlay::watch_game_focus(app.handle().clone());
            menu::watch_esc_hold(app.handle().clone());

            for (code, mv) in [(Code::PageDown, 0), (Code::PageUp, 1)] {
                let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), code);
                app.global_shortcut()
                    .on_shortcut(shortcut, move |app, _shortcut, event| {
                        if event.state() == ShortcutState::Pressed {
                            let mv = if mv == 0 {
                                RouteMove::Next
                            } else {
                                RouteMove::Prev
                            };
                            runtime::move_route(app, mv);
                        }
                    })?;
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
