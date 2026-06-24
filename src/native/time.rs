use bevy::ecs::component::ComponentId;
use bevy::prelude::*;
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

pub fn sync(time: Res<Time>, binding: Res<TimeBinding>, mut registry: ResMut<SchemaRegistry>) {
    let Some(data) = registry.resource_data.get_mut(&binding.component_id) else {
        return;
    };
    let delta_secs = f64::from(time.delta_secs());
    let elapsed_secs = time.elapsed().as_secs_f64();
    unsafe {
        data.as_mut_ptr()
            .add(binding.delta_offset)
            .cast::<f64>()
            .write_unaligned(delta_secs);
        data.as_mut_ptr()
            .add(binding.elapsed_offset)
            .cast::<f64>()
            .write_unaligned(elapsed_secs);
    }
}
