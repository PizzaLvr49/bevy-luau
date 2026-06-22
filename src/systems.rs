use std::cell::Cell;
use std::rc::Rc;

use bevy::prelude::*;
use bumpalo::Bump;
use mluau::prelude::*;
use smallvec::SmallVec;

use crate::bridge::DynamicComponentBridge;
use crate::commands::{CommandBuffer, LuaCommandsHandle, TriggerCmd};
use crate::pool::EngineStringPool;
use crate::query::{LuaTime, QuerySnapshot, query_entities, writeback_snapshot};
use crate::runtime::{LuaObserverDescriptor, LuaParam, LuaSystemDescriptor, ScriptingRuntime};
use crate::schema::{SchemaRegistry, extract_resource_table, writeback_resource_table};
use crate::types::{LuaEntityHandle, LuaSchedule};

#[derive(Default)]
pub struct FrameArena(pub Bump);

/// # Panics
pub fn reset_frame_arena(
    mut arena: NonSendMut<FrameArena>,
    runtime: Option<NonSendMut<ScriptingRuntime>>,
) {
    arena.0.reset();

    if let Some(rt) = runtime {
        rt.lua.gc_step().unwrap();
    }
}

/// # Panics
pub fn lua_startup_system(world: &mut World) {
    let arena = world.remove_non_send::<FrameArena>().unwrap();
    let runtime = world.remove_non_send::<ScriptingRuntime>();
    let pool = world.remove_non_send::<EngineStringPool>();

    if let (Some(runtime), Some(mut pool)) = (runtime, pool) {
        let indices: SmallVec<[usize; 4]> = (0..runtime.systems.len())
            .filter(|&i| matches!(runtime.systems[i].schedule, LuaSchedule::Startup))
            .collect();

        for i in indices {
            run_lua_system(
                world,
                &runtime.lua,
                &mut pool,
                &arena.0,
                &runtime.observers,
                &runtime.systems[i],
            );
        }

        world.insert_non_send(runtime);
        world.insert_non_send(pool);
    }
    world.insert_non_send(arena);
}

/// # Panics
pub fn lua_update_system(world: &mut World) {
    let arena = world.remove_non_send::<FrameArena>().unwrap();
    let runtime = world.remove_non_send::<ScriptingRuntime>();
    let pool = world.remove_non_send::<EngineStringPool>();

    if let (Some(runtime), Some(mut pool)) = (runtime, pool) {
        let indices: SmallVec<[usize; 4]> = (0..runtime.systems.len())
            .filter(|&i| matches!(runtime.systems[i].schedule, LuaSchedule::Update))
            .collect();

        for i in indices {
            run_lua_system(
                world,
                &runtime.lua,
                &mut pool,
                &arena.0,
                &runtime.observers,
                &runtime.systems[i],
            );
        }

        world.insert_non_send(runtime);
        world.insert_non_send(pool);
    }
    world.insert_non_send(arena);
}

/// # Panics
pub fn run_lua_system(
    world: &mut World,
    lua: &Lua,
    pool: &mut EngineStringPool,
    bump: &Bump,
    observers: &[LuaObserverDescriptor],
    system: &LuaSystemDescriptor,
) {
    let delta_secs = f64::from(world.resource::<Time>().delta_secs());
    let elapsed_secs = world.resource::<Time>().elapsed().as_secs_f64();

    let mut cmd_buffer = CommandBuffer::default();
    let cmd_ptr = std::ptr::addr_of_mut!(cmd_buffer);
    let scope_valid = Rc::new(Cell::new(true));

    world.resource_scope(|world, mut registry: Mut<SchemaRegistry>| {
        let mut args = SmallVec::<[LuaValue; 8]>::new();

        for param in &system.params {
            let arg: LuaResult<LuaValue> = match param {
                LuaParam::Commands => lua
                    .create_userdata(LuaCommandsHandle::new(cmd_ptr, scope_valid.clone()))
                    .map(LuaValue::UserData),
                LuaParam::Time => lua
                    .create_userdata(LuaTime {
                        delta_secs,
                        elapsed_secs,
                    })
                    .map(LuaValue::UserData),
                LuaParam::Query(desc) => {
                    let entities = query_entities(world, desc);
                    let snap = QuerySnapshot::new(
                        desc.clone(),
                        entities,
                        std::ptr::from_mut::<World>(world),
                        &raw const *registry,
                        std::ptr::from_mut::<EngineStringPool>(pool),
                        scope_valid.clone(),
                    );
                    lua.create_userdata(snap).map(LuaValue::UserData)
                }
                LuaParam::Resource(id) => match extract_resource_table(&registry, pool, lua, *id) {
                    Ok(Some(t)) => Ok(LuaValue::Table(t)),
                    Ok(None) => lua.create_table().map(LuaValue::Table),
                    Err(e) => Err(e),
                },
            };

            match arg {
                Ok(v) => args.push(v),
                Err(e) => {
                    error!("failed to prepare Lua system argument: {e}");
                    return;
                }
            }
        }

        if let Err(e) = system
            .func
            .call::<LuaMultiValue>(LuaMultiValue::from_vec(args.iter().cloned().collect()))
        {
            error!("{e}");
        }

        scope_valid.set(false);

        for (param, val) in system.params.iter().zip(args.iter()) {
            match (param, val) {
                (LuaParam::Query(_), LuaValue::UserData(ud)) => {
                    if let Ok(snap) = ud.borrow::<QuerySnapshot>()
                        && let Err(e) = writeback_snapshot(world, pool, &registry, lua, bump, &snap)
                    {
                        error!("query writeback failed: {e}");
                    }
                }
                (LuaParam::Resource(id), LuaValue::Table(t)) => {
                    if let Err(e) = writeback_resource_table(&mut registry, pool, lua, *id, t) {
                        error!("resource writeback failed: {e}");
                    }
                }
                _ => {}
            }
        }
    });

    flush_commands(world, pool, lua, cmd_buffer, observers, bump);
}

