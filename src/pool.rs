use bevy::{ecs::resource::Resource, platform::collections::HashMap};
use lasso::{Rodeo, Spur};
use mluau::prelude::*;

/// A struct for syncing string interning between luau and rust
#[derive(Default, Resource)]
pub struct EngineStringPool {
    rodeo: Rodeo,
    bridge: HashMap<Spur, LuaString>,
}

impl EngineStringPool {
    /// # Panics
    #[must_use]
    pub fn get_lua_str(&self, spur: Spur) -> &LuaString {
        self.bridge.get(&spur).expect("unregistered spur")
    }

    /// # Panics
    pub fn intern(&mut self, lua: &Lua, s: &str) -> Spur {
        let spur = self.rodeo.get_or_intern(s);
        self.bridge
            .entry(spur)
            .or_insert_with(|| lua.create_string(s).unwrap());
        spur
    }

    /// # Panics
    /// # Errors
    pub fn intern_lua(&mut self, s: LuaString) -> Result<Spur, LuaError> {
        let spur = self.rodeo.get_or_intern(s.to_str()?.as_ref());
        self.bridge.entry(spur).or_insert_with(|| s);
        Ok(spur)
    }
}
