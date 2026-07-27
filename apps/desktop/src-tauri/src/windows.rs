use std::thread;
use std::time::{Duration, Instant};

use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, Monitor, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::{
    behavior::{self, BehaviorEvent},
    character_store,
    state::{AppState, RuntimeState},
};

pub const WORKSHOP_LABEL: &str = "workshop";
pub const PET_LABEL: &str = "pet-overlay";
const PET_BASE_SIZE: f64 = 320.0;
const SAFE_MARGIN_LOGICAL: f64 = 24.0;
const AUTONOMOUS_MOVE_TICK: Duration = Duration::from_millis(100);
const AUTONOMOUS_MOVE_PERSIST_INTERVAL: Duration = Duration::from_secs(1);
const AUTONOMOUS_MOVE_SPEED_LOGICAL: f64 = 48.0;
const AUTONOMOUS_IDLE_DURATION: Duration = Duration::from_secs(2);
const AUTONOMOUS_WALK_DURATION: Duration = Duration::from_secs(8);
const INACTIVITY_POLL_INTERVAL: Duration = Duration::from_secs(1);
const MONITOR_TOPOLOGY_POLL_INTERVAL: Duration = Duration::from_secs(2);

pub fn show_workshop(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(WORKSHOP_LABEL)
        .ok_or_else(|| "角色工坊窗口不存在".to_owned())?;
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
    .title("Epet 桌面角色")
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
    window
        .set_always_on_top(snapshot.always_on_top)
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
        let _ = persist_pet_geometry(&app, &window, true);
    });
}

pub fn start_autonomous_movement(app: &AppHandle) {
    let app = app.clone();
    thread::spawn(move || {
        let mut direction = -1;
        let mut phase_started = Instant::now();
        let mut last_tick = Instant::now();
        let mut last_persist = Instant::now();

        loop {
            thread::sleep(AUTONOMOUS_MOVE_TICK);
            let state = app.state::<AppState>();
            if state.is_exiting() {
                return;
            }

            let Ok(snapshot) = state.snapshot() else {
                continue;
            };
            let can_move = can_autonomous_move(&snapshot);

            if !can_move {
                if snapshot.last_behavior_state == "walk"
                    && let Ok(idle) = state.update(|runtime| {
                        runtime.last_behavior_state = behavior::transition(
                            &runtime.last_behavior_state,
                            BehaviorEvent::StopWalk,
                        )
                        .to_owned();
                    })
                {
                    emit_runtime_state(&app, &idle);
                }
                phase_started = Instant::now();
                last_tick = Instant::now();
                continue;
            }

            let Some(window) = app.get_webview_window(PET_LABEL) else {
                continue;
            };

            if snapshot.last_behavior_state == "idle" {
                if phase_started.elapsed() < AUTONOMOUS_IDLE_DURATION {
                    last_tick = Instant::now();
                    continue;
                }
                if let Ok(next) = state.update(|runtime| {
                    runtime.last_behavior_state = behavior::transition(
                        &runtime.last_behavior_state,
                        BehaviorEvent::StartWalk,
                    )
                    .to_owned();
                }) {
                    emit_runtime_state(&app, &next);
                }
                phase_started = Instant::now();
                last_tick = Instant::now();
                continue;
            }

            if phase_started.elapsed() >= AUTONOMOUS_WALK_DURATION {
                if let Ok(idle) = state.update(|runtime| {
                    runtime.last_behavior_state =
                        behavior::transition(&runtime.last_behavior_state, BehaviorEvent::StopWalk)
                            .to_owned();
                }) {
                    emit_runtime_state(&app, &idle);
                }
                phase_started = Instant::now();
                last_tick = Instant::now();
                continue;
            }

            let Ok(position) = window.outer_position() else {
                continue;
            };
            let Ok(size) = window.outer_size() else {
                continue;
            };
            let Ok(Some(monitor)) = window.current_monitor() else {
                continue;
            };

            let now = Instant::now();
            let elapsed = now.duration_since(last_tick).as_secs_f64();
            last_tick = now;
            let step = autonomous_movement_step(elapsed, monitor.scale_factor());
            let (x, y, next_direction) = advance_horizontal(
                position.x,
                size.width,
                size.height,
                monitor_bounds(&monitor),
                direction,
                step,
            );
            direction = next_direction;

            if window.set_position(PhysicalPosition::new(x, y)).is_err() {
                continue;
            }
            let moved = ((x - position.x).abs() + (y - position.y).abs()) as f64;
            if moved > 0.0 {
                let _ = app.emit("pet-movement-distance", moved);
            }

            if last_persist.elapsed() >= AUTONOMOUS_MOVE_PERSIST_INTERVAL {
                let _ = persist_pet_geometry(&app, &window, false);
                last_persist = Instant::now();
            }
        }
    });
}

