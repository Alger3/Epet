use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tauri_plugin_autostart::ManagerExt;

use crate::behavior::{self, BehaviorEvent};
use crate::character_store::{self, CharacterLibraryItem, RuntimeCharacterDefinition};
use crate::package::{self, PackageSummary};
use crate::state::{
    AppState, DEFAULT_CHARACTER_ID, MAX_PET_SCALE, MIN_PET_SCALE, RuntimeState,
    SLEEP_AFTER_MINUTE_OPTIONS,
};
use crate::workshop::{self, CreationDraft, SavePhotoInput, WorkshopSnapshot};
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
pub fn get_workshop_snapshot(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<WorkshopSnapshot, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    let data_root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let state = app.state::<AppState>();
    let database = state.database().map_err(|error| error.to_string())?;
    workshop::snapshot(&database, &data_root.join("workshop-drafts"))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_character_draft(
    app: AppHandle,
    window: WebviewWindow,
    subject_kind: String,
    authorization_confirmed: bool,
) -> Result<CreationDraft, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    let data_root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let state = app.state::<AppState>();
    let _operation = state
        .workshop_operation()
        .map_err(|error| error.to_string())?;
    let mut database = state.database().map_err(|error| error.to_string())?;
    workshop::create_draft(
        &mut database,
        &data_root.join("workshop-drafts"),
        &subject_kind,
        authorization_confirmed,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn save_draft_photo(
    app: AppHandle,
    window: WebviewWindow,
    draft_id: String,
    role: String,
    original_name: String,
    encoded_bytes: Vec<u8>,
    crop_x: f64,
    crop_y: f64,
    crop_width: f64,
    crop_height: f64,
) -> Result<CreationDraft, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    tauri::async_runtime::spawn_blocking(move || {
        let data_root = app
            .path()
            .app_data_dir()
            .map_err(|error| error.to_string())?;
        let state = app.state::<AppState>();
        let _operation = state
            .workshop_operation()
            .map_err(|error| error.to_string())?;
        let mut database = state.database().map_err(|error| error.to_string())?;
        workshop::save_photo(
            &mut database,
            &data_root.join("workshop-drafts"),
            SavePhotoInput {
                draft_id: &draft_id,
                role: &role,
                original_name: &original_name,
                encoded_bytes: &encoded_bytes,
                crop_x,
                crop_y,
                crop_width,
                crop_height,
            },
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("照片处理任务异常结束：{error}"))?
}

#[tauri::command]
pub fn remove_draft_photo(
    app: AppHandle,
    window: WebviewWindow,
    draft_id: String,
    role: String,
) -> Result<CreationDraft, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    let data_root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let state = app.state::<AppState>();
    let _operation = state
        .workshop_operation()
        .map_err(|error| error.to_string())?;
    let mut database = state.database().map_err(|error| error.to_string())?;
    workshop::remove_photo(
        &mut database,
        &data_root.join("workshop-drafts"),
        &draft_id,
        &role,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn start_draft_generation(
    app: AppHandle,
    window: WebviewWindow,
    draft_id: String,
) -> Result<CreationDraft, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    let data_root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let state = app.state::<AppState>();
    let _operation = state
        .workshop_operation()
        .map_err(|error| error.to_string())?;
    let mut database = state.database().map_err(|error| error.to_string())?;
    workshop::start_generation(&mut database, &data_root.join("workshop-drafts"), &draft_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn cancel_character_draft(
    app: AppHandle,
    window: WebviewWindow,
    draft_id: String,
) -> Result<CreationDraft, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    let data_root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let state = app.state::<AppState>();
    let _operation = state
        .workshop_operation()
        .map_err(|error| error.to_string())?;
    let mut database = state.database().map_err(|error| error.to_string())?;
    workshop::cancel_draft(&mut database, &data_root.join("workshop-drafts"), &draft_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_character_draft(
    app: AppHandle,
    window: WebviewWindow,
    draft_id: String,
) -> Result<bool, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    let data_root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let state = app.state::<AppState>();
    let _operation = state
        .workshop_operation()
        .map_err(|error| error.to_string())?;
    let mut database = state.database().map_err(|error| error.to_string())?;
    workshop::delete_draft(&mut database, &data_root.join("workshop-drafts"), &draft_id)
        .map_err(|error| error.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn rename_installed_character(
    window: WebviewWindow,
    state: State<'_, AppState>,
    character_id: String,
    custom_name: String,
) -> Result<(), String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    let database = state.database().map_err(|error| error.to_string())?;
    workshop::rename_character(&database, &character_id, &custom_name)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn inspect_pet_package(
    window: WebviewWindow,
    path: String,
    expected_sha256: Option<String>,
) -> Result<PackageSummary, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    let package = package::load_epet(std::path::Path::new(&path), expected_sha256.as_deref())
        .map_err(|error| error.to_string())?;
    Ok(PackageSummary::from(&package))
}

#[tauri::command]
pub fn list_character_library(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Vec<CharacterLibraryItem>, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    let data_root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let database = state.database().map_err(|error| error.to_string())?;
    let mut characters =
        character_store::list_characters(&database).map_err(|error| error.to_string())?;
    for character in &mut characters {
        if character.built_in {
            character.local_available = true;
            continue;
        }
        for version in &mut character.versions {
            version.local_available = data_root
                .join("characters")
                .join(&character.id)
                .join("versions")
                .join(&version.package_sha256)
                .join("package.epet")
                .is_file();
        }
        character.local_available = character
            .versions
            .iter()
            .any(|version| version.current && version.local_available);
    }
    Ok(characters)
}

#[tauri::command]
pub fn get_character_definition(
    app: AppHandle,
    window: WebviewWindow,
    character_id: String,
) -> Result<RuntimeCharacterDefinition, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL, windows::PET_LABEL])?;
    let data_root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let state = app.state::<AppState>();
    let database = state.database().map_err(|error| error.to_string())?;
    character_store::load_runtime_definition(
        &database,
        &data_root.join("characters"),
        &character_id,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn install_pet_package_from_url(
    app: AppHandle,
    window: WebviewWindow,
    url: String,
    expected_sha256: String,
) -> Result<CharacterLibraryItem, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    character_store::validate_expected_sha256(&expected_sha256)
        .map_err(|error| error.to_string())?;
    let task_app = app.clone();
    let item = tauri::async_runtime::spawn_blocking(move || {
        let data_root = task_app
            .path()
            .app_data_dir()
            .map_err(|error| error.to_string())?;
        let cache_root = task_app
            .path()
            .app_cache_dir()
            .map_err(|error| error.to_string())?;
        let state = task_app.state::<AppState>();
        let _operation = state
            .package_operation()
            .map_err(|error| error.to_string())?;
        let downloaded =
            character_store::download_to_temporary(&cache_root.join("package-downloads"), &url)
                .map_err(|error| error.to_string())?;
        let mut database = state.database().map_err(|error| error.to_string())?;
        character_store::install_package(
            &mut database,
            &data_root.join("characters"),
            downloaded.path(),
            Some(&expected_sha256),
            None,
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("角色包安装任务异常结束：{error}"))??;
    app.emit("character-definition-changed", &item.id)
        .map_err(|error| error.to_string())?;
    Ok(item)
}

#[tauri::command]
pub async fn install_local_pet_package(
    app: AppHandle,
    window: WebviewWindow,
    path: String,
    expected_sha256: Option<String>,
) -> Result<CharacterLibraryItem, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    let task_app = app.clone();
    let item = tauri::async_runtime::spawn_blocking(move || {
        let data_root = task_app
            .path()
            .app_data_dir()
            .map_err(|error| error.to_string())?;
        let state = task_app.state::<AppState>();
        let _operation = state
            .package_operation()
            .map_err(|error| error.to_string())?;
        let mut database = state.database().map_err(|error| error.to_string())?;
        character_store::install_package(
            &mut database,
            &data_root.join("characters"),
            std::path::Path::new(&path),
            expected_sha256.as_deref(),
            None,
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("角色包安装任务异常结束：{error}"))??;
    app.emit("character-definition-changed", &item.id)
        .map_err(|error| error.to_string())?;
    Ok(item)
}

#[tauri::command]
pub fn activate_character_version(
    app: AppHandle,
    window: WebviewWindow,
    character_id: String,
    package_sha256: String,
) -> Result<CharacterLibraryItem, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    let data_root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let state = app.state::<AppState>();
    let _operation = state
        .package_operation()
        .map_err(|error| error.to_string())?;
    let mut database = state.database().map_err(|error| error.to_string())?;
    let item = character_store::activate_version(
        &mut database,
        &data_root.join("characters"),
        &character_id,
        &package_sha256,
    )
    .map_err(|error| error.to_string())?;
    app.emit("character-definition-changed", &character_id)
        .map_err(|error| error.to_string())?;
    Ok(item)
}

#[tauri::command]
pub fn delete_character_version(
    app: AppHandle,
    window: WebviewWindow,
    character_id: String,
    package_sha256: String,
) -> Result<CharacterLibraryItem, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    let data_root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let state = app.state::<AppState>();
    let _operation = state
        .package_operation()
        .map_err(|error| error.to_string())?;
    let mut database = state.database().map_err(|error| error.to_string())?;
    character_store::delete_version(
        &mut database,
        &data_root.join("characters"),
        &character_id,
        &package_sha256,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_installed_character(
    app: AppHandle,
    window: WebviewWindow,
    character_id: String,
) -> Result<(), String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    let data_root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let state = app.state::<AppState>();
    let _operation = state
        .package_operation()
        .map_err(|error| error.to_string())?;
    let active_character_id = state
        .snapshot()
        .map_err(|error| error.to_string())?
        .active_character_id;
    let active_character_id = if active_character_id == character_id {
        if let Some(pet) = app.get_webview_window(windows::PET_LABEL) {
            windows::set_pet_hitbox_profile(&pet, DEFAULT_CHARACTER_ID)?;
            pet.hide().map_err(|error| error.to_string())?;
        }
        let snapshot = state
            .update(|runtime| {
                runtime.active_character_id = DEFAULT_CHARACTER_ID.to_owned();
                runtime.visible = false;
            })
            .map_err(|error| error.to_string())?;
        windows::emit_runtime_state(&app, &snapshot);
        tray::sync_checks(&app, &snapshot);
        snapshot.active_character_id
    } else {
        active_character_id
    };
    let mut database = state.database().map_err(|error| error.to_string())?;
    character_store::delete_character(
        &mut database,
        &data_root.join("characters"),
        &character_id,
        &active_character_id,
    )
    .map_err(|error| error.to_string())
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
    state
        .clear_wake_clicks()
        .map_err(|error| error.to_string())?;
    if !state
        .character_exists(&character_id)
        .map_err(|error| error.to_string())?
    {
        return Err("角色不存在，或该安装包尚未接入桌宠渲染运行时".to_owned());
    }
    if character_id.starts_with("pet_") {
        let data_root = app
            .path()
            .app_data_dir()
            .map_err(|error| error.to_string())?;
        let database = state.database().map_err(|error| error.to_string())?;
        character_store::load_runtime_definition(
            &database,
            &data_root.join("characters"),
            &character_id,
        )
        .map_err(|error| error.to_string())?;
    }

    if app.get_webview_window(windows::PET_LABEL).is_none() {
        windows::recreate_pet_window(&app)?;
    }
    let pet = windows::pet_window(&app)?;
    windows::set_pet_hitbox_profile(&pet, &character_id)?;
    pet.show().map_err(|error| error.to_string())?;

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
pub fn set_autonomous_movement(
    app: AppHandle,
    window: WebviewWindow,
    enabled: bool,
) -> Result<RuntimeState, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    set_autonomous_movement_internal(&app, enabled)
}

#[tauri::command]
pub fn set_sleep_after_minutes(
    app: AppHandle,
    window: WebviewWindow,
    minutes: u32,
) -> Result<RuntimeState, String> {
    ensure_caller(&window, &[windows::WORKSHOP_LABEL])?;
    if !SLEEP_AFTER_MINUTE_OPTIONS.contains(&minutes) {
        return Err("睡眠等待时间只支持从角色工坊提供的预设值中选择".to_owned());
    }

    let state = app.state::<AppState>();
    state
        .clear_wake_clicks()
        .map_err(|error| error.to_string())?;
    let snapshot = state
        .update(|runtime| runtime.sleep_after_minutes = minutes)
        .map_err(|error| error.to_string())?;
    windows::emit_runtime_state(&app, &snapshot);
    Ok(snapshot)
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
    let current = state.snapshot().map_err(|error| error.to_string())?;
    state
        .clear_wake_clicks()
        .map_err(|error| error.to_string())?;
    if current.last_behavior_state == "sleep" {
        let drag_result = window.start_dragging();
        windows::restore_previous_foreground();
        if let Err(error) = drag_result {
            return Err(error.to_string());
        }
        windows::schedule_position_persist(window);
        return state.snapshot().map_err(|error| error.to_string());
    }
    let dragging = state
        .update(|runtime| {
            runtime.last_behavior_state =
                behavior::transition(&runtime.last_behavior_state, BehaviorEvent::DragStart)
                    .to_owned();
        })
        .map_err(|error| error.to_string())?;
    windows::emit_runtime_state(&app, &dragging);

    let drag_result = window.start_dragging();
    windows::restore_previous_foreground();
    if let Err(error) = drag_result {
        let restored = state
            .update(|runtime| {
                let dropped =
                    behavior::transition(&runtime.last_behavior_state, BehaviorEvent::DragEnd)
                        .to_owned();
                runtime.last_behavior_state =
                    behavior::transition(&dropped, BehaviorEvent::AnimationFinished).to_owned();
            })
            .map_err(|state_error| state_error.to_string())?;
        windows::emit_runtime_state(&app, &restored);
        return Err(error.to_string());
    }

    windows::schedule_position_persist(window);
    state.snapshot().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn trigger_pet_tap(app: AppHandle, window: WebviewWindow) -> Result<RuntimeState, String> {
    ensure_caller(&window, &[windows::PET_LABEL])?;
    let state = app.state::<AppState>();
    let current = state.snapshot().map_err(|error| error.to_string())?;
    if current.last_behavior_state == "sleep"
        && !current.paused
        && current.visible
        && !current.click_through
    {
        let click_count = state
            .register_sleep_click()
            .map_err(|error| error.to_string())?;
        let next = if click_count >= 3 {
            state
                .update(|runtime| {
                    runtime.last_behavior_state =
                        behavior::transition(&runtime.last_behavior_state, BehaviorEvent::Wake)
                            .to_owned();
                })
                .map_err(|error| error.to_string())?
        } else {
            current
        };
        windows::emit_runtime_state(&app, &next);
        windows::restore_previous_foreground();
        if next.last_behavior_state == "wake" {
            windows::schedule_behavior_timeout(
                app,
                "wake",
                BehaviorEvent::AnimationFinished,
                std::time::Duration::from_millis(650),
            );
        }
        return Ok(next);
    }

    state
        .clear_wake_clicks()
        .map_err(|error| error.to_string())?;
    let tapped = state
        .update(|runtime| {
            if !runtime.paused && runtime.visible && !runtime.click_through {
                runtime.last_behavior_state =
                    behavior::transition(&runtime.last_behavior_state, BehaviorEvent::Tap)
                        .to_owned();
            }
        })
        .map_err(|error| error.to_string())?;
    windows::emit_runtime_state(&app, &tapped);
    windows::restore_previous_foreground();
    if tapped.last_behavior_state == "tap" {
        windows::schedule_behavior_timeout(
            app,
            "tap",
            BehaviorEvent::AnimationFinished,
            std::time::Duration::from_millis(420),
        );
    }
    Ok(tapped)
}

#[tauri::command]
pub fn restore_pet_focus(app: AppHandle, window: WebviewWindow) -> Result<RuntimeState, String> {
    ensure_caller(&window, &[windows::PET_LABEL])?;
    windows::restore_previous_foreground();
    app.state::<AppState>()
        .snapshot()
        .map_err(|error| error.to_string())
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

    let state = app.state::<AppState>();
    state
        .clear_wake_clicks()
        .map_err(|error| error.to_string())?;
    let snapshot = state
        .update(|runtime| {
            runtime.visible = visible;
            if !visible {
                runtime.last_behavior_state = "idle".to_owned();
            }
        })
        .map_err(|error| error.to_string())?;
    windows::emit_runtime_state(app, &snapshot);
    tray::sync_checks(app, &snapshot);
    Ok(snapshot)
}

pub fn set_paused_internal(app: &AppHandle, paused: bool) -> Result<RuntimeState, String> {
    let state = app.state::<AppState>();
    state
        .clear_wake_clicks()
        .map_err(|error| error.to_string())?;
    let snapshot = state
        .update(|runtime| {
            runtime.paused = paused;
            runtime.last_behavior_state = "idle".to_owned();
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

    let state = app.state::<AppState>();
    state
        .clear_wake_clicks()
        .map_err(|error| error.to_string())?;
    let snapshot = state
        .update(|runtime| {
            runtime.click_through = click_through;
            if click_through && matches!(runtime.last_behavior_state.as_str(), "tap" | "drag") {
                runtime.last_behavior_state = "idle".to_owned();
            }
        })
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

pub fn set_autonomous_movement_internal(
    app: &AppHandle,
    enabled: bool,
) -> Result<RuntimeState, String> {
    let snapshot = app
        .state::<AppState>()
        .update(|runtime| {
            runtime.autonomous_movement = enabled;
            if !enabled && runtime.last_behavior_state == "walk" {
                runtime.last_behavior_state = "idle".to_owned();
            }
        })
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
