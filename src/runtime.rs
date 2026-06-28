use bevy::prelude::*;
use mluau::prelude::*;

/// Holds a [`Lua`] and other relevant state
#[derive(Resource)]
pub struct LuauRuntime {
    pub(crate) lua: Lua,
    pub(crate) state: RuntimeState,
}

pub(crate) struct RuntimeState {
    pub(crate) query_id: u32,
}