pub fn start_inactivity_sleep_monitor(app: &AppHandle) {
    let app = app.clone();
    thread::spawn(move || {
        loop {
            thread::sleep(INACTIVITY_POLL_INTERVAL);
            let state = app.state::<AppState>();
            if state.is_exiting() {
                return;
            }

            let (Ok(snapshot), Some(idle_for)) = (state.snapshot(), system_idle_duration()) else {
                continue;
            };
            if !should_enter_sleep(&snapshot, idle_for) {
                continue;
            }

            let _ = state.clear_wake_clicks();
            if let Ok(sleeping) = state.update(|runtime| {
                runtime.last_behavior_state =
                    behavior::transition(&runtime.last_behavior_state, BehaviorEvent::FallAsleep)
                        .to_owned();
            }) {
                emit_runtime_state(&app, &sleeping);
            }
        }
    });
}

pub fn start_monitor_topology_watcher(app: &AppHandle) {
    let app = app.clone();
    thread::spawn(move || {
        let mut previous = monitor_topology_signature(&app).ok();
        loop {
            thread::sleep(MONITOR_TOPOLOGY_POLL_INTERVAL);
            let state = app.state::<AppState>();
            if state.is_exiting() {
                return;
            }

            let Ok(current) = monitor_topology_signature(&app) else {
                continue;
            };
            let refresh_app = app.clone();
            let _ = app.run_on_main_thread(move || {
                if let Some(window) = refresh_app.get_webview_window(PET_LABEL) {
                    let _ = refresh_no_activate_descendants(&window);
                }
            });
            let changed = previous.as_ref().is_some_and(|value| value != &current);
            previous = Some(current);
            if !changed {
                continue;
            }

            let callback_app = app.clone();
            let _ = app.run_on_main_thread(move || {
                let Some(window) = callback_app.get_webview_window(PET_LABEL) else {
                    return;
                };
                if let Err(error) = restore_pet_window(&callback_app, &window)
                    && let Some(state) = callback_app.try_state::<AppState>()
                    && let Ok(snapshot) =
                        state.set_diagnostic(format!("显示器拓扑恢复失败：{error}"))
                {
                    emit_runtime_state(&callback_app, &snapshot);
                }
            });
        }
    });
}

fn monitor_topology_signature(app: &AppHandle) -> Result<Vec<String>, String> {
    let mut signature = app
        .available_monitors()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            format!(
                "{}:{}:{}:{}:{}:{:.4}",
                monitor.name().map(String::as_str).unwrap_or(""),
                position.x,
                position.y,
                size.width,
                size.height,
                monitor.scale_factor()
            )
        })
        .collect::<Vec<_>>();
    signature.sort();
    Ok(signature)
}

pub fn schedule_behavior_timeout(
    app: AppHandle,
    expected_state: &'static str,
    event: BehaviorEvent,
    delay: Duration,
) {
    thread::spawn(move || {
        thread::sleep(delay);
        let state = app.state::<AppState>();
        let Ok(snapshot) = state.snapshot() else {
            return;
        };
        if snapshot.last_behavior_state != expected_state {
            return;
        }
        if let Ok(next) = state.update(|runtime| {
            runtime.last_behavior_state =
                behavior::transition(&runtime.last_behavior_state, event).to_owned();
        }) {
            emit_runtime_state(&app, &next);
        }
    });
}

pub fn emit_runtime_state(app: &AppHandle, state: &RuntimeState) {
    let _ = app.emit("runtime-state-changed", state.clone());
}

