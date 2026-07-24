mod commands;
mod state;
mod tray;
mod windows;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = windows::show_workshop(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .setup(|app| {
            let app_state = state::AppState::open(app.handle())?;
            let snapshot = app_state.snapshot()?;
            app.manage(app_state);

            windows::initialize_pet_window(app.handle()).map_err(std::io::Error::other)?;
            tray::create(app, &snapshot)?;
            if std::env::args().any(|argument| argument == "--autostart")
                && let Some(workshop) = app.get_webview_window(windows::WORKSHOP_LABEL)
            {
                workshop.hide()?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_runtime_state,
            commands::set_active_character,
            commands::set_pet_visible,
            commands::set_paused,
            commands::set_click_through,
            commands::reset_pet_position,
            commands::adjust_pet_scale,
            commands::begin_pet_drag,
            commands::show_workshop,
            commands::get_autostart_enabled,
            commands::set_autostart_enabled,
        ])
        .on_window_event(|window, event| {
            let label = window.label();
            let app = window.app_handle();
            let exiting = app
                .try_state::<state::AppState>()
                .is_some_and(|state| state.is_exiting());

            if exiting {
                return;
            }

            match event {
                tauri::WindowEvent::CloseRequested { api, .. }
                    if label == windows::WORKSHOP_LABEL =>
                {
                    api.prevent_close();
                    let _ = window.hide();
                }
                tauri::WindowEvent::CloseRequested { api, .. } if label == windows::PET_LABEL => {
                    api.prevent_close();
                    let _ = commands::set_pet_visible_internal(app, false);
                }
                tauri::WindowEvent::Moved(_) if label == windows::PET_LABEL => {
                    if let Some(pet) = app.get_webview_window(windows::PET_LABEL) {
                        windows::schedule_position_persist(pet);
                    }
                }
                tauri::WindowEvent::ScaleFactorChanged { .. } if label == windows::PET_LABEL => {
                    if let Some(pet) = app.get_webview_window(windows::PET_LABEL) {
                        let _ = windows::restore_pet_window(app, &pet);
                    }
                }
                tauri::WindowEvent::Destroyed if label == windows::PET_LABEL => {
                    let Some(state) = app.try_state::<state::AppState>() else {
                        return;
                    };
                    if !state.claim_recreate_attempt() {
                        return;
                    }

                    let app = app.clone();
                    let callback_app = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        if let Err(error) = windows::recreate_pet_window(&callback_app) {
                            eprintln!("pet window recreation failed: {error}");
                            let _ = windows::show_workshop(&callback_app);
                        }
                    });
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("Epet desktop runtime failed");
}
