use bevy::{
    ecs::{component::ComponentId, entity::EntityHashMap},
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
    pub descriptor: LuaQueryDescriptor,
    /// The cache
    pub cache: LuaQueryCache,
}

/// A descriptor describing a query defined in luau
pub struct LuaQueryDescriptor {
    /// The list of components to read
    pub read: SmallVec<[ComponentId; 4]>,
    /// The list of components to write
    pub write: SmallVec<[ComponentId; 4]>,
    /// The list of components the archetype should also contain
    pub with: SmallVec<[ComponentId; 4]>,
}
