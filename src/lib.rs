//! A experimental bevy and luau integration layer

use bevy::{asset::AssetPath, prelude::*};

/// Lua Query state
pub mod query;

/// Loads and initilizes luau scripts
pub mod loading;

/// Resources and systems managing vm state
pub mod runtime;

/// A plugin to integrate bevy and luau
#[derive(Resource, Clone, Default)]
pub struct BevyLuauPlugin {
    /// The path to find the luau file if using the builtin asset loader to load luau file from disk
    ///
    /// Leave none to signify that you will manually insert the resource
    pub file_path: Option<AssetPath<'static>>,
}

impl Plugin for BevyLuauPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.clone());

        app.add_systems(Update, loading::watch_entrypoint_ready);
        app.add_systems(Startup, loading::load_luau_entrypoint);
        app.add_observer(loading::init_luau);
    }
}
