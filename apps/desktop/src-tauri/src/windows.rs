use std::thread;
use std::time::Duration;

use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, Monitor, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::state::{AppState, RuntimeState};

pub const WORKSHOP_LABEL: &str = "workshop";
pub const PET_LABEL: &str = "pet-overlay";
const PET_BASE_SIZE: f64 = 320.0;
const SAFE_MARGIN_LOGICAL: f64 = 24.0;

pub fn show_workshop(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(WORKSHOP_LABEL)
        .ok_or_else(|| "宠物工坊窗口不存在".to_owned())?;
    window.unminimize().map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

pub fn pet_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window(PET_LABEL)
        .ok_or_else(|| "桌宠窗口不存在".to_owned())
}

pub fn initialize_pet_window(app: &AppHandle) -> Result<RuntimeState, String> {
    let window = pet_window(app)?;
    apply_native_pet_styles(&window)?;
    restore_pet_window(app, &window)
}

pub fn recreate_pet_window(app: &AppHandle) -> Result<(), String> {
    if app.get_webview_window(PET_LABEL).is_some() {
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(
        app,
        PET_LABEL,
        WebviewUrl::App("index.html?window=pet-overlay".into()),
    )
    .title("Epet 桌宠")
    .inner_size(PET_BASE_SIZE, PET_BASE_SIZE)
    .transparent(true)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .shadow(false)
    .focused(false)
    .build()
    .map_err(|error| error.to_string())?;

    apply_native_pet_styles(&window)?;
    restore_pet_window(app, &window)?;
    Ok(())
}

pub fn restore_pet_window(app: &AppHandle, window: &WebviewWindow) -> Result<RuntimeState, String> {
    let state = app.state::<AppState>();
    let snapshot = state.snapshot().map_err(|error| error.to_string())?;
    apply_pet_size(window, snapshot.scale)?;

    let monitors = app
        .available_monitors()
        .map_err(|error| error.to_string())?;
    let monitor = select_monitor(app, &monitors, &snapshot)?;
    let (left, top, right, bottom) = monitor_bounds(&monitor);
    let size = window.outer_size().map_err(|error| error.to_string())?;
    let scale_factor = monitor.scale_factor();
    let margin = (SAFE_MARGIN_LOGICAL * scale_factor).round() as i32;

    let default_x = right - size.width as i32 - margin;
    let default_y = bottom - size.height as i32 - margin;
    let (requested_x, requested_y) =
        position_from_saved_anchor(&snapshot, &monitor, size).unwrap_or((default_x, default_y));
    let (x, y) = clamp_position(
        requested_x,
        requested_y,
        size.width,
        size.height,
        (left, top, right, bottom),
    );

    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|error| error.to_string())?;
    window
        .set_ignore_cursor_events(snapshot.click_through)
        .map_err(|error| error.to_string())?;

    if snapshot.visible {
        window.show().map_err(|error| error.to_string())?;
    } else {
        window.hide().map_err(|error| error.to_string())?;
    }

    let geometry = geometry_snapshot(&monitor, PhysicalPosition::new(x, y), size, snapshot.scale);
    let next = state
        .update(|runtime| {
            apply_geometry(runtime, &geometry);
        })
        .map_err(|error| error.to_string())?;
    emit_runtime_state(app, &next);
    Ok(next)
}

pub fn reset_pet_position(app: &AppHandle) -> Result<RuntimeState, String> {
    let state = app.state::<AppState>();
    state
        .update(|runtime| {
            runtime.monitor_id = None;
            runtime.x = None;
            runtime.y = None;
            runtime.work_area_width = None;
            runtime.work_area_height = None;
            runtime.dpi_scale = None;
            runtime.foot_anchor_x = None;
            runtime.foot_anchor_y = None;
        })
        .map_err(|error| error.to_string())?;
    let window = pet_window(app)?;
    restore_pet_window(app, &window)
}

