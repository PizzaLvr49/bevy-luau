use std::path::{Path, PathBuf};

use bevy::ecs::component::ComponentId;
use mluau::prelude::*;
use smallvec::SmallVec;

use crate::types::LuaSchedule;

#[derive(Clone, Default)]
pub struct ResolvedQuery {
    pub mutable: SmallVec<[ComponentId; 4]>,
    pub immutable: SmallVec<[ComponentId; 4]>,
    pub with: SmallVec<[ComponentId; 4]>,
    pub without: SmallVec<[ComponentId; 4]>,
}

#[derive(Clone)]
pub enum LuaParam {
    Commands,
    Time,
    Query(ResolvedQuery),
    Resource(ComponentId),
}

pub struct LuaSystemDescriptor {
    pub func: LuaFunction,
    pub schedule: LuaSchedule,
    pub params: SmallVec<[LuaParam; 6]>,
}

pub struct LuaObserverDescriptor {
    pub event_id: ComponentId,
    pub func: LuaFunction,
    pub params: SmallVec<[LuaParam; 6]>,
}

pub struct ScriptingRuntime {
    pub lua: Lua,
    pub systems: Vec<LuaSystemDescriptor>,
    pub observers: Vec<LuaObserverDescriptor>,
}

impl Default for ScriptingRuntime {
    fn default() -> Self {
        Self {
            lua: Lua::new(),
            systems: Vec::new(),
            observers: Vec::new(),
        }
    }
}

pub(crate) struct LuauResolver {
    current_path: PathBuf,
    root: PathBuf,
    entry_point: PathBuf,
}

impl LuauResolver {
    pub fn new(entry_point: PathBuf) -> Self {
        let root = entry_point
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        Self {
            current_path: root.clone(),
            root,
            entry_point,
        }
    }
}

impl LuaRequire for LuauResolver {
    fn is_require_allowed(&self, _chunk_name: &str) -> bool {
        true
    }

    fn reset(&mut self, chunk_name: &str) -> Result<(), LuaNavigateError> {
        let base_path = if chunk_name.is_empty() {
            self.entry_point.clone()
        } else {
            PathBuf::from(chunk_name)
        };
        self.current_path = base_path;
        Ok(())
    }

    fn jump_to_alias(&mut self, _path: &str) -> Result<(), LuaNavigateError> {
        Err(LuaNavigateError::NotFound)
    }

    fn to_parent(&mut self) -> Result<(), LuaNavigateError> {
        if self.current_path.pop() && self.current_path.starts_with(&self.root) {
            Ok(())
        } else {
            Err(LuaNavigateError::NotFound)
        }
    }

    fn to_child(&mut self, name: &str) -> Result<(), LuaNavigateError> {
        self.current_path.push(name);
        Ok(())
    }

    fn has_module(&self) -> bool {
        self.current_path.with_extension("luau").exists()
            || self.current_path.with_extension("lua").exists()
    }

    fn cache_key(&self) -> String {
        self.current_path.to_string_lossy().into_owned()
    }

    fn has_config(&self) -> bool {
        false
    }

    fn config(&self) -> std::io::Result<Vec<u8>> {
        Ok(Vec::new())
    }

    fn loader(&self, lua: &Lua) -> LuaResult<LuaFunction> {
        let file_path = if self.current_path.with_extension("luau").exists() {
            self.current_path.with_extension("luau")
        } else {
            self.current_path.with_extension("lua")
        };

        let source = std::fs::read_to_string(&file_path).map_err(|e| {
            LuaError::RuntimeError(format!(
                "Failed to read module {}: {e}",
                file_path.display()
            ))
        })?;

        lua.load(&source)
            .set_name(file_path.to_string_lossy())
            .into_function()
    }
}
