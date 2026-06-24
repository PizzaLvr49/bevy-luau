pub mod time;

use bevy::ecs::component::ComponentId;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use lasso::Spur;
use mluau::prelude::*;
use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::pool::EngineStringPool;
use crate::runtime::ScriptingRuntime;
use crate::schema::SchemaRegistry;

#[derive(Resource, Default)]
pub struct NativeRegistry {
    pub by_name: HashMap<Spur, ComponentId>,
}

#[derive(Clone)]
pub struct NativeFields(SmallVec<[(Spur, usize); 4]>);

impl NativeFields {
    /// # Panics
    #[must_use]
    pub fn offset_of(&self, spur: Spur) -> usize {
        self.0
            .iter()
            .find(|(s, _)| *s == spur)
            .map(|&(_, offset)| offset)
            .expect("offset_of called with a spur this resource wasn't registered with")
    }
}

/// # Errors
pub fn register_native_resource(
    world: &mut World,
    lua: &Lua,
    pool: &mut EngineStringPool,
    name: &str,
    fields: &[(&str, LuaValue)],
) -> LuaResult<(ComponentId, NativeFields)> {
    let name_spur = pool.intern(lua, name);

    let interned: SmallVec<[(Spur, LuaValue); 4]> = fields
        .iter()
        .map(|pair| (pool.intern(lua, pair.0), pair.1.clone()))
        .collect();

    let (id, offsets) =
        SchemaRegistry::register_dynamic(world, lua, pool, SmolStr::new(name), &interned, true)?;

    world
        .resource_mut::<NativeRegistry>()
        .by_name
        .insert(name_spur, id);

    let field_offsets: SmallVec<[(Spur, usize); 4]> = offsets
        .into_iter()
        .map(|(spur, offset, _)| (spur, offset))
        .collect();
    Ok((id, NativeFields(field_offsets)))
}

pub trait NativeBuiltin {
    fn register(world: &mut World, lua: &Lua, pool: &mut EngineStringPool);
}

pub trait AppNativeExt {
    fn register_native<T: NativeBuiltin>(&mut self) -> &mut Self;
}

impl AppNativeExt for App {
    /// # Panics
    fn register_native<T: NativeBuiltin>(&mut self) -> &mut Self {
        let world = self.world_mut();
        let runtime = world
            .remove_non_send::<ScriptingRuntime>()
            .expect("ScriptingRuntime must be inserted before registering a native type");
        let mut pool = world
            .remove_non_send::<EngineStringPool>()
            .expect("EngineStringPool must be inserted before registering a native type");

        T::register(world, &runtime.lua, &mut pool);

        world.insert_non_send(runtime);
        world.insert_non_send(pool);
        self
    }
}
