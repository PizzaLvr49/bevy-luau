use std::alloc::Layout;

use bevy::{
    ecs::{
        component::{ComponentId, StorageType},
        query::FilteredAccess,
    },
    prelude::*,
};
use lasso::Spur;
use mluau::prelude::*;
use smallvec::SmallVec;

use crate::{fields::LuauFieldType, query::BuildLuauQuery};

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

pub(crate) enum ComponentSlot {
    Pending {
        layout: Layout,
        storage: StorageType,
        offsets: SmallVec<[(Spur, usize, LuauFieldType); 6]>,
    },
    Built(ComponentId),
}

pub(crate) struct RuntimeState {
    pub(crate) queries: SmallVec<[QuerySlot; 8]>,
    pub(crate) components: SmallVec<[ComponentSlot; 8]>,
}

pub(crate) fn flush_pending_queries(world: &mut World) {
    world.resource_scope(|world, mut runtime: Mut<LuauRuntime>| {
        for slot in &mut runtime.state.queries {
            let QuerySlot::Pending { access, order } = slot else {
                continue;
            };

            let taken_access = std::mem::replace(access, FilteredAccess::matches_nothing());
            let taken_order = std::mem::take(order);

            let entity = BuildLuauQuery {
                access: taken_access,
                order: taken_order,
            }
            .apply(world)
            .unwrap();

            *slot = QuerySlot::Built(entity);
        }
    });
}

pub(crate) const fn flush_pending_components(_world: &mut World) {}
