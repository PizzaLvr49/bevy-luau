use std::alloc::Layout;

use crate::{
    fields::infer_field_type,
    pool::EngineStringPool,
    runtime::{self, ComponentSlot, QuerySlot, RuntimeState},
};

use bevy::{
    ecs::{
        component::{ComponentId, StorageType},
        query::FilteredAccess,
    },
    prelude::*,
};
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
use smallvec::SmallVec;
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
        error!("LuauEntrypointReady fired, but the asset was missing!");
        return;
    };

    let lua = Lua::new();

    lua.set_app_data(EngineStringPool::default());

    lua.set_app_data(RuntimeState {
        queries: SmallVec::new(),
        components: SmallVec::new(),
    });

    let globals = lua.globals();

    globals.set("Ecs", EcsHandle).unwrap();

    lua.load(&script.bytecode).exec().unwrap();

    globals.set("Ecs", LuaNil).unwrap();

    let state = lua.remove_app_data().unwrap();

    let string_pool: EngineStringPool = lua.remove_app_data().unwrap();

    commands.insert_resource(string_pool);

    commands.insert_resource(LuauRuntime { lua, state });

    commands.queue(runtime::flush_pending_components);
    commands.queue(runtime::flush_pending_queries);
}

struct LuauComponentMarker(pub(crate) usize);

impl LuaUserData for LuauComponentMarker {}

struct LuauQueryMarker(pub(crate) usize);

impl LuaUserData for LuauQueryMarker {}

struct EcsHandle;

impl LuaUserData for EcsHandle {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("RegisterComponent", |lua, _, args: LuaMultiValue| {
            let component = args
                .front()
                .expect("missing component")
                .as_table()
                .expect("not a table");

            let mut runtime_state = lua.app_data_mut::<RuntimeState>().unwrap();
            let mut string_pool = lua.app_data_mut::<EngineStringPool>().unwrap();

            let mut layout = Layout::from_size_align(0, 1).unwrap();
            let mut offsets = SmallVec::new();

            for r in component.pairs() {
                let (key, value): (_, LuaValue) = r?;
                let spur = string_pool.intern_lua(key)?;

                let ft = infer_field_type(&value)?;

                let field_layout = ft.layout();

                let (new_layout, offset) = layout.extend(field_layout).unwrap();
                layout = new_layout;
                offsets.push((spur, offset, ft));
            }

            layout = layout.pad_to_align();

            let config = args.get(1);
            let storage = config
                .and_then(|v| v.as_table())
                .and_then(|t| t.get::<LuaString>("StorageType").ok())
                .and_then(|v| {
                    matches!(v.to_str().as_deref(), Ok("Table")).then_some(StorageType::Table)
                })
                .unwrap_or(StorageType::SparseSet);

            let component_id = runtime_state.components.len();

            runtime_state.components.push(ComponentSlot::Pending {
                layout,
                storage,
                offsets,
            });

            drop(runtime_state);
            drop(string_pool);

            Ok(LuauComponentMarker(component_id))
        });

        methods.add_method("Query", |lua, _, query: LuaTable| {
            let mut runtime_state = lua.app_data_mut::<RuntimeState>().unwrap();
            let query_id = runtime_state.queries.len();

            let mut query_access = FilteredAccess::matches_nothing();
            let mut order = SmallVec::<[ComponentId; 8]>::new();

            let access_mappings: [(_, fn(&mut FilteredAccess, ComponentId), _); _] = [
                ("With", |qa, id| qa.and_with(id), false),
                ("Without", |qa, id| qa.and_without(id), false),
                ("Mutable", |qa, id| qa.add_write(id), true),
                ("Immutable", |qa, id| qa.add_read(id), true),
            ];

            for (key, update_access, is_data) in access_mappings {
                let table: LuaTable = query.get(key).unwrap();

                for value in table.sequence_values::<LuaValue>() {
                    let value = value?;

                    let marker = match value {
                        LuaValue::UserData(ud) => ud.borrow::<LuauComponentMarker>()?,
                        _ => return Err(LuaError::runtime("expected userdata")),
                    };

                    let component_id = marker.0;
                    let id = ComponentId::new(component_id);
                    if is_data {
                        order.push(id);
                    }

                    update_access(&mut query_access, id);
                }
            }

            runtime_state.queries.push(QuerySlot::Pending {
                access: query_access,
                order,
            });

            drop(runtime_state);

            Ok(LuauQueryMarker(query_id))
        });
    }
}
