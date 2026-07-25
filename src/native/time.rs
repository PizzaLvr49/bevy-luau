use bevy::ecs::component::ComponentId;
use bevy::prelude::*;
use bevy::ptr::OwningPtr;
use bumpalo::Bump;
use mluau::prelude::*;

use crate::pool::EngineStringPool;
use crate::schema::SchemaRegistry;

use super::{NativeBuiltin, register_native_resource};

#[derive(Resource, Clone, Copy)]
pub struct TimeBinding {
    pub component_id: ComponentId,
    delta_offset: usize,
    elapsed_offset: usize,
}

pub struct TimeBuiltin;

impl NativeBuiltin for TimeBuiltin {
    /// # Panics
    fn register(world: &mut World, lua: &Lua, pool: &mut EngineStringPool) {
        let (component_id, fields) = register_native_resource(
            world,
            lua,
            pool,
            "Time",
            &[
                ("delta_secs", LuaValue::Number(0.0)),
                ("elapsed_secs", LuaValue::Number(0.0)),
            ],
        )
        .expect("the built-in Time resource schema is always valid");

        let delta_spur = pool.intern(lua, "delta_secs");
        let elapsed_spur = pool.intern(lua, "elapsed_secs");

        world.insert_resource(TimeBinding {
            component_id,
            delta_offset: fields.offset_of(delta_spur),
            elapsed_offset: fields.offset_of(elapsed_spur),
        });
    }
}

pub fn sync(world: &mut World) {
    let delta_secs = f64::from(world.resource::<Time>().delta_secs());
    let elapsed_secs = world.resource::<Time>().elapsed().as_secs_f64();
    let binding = *world.resource::<TimeBinding>();

    world.resource_scope(|world, registry: Mut<SchemaRegistry>| {
        let Some(schema) = registry.id_to_schema.get(&binding.component_id) else {
            return;
        };
        let layout = schema.layout;
        let bump = Bump::new();

        let ptr = bump.alloc_layout(layout);
        unsafe {
            std::ptr::copy_nonoverlapping(
                schema.default_template.as_ptr(),
                ptr.as_ptr(),
                layout.size(),
            );
            ptr.as_ptr()
                .add(binding.delta_offset)
                .cast::<f64>()
                .write_unaligned(delta_secs);
            ptr.as_ptr()
                .add(binding.elapsed_offset)
                .cast::<f64>()
                .write_unaligned(elapsed_secs);
        }

        let owning = unsafe { OwningPtr::new(ptr) };
        unsafe {
            world
                .entity_mut(registry.resource_entity)
                .insert_by_id(binding.component_id, owning);
        }
    });
}
