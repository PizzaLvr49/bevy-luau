use lasso::Spur;
use mluau::prelude::*;

use crate::pool::EngineStringPool;
use crate::types::LuauFieldType;

/// # Safety
/// # Errors
pub(crate) unsafe fn write_field(
    field_ptr: *mut u8,
    value: &LuaValue,
    ft: LuauFieldType,
    pool: &mut EngineStringPool,
    lua: &Lua,
) -> LuaResult<()> {
    match (value, ft) {
        (LuaValue::Boolean(b), LuauFieldType::Bool) => unsafe {
            field_ptr.cast::<bool>().write_unaligned(*b);
        },
        (LuaValue::Integer(i), LuauFieldType::Integer) => unsafe {
            field_ptr.cast::<i64>().write_unaligned(*i);
        },
        (LuaValue::Number(n), LuauFieldType::Number) => unsafe {
            field_ptr.cast::<f64>().write_unaligned(*n);
        },
        (LuaValue::Vector(v), LuauFieldType::Vector4) => unsafe {
            field_ptr
                .cast::<[f32; 4]>()
                .write_unaligned([v.x(), v.y(), v.z(), v.w()]);
        },
        (LuaValue::String(s), LuauFieldType::String) => {
            let spur = pool.intern(lua, s.to_str()?.as_ref());
            unsafe { field_ptr.cast::<Spur>().write_unaligned(spur) };
        }
        (LuaValue::Buffer(b), LuauFieldType::Buffer(len)) => {
            b.with_bytes(|bytes| {
                let n = bytes.len().min(len);
                unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), field_ptr, n) };
            });
        }
        _ => {}
    }
    Ok(())
}

/// # Safety
/// # Errors
pub(crate) unsafe fn read_field(
    field_ptr: *const u8,
    ft: LuauFieldType,
    pool: &EngineStringPool,
    lua: &Lua,
) -> LuaResult<LuaValue> {
    Ok(match ft {
        LuauFieldType::Bool => LuaValue::Boolean(unsafe { field_ptr.cast::<bool>().read() }),
        LuauFieldType::Integer => {
            LuaValue::Integer(unsafe { field_ptr.cast::<i64>().read_unaligned() })
        }
        LuauFieldType::Number => {
            LuaValue::Number(unsafe { field_ptr.cast::<f64>().read_unaligned() })
        }
        LuauFieldType::Vector4 => {
            let v = unsafe { field_ptr.cast::<[f32; 4]>().read_unaligned() };
            LuaValue::Vector(mluau::Vector::new(v[0], v[1], v[2], v[3]))
        }
        LuauFieldType::String => {
            let spur = unsafe { field_ptr.cast::<Spur>().read_unaligned() };
            LuaValue::String(pool.get_lua_str(spur).clone())
        }
        LuauFieldType::Buffer(len) => {
            let slice = unsafe { std::slice::from_raw_parts(field_ptr, len) };
            LuaValue::Buffer(lua.create_buffer(slice)?)
        }
    })
}
