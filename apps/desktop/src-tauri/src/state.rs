use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use rusqlite::{Connection, OptionalExtension, named_params};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use thiserror::Error;

pub const RUNTIME_SCHEMA_VERSION: i64 = 4;
pub const DEFAULT_PET_SCALE: f64 = 0.8;
pub const MIN_PET_SCALE: f64 = 0.5;
pub const MAX_PET_SCALE: f64 = 1.5;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeState {
    pub active_character_id: String,
    pub monitor_id: Option<String>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub work_area_width: Option<f64>,
    pub work_area_height: Option<f64>,
    pub dpi_scale: Option<f64>,
    pub pet_logical_size: f64,
    pub foot_anchor_x: Option<f64>,
    pub foot_anchor_y: Option<f64>,
    pub scale: f64,
    pub visible: bool,
    pub click_through: bool,
    pub always_on_top: bool,
    pub paused: bool,
    pub last_behavior_state: String,
    pub runtime_version: i64,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            active_character_id: "builtin-orange-tabby".to_owned(),
            monitor_id: None,
            x: None,
            y: None,
            work_area_width: None,
            work_area_height: None,
            dpi_scale: None,
            pet_logical_size: 320.0,
            foot_anchor_x: None,
            foot_anchor_y: None,
            scale: DEFAULT_PET_SCALE,
            visible: true,
            click_through: false,
            always_on_top: true,
            paused: false,
            last_behavior_state: "idle".to_owned(),
            runtime_version: RUNTIME_SCHEMA_VERSION,
        }
    }
}

#[derive(Debug, Error)]
pub enum StateError {
    #[error("无法解析应用数据目录：{0}")]
    AppData(String),
    #[error("无法创建应用数据目录 {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("本地数据库操作失败：{0}")]
    Database(#[from] rusqlite::Error),
    #[error("运行状态锁已损坏")]
    Poisoned,
}

pub struct AppState {
    runtime: Mutex<RuntimeState>,
    database: Mutex<Connection>,
    diagnostic: Mutex<Option<String>>,
    position_generation: AtomicU64,
    recreate_attempted: AtomicBool,
    exiting: AtomicBool,
}

impl AppState {
    pub fn open(app: &AppHandle) -> Result<Self, StateError> {
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| StateError::AppData(error.to_string()))?;

        std::fs::create_dir_all(&data_dir).map_err(|source| StateError::CreateDirectory {
            path: data_dir.clone(),
            source,
        })?;

        let connection = Connection::open(data_dir.join("epet.sqlite3"))?;
        connection.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        apply_migrations(&connection)?;

        let mut runtime = connection
            .query_row(
                "SELECT active_character_id, monitor_id, x, y, work_area_width,
                        work_area_height, dpi_scale, pet_logical_size,
                        foot_anchor_x, foot_anchor_y, scale, visible,
                        click_through, paused, last_behavior_state, runtime_version,
                        always_on_top
                 FROM runtime_state WHERE singleton = 1",
                [],
                |row| {
                    Ok(RuntimeState {
                        active_character_id: row.get(0)?,
                        monitor_id: row.get(1)?,
                        x: row.get(2)?,
                        y: row.get(3)?,
                        work_area_width: row.get(4)?,
                        work_area_height: row.get(5)?,
                        dpi_scale: row.get(6)?,
                        pet_logical_size: row.get(7)?,
                        foot_anchor_x: row.get(8)?,
                        foot_anchor_y: row.get(9)?,
                        scale: row.get::<_, f64>(10)?.clamp(MIN_PET_SCALE, MAX_PET_SCALE),
                        visible: row.get::<_, i64>(11)? != 0,
                        click_through: row.get::<_, i64>(12)? != 0,
                        paused: row.get::<_, i64>(13)? != 0,
                        last_behavior_state: row.get(14)?,
                        runtime_version: row.get(15)?,
                        always_on_top: row.get::<_, i64>(16)? != 0,
                    })
                },
            )
            .optional()?
            .unwrap_or_default();