pub fn resize_pet_around_foot(app: &AppHandle, scale: f64) -> Result<RuntimeState, String> {
    let window = pet_window(app)?;
    let old_position = window.outer_position().map_err(|error| error.to_string())?;
    let old_size = window.outer_size().map_err(|error| error.to_string())?;
    let foot_x = old_position.x + old_size.width as i32 / 2;
    let foot_y = old_position.y + old_size.height as i32;

    apply_pet_size(&window, scale)?;
    let new_size = window.outer_size().map_err(|error| error.to_string())?;
    let requested_x = foot_x - new_size.width as i32 / 2;
    let requested_y = foot_y - new_size.height as i32;
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or_else(|| app.primary_monitor().ok().flatten())
        .ok_or_else(|| "未检测到可用显示器".to_owned())?;
    let bounds = monitor_bounds(&monitor);
    let (x, y) = clamp_position(
        requested_x,
        requested_y,
        new_size.width,
        new_size.height,
        bounds,
    );
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|error| error.to_string())?;

    let geometry = geometry_snapshot(&monitor, PhysicalPosition::new(x, y), new_size, scale);
    let next = app
        .state::<AppState>()
        .update(|runtime| {
            runtime.scale = scale;
            apply_geometry(runtime, &geometry);
        })
        .map_err(|error| error.to_string())?;
    emit_runtime_state(app, &next);
    Ok(next)
}

pub fn schedule_position_persist(window: WebviewWindow) {
    let app = window.app_handle().clone();
    let generation = app.state::<AppState>().next_position_generation();

    thread::spawn(move || {
        thread::sleep(Duration::from_millis(250));
        let state = app.state::<AppState>();
        if state.position_generation() != generation || state.is_exiting() {
            return;
        }

        let Some(window) = app.get_webview_window(PET_LABEL) else {
            return;
        };
        let Ok(position) = window.outer_position() else {
            return;
        };
        let Ok(size) = window.outer_size() else {
            return;
        };
        let Ok(Some(monitor)) = window.current_monitor() else {
            return;
        };
        let bounds = monitor_bounds(&monitor);
        let (x, y) = clamp_position(position.x, position.y, size.width, size.height, bounds);
        if (x, y) != (position.x, position.y) {
            let _ = window.set_position(PhysicalPosition::new(x, y));
        }
        let scale = state.snapshot().map(|state| state.scale).unwrap_or(1.0);
        let geometry = geometry_snapshot(&monitor, PhysicalPosition::new(x, y), size, scale);
        let Ok(snapshot) = state.update(|runtime| {
            apply_geometry(runtime, &geometry);
            if runtime.last_behavior_state == "drag" {
                runtime.last_behavior_state = "idle".to_owned();
            }
        }) else {
            return;
        };

        emit_runtime_state(&app, &snapshot);
        crate::tray::sync_checks(&app, &snapshot);
    });
}

pub fn emit_runtime_state(app: &AppHandle, state: &RuntimeState) {
    let _ = app.emit("runtime-state-changed", state.clone());
}

fn apply_pet_size(window: &WebviewWindow, scale: f64) -> Result<(), String> {
    let side = PET_BASE_SIZE * scale;
    window
        .set_size(LogicalSize::new(side, side))
        .map_err(|error| error.to_string())
}

struct GeometrySnapshot {
    monitor_id: String,
    x: f64,
    y: f64,
    work_area_width: f64,
    work_area_height: f64,
    dpi_scale: f64,
    pet_logical_size: f64,
    foot_anchor_x: f64,
    foot_anchor_y: f64,
}

fn geometry_snapshot(
    monitor: &Monitor,
    position: PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
    scale: f64,
) -> GeometrySnapshot {
    let (left, top, right, bottom) = monitor_bounds(monitor);
    let width = f64::from((right - left).max(1));
    let height = f64::from((bottom - top).max(1));
    let foot_x = position.x + size.width as i32 / 2;
    let foot_y = position.y + size.height as i32;
    GeometrySnapshot {
        monitor_id: monitor_id(monitor),
        x: f64::from(position.x),
        y: f64::from(position.y),
        work_area_width: width,
        work_area_height: height,
        dpi_scale: monitor.scale_factor(),
        pet_logical_size: PET_BASE_SIZE * scale,
        foot_anchor_x: (f64::from(foot_x - left) / width).clamp(0.0, 1.0),
        foot_anchor_y: (f64::from(foot_y - top) / height).clamp(0.0, 1.0),
    }
}

fn apply_geometry(runtime: &mut RuntimeState, geometry: &GeometrySnapshot) {
    runtime.monitor_id = Some(geometry.monitor_id.clone());
    runtime.x = Some(geometry.x);
    runtime.y = Some(geometry.y);
    runtime.work_area_width = Some(geometry.work_area_width);
    runtime.work_area_height = Some(geometry.work_area_height);
    runtime.dpi_scale = Some(geometry.dpi_scale);
    runtime.pet_logical_size = geometry.pet_logical_size;
    runtime.foot_anchor_x = Some(geometry.foot_anchor_x);
    runtime.foot_anchor_y = Some(geometry.foot_anchor_y);
}

