use bevy::{ecs::component::ComponentId, prelude::*, ptr::OwningPtr};
use bumpalo::Bump;
use mluau::prelude::*;

use crate::fields::{read_field, write_field};
use crate::pool::EngineStringPool;
use crate::schema::SchemaRegistry;

pub struct DynamicComponentBridge;

impl DynamicComponentBridge {
    /// # Safety
    /// # Panics
    /// # Errors
    pub unsafe fn insert_from_table(
        world: &mut World,
        entity: Entity,
        component_id: ComponentId,
        registry: &SchemaRegistry,
        pool: &mut EngineStringPool,
        table: &LuaTable,
        lua: &Lua,
        bump: &Bump,
    ) -> LuaResult<()> {
        let schema = registry
            .id_to_schema
            .get(&component_id)
            .expect("schema not registered");
        let layout = schema.layout;

        let ptr = bump.alloc_layout(layout);
        unsafe {
            std::ptr::copy_nonoverlapping(
                schema.default_template.as_ptr(),
                ptr.as_ptr(),
                layout.size(),
            );
        }

        for &(spur, offset, ft) in &schema.fields {
            let value = {
                let lua_str = pool.get_lua_str(spur);
                table.raw_get::<LuaValue>(lua_str)?
            };
            if matches!(value, LuaValue::Nil) {
                continue;
            }
            let field_ptr = unsafe { ptr.as_ptr().add(offset) };
            unsafe { write_field(field_ptr, &value, ft, pool, lua)? };
        }

        let owning = unsafe { OwningPtr::new(ptr) };
        unsafe { world.entity_mut(entity).insert_by_id(component_id, owning) };
        Ok(())
    }

    /// # Safety
    /// # Panics
    pub unsafe fn insert_default(
        world: &mut World,
        entity: Entity,
        component_id: ComponentId,
        registry: &SchemaRegistry,
        bump: &Bump,
    ) {
        let schema = registry
            .id_to_schema
            .get(&component_id)
            .expect("schema not registered");
        let layout = schema.layout;

        let ptr = bump.alloc_layout(layout);
        unsafe {
            std::ptr::copy_nonoverlapping(
                schema.default_template.as_ptr(),
                ptr.as_ptr(),
                layout.size(),
            );
        }

        let owning = unsafe { OwningPtr::new(ptr) };
        unsafe { world.entity_mut(entity).insert_by_id(component_id, owning) };
    }

    /// # Safety
    /// # Errors
    pub unsafe fn extract_to_table(
        world: &World,
        entity: Entity,
        component_id: ComponentId,
        registry: &SchemaRegistry,
        pool: &EngineStringPool,
        lua: &Lua,
    ) -> LuaResult<Option<LuaTable>> {
        let Some(schema) = registry.id_to_schema.get(&component_id) else {
            return Ok(None);
        };
        let Ok(ptr) = world.entity(entity).get_by_id(component_id) else {
            return Ok(None);
        };

        let raw = ptr.as_ptr();
        let table = lua.create_table()?;

        for &(spur, offset, ft) in &schema.fields {
            let field_ptr = unsafe { raw.add(offset) };
            let value = unsafe { read_field(field_ptr, ft, pool, lua)? };
            let lua_str = pool.get_lua_str(spur);
            table.raw_set(lua_str, value)?;
        }

        Ok(Some(table))
    }
}
