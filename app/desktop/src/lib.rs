//! The Exile Companion Tauri v2 shell. It owns the overlay window, plugins, and IPC, and delegates all
//! campaign logic to `exile-core`.

mod assistant;
mod commands;
mod hotkeys;
mod menu;
mod overlay;
mod pob;
mod runtime;
mod settings;
mod state;

use tauri::Manager;

pub use state::AppState;

// Builds before the rename kept their data under com.poebo.app.
fn carry_over_old_install(app: &tauri::AppHandle) {
    use tauri::Manager;
    let Ok(new) = app.path().app_config_dir() else {
        return;
    };
    let Some(old) = new.parent().map(|p| p.join("com.poebo.app")) else {
        return;
    };
    match exile_core::migrate::carry_over(&old, &new, &["settings.json", "build.xml"]) {
        Ok(copied) if !copied.is_empty() => {
            log::info!("carried over {copied:?} from {}", old.display())
        }
        Ok(_) => {}
        Err(e) => log::warn!("could not carry over {}: {e}", old.display()),
    }
}

/// The overlay has to stay above the game and place itself on the screen, and GNOME allows both
/// only to X11 windows. The game runs as one under Proton anyway.
#[cfg(target_os = "linux")]
fn linux_env() {
    if std::env::var_os("GDK_BACKEND").is_none() {
        std::env::set_var("GDK_BACKEND", "x11");
    }
    // WebKitGTK's DMA-BUF renderer leaves the window blank on NVIDIA's driver.
    if std::path::Path::new("/proc/driver/nvidia").exists()
        && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none()
    {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}

/// Build and run the application.
pub fn run() {
    // Before GTK starts and reads them.
    #[cfg(target_os = "linux")]
    linux_env();
    log_panics();
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .clear_targets()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                // The default name is the product name, which has a space in it.
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("exile-companion".into()),
                    },
                ))
                .level(log::LevelFilter::Info)
                .max_file_size(1_000_000)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            log::info!("Exile Companion {} starting", app.package_info().version);
            let data = runtime::load_data_pack()?;
            carry_over_old_install(app.handle());
            let saved = settings::load(app.handle()).unwrap_or_default();
            let route = runtime::load_route(&data, saved.league_start);
            app.manage(AppState::new(
                data,
                runtime::load_gems(),
                route,
                saved.route_edge,
                saved.route_enabled,
            ));
            // The windows are created here, after manage(): their events and the first IPC calls
            // read AppState, and a panic inside a window procedure aborts the process.
            for window in app.config().app.windows.clone() {
                tauri::WebviewWindowBuilder::from_config(app.handle(), &window)?.build()?;
            }
            overlay::restore_anchor(app.handle(), &saved);
            runtime::restore_build(app.handle());
            runtime::watch_build(app.handle().clone());

            if let Some(log) = exile_core::logfile::find_client_log(saved.last_log_path.as_deref())
            {
                let path = log.to_string_lossy().into_owned();
                log::info!("watching {path}");
                if let Err(e) = runtime::start_live(app.handle().clone(), path) {
                    log::error!("could not watch {}: {e}", log.display());
                }
            } else {
                log::info!("no Client.txt found");
            }

            overlay::create_tray(app.handle())?;
            overlay::set_grab(app.handle(), false);
            overlay::watch_game_focus(app.handle().clone());
            menu::watch_menu_hold(app.handle().clone());

            for e in hotkeys::register_all(app.handle()) {
                log::warn!("hotkey {} ({}): {}", e.action, e.shortcut, e.error);
            }

            log::info!("started");
            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::Moved(_) => overlay::on_moved(window),
            tauri::WindowEvent::Focused(false) if window.label() == menu::MENU_LABEL => {
                let _ = window.hide();
            }
            // Settings is created once at startup. Closing it only hides it.
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
            commands::get_pob,
            commands::get_assistant,
            commands::connect_agent,
            commands::copy_assistant_config,
            commands::pick_pob_folder,
            commands::get_pob_sync,
            commands::set_pob_sync,
            commands::pick_pob_build,
            commands::unpick_pob_build,
            commands::open_pob,
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
            commands::lock_overlay,
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
        .expect("error while running Exile Companion");
}

// The release build has no console, so a panic would vanish without this.
fn log_panics() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("panic: {info}");
        log::logger().flush();
        default(info);
    }));
}
