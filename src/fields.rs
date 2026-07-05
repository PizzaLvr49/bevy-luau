use mluau::prelude::*;
use std::alloc::Layout;

use lasso::Spur;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LuauFieldType {
    Bool,
    Integer,
    Number,
    Vector4,
    String,
}

impl LuauFieldType {
    pub(crate) const fn layout(self) -> Layout {
        match self {
            Self::Bool => Layout::new::<bool>(),
            Self::Integer => Layout::new::<mluau::Integer>(),
            Self::Number => Layout::new::<f64>(),
            Self::Vector4 => Layout::new::<[f32; 4]>(),
            Self::String => Layout::new::<Spur>(),
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
        other => Err(LuaError::runtime(format!(
            "cannot infer field type from '{}'",
            other.type_name()
        ))),
    }
}

#[expect(
    unsafe_code,
    reason = "Reading and writing to pointers is inherintly unsafe"
)]
pub(crate) mod ffi {
    use super::LuauFieldType;
    use crate::pool::EngineStringPool;
    use bevy::ptr::{Ptr, PtrMut};
    use lasso::Spur;
    use mluau::prelude::*;

    pub fn read_field(
        ptr: Ptr<'_>,
        offset: usize,
        ft: LuauFieldType,
        pool: &EngineStringPool,
    ) -> LuaValue {
        match ft {
            LuauFieldType::Bool => {
                LuaValue::Boolean(*unsafe { ptr.byte_add(offset).deref::<bool>() })
            }
            LuauFieldType::Integer => {
                LuaValue::Integer(*unsafe { ptr.byte_add(offset).deref::<mluau::Integer>() })
            }
            LuauFieldType::Number => {
                LuaValue::Number(*unsafe { ptr.byte_add(offset).deref::<f64>() })
            }
            LuauFieldType::Vector4 => {
                let v = *unsafe { ptr.byte_add(offset).deref::<[f32; 4]>() };
                LuaValue::Vector(mluau::Vector::new(v[0], v[1], v[2], v[3]))
            }
            LuauFieldType::String => {
                let spur = *unsafe { ptr.byte_add(offset).deref::<Spur>() };
                LuaValue::String(pool.get_lua_str(spur).clone())
            }
        }
    }

    pub fn write_field(
        ptr: PtrMut<'_>,
        offset: usize,
        ft: LuauFieldType,
        value: &LuaValue,
        pool: &mut EngineStringPool,
        lua: &Lua,
    ) -> LuaResult<()> {
        match (ft, value) {
            (LuauFieldType::Bool, LuaValue::Boolean(b)) => {
                *unsafe { ptr.byte_add(offset).deref_mut::<bool>() } = *b;
            }
            (LuauFieldType::Integer, LuaValue::Integer(i)) => {
                *unsafe { ptr.byte_add(offset).deref_mut::<mluau::Integer>() } = *i;
            }
            (LuauFieldType::Number, LuaValue::Number(n)) => {
                *unsafe { ptr.byte_add(offset).deref_mut::<f64>() } = *n;
            }
            (LuauFieldType::Vector4, LuaValue::Vector(v)) => {
                *unsafe { ptr.byte_add(offset).deref_mut::<[f32; 4]>() } =
                    [v.x(), v.y(), v.z(), v.w()];
            }
            (LuauFieldType::String, LuaValue::String(s)) => {
                let spur = pool.intern(lua, s.to_str()?.as_ref());
                *unsafe { ptr.byte_add(offset).deref_mut::<Spur>() } = spur;
            }
            (_, LuaValue::Nil) => {}
            (ft, other) => {
                return Err(LuaError::runtime(format!(
                    "field type mismatch: expected {ft:?}, got '{}'",
                    other.type_name()
                )));
            }
        }
        Ok(())
    }
}
