use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, Wry};
use tauri_plugin_autostart::ManagerExt;

use crate::commands;
use crate::state::{AppState, RuntimeState};
use crate::windows;

pub struct TrayControls {
    visible: CheckMenuItem<Wry>,
    click_through: CheckMenuItem<Wry>,
    paused: CheckMenuItem<Wry>,
    autostart: CheckMenuItem<Wry>,
}

pub fn create(app: &mut App, snapshot: &RuntimeState) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open-workshop", "打开角色工坊", true, None::<&str>)?;
    let visible = CheckMenuItem::with_id(
        app,
        "pet-visible",
        "显示角色",
        true,
        snapshot.visible,
        None::<&str>,
    )?;
    let click_through = CheckMenuItem::with_id(
        app,
        "click-through",
        "鼠标穿透",
        true,
        snapshot.click_through,
        None::<&str>,
    )?;
    let paused = CheckMenuItem::with_id(
        app,
        "paused",
        "暂停动画",
        true,
        snapshot.paused,
        None::<&str>,
    )?;
    let reset = MenuItem::with_id(app, "reset-position", "重置角色位置", true, None::<&str>)?;
    let autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);
    let autostart = CheckMenuItem::with_id(
        app,
        "autostart",
        "开机启动",
        true,
        autostart_enabled,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "退出 Epet", true, None::<&str>)?;
    let first_separator = PredefinedMenuItem::separator(app)?;
    let second_separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &first_separator,
            &visible,
            &click_through,
            &paused,
            &reset,
            &autostart,
            &second_separator,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .tooltip("Epet 桌面角色")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let result = match event.id().as_ref() {
                "open-workshop" => windows::show_workshop(app),
                "pet-visible" => app
                    .state::<AppState>()
                    .snapshot()
                    .map_err(|error| error.to_string())
                    .and_then(|state| commands::set_pet_visible_internal(app, !state.visible))
                    .map(|_| ()),
                "click-through" => app
                    .state::<AppState>()
                    .snapshot()
                    .map_err(|error| error.to_string())
                    .and_then(|state| {
                        commands::set_click_through_internal(app, !state.click_through)
                    })
                    .map(|_| ()),
                "paused" => app
                    .state::<AppState>()
                    .snapshot()
                    .map_err(|error| error.to_string())
                    .and_then(|state| commands::set_paused_internal(app, !state.paused))
                    .map(|_| ()),
                "reset-position" => commands::reset_pet_position_internal(app).map(|_| ()),
                "autostart" => toggle_autostart(app),
                "quit" => {
                    app.state::<AppState>().begin_exit();
                    app.exit(0);
                    Ok(())
                }
                _ => Ok(()),
            };

            if let Err(error) = result {
                eprintln!("tray action failed: {error}");
            }
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                let _ = windows::show_workshop(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(app)?;

    app.manage(TrayControls {
        visible,
        click_through,
        paused,
        autostart,
    });
    Ok(())
}

pub fn sync_checks(app: &AppHandle, snapshot: &RuntimeState) {
    let Some(controls) = app.try_state::<TrayControls>() else {
        return;
    };
    let _ = controls.visible.set_checked(snapshot.visible);
    let _ = controls.click_through.set_checked(snapshot.click_through);
    let _ = controls.paused.set_checked(snapshot.paused);
}

pub fn sync_autostart(app: &AppHandle, enabled: bool) {
    if let Some(controls) = app.try_state::<TrayControls>() {
        let _ = controls.autostart.set_checked(enabled);
    }
}

fn toggle_autostart(app: &AppHandle) -> Result<(), String> {
    let manager = app.autolaunch();
    let enabled = manager.is_enabled().map_err(|error| error.to_string())?;
    if enabled {
        manager.disable().map_err(|error| error.to_string())
    } else {
        manager.enable().map_err(|error| error.to_string())
    }?;
    let actual = manager.is_enabled().map_err(|error| error.to_string())?;
    sync_autostart(app, actual);
    Ok(())
}