fn persist_pet_geometry(
    app: &AppHandle,
    window: &WebviewWindow,
    settle_drag: bool,
) -> Result<RuntimeState, String> {
    if should_defer_position_settle(settle_drag, primary_button_down()) {
        schedule_position_persist(window.clone());
        return app
            .state::<AppState>()
            .snapshot()
            .map_err(|error| error.to_string());
    }

    let (position, size) = pet_window_geometry(window)?;
    let state = app.state::<AppState>();
    let scale = state.snapshot().map_err(|error| error.to_string())?.scale;
    let monitor = window_monitor_snapshot(window, position, size)?;
    let bounds = monitor.bounds;
    let (x, y) = clamp_position(position.x, position.y, size.width, size.height, bounds);
    if (x, y) != (position.x, position.y) {
        set_pet_physical_position(window, PhysicalPosition::new(x, y))?;
    }

    let geometry = geometry_snapshot_for_bounds(
        monitor.id,
        bounds,
        monitor.dpi_scale,
        PhysicalPosition::new(x, y),
        size,
        scale,
    );
    let snapshot = state
        .update(|runtime| {
            apply_geometry(runtime, &geometry);
            if settle_drag && runtime.last_behavior_state == "drag" {
                runtime.last_behavior_state =
                    behavior::transition(&runtime.last_behavior_state, BehaviorEvent::DragEnd)
                        .to_owned();
            }
        })
        .map_err(|error| error.to_string())?;

    emit_runtime_state(app, &snapshot);
    if settle_drag && snapshot.last_behavior_state == "drag" {
        schedule_position_persist(window.clone());
    } else if snapshot.last_behavior_state == "drop" {
        restore_previous_foreground();
        schedule_behavior_timeout(
            app.clone(),
            "drop",
            BehaviorEvent::AnimationFinished,
            Duration::from_millis(220),
        );
    }
    crate::tray::sync_checks(app, &snapshot);
    Ok(snapshot)
}

#[cfg(windows)]
fn pet_window_geometry(
    window: &WebviewWindow,
) -> Result<(PhysicalPosition<i32>, tauri::PhysicalSize<u32>), String> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

    let raw = window.hwnd().map_err(|error| error.to_string())?;
    let hwnd = HWND(raw.0 as *mut _);
    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect) }.map_err(|error| error.to_string())?;
    Ok((
        PhysicalPosition::new(rect.left, rect.top),
        tauri::PhysicalSize::new(
            (rect.right - rect.left).max(1) as u32,
            (rect.bottom - rect.top).max(1) as u32,
        ),
    ))
}

#[cfg(not(windows))]
fn pet_window_geometry(
    window: &WebviewWindow,
) -> Result<(PhysicalPosition<i32>, tauri::PhysicalSize<u32>), String> {
    Ok((
        window.outer_position().map_err(|error| error.to_string())?,
        window.outer_size().map_err(|error| error.to_string())?,
    ))
}

#[cfg(windows)]
fn set_pet_physical_position(
    window: &WebviewWindow,
    position: PhysicalPosition<i32>,
) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
    };

    let raw = window.hwnd().map_err(|error| error.to_string())?;
    let hwnd = HWND(raw.0 as *mut _);
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            position.x,
            position.y,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER,
        )
    }
    .map_err(|error| error.to_string())
}

#[cfg(not(windows))]
fn set_pet_physical_position(
    window: &WebviewWindow,
    position: PhysicalPosition<i32>,
) -> Result<(), String> {
    window
        .set_position(position)
        .map_err(|error| error.to_string())
}

fn should_defer_position_settle(settle_drag: bool, primary_button_down: bool) -> bool {
    settle_drag && primary_button_down
}

#[cfg(windows)]
fn primary_button_down() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};

    unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) < 0 }
}

#[cfg(not(windows))]
fn primary_button_down() -> bool {
    false
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

struct WindowMonitorSnapshot {
    id: String,
    bounds: (i32, i32, i32, i32),
    dpi_scale: f64,
}

#[cfg(windows)]
fn window_monitor_snapshot(
    _window: &WebviewWindow,
    position: PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
) -> Result<WindowMonitorSnapshot, String> {
    use std::mem::size_of;
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MONITORINFOEXW, MonitorFromPoint,
    };
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

    let center = POINT {
        x: position.x + size.width as i32 / 2,
        y: position.y + size.height as i32 / 2,
    };
    let monitor = unsafe { MonitorFromPoint(center, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFOEXW {
        monitorInfo: MONITORINFO {
            cbSize: size_of::<MONITORINFOEXW>() as u32,
            ..Default::default()
        },
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, (&raw mut info).cast::<MONITORINFO>()) }.as_bool() {
        return Err("无法读取桌宠所在显示器的原生工作区".to_owned());
    }

    let device_end = info
        .szDevice
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(info.szDevice.len());
    let mut dpi_x = 96_u32;
    let mut dpi_y = 96_u32;
    let _ = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    Ok(WindowMonitorSnapshot {
        id: String::from_utf16_lossy(&info.szDevice[..device_end]),
        bounds: (
            info.monitorInfo.rcWork.left,
            info.monitorInfo.rcWork.top,
            info.monitorInfo.rcWork.right,
            info.monitorInfo.rcWork.bottom,
        ),
        dpi_scale: if dpi_x == 0 {
            1.0
        } else {
            f64::from(dpi_x) / 96.0
        },
    })
}

#[cfg(not(windows))]
fn window_monitor_snapshot(
    window: &WebviewWindow,
    _position: PhysicalPosition<i32>,
    _size: tauri::PhysicalSize<u32>,
) -> Result<WindowMonitorSnapshot, String> {
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "桌宠窗口当前不在可用显示器中".to_owned())?;
    Ok(WindowMonitorSnapshot {
        id: monitor_id(&monitor),
        bounds: monitor_bounds(&monitor),
        dpi_scale: monitor.scale_factor(),
    })
}

