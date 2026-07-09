use std::{cell::RefCell, rc::Rc};

use bevy::{
    ecs::{component::ComponentId, schedule::FixedBitSet, world::FilteredEntityMut},
    prelude::*,
};
use mluau::prelude::*;
use smallvec::SmallVec;

use crate::{fields, pool::EngineStringPool, schema::SchemaRegistry};

/// A cached luau query state
#[derive(Component)]
pub struct LuaQueryEntry {
    /// The cached query state
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

pub(crate) fn extract(
    world: &mut World,
    query_entity: Entity,
    lua: &Lua,
    pool: &EngineStringPool,
    schema: &SchemaRegistry,
) -> LuaResult<Vec<(Entity, LuaTable, Rc<RefCell<FixedBitSet>>)>> {
    let mut lqe = world
        .entity_mut(query_entity)
        .take::<LuaQueryEntry>()
        .unwrap();

    let LuaQueryEntry { state, order } = &mut lqe;

    let mut out = Vec::new();

    for filtered in state.iter_mut(world) {
        let table = lua.create_table()?;
        let total_fields: usize = order.iter().map(|&cid| schema.get(cid).len()).sum();
        let dirty = Rc::new(RefCell::new(FixedBitSet::with_capacity(total_fields)));

        for &cid in order.iter() {
            let ptr = filtered.get_by_id(cid).unwrap();
            for &(spur, offset, ft) in schema.get(cid) {
                let key = pool.get_lua_str(spur);
                let value = fields::ffi::read_field(ptr, offset, ft, pool);
                table.raw_set(key.clone(), value)?;
            }
        }

        let meta = lua.create_table()?;
        meta.set(
            "__newindex",
            lua.create_function(move |_, (t, k, v): (LuaTable, LuaValue, LuaValue)| {
                t.raw_set(k, v)?;
                Ok(())
            })?,
        )?;
        table.set_metatable(Some(meta))?;

        out.push((filtered.id(), table, dirty));
    }

    world.entity_mut(query_entity).insert(lqe);

    Ok(out)
}

pub(crate) fn writeback(
    world: &mut World,
    lua: &Lua,
    schema: &SchemaRegistry,
    pool: &mut EngineStringPool,
    rows: Vec<(Entity, LuaTable, Rc<RefCell<FixedBitSet>>)>,
    order: &[ComponentId],
) -> LuaResult<()> {
    for (entity, table, dirty) in rows {
        let dirty = dirty.borrow();
        if dirty.is_clear() {
            continue;
        }

        let mut entity_mut = world.entity_mut(entity);
        let mut field_idx = 0usize;
        for &cid in order {
            let fields = schema.get(cid);
            let any_dirty = (field_idx..field_idx + fields.len()).any(|i| dirty[i]);
            if any_dirty {
                let mut mu = entity_mut.get_mut_by_id(cid).unwrap();
                let mut ptr = mu.as_mut();
                for &(spur, offset, ft) in fields {
                    if dirty[field_idx] {
                        let key = pool.get_lua_str(spur).clone();
                        let value: LuaValue = table.raw_get(key)?;
                        fields::ffi::write_field(ptr.reborrow(), offset, ft, &value, pool, lua)?;
                    }
                    field_idx += 1;
                }
            } else {
                field_idx += fields.len();
            }
        }
    }
    Ok(())
}
