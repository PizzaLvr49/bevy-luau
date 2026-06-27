use std::convert::Infallible;

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
    pub access: FilteredAccess,
    /// The cached state
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

/// A command that constructs and spawns a [`LuaQueryEntry`]
pub struct BuildLuauQuery {
    /// The query descriptor describing what components and filters the query has
    pub access: FilteredAccess,
    /// The order of the components in the query
    pub order: SmallVec<[ComponentId; 8]>,
}

impl Command for BuildLuauQuery {
    type Out = Result<Entity, Infallible>;

    fn apply(self, world: &mut World) -> Self::Out {
        let mut builder = QueryBuilder::<FilteredEntityMut>::new(world);
        builder.extend_access(self.access.clone());
        let state: QueryState<FilteredEntityMut<'static, 'static>> = builder.build();

        let entity = world
            .spawn(LuaQueryEntry {
                access: self.access,
                state,
                order: self.order,
            })
            .id();

        Ok(entity)
    }
}
