use bevy::{
    ecs::{component::ComponentId, world::FilteredEntityMut},
    prelude::*,
};
use mluau::prelude::*;
use smallvec::SmallVec;

/// A cached luau query state
#[derive(Component)]
pub struct LuaQueryEntry {
    /// The cached query state
    pub state: QueryState<FilteredEntityMut<'static, 'static>>,
    /// The order of components
    pub order: SmallVec<[ComponentId; 8]>,
}

/// The lua components a entity has
#[derive(Component, Default)]
pub struct LuaComponents {
    /// The list of keys to the cached lua state for this entity's components
    pub keys: SmallVec<[(ComponentId, LuaRegistryKey); 8]>,
}
