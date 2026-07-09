use std::alloc::Layout;

use bevy::{
    ecs::{
        component::{ComponentCloneBehavior, ComponentDescriptor, ComponentId, StorageType},
        query::FilteredAccess,
        world::FilteredEntityMut,
    },
    prelude::*,
};
use lasso::Spur;
use mluau::prelude::*;
use smallvec::SmallVec;

use crate::{fields::LuauFieldType, query::LuaQueryEntry, schema::SchemaRegistry};

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

            let taken_access = std::mem::take(access);
            let taken_order = std::mem::take(order);

            let mut builder = QueryBuilder::<FilteredEntityMut>::new(world);
            builder.extend_access(taken_access.clone());
            let state = builder.build();
            let entity = world
                .spawn(LuaQueryEntry {
                    state,
                    order: taken_order,
                })
                .id();

            *slot = QuerySlot::Built(entity);
        }
    });
}

pub(crate) fn flush_pending_components(world: &mut World) {
    world.resource_scope(|world, mut runtime: Mut<LuauRuntime>| {
        world.resource_scope(|world, mut schema: Mut<SchemaRegistry>| {
            for slot in &mut runtime.state.components {
                let ComponentSlot::Pending {
                    layout,
                    storage,
                    offsets,
                } = slot
                else {
                    continue;
                };
                let (layout, storage) = (*layout, *storage);
                let offsets = std::mem::take(offsets);

                #[expect(unsafe_code, reason = "All POD and validated layout")]
                let descriptor = unsafe {
                    ComponentDescriptor::new_with_layout(
                        "LuauComponent",
                        storage,
                        layout,
                        None,
                        true,
                        ComponentCloneBehavior::Default,
                        None,
                    )
                };

                let id = world.register_component_with_descriptor(descriptor);
                schema.insert(id, offsets);
                *slot = ComponentSlot::Built(id);
            }
        });
    });
}