        let active_character_exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM characters WHERE id = ?1)",
            [&runtime.active_character_id],
            |row| row.get(0),
        )?;
        if !active_character_exists {
            runtime.active_character_id = RuntimeState::default().active_character_id;
        }
        runtime.runtime_version = RUNTIME_SCHEMA_VERSION;

        persist_runtime(&connection, &runtime)?;

        Ok(Self {
            runtime: Mutex::new(runtime),
            database: Mutex::new(connection),
            diagnostic: Mutex::new(None),
            position_generation: AtomicU64::new(0),
            recreate_attempted: AtomicBool::new(false),
            exiting: AtomicBool::new(false),
        })
    }

    pub fn snapshot(&self) -> Result<RuntimeState, StateError> {
        self.runtime
            .lock()
            .map(|state| state.clone())
            .map_err(|_| StateError::Poisoned)
    }

    pub fn update(
        &self,
        mutation: impl FnOnce(&mut RuntimeState),
    ) -> Result<RuntimeState, StateError> {
        let mut guard = self.runtime.lock().map_err(|_| StateError::Poisoned)?;
        let mut next = guard.clone();
        mutation(&mut next);
        next.scale = next.scale.clamp(MIN_PET_SCALE, MAX_PET_SCALE);

        let database = self.database.lock().map_err(|_| StateError::Poisoned)?;
        persist_runtime(&database, &next)?;
        *guard = next.clone();
        Ok(next)
    }

    pub fn character_exists(&self, character_id: &str) -> Result<bool, StateError> {
        let database = self.database.lock().map_err(|_| StateError::Poisoned)?;
        database
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM characters WHERE id = ?1)",
                [character_id],
                |row| row.get(0),
            )
            .map_err(StateError::from)
    }

    pub fn next_position_generation(&self) -> u64 {
        self.position_generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn position_generation(&self) -> u64 {
        self.position_generation.load(Ordering::SeqCst)
    }

    pub fn set_diagnostic(&self, message: impl Into<String>) -> Result<(), StateError> {
        *self.diagnostic.lock().map_err(|_| StateError::Poisoned)? = Some(message.into());
        Ok(())
    }

    pub fn diagnostic(&self) -> Result<Option<String>, StateError> {
        self.diagnostic
            .lock()
            .map(|value| value.clone())
            .map_err(|_| StateError::Poisoned)
    }

    pub fn claim_recreate_attempt(&self) -> bool {
        !self.recreate_attempted.swap(true, Ordering::SeqCst)
    }

    pub fn begin_exit(&self) {
        self.exiting.store(true, Ordering::SeqCst);
    }

    pub fn is_exiting(&self) -> bool {
        self.exiting.load(Ordering::SeqCst)
    }
}

fn apply_migrations(connection: &Connection) -> Result<(), rusqlite::Error> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version < 1 {
        connection.execute_batch(include_str!("../migrations/0001-runtime-state.sql"))?;
    }
    if version < 2 {
        connection.execute_batch(include_str!("../migrations/0002-monitor-restoration.sql"))?;
    }
    if version < 3 {
        connection.execute_batch(include_str!("../migrations/0003-character-library.sql"))?;
    }
    if version < 4 {
        connection.execute_batch(include_str!("../migrations/0004-always-on-top.sql"))?;
    }
    Ok(())
}

fn persist_runtime(connection: &Connection, state: &RuntimeState) -> Result<(), rusqlite::Error> {
    connection.execute(
        "INSERT INTO runtime_state (
           singleton, active_pet_id, active_character_id, monitor_id, x, y, work_area_width,
           work_area_height, dpi_scale, pet_logical_size, foot_anchor_x,
           foot_anchor_y, scale, visible, click_through, paused,
           last_behavior_state, runtime_version, always_on_top
         ) VALUES (
           1, :active_character_id, :active_character_id, :monitor_id, :x, :y, :work_area_width,
           :work_area_height, :dpi_scale, :pet_logical_size, :foot_anchor_x,
           :foot_anchor_y, :scale, :visible, :click_through, :paused,
           :last_behavior_state, :runtime_version, :always_on_top
         )
         ON CONFLICT(singleton) DO UPDATE SET
           active_pet_id = excluded.active_character_id,
           active_character_id = excluded.active_character_id,
           monitor_id = excluded.monitor_id,
           x = excluded.x,
           y = excluded.y,
           work_area_width = excluded.work_area_width,
           work_area_height = excluded.work_area_height,
           dpi_scale = excluded.dpi_scale,
           pet_logical_size = excluded.pet_logical_size,
           foot_anchor_x = excluded.foot_anchor_x,
           foot_anchor_y = excluded.foot_anchor_y,
           scale = excluded.scale,
           visible = excluded.visible,
           click_through = excluded.click_through,
           paused = excluded.paused,
           last_behavior_state = excluded.last_behavior_state,
           runtime_version = excluded.runtime_version,
           always_on_top = excluded.always_on_top",
        named_params! {
            ":active_character_id": state.active_character_id,
            ":monitor_id": state.monitor_id,
            ":x": state.x,
            ":y": state.y,
            ":work_area_width": state.work_area_width,
            ":work_area_height": state.work_area_height,
            ":dpi_scale": state.dpi_scale,
            ":pet_logical_size": state.pet_logical_size,
            ":foot_anchor_x": state.foot_anchor_x,
            ":foot_anchor_y": state.foot_anchor_y,
            ":scale": state.scale,
            ":visible": i64::from(state.visible),
            ":click_through": i64::from(state.click_through),
            ":paused": i64::from(state.paused),
            ":last_behavior_state": state.last_behavior_state,
            ":runtime_version": state.runtime_version,
            ":always_on_top": i64::from(state.always_on_top),
        },
    )?;
    Ok(())
}