/// # Panics
pub fn run_lua_observer(
    world: &mut World,
    pool: &mut EngineStringPool,
    lua: &Lua,
    bump: &Bump,
    observer: &LuaObserverDescriptor,
    entity: Entity,
    event_data: &LuaTable,
    observers: &[LuaObserverDescriptor],
) {
    let mut cmd_buffer = CommandBuffer::default();
    let cmd_ptr = std::ptr::addr_of_mut!(cmd_buffer);
    let scope_valid = Rc::new(Cell::new(true));

    world.resource_scope(|world, registry: Mut<SchemaRegistry>| {
        let mut args = SmallVec::<[LuaValue; 8]>::new();

        match lua.create_userdata(LuaEntityHandle(entity)) {
            Ok(ud) => args.push(LuaValue::UserData(ud)),
            Err(e) => {
                error!("failed to create entity handle for observer: {e}");
                return;
            }
        }
        args.push(LuaValue::Table(event_data.clone()));

        for param in &observer.params {
            let arg: LuaResult<LuaValue> = match param {
                LuaParam::Commands => lua
                    .create_userdata(LuaCommandsHandle::new(cmd_ptr, scope_valid.clone()))
                    .map(LuaValue::UserData),
                LuaParam::Query(desc) => {
                    let entities = query_entities(world, desc);
                    let snap = QuerySnapshot::new(
                        desc.clone(),
                        entities,
                        std::ptr::from_mut::<World>(world),
                        &raw const *registry,
                        std::ptr::from_mut::<EngineStringPool>(pool),
                        scope_valid.clone(),
                    );
                    lua.create_userdata(snap).map(LuaValue::UserData)
                }
                _ => Ok(LuaValue::Nil),
            };

            match arg {
                Ok(v) => args.push(v),
                Err(e) => {
                    error!("failed to prepare Lua observer argument: {e}");
                    return;
                }
            }
        }

        if let Err(e) = observer
            .func
            .call::<LuaMultiValue>(LuaMultiValue::from_vec(args.iter().cloned().collect()))
        {
            error!("{e}");
        }

        scope_valid.set(false);

        for (param, val) in observer.params.iter().zip(args[2..].iter()) {
            if let (LuaParam::Query(_), LuaValue::UserData(ud)) = (param, val)
                && let Ok(snap) = ud.borrow::<QuerySnapshot>()
                && let Err(e) = writeback_snapshot(world, pool, &registry, lua, bump, &snap)
            {
                error!("query writeback failed: {e}");
            }
        }
    });

    flush_commands(world, pool, lua, cmd_buffer, observers, bump);
}

fn dispatch_trigger(
    world: &mut World,
    pool: &mut EngineStringPool,
    lua: &Lua,
    bump: &Bump,
    trigger: TriggerCmd,
    observers: &[LuaObserverDescriptor],
) {
    let indices: SmallVec<[usize; 4]> = observers
        .iter()
        .enumerate()
        .filter(|(_, o)| o.event_id == trigger.event_id)
        .map(|(i, _)| i)
        .collect();

    for idx in indices {
        run_lua_observer(
            world,
            pool,
            lua,
            bump,
            &observers[idx],
            trigger.entity,
            &trigger.data_table,
            observers,
        );
    }
}

fn flush_commands(
    world: &mut World,
    pool: &mut EngineStringPool,
    lua: &Lua,
    buffer: CommandBuffer,
    observers: &[LuaObserverDescriptor],
    bump: &Bump,
) {
    world.resource_scope(|world, registry: Mut<SchemaRegistry>| {
        for spawn in buffer.spawns {
            let entity = world.spawn_empty().id();
            for (comp_id, data) in spawn.components {
                match data {
                    Some(ref table) => {
                        if let Err(e) = unsafe {
                            DynamicComponentBridge::insert_from_table(
                                world, entity, comp_id, &registry, pool, table, lua, bump,
                            )
                        } {
                            error!("failed to spawn component on entity {entity:?}: {e}");
                        }
                    }
                    None => unsafe {
                        DynamicComponentBridge::insert_default(
                            world, entity, comp_id, &registry, bump,
                        );
                    },
                }
            }
        }
    });

    for entity in buffer.despawns {
        world.despawn(entity);
    }

    for trigger in buffer.triggers {
        dispatch_trigger(world, pool, lua, bump, trigger, observers);
    }
}
