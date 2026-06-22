use std::cell::Cell;
use std::rc::Rc;

use bevy::{
    ecs::{query::QueryBuilder, world::FilteredEntityMut},
    platform::collections::{HashMap, HashSet},
    prelude::*,
};
use bumpalo::Bump;
use mluau::prelude::*;
use smallvec::SmallVec;

use crate::bridge::DynamicComponentBridge;
use crate::pool::EngineStringPool;
use crate::runtime::ResolvedQuery;
use crate::schema::SchemaRegistry;
use crate::types::LuaEntityHandle;

pub struct LuaTime {
    pub delta_secs: f64,
    pub elapsed_secs: f64,
}

impl LuaUserData for LuaTime {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("dt", |_, this, ()| Ok(this.delta_secs));
        methods.add_method("elapsed", |_, this, ()| Ok(this.elapsed_secs));
    }
}

pub(crate) struct ExtractedRow {
    pub mutable_tables: SmallVec<[LuaTable; 4]>,
    pub immutable_tables: SmallVec<[LuaTable; 4]>,
}

pub struct QuerySnapshot {
    pub(crate) desc: ResolvedQuery,
    entities: SmallVec<[Entity; 8]>,
    entity_set: HashSet<Entity>,
    pub(crate) cache: HashMap<Entity, ExtractedRow>,
    world: *mut World,
    registry: *const SchemaRegistry,
    pool: *mut EngineStringPool,
    scope_valid: Rc<Cell<bool>>,
}

impl QuerySnapshot {
    pub(crate) fn new(
        desc: ResolvedQuery,
        entities: SmallVec<[Entity; 8]>,
        world: *mut World,
        registry: *const SchemaRegistry,
        pool: *mut EngineStringPool,
        scope_valid: Rc<Cell<bool>>,
    ) -> Self {
        let entity_set = entities.iter().copied().collect();
        Self {
            desc,
            entities,
            entity_set,
            cache: HashMap::default(),
            world,
            registry,
            pool,
            scope_valid,
        }
    }

    fn check_valid(&self) -> LuaResult<()> {
        if self.scope_valid.get() {
            Ok(())
        } else {
            Err(LuaError::runtime(
                "query handle used outside its originating system/observer call",
            ))
        }
    }

    fn get_or_extract(&mut self, lua: &Lua, entity: Entity) -> LuaResult<Option<&ExtractedRow>> {
        self.check_valid()?;
        if !self.entity_set.contains(&entity) {
            return Ok(None);
        }
        if !self.cache.contains_key(&entity) {
            let world = unsafe { &mut *self.world };
            let registry = unsafe { &*self.registry };
            let pool = unsafe { &mut *self.pool };
            let row = extract_row(world, pool, registry, lua, &self.desc, entity)?;
            self.cache.insert(entity, row);
        }
        Ok(self.cache.get(&entity))
    }
}

impl LuaUserData for QuerySnapshot {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("get", |lua, this, entity: LuaEntityHandle| {
            this.get_or_extract(lua, entity.0)?.map_or_else(
                || Ok(LuaMultiValue::new()),
                |row| {
                    let vals: Vec<LuaValue> = row
                        .mutable_tables
                        .iter()
                        .cloned()
                        .map(LuaValue::Table)
                        .collect();
                    Ok(LuaMultiValue::from_vec(vals))
                },
            )
        });

        methods.add_meta_function(LuaMetaMethod::Iter, |lua, ud: LuaAnyUserData| {
            let entities = {
                let snap = ud.borrow::<Self>()?;
                snap.check_valid()?;
                snap.entities.clone()
            };
            let mut index = 0usize;
            lua.create_function_mut(move |lua, ()| {
                loop {
                    if index >= entities.len() {
                        return Ok(LuaMultiValue::new());
                    }
                    let entity = entities[index];
                    index += 1;
                    let mut snap = ud.borrow_mut::<Self>()?;
                    if let Some(row) = snap.get_or_extract(lua, entity)? {
                        let mut vals = vec![LuaValue::UserData(
                            lua.create_userdata(LuaEntityHandle(entity))?,
                        )];
                        vals.extend(row.mutable_tables.iter().cloned().map(LuaValue::Table));
                        vals.extend(row.immutable_tables.iter().cloned().map(LuaValue::Table));
                        return Ok(LuaMultiValue::from_vec(vals));
                    }
                }
            })
        });
    }
}

pub fn query_entities(world: &mut World, desc: &ResolvedQuery) -> SmallVec<[Entity; 8]> {
    let mut builder = QueryBuilder::<FilteredEntityMut>::new(world);
    for &id in &desc.mutable {
        builder.mut_id(id);
    }
    for &id in &desc.immutable {
        builder.ref_id(id);
    }
    for &id in &desc.with {
        builder.with_id(id);
    }
    for &id in &desc.without {
        builder.without_id(id);
    }
    let mut state = builder.build();
    state.iter_mut(world).map(|e| e.id()).collect()
}

/// # Errors
fn extract_row(
    world: &mut World,
    pool: &mut EngineStringPool,
    registry: &SchemaRegistry,
    lua: &Lua,
    desc: &ResolvedQuery,
    entity: Entity,
) -> LuaResult<ExtractedRow> {
    let mut mutable_tables = SmallVec::new();
    let mut immutable_tables = SmallVec::new();

    for &comp_id in &desc.mutable {
        if let Some(t) = unsafe {
            DynamicComponentBridge::extract_to_table(world, entity, comp_id, registry, pool, lua)?
        } {
            mutable_tables.push(t);
        }
    }
    for &comp_id in &desc.immutable {
        if let Some(t) = unsafe {
            DynamicComponentBridge::extract_to_table(world, entity, comp_id, registry, pool, lua)?
        } {
            immutable_tables.push(t);
        }
    }

    Ok(ExtractedRow {
        mutable_tables,
        immutable_tables,
    })
}

/// # Errors
pub fn writeback_snapshot(
    world: &mut World,
    pool: &mut EngineStringPool,
    registry: &SchemaRegistry,
    lua: &Lua,
    bump: &Bump,
    snapshot: &QuerySnapshot,
) -> LuaResult<()> {
    for (&entity, row) in &snapshot.cache {
        for (&comp_id, table) in snapshot.desc.mutable.iter().zip(&row.mutable_tables) {
            unsafe {
                DynamicComponentBridge::insert_from_table(
                    world, entity, comp_id, registry, pool, table, lua, bump,
                )?;
            }
        }
    }
    Ok(())
}
