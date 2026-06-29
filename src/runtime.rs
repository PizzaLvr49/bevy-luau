use bevy::{
    ecs::{component::ComponentId, query::FilteredAccess},
    prelude::*,
};
use mluau::prelude::*;
use smallvec::SmallVec;

use crate::query::BuildLuauQuery;

/// Holds a [`Lua`] and other relevant state
#[derive(Resource)]
pub struct LuauRuntime {
    pub(crate) lua: Lua,
    pub(crate) state: RuntimeState,
}

pub(crate) enum QuerySlot {
    Pending {
        access: FilteredAccess,
        order: SmallVec<[ComponentId; 8]>,
    },
    Built(Entity),
}

pub(crate) struct RuntimeState {
    pub(crate) queries: SmallVec<[QuerySlot; 8]>,
}

pub(crate) fn flush_pending_queries(world: &mut World) {
    world.resource_scope(|world, mut runtime: Mut<LuauRuntime>| {
        for slot in &mut runtime.state.queries {
            if !matches!(slot, QuerySlot::Pending { .. }) {
                continue;
            }
            let QuerySlot::Pending { access, order } =
                std::mem::replace(slot, QuerySlot::Built(Entity::PLACEHOLDER))
            else {
                unreachable!()
            };
            let entity = BuildLuauQuery { access, order }.apply(world).unwrap();
            *slot = QuerySlot::Built(entity);
        }
    });
}