fn geometry_snapshot(
    monitor: &Monitor,
    position: PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
    scale: f64,
) -> GeometrySnapshot {
    geometry_snapshot_for_bounds(
        monitor_id(monitor),
        monitor_bounds(monitor),
        monitor.scale_factor(),
        position,
        size,
        scale,
    )
}

fn geometry_snapshot_for_bounds(
    monitor_id: String,
    bounds: (i32, i32, i32, i32),
    dpi_scale: f64,
    position: PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
    scale: f64,
) -> GeometrySnapshot {
    let (left, top, right, bottom) = bounds;
    let width = f64::from((right - left).max(1));
    let height = f64::from((bottom - top).max(1));
    let foot_x = position.x + size.width as i32 / 2;
    let foot_y = position.y + size.height as i32;
    GeometrySnapshot {
        monitor_id,
        x: f64::from(position.x),
        y: f64::from(position.y),
        work_area_width: width,
        work_area_height: height,
        dpi_scale,
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

fn advance_horizontal(
    x: i32,
    width: u32,
    height: u32,
    bounds: (i32, i32, i32, i32),
    direction: i32,
    step: i32,
) -> (i32, i32, i32) {
    let (left, _top, right, bottom) = bounds;
    let max_x = (right - width as i32).max(left);
    let y = (bottom - height as i32).max(bounds.1);
    let requested = if direction < 0 {
        x.saturating_sub(step.max(1))
    } else {
        x.saturating_add(step.max(1))
    };

    if requested <= left {
        (left, y, 1)
    } else if requested >= max_x {
        (max_x, y, -1)
    } else {
        (requested, y, if direction < 0 { -1 } else { 1 })
    }
}

fn autonomous_movement_step(elapsed_seconds: f64, dpi_scale: f64) -> i32 {
    (AUTONOMOUS_MOVE_SPEED_LOGICAL * dpi_scale * elapsed_seconds)
        .round()
        .max(1.0) as i32
}

fn can_autonomous_move(state: &RuntimeState) -> bool {
    state.autonomous_movement
        && state.visible
        && !state.paused
        && matches!(state.last_behavior_state.as_str(), "idle" | "walk")
}

fn should_enter_sleep(state: &RuntimeState, idle_for: Duration) -> bool {
    state.sleep_after_minutes > 0
        && state.visible
        && !state.paused
        && matches!(state.last_behavior_state.as_str(), "idle" | "walk")
        && idle_for >= Duration::from_secs(u64::from(state.sleep_after_minutes) * 60)
}

#[cfg(windows)]
fn system_idle_duration() -> Option<Duration> {
    use std::mem::size_of;
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    let mut info = LASTINPUTINFO {
        cbSize: size_of::<LASTINPUTINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetLastInputInfo(&mut info) }.as_bool() {
        return None;
    }

    let elapsed_ms = unsafe { GetTickCount() }.wrapping_sub(info.dwTime);
    Some(Duration::from_millis(u64::from(elapsed_ms)))
}

#[cfg(not(windows))]
fn system_idle_duration() -> Option<Duration> {
    None
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
    install_native_hit_test(window, hwnd)?;
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum HitboxShape {
    Rectangle,
    Ellipse,
}

#[derive(Clone, Copy, Debug)]
struct HitboxRegion {
    shape: HitboxShape,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl HitboxRegion {
    const fn rectangle(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            shape: HitboxShape::Rectangle,
            x,
            y,
            width,
            height,
        }
    }

    const fn ellipse(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            shape: HitboxShape::Ellipse,
            x,
            y,
            width,
            height,
        }
    }

    fn contains(self, normalized_x: f64, normalized_y: f64) -> bool {
        if self.width <= 0.0 || self.height <= 0.0 {
            return false;
        }

        match self.shape {
            HitboxShape::Rectangle => {
                normalized_x >= self.x
                    && normalized_x <= self.x + self.width
                    && normalized_y >= self.y
                    && normalized_y <= self.y + self.height
            }
            HitboxShape::Ellipse => {
                let radius_x = self.width / 2.0;
                let radius_y = self.height / 2.0;
                let center_x = self.x + radius_x;
                let center_y = self.y + radius_y;
                let dx = (normalized_x - center_x) / radius_x;
                let dy = (normalized_y - center_y) / radius_y;
                dx * dx + dy * dy <= 1.0
            }
        }
    }
}

fn builtin_hitboxes(character_id: &str) -> Vec<HitboxRegion> {
    match character_id {
        "builtin-forest-guide" => vec![
            HitboxRegion::ellipse(0.28, 0.04, 0.44, 0.36),
            HitboxRegion::rectangle(0.3, 0.28, 0.4, 0.67),
        ],
        _ => vec![
            HitboxRegion::ellipse(0.1, 0.08, 0.55, 0.78),
            HitboxRegion::ellipse(0.42, 0.12, 0.5, 0.58),
        ],
    }
}

fn hitboxes_contain(hitboxes: &[HitboxRegion], x: f64, y: f64) -> bool {
    hitboxes.iter().any(|hitbox| hitbox.contains(x, y))
}

#[cfg(windows)]
const PET_HIT_TEST_SUBCLASS_ID: usize = 0x4550_4554;
#[cfg(windows)]
const PET_NO_ACTIVATE_SUBCLASS_ID: usize = 0x4550_4E41;

#[cfg(windows)]
static PET_HITBOXES: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<isize, Vec<HitboxRegion>>>,
> = std::sync::OnceLock::new();
#[cfg(windows)]
static PREVIOUS_FOREGROUND_WINDOW: std::sync::atomic::AtomicIsize =
    std::sync::atomic::AtomicIsize::new(0);

#[cfg(windows)]
unsafe extern "system" fn pet_hit_test_proc(
    hwnd: windows::Win32::Foundation::HWND,
    message: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
    _subclass_id: usize,
    _reference_data: usize,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::{LRESULT, POINT};
    use windows::Win32::Graphics::Gdi::ScreenToClient;
    use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClientRect, HTTRANSPARENT, MA_NOACTIVATE, WA_INACTIVE, WM_ACTIVATE, WM_MOUSEACTIVATE,
        WM_NCDESTROY, WM_NCHITTEST,
    };

    if message == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE as isize);
    } else if message == WM_NCHITTEST {
        let mut client_rect = windows::Win32::Foundation::RECT::default();
        if unsafe { GetClientRect(hwnd, &mut client_rect) }.is_ok() {
            let raw = lparam.0 as u64;
            let mut point = POINT {
                x: (raw as u16 as i16) as i32,
                y: ((raw >> 16) as u16 as i16) as i32,
            };
            if unsafe { ScreenToClient(hwnd, &mut point) }.as_bool() {
                let width = (client_rect.right - client_rect.left).max(1) as f64;
                let height = (client_rect.bottom - client_rect.top).max(1) as f64;
                let normalized_x = f64::from(point.x - client_rect.left) / width;
                let normalized_y = f64::from(point.y - client_rect.top) / height;
                let contains = PET_HITBOXES
                    .get()
                    .and_then(|map| map.lock().ok())
                    .and_then(|map| map.get(&(hwnd.0 as isize)).cloned())
                    .is_none_or(|hitboxes| hitboxes_contain(&hitboxes, normalized_x, normalized_y));
                if !contains {
                    return LRESULT(HTTRANSPARENT as isize);
                }
            }
        }
    } else if message == WM_NCDESTROY {
        if let Some(map) = PET_HITBOXES.get()
            && let Ok(mut map) = map.lock()
        {
            map.remove(&(hwnd.0 as isize));
        }
        unsafe {
            let _ = RemoveWindowSubclass(hwnd, Some(pet_hit_test_proc), PET_HIT_TEST_SUBCLASS_ID);
        }
    }

    let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    if message == WM_ACTIVATE && (wparam.0 & 0xffff) as u32 != WA_INACTIVE && lparam.0 != 0 {
        PREVIOUS_FOREGROUND_WINDOW.store(lparam.0, std::sync::atomic::Ordering::Release);
    }
    result
}

