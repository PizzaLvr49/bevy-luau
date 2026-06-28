use bevy::prelude::*;
use mluau::{Compiler, prelude::*};
use serde::{Deserialize, Serialize};

/// The bytecode for the luau entrypoint.
#[derive(Asset, TypePath)]
pub struct LuauScriptAsset {
    pub(crate) bytecode: Vec<u8>,
}

/// A handle to the entrypoint luau script
#[derive(Resource)]
pub struct LuauEntrypoint(pub Handle<LuauScriptAsset>);

use bevy::asset::{AssetLoader, LoadContext, io::Reader};
use thiserror::Error;

use crate::runtime::LuauRuntime;

/// Loads luau scripts
#[derive(Default, TypePath)]
pub struct LuauScriptLoader;

/// Represents an error in loading luau scripts.
#[derive(Error, Debug)]
pub enum LuauLoaderError {
    /// An Io related error.
    #[error("An error occurred while reading the file: {0}")]
    Io(#[from] std::io::Error),
    /// An error in the luau compilation pipeline.
    #[error("An error occurred while compiling luau: {0}")]
    CompilationError(#[from] mluau::Error),
}

/// Settings for compiling a [`LuauScriptAsset`]
#[derive(Default, Serialize, Deserialize, Asset, TypePath, Clone)]
pub struct LuauCompilerSettings {
    /// The optimization level to use
    pub optimization_level: u8,
    /// The debug level to use
    pub debug_level: u8,
    /// The type info level to use
    pub type_info_level: u8,
    /// The coverage level to use
    pub coverage_level: u8,
}

impl AssetLoader for LuauScriptLoader {
    type Asset = LuauScriptAsset;
    type Settings = LuauCompilerSettings;
    type Error = LuauLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut source = Vec::new();
        reader.read_to_end(&mut source).await?;

        let compiler = Compiler::new()
            .set_optimization_level(settings.optimization_level)
            .set_debug_level(settings.debug_level)
            .set_type_info_level(settings.type_info_level)
            .set_coverage_level(settings.coverage_level);

        let bytecode = compiler.compile(source)?;

        Ok(LuauScriptAsset { bytecode })
    }

    fn extensions(&self) -> &[&str] {
        &["luau", "lua"]
    }
}

pub(crate) fn load_luau_entrypoint(
    asset_server: Res<AssetServer>,
    plugin: Res<crate::BevyLuauPlugin>,
    mut commands: Commands,
) {
    let Some(ref file_path) = plugin.file_path else {
        return;
    };
    let handle: Handle<LuauScriptAsset> = asset_server.load(file_path);
    commands.spawn(LuauEntrypoint(handle));
}

/// Fires when the [`LuauEntrypoint`] exists and its handle is loaded.
#[derive(Event)]
pub struct LuauEntrypointReady;

pub(crate) fn watch_entrypoint_ready(
    entry: Option<Res<LuauEntrypoint>>,
    mut events: MessageReader<AssetEvent<LuauScriptAsset>>,
    mut commands: Commands,
) {
    let Some(entry) = entry else { return };
    let id = entry.0.id();

    for event in events.read() {
        let matches = match event {
            AssetEvent::Added { id: ev_id }
            | AssetEvent::Modified { id: ev_id }
            | AssetEvent::LoadedWithDependencies { id: ev_id } => *ev_id == id,
            _ => false,
        };
        if matches {
            commands.trigger(LuauEntrypointReady);
        }
    }
}

pub(crate) fn init_luau(
    _: On<LuauEntrypointReady>,
    luau_entrypoint: Res<LuauEntrypoint>,
    luau_scripts: Res<Assets<LuauScriptAsset>>,
    mut commands: Commands,
) {
    let Some(script) = luau_scripts.get(&luau_entrypoint.0) else {
        warn!("LuauEntrypointReady fired, but the asset was missing!");
        return;
    };

    let lua = Lua::new();

    let globals = lua.globals();

    globals.set("Ecs", EcsHandle).unwrap();

    lua.load(&script.bytecode).exec().unwrap();

    commands.spawn(LuauRuntime { lua });
}

struct RuntimeState {
    query_id: i32,
}

struct EcsHandle;

impl LuaUserData for EcsHandle {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("RegisterComponent", |lua, _, _component: LuaTable| {
            let mut _runtime_state = lua.app_data_mut::<RuntimeState>().unwrap();
            Ok(())
        });

        methods.add_method("Query", |lua, _, _query: LuaTable| {
            let mut runtime_state = lua.app_data_mut::<RuntimeState>().unwrap();
            runtime_state.query_id += 1;
            Ok(runtime_state.query_id)
        });
    }
}
