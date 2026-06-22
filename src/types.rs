use bevy::prelude::Entity;
use lasso::Spur;
use mluau::prelude::*;
use std::alloc::Layout;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LuauFieldType {
    Bool,
    Integer,
    Number,
    Vector4,
    String,
    Buffer(usize),
}

impl LuauFieldType {
    /// # Panics
    #[must_use]
    pub fn layout(self) -> Layout {
        match self {
            Self::Bool => Layout::new::<bool>(),
            Self::Integer => Layout::new::<i64>(),
            Self::Number => Layout::new::<f64>(),
            Self::Vector4 => Layout::new::<[f32; 4]>(),
            Self::String => Layout::new::<Spur>(),
            Self::Buffer(n) => Layout::array::<u8>(n).unwrap(),
        }
    }
}

pub(crate) fn infer_field_type(value: &LuaValue) -> LuaResult<LuauFieldType> {
    match value {
        LuaValue::Boolean(_) => Ok(LuauFieldType::Bool),
        LuaValue::Integer(_) => Ok(LuauFieldType::Integer),
        LuaValue::Number(_) => Ok(LuauFieldType::Number),
        LuaValue::Vector(_) => Ok(LuauFieldType::Vector4),
        LuaValue::String(_) => Ok(LuauFieldType::String),
        LuaValue::Buffer(b) => Ok(LuauFieldType::Buffer(b.len())),
        other => Err(LuaError::runtime(format!(
            "cannot infer field type from '{}'",
            other.type_name()
        ))),
    }
}

#[derive(Clone, Copy)]
pub enum LuaSchedule {
    Startup,
    Update,
}

pub(crate) const fn align_up(offset: usize, align: usize) -> usize {
    (offset + align - 1) & !(align - 1)
}

#[derive(Clone, Copy)]
pub struct LuaEntityHandle(pub Entity);

impl LuaUserData for LuaEntityHandle {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(LuaMetaMethod::Eq, |_, this, other: Self| {
            Ok(this.0 == other.0)
        });
        methods.add_meta_method(LuaMetaMethod::ToString, |lua, this, ()| {
            lua.create_string(format!("{:?}", this.0))
        });
    }
}

impl FromLua for LuaEntityHandle {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        match value {
            LuaValue::UserData(ud) => Ok(*ud.borrow::<Self>()?),
            _ => Err(LuaError::FromLuaConversionError {
                from: value.type_name(),
                to: "Entity".to_string(),
                message: Some("expected an entity handle".to_string()),
            }),
        }
    }
}