#[cfg(windows)]
pub fn restore_previous_foreground() {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

    let previous = PREVIOUS_FOREGROUND_WINDOW.swap(0, std::sync::atomic::Ordering::AcqRel);
    if previous == 0 {
        return;
    }
    unsafe {
        let _ = SetForegroundWindow(HWND(previous as *mut _));
    }
}

#[cfg(not(windows))]
pub fn restore_previous_foreground() {}

#[cfg(windows)]
unsafe extern "system" fn pet_no_activate_proc(
    hwnd: windows::Win32::Foundation::HWND,
    message: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
    _subclass_id: usize,
    _reference_data: usize,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::LRESULT;
    use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{MA_NOACTIVATE, WM_MOUSEACTIVATE, WM_NCDESTROY};

    if message == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE as isize);
    }
    if message == WM_NCDESTROY {
        unsafe {
            let _ = RemoveWindowSubclass(
                hwnd,
                Some(pet_no_activate_proc),
                PET_NO_ACTIVATE_SUBCLASS_ID,
            );
        }
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

#[cfg(windows)]
unsafe extern "system" fn install_no_activate_on_child(
    hwnd: windows::Win32::Foundation::HWND,
    _lparam: windows::Win32::Foundation::LPARAM,
) -> windows::core::BOOL {
    use windows::Win32::UI::Shell::SetWindowSubclass;

    unsafe {
        SetWindowSubclass(
            hwnd,
            Some(pet_no_activate_proc),
            PET_NO_ACTIVATE_SUBCLASS_ID,
            0,
        )
    }
}

#[cfg(windows)]
fn refresh_no_activate_descendants(window: &WebviewWindow) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::SetWindowSubclass;
    use windows::Win32::UI::WindowsAndMessaging::EnumChildWindows;

    let raw = window.hwnd().map_err(|error| error.to_string())?;
    let hwnd = HWND(raw.0 as *mut _);
    unsafe {
        let _ = SetWindowSubclass(
            hwnd,
            Some(pet_no_activate_proc),
            PET_NO_ACTIVATE_SUBCLASS_ID,
            0,
        );
        let _ = EnumChildWindows(
            Some(hwnd),
            Some(install_no_activate_on_child),
            Default::default(),
        );
    }
    Ok(())
}

