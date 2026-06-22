#![expect(
    unsafe_code,
    reason = "Unsafe code is needed to work with dynamic components"
)]

pub mod bridge;
pub mod commands;
pub mod fields;
pub mod loading;
pub mod pool;
pub mod query;
pub mod runtime;
pub mod schema;
pub mod systems;
pub mod types;

use std::path::PathBuf;

use bevy::prelude::*;
use loading::load_scripts;
use pool::EngineStringPool;
use runtime::ScriptingRuntime;
use schema::SchemaRegistry;
use systems::{FrameArena, lua_startup_system, lua_update_system, reset_frame_arena};

#[derive(Resource, Clone)]
pub struct ScriptConfig {
    pub entry_point: PathBuf,
}

impl Default for ScriptConfig {
    fn default() -> Self {
        Self {
            entry_point: PathBuf::from("assets/scripts/main.luau"),
        }
    }
}

pub struct ScriptingPlugin {
    pub entry_point: PathBuf,
}

impl ScriptingPlugin {
    pub fn new(entry_point: impl Into<PathBuf>) -> Self {
        Self {
            entry_point: entry_point.into(),
        }
    }
}

impl Default for ScriptingPlugin {
    fn default() -> Self {
        Self {
            entry_point: ScriptConfig::default().entry_point,
        }
    }
}

impl Plugin for ScriptingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ScriptConfig {
            entry_point: self.entry_point.clone(),
        })
        .init_non_send::<ScriptingRuntime>()
        .init_non_send::<EngineStringPool>()
        .init_resource::<SchemaRegistry>()
        .init_non_send::<FrameArena>()
        .add_systems(PreUpdate, reset_frame_arena)
        .add_systems(Startup, (load_scripts, lua_startup_system).chain())
        .add_systems(Update, lua_update_system);
    }
}