fn position_from_saved_anchor(
    state: &RuntimeState,
    monitor: &Monitor,
    size: tauri::PhysicalSize<u32>,
) -> Option<(i32, i32)> {
    let anchor_x = state.foot_anchor_x?;
    let anchor_y = state.foot_anchor_y?;
    let (left, top, right, bottom) = monitor_bounds(monitor);
    let foot_x = f64::from(left) + anchor_x.clamp(0.0, 1.0) * f64::from(right - left);
    let foot_y = f64::from(top) + anchor_y.clamp(0.0, 1.0) * f64::from(bottom - top);
    Some((
        foot_x.round() as i32 - size.width as i32 / 2,
        foot_y.round() as i32 - size.height as i32,
    ))
}

fn select_monitor(
    app: &AppHandle,
    monitors: &[Monitor],
    state: &RuntimeState,
) -> Result<Monitor, String> {
    if let Some(requested) = state.monitor_id.as_deref()
        && let Some(monitor) = monitors
            .iter()
            .find(|monitor| monitor_id(monitor) == requested)
    {
        return Ok(monitor.clone());
    }

    if let (Some(saved_width), Some(saved_height)) = (state.work_area_width, state.work_area_height)
        && let Some(monitor) = monitors.iter().min_by(|left, right| {
            let score = |monitor: &Monitor| {
                let (x1, y1, x2, y2) = monitor_bounds(monitor);
                let width = f64::from(x2 - x1);
                let height = f64::from(y2 - y1);
                (width - saved_width).abs() + (height - saved_height).abs()
            };
            score(left).total_cmp(&score(right))
        })
    {
        return Ok(monitor.clone());
    }

    app.primary_monitor()
        .map_err(|error| error.to_string())?
        .or_else(|| monitors.first().cloned())
        .ok_or_else(|| "未检测到可用显示器".to_owned())
}

fn monitor_id(monitor: &Monitor) -> String {
    monitor.name().map(ToOwned::to_owned).unwrap_or_else(|| {
        let position = monitor.position();
        let size = monitor.size();
        format!(
            "{}:{}:{}x{}",
            position.x, position.y, size.width, size.height
        )
    })
}

#[cfg(not(windows))]
fn monitor_bounds(monitor: &Monitor) -> (i32, i32, i32, i32) {
    let position = monitor.position();
    let size = monitor.size();
    (
        position.x,
        position.y,
        position.x + size.width as i32,
        position.y + size.height as i32,
    )
}

#[cfg(windows)]
fn monitor_bounds(monitor: &Monitor) -> (i32, i32, i32, i32) {
    use std::mem::size_of;
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
    };

    let position = monitor.position();
    let point = POINT {
        x: position.x + 1,
        y: position.y + 1,
    };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };

    unsafe {
        let handle = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        if GetMonitorInfoW(handle, &mut info).as_bool() {
            return (
                info.rcWork.left,
                info.rcWork.top,
                info.rcWork.right,
                info.rcWork.bottom,
            );
        }
    }

    let size = monitor.size();
    (
        position.x,
        position.y,
        position.x + size.width as i32,
        position.y + size.height as i32,
    )
}

fn clamp_position(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    bounds: (i32, i32, i32, i32),
) -> (i32, i32) {
    let (left, top, right, bottom) = bounds;
    let max_x = (right - width as i32).max(left);
    let max_y = (bottom - height as i32).max(top);
    (x.clamp(left, max_x), y.clamp(top, max_y))
}

#[cfg(not(windows))]
fn apply_native_pet_styles(_window: &WebviewWindow) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
fn apply_native_pet_styles(window: &WebviewWindow) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, SetWindowLongPtrW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };

    let raw = window.hwnd().map_err(|error| error.to_string())?;
    let hwnd = HWND(raw.0 as *mut _);
    unsafe {
        let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let desired = current | (WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0) as isize;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, desired);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::clamp_position;

    #[test]
    fn clamps_window_inside_negative_origin_monitor() {
        assert_eq!(
            clamp_position(-5000, 4000, 320, 320, (-1920, 0, 0, 1080)),
            (-1920, 760)
        );
    }

    #[test]
    fn clamps_oversized_window_to_monitor_origin() {
        assert_eq!(
            clamp_position(100, 100, 1200, 900, (0, 0, 800, 600)),
            (0, 0)
        );
    }
}