#[cfg(not(windows))]
fn refresh_no_activate_descendants(_window: &WebviewWindow) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
pub fn set_pet_hitbox_profile(window: &WebviewWindow, character_id: &str) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;

    let raw = window.hwnd().map_err(|error| error.to_string())?;
    let hwnd = HWND(raw.0 as *mut _);
    let hitboxes = if character_id.starts_with("pet_") {
        let data_root = window
            .app_handle()
            .path()
            .app_data_dir()
            .map_err(|error| error.to_string())?;
        let state = window.app_handle().state::<AppState>();
        let database = state.database().map_err(|error| error.to_string())?;
        character_store::load_runtime_hitboxes(
            &database,
            &data_root.join("characters"),
            character_id,
        )
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|hitbox| match hitbox.shape.as_str() {
            "rectangle" => HitboxRegion::rectangle(hitbox.x, hitbox.y, hitbox.width, hitbox.height),
            _ => HitboxRegion::ellipse(hitbox.x, hitbox.y, hitbox.width, hitbox.height),
        })
        .collect()
    } else {
        builtin_hitboxes(character_id)
    };
    PET_HITBOXES
        .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
        .lock()
        .map_err(|_| "桌宠命中区域锁已损坏".to_owned())?
        .insert(hwnd.0 as isize, hitboxes);
    Ok(())
}

