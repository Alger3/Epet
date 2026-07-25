use tauri::{AppHandle, Manager, State, WebviewWindow};
use tauri_plugin_autostart::ManagerExt;

use crate::state::{AppState, MAX_PET_SCALE, MIN_PET_SCALE, RuntimeState};
use crate::{tray, windows};

#[tauri::command]
pub fn get_runtime_state(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<RuntimeState, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL, windows::PET_LABEL])?;
    state.snapshot().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_active_character(
    app: AppHandle,
    window: WebviewWindow,
    character_id: String,
) -> Result<RuntimeState, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    if character_id.len() > 80
        || !character_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("角色 ID 格式无效".to_owned());
    }

    let state = app.state::<AppState>();
    if !state
        .character_exists(&character_id)
        .map_err(|error| error.to_string())?
    {
        return Err("角色不存在或尚未安装".to_owned());
    }

    if app.get_webview_window(windows::PET_LABEL).is_none() {
        windows::recreate_pet_window(&app)?;
    }
    windows::pet_window(&app)?
        .show()
        .map_err(|error| error.to_string())?;

    let snapshot = state
        .update(|runtime| {
            runtime.active_character_id = character_id;
            runtime.visible = true;
        })
        .map_err(|error| error.to_string())?;
    windows::emit_runtime_state(&app, &snapshot);
    tray::sync_checks(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn set_pet_visible(
    app: AppHandle,
    window: WebviewWindow,
    visible: bool,
) -> Result<RuntimeState, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    set_pet_visible_internal(&app, visible)
}

#[tauri::command]
pub fn set_paused(
    app: AppHandle,
    window: WebviewWindow,
    paused: bool,
) -> Result<RuntimeState, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    set_paused_internal(&app, paused)
}

#[tauri::command]
pub fn set_click_through(
    app: AppHandle,
    window: WebviewWindow,
    click_through: bool,
) -> Result<RuntimeState, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    set_click_through_internal(&app, click_through)
}

#[tauri::command]
pub fn set_always_on_top(
    app: AppHandle,
    window: WebviewWindow,
    always_on_top: bool,
) -> Result<RuntimeState, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    set_always_on_top_internal(&app, always_on_top)
}

#[tauri::command]
pub fn reset_pet_position(app: AppHandle, window: WebviewWindow) -> Result<RuntimeState, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    let snapshot = windows::reset_pet_position(&app)?;
    tray::sync_checks(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn adjust_pet_scale(
    app: AppHandle,
    window: WebviewWindow,
    delta: f64,
) -> Result<RuntimeState, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL, windows::PET_LABEL])?;
    if !delta.is_finite() || delta.abs() > 0.25 {
        return Err("缩放增量超出允许范围".to_owned());
    }
    let current = app
        .state::<AppState>()
        .snapshot()
        .map_err(|error| error.to_string())?;
    let scale = (current.scale + delta).clamp(MIN_PET_SCALE, MAX_PET_SCALE);
    let snapshot = windows::resize_pet_around_foot(&app, scale)?;
    tray::sync_checks(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn begin_pet_drag(app: AppHandle, window: WebviewWindow) -> Result<RuntimeState, String> {
    ensure_caller(&window, &[windows::PET_LABEL])?;
    let state = app.state::<AppState>();
    let dragging = state
        .update(|runtime| runtime.last_behavior_state = "drag".to_owned())
        .map_err(|error| error.to_string())?;
    windows::emit_runtime_state(&app, &dragging);

    if let Err(error) = window.start_dragging() {
        let restored = state
            .update(|runtime| runtime.last_behavior_state = "idle".to_owned())
            .map_err(|state_error| state_error.to_string())?;
        windows::emit_runtime_state(&app, &restored);
        return Err(error.to_string());
    }

    windows::schedule_position_persist(window);
    state.snapshot().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn show_workshop(app: AppHandle, window: WebviewWindow) -> Result<RuntimeState, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL, windows::PET_LABEL])?;
    windows::show_workshop(&app)?;
    app.state::<AppState>()
        .snapshot()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_autostart_enabled(window: WebviewWindow) -> Result<bool, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    window
        .app_handle()
        .autolaunch()
        .is_enabled()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_autostart_enabled(
    app: AppHandle,
    window: WebviewWindow,
    enabled: bool,
) -> Result<bool, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|error| error.to_string())?;
    } else {
        manager.disable().map_err(|error| error.to_string())?;
    }
    let actual = manager.is_enabled().map_err(|error| error.to_string())?;
    tray::sync_autostart(&app, actual);
    Ok(actual)
}

pub fn set_pet_visible_internal(app: &AppHandle, visible: bool) -> Result<RuntimeState, String> {
    if visible && app.get_webview_window(windows::PET_LABEL).is_none() {
        windows::recreate_pet_window(app)?;
    }

    let pet = windows::pet_window(app)?;
    if visible {
        pet.show().map_err(|error| error.to_string())?;
    } else {
        pet.hide().map_err(|error| error.to_string())?;
    }

    let snapshot = app
        .state::<AppState>()
        .update(|runtime| runtime.visible = visible)
        .map_err(|error| error.to_string())?;
    windows::emit_runtime_state(app, &snapshot);
    tray::sync_checks(app, &snapshot);
    Ok(snapshot)
}

pub fn set_paused_internal(app: &AppHandle, paused: bool) -> Result<RuntimeState, String> {
    let snapshot = app
        .state::<AppState>()
        .update(|runtime| {
            runtime.paused = paused;
            runtime.last_behavior_state = if paused { "paused" } else { "idle" }.to_owned();
        })
        .map_err(|error| error.to_string())?;
    windows::emit_runtime_state(app, &snapshot);
    tray::sync_checks(app, &snapshot);
    Ok(snapshot)
}

pub fn set_click_through_internal(
    app: &AppHandle,
    click_through: bool,
) -> Result<RuntimeState, String> {
    let pet = windows::pet_window(app)?;
    pet.set_ignore_cursor_events(click_through)
        .map_err(|error| error.to_string())?;

    let snapshot = app
        .state::<AppState>()
        .update(|runtime| runtime.click_through = click_through)
        .map_err(|error| error.to_string())?;
    windows::emit_runtime_state(app, &snapshot);
    tray::sync_checks(app, &snapshot);
    Ok(snapshot)
}

pub fn set_always_on_top_internal(
    app: &AppHandle,
    always_on_top: bool,
) -> Result<RuntimeState, String> {
    let pet = windows::pet_window(app)?;
    pet.set_always_on_top(always_on_top)
        .map_err(|error| error.to_string())?;

    let snapshot = app
        .state::<AppState>()
        .update(|runtime| runtime.always_on_top = always_on_top)
        .map_err(|error| error.to_string())?;
    windows::emit_runtime_state(app, &snapshot);
    tray::sync_checks(app, &snapshot);
    Ok(snapshot)
}

pub fn reset_pet_position_internal(app: &AppHandle) -> Result<RuntimeState, String> {
    let snapshot = windows::reset_pet_position(app)?;
    tray::sync_checks(app, &snapshot);
    Ok(snapshot)
}

fn ensure_caller(window: &WebviewWindow, allowed: &[&str]) -> Result<(), String> {
    if allowed.contains(&window.label()) {
        Ok(())
    } else {
        Err(format!("窗口 {} 无权调用此命令", window.label()))
    }
}
