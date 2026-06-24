use bevy::{
    ecs::component::{ComponentCloneBehavior, ComponentDescriptor, ComponentId, StorageType},
    platform::collections::{HashMap, HashSet},
    prelude::*,
};
use lasso::Spur;
use mluau::prelude::*;
use smallvec::SmallVec;
use smol_str::SmolStr;
use std::alloc::Layout;

use crate::fields::{read_field, write_field};
use crate::pool::EngineStringPool;
use crate::types::{LuauFieldType, align_up, infer_field_type};

#[derive(Debug)]
pub struct DynamicComponentSchema {
    pub name: SmolStr,
    pub fields: SmallVec<[(Spur, usize, LuauFieldType); 8]>,
    pub layout: Layout,
    pub default_template: Box<[u8]>,
}

#[derive(Resource, Default)]
pub struct SchemaRegistry {
    pub name_to_id: HashMap<SmolStr, ComponentId>,
    pub id_to_schema: HashMap<ComponentId, DynamicComponentSchema>,
    pub resource_ids: HashSet<ComponentId>,
    pub resource_data: HashMap<ComponentId, Vec<u8>>,
}

impl SchemaRegistry {
    /// # Panics
    /// # Errors
    pub fn build(
        name: SmolStr,
        fields: &[(Spur, LuaValue)],
        pool: &mut EngineStringPool,
        lua: &Lua,
    ) -> LuaResult<(DynamicComponentSchema, ComponentDescriptor)> {
        let mut offset = 0usize;
        let mut field_info = SmallVec::<[(Spur, usize, LuauFieldType); 8]>::new();

        for (spur, value) in fields {
            let ft = infer_field_type(value)?;
            let field_layout = ft.layout();
            offset = align_up(offset, field_layout.align());
            field_info.push((*spur, offset, ft));
            offset += field_layout.size();
        }

        let align = field_info
            .iter()
            .map(|&(_, _, ft)| ft.layout().align())
            .max()
            .unwrap_or(1);
        let size = align_up(offset, align).max(1);
        let layout = Layout::from_size_align(size, align).expect("invalid layout");

        let mut default_template = vec![0u8; size];
        for (i, (_, value)) in fields.iter().enumerate() {
            let (_, field_offset, ft) = field_info[i];
            unsafe {
                write_field(
                    default_template.as_mut_ptr().add(field_offset),
                    value,
                    ft,
                    pool,
                    lua,
                )?;
            }
        }

        let schema = DynamicComponentSchema {
            name: name.clone(),
            fields: field_info,
            layout,
            default_template: default_template.into_boxed_slice(),
        };

        let descriptor = unsafe {
            ComponentDescriptor::new_with_layout(
                name.to_string(),
                StorageType::Table,
                layout,
                None,
                true,
                ComponentCloneBehavior::Ignore,
                None,
            )
        };

        Ok((schema, descriptor))
    }

    pub fn insert(&mut self, id: ComponentId, schema: DynamicComponentSchema) {
        self.name_to_id.insert(schema.name.clone(), id);
        self.id_to_schema.insert(id, schema);
    }

    /// # Errors
    pub fn register_dynamic(
        world: &mut World,
        lua: &Lua,
        pool: &mut EngineStringPool,
        name: SmolStr,
        fields: &[(Spur, LuaValue)],
        is_resource: bool,
    ) -> LuaResult<(ComponentId, SmallVec<[(Spur, usize, LuauFieldType); 8]>)> {
        let (schema, descriptor) = Self::build(name, fields, pool, lua)?;
        let field_offsets = schema.fields.clone();
        let id = world.register_component_with_descriptor(descriptor);

        let mut registry = world.resource_mut::<Self>();
        if is_resource {
            registry.resource_ids.insert(id);
            registry
                .resource_data
                .insert(id, schema.default_template.to_vec());
        }
        registry.insert(id, schema);

        Ok((id, field_offsets))
    }
}

/// # Errors
pub fn extract_resource_table(
    registry: &SchemaRegistry,
    pool: &EngineStringPool,
    lua: &Lua,
    id: ComponentId,
) -> LuaResult<Option<LuaTable>> {
    let Some(data) = registry.resource_data.get(&id) else {
        return Ok(None);
    };
    let Some(schema) = registry.id_to_schema.get(&id) else {
        return Ok(None);
    };

    let table = lua.create_table()?;
    for &(spur, offset, ft) in &schema.fields {
        let field_ptr = unsafe { data.as_ptr().add(offset) };
        let value = unsafe { read_field(field_ptr, ft, pool, lua)? };
        let lua_str = pool.get_lua_str(spur);
        table.raw_set(lua_str, value)?;
    }
    Ok(Some(table))
}

/// # Errors
pub fn writeback_resource_table(
    registry: &mut SchemaRegistry,
    pool: &mut EngineStringPool,
    lua: &Lua,
    id: ComponentId,
    table: &LuaTable,
) -> LuaResult<()> {
    let Some(schema) = registry.id_to_schema.get(&id) else {
        return Ok(());
    };
    let fields = schema.fields.clone();
    let Some(data) = registry.resource_data.get_mut(&id) else {
        return Ok(());
    };
    for (spur, offset, ft) in fields {
        let value = {
            let lua_str = pool.get_lua_str(spur);
            table.raw_get::<LuaValue>(lua_str)?
        };
        if matches!(value, LuaValue::Nil) {
            continue;
        }
        let field_ptr = unsafe { data.as_mut_ptr().add(offset) };
        unsafe { write_field(field_ptr, &value, ft, pool, lua)? };
    }
    Ok(())
}
