use bevy::{
    ecs::{component::ComponentId, query::FilteredAccess, world::FilteredEntityMut},
    prelude::*,
};
use mluau::prelude::*;
use smallvec::SmallVec;

/// A cached luau query state
#[derive(Component)]
pub struct LuaQueryEntry {
    /// Descriptor of the query
    pub descriptor: FilteredAccess,
    /// The cached state
    pub state: QueryState<FilteredEntityMut<'static, 'static>>,
    /// The order of components
    pub order: SmallVec<[ComponentId; 8]>,
}

/// The lua components a entity has
#[derive(Component, Default)]
pub struct LuaComponents {
    /// The list of keys to the cached lua state for this entities components
    pub keys: SmallVec<[(ComponentId, LuaRegistryKey); 8]>,
}
