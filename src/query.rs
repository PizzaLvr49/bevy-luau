use bevy::{
    ecs::{entity::EntityHashMap, query::FilteredAccess},
    prelude::*,
};
use mluau::prelude::*;
use smallvec::SmallVec;

/// A cached luau query state
pub struct LuaQueryCache {
    /// Maps an entity to its components
    pub rows: EntityHashMap<SmallVec<[LuaTable; 16]>>,
}

/// A luau query descriptor and its associated cache
#[derive(Component)]
pub struct LuaQueryEntry {
    /// The descriptor
    pub descriptor: FilteredAccess,
    /// The cache
    pub cache: LuaQueryCache,
}