#[cfg(not(windows))]
pub fn set_pet_hitbox_profile(_window: &WebviewWindow, _character_id: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
fn install_native_hit_test(
    window: &WebviewWindow,
    hwnd: windows::Win32::Foundation::HWND,
) -> Result<(), String> {
    use windows::Win32::UI::Shell::SetWindowSubclass;

    let character_id = window
        .app_handle()
        .state::<AppState>()
        .snapshot()
        .map_err(|error| error.to_string())?
        .active_character_id;
    set_pet_hitbox_profile(window, &character_id)?;
    refresh_no_activate_descendants(window)?;

    if unsafe { SetWindowSubclass(hwnd, Some(pet_hit_test_proc), PET_HIT_TEST_SUBCLASS_ID, 0) }
        .as_bool()
    {
        Ok(())
    } else {
        Err("无法安装桌宠透明区域命中测试".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HitboxRegion, advance_horizontal, autonomous_movement_step, can_autonomous_move,
        clamp_position, hitboxes_contain, should_defer_position_settle, should_enter_sleep,
    };
    use crate::state::RuntimeState;
    use std::time::Duration;

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

    #[test]
    fn position_settle_waits_until_cross_monitor_drag_is_released() {
        assert!(should_defer_position_settle(true, true));
        assert!(!should_defer_position_settle(true, false));
        assert!(!should_defer_position_settle(false, true));
    }

    #[test]
    fn autonomous_movement_stays_on_the_work_area_floor() {
        assert_eq!(
            advance_horizontal(400, 320, 320, (0, 0, 1920, 1040), -1, 5),
            (395, 720, -1)
        );
    }

    #[test]
    fn autonomous_movement_reverses_at_both_edges() {
        assert_eq!(
            advance_horizontal(1, 320, 320, (0, 0, 1920, 1040), -1, 5),
            (0, 720, 1)
        );
        assert_eq!(
            advance_horizontal(1599, 320, 320, (0, 0, 1920, 1040), 1, 5),
            (1600, 720, -1)
        );
    }

    #[test]
    fn autonomous_movement_supports_negative_origin_monitors() {
        assert_eq!(
            advance_horizontal(-1919, 320, 320, (-1920, 0, 0, 1080), -1, 5),
            (-1920, 760, 1)
        );
    }

    #[test]
    fn autonomous_movement_scales_for_gate_a_dpi_matrix() {
        assert_eq!(autonomous_movement_step(0.1, 1.25), 6);
        assert_eq!(autonomous_movement_step(0.1, 1.5), 7);
        assert_eq!(autonomous_movement_step(0.1, 2.0), 10);
    }

    #[test]
    fn autonomous_movement_respects_runtime_priorities() {
        let mut state = RuntimeState {
            autonomous_movement: true,
            ..RuntimeState::default()
        };
        assert!(can_autonomous_move(&state));

        state.visible = false;
        assert!(!can_autonomous_move(&state));
        state.visible = true;
        state.paused = true;
        assert!(!can_autonomous_move(&state));
        state.paused = false;
        for behavior in ["tap", "drag", "drop", "sleep", "wake"] {
            state.last_behavior_state = behavior.to_owned();
            assert!(
                !can_autonomous_move(&state),
                "{behavior} must block movement"
            );
        }
        state.last_behavior_state = "idle".to_owned();
        state.autonomous_movement = false;
        assert!(!can_autonomous_move(&state));
    }

    #[test]
    fn inactivity_sleep_uses_configured_threshold_and_runtime_guards() {
        let mut state = RuntimeState {
            sleep_after_minutes: 10,
            ..RuntimeState::default()
        };
        assert!(!should_enter_sleep(&state, Duration::from_secs(599)));
        assert!(should_enter_sleep(&state, Duration::from_secs(600)));

        state.last_behavior_state = "walk".to_owned();
        assert!(should_enter_sleep(&state, Duration::from_secs(600)));
        state.last_behavior_state = "sleep".to_owned();
        assert!(!should_enter_sleep(&state, Duration::from_secs(600)));

        state.last_behavior_state = "idle".to_owned();
        state.sleep_after_minutes = 0;
        assert!(!should_enter_sleep(&state, Duration::from_secs(3600)));
        state.sleep_after_minutes = 10;
        state.paused = true;
        assert!(!should_enter_sleep(&state, Duration::from_secs(600)));
    }

    #[test]
    fn normalized_hitboxes_reject_transparent_corners() {
        let hitboxes = [
            HitboxRegion::ellipse(0.1, 0.1, 0.8, 0.8),
            HitboxRegion::rectangle(0.4, 0.0, 0.2, 1.0),
        ];
        assert!(hitboxes_contain(&hitboxes, 0.5, 0.5));
        assert!(hitboxes_contain(&hitboxes, 0.5, 0.02));
        assert!(!hitboxes_contain(&hitboxes, 0.02, 0.02));
        assert!(!hitboxes_contain(&hitboxes, 0.98, 0.98));
    }
}
