//! `librelate_ffi` v2 参数构造与返回值解码辅助模块（从 `ffi.rs` 拆分以维持 `<1000L` 模块化红线）

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde_json::{Map, Number, Value};

use crate::ffi::{
    RelateFfiLib, RelateParamsPtr, RelateQueryResultPtr, RelateValueKind, RelateValuePtr,
    RELATE_OK,
};

pub(crate) struct ResultDestroyGuard {
    pub(crate) lib: Arc<RelateFfiLib>,
    pub(crate) ptr: RelateQueryResultPtr,
}

impl Drop for ResultDestroyGuard {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                (self.lib.syms.relate_destroy_result)(self.ptr);
            }
            self.ptr = ptr::null_mut();
        }
    }
}

pub(crate) struct ParamsBuilderGuard {
    lib: Arc<RelateFfiLib>,
    root: RelateParamsPtr,
}

impl ParamsBuilderGuard {
    pub(crate) fn from_json_map(lib: &Arc<RelateFfiLib>, map: &Map<String, Value>) -> Result<Self> {
        if map.is_empty() {
            return Ok(Self {
                lib: Arc::clone(lib),
                root: ptr::null_mut(),
            });
        }
        let root = unsafe { (lib.syms.relate_params_new)() };
        if root.is_null() {
            bail!("relate_params_new 返回空指针");
        }
        let guard = Self {
            lib: Arc::clone(lib),
            root,
        };
        for (k, v) in map {
            let ck = CString::new(k.as_str()).context("参数名不能包含 NUL 字节")?;
            unsafe { put_json_value(lib, guard.root, ck.as_ptr(), v)? };
        }
        Ok(guard)
    }

    pub(crate) fn as_ptr(&self) -> RelateParamsPtr {
        self.root
    }
}

impl Drop for ParamsBuilderGuard {
    fn drop(&mut self) {
        if !self.root.is_null() {
            unsafe {
                (self.lib.syms.relate_params_free)(self.root);
            }
            self.root = ptr::null_mut();
        }
    }
}

/// 将 JSON 值递归写入 `relate_params`（对 MAP 传入非空 key 指针，对 LIST 传入 `ptr::null()`）
unsafe fn put_json_value(
    lib: &RelateFfiLib,
    builder: RelateParamsPtr,
    name_ptr: *const c_char,
    val: &Value,
) -> Result<()> {
    let mut err: *mut c_char = ptr::null_mut();
    let status = match val {
        Value::Null => (lib.syms.relate_params_put_null)(builder, name_ptr, &mut err),
        Value::Bool(b) => (lib.syms.relate_params_put_bool)(builder, name_ptr, u8::from(*b), &mut err),
        Value::Number(num) => {
            if let Some(i) = num.as_i64() {
                (lib.syms.relate_params_put_int)(builder, name_ptr, i, &mut err)
            } else if let Some(u) = num.as_u64() {
                if let Ok(i) = i64::try_from(u) {
                    (lib.syms.relate_params_put_int)(builder, name_ptr, i, &mut err)
                } else {
                    (lib.syms.relate_params_put_float)(builder, name_ptr, u as f64, &mut err)
                }
            } else if let Some(f) = num.as_f64() {
                (lib.syms.relate_params_put_float)(builder, name_ptr, f, &mut err)
            } else {
                bail!("不支持的 JSON 数值类型: {}", num);
            }
        }
        Value::String(s) => {
            let bytes = s.as_bytes();
            let ptr = if bytes.is_empty() {
                ptr::null()
            } else {
                bytes.as_ptr() as *const c_char
            };
            (lib.syms.relate_params_put_string)(builder, name_ptr, ptr, bytes.len(), &mut err)
        }
        Value::Array(items) => {
            let mut child: RelateParamsPtr = ptr::null_mut();
            let st = (lib.syms.relate_params_put_list)(builder, name_ptr, &mut child, &mut err);
            if st != RELATE_OK || child.is_null() {
                let msg = lib.take_owned_error(err);
                bail!("relate_params_put_list 失败: {}", msg);
            }
            if !err.is_null() {
                (lib.syms.relate_free_string)(err);
            }
            for item in items {
                put_json_value(lib, child, ptr::null(), item)?;
            }
            return Ok(());
        }
        Value::Object(obj) => {
            let mut child: RelateParamsPtr = ptr::null_mut();
            let st = (lib.syms.relate_params_put_map)(builder, name_ptr, &mut child, &mut err);
            if st != RELATE_OK || child.is_null() {
                let msg = lib.take_owned_error(err);
                bail!("relate_params_put_map 失败: {}", msg);
            }
            if !err.is_null() {
                (lib.syms.relate_free_string)(err);
            }
            for (k, v) in obj {
                let ck = CString::new(k.as_str()).context("嵌套 Map 键不能包含 NUL 字节")?;
                put_json_value(lib, child, ck.as_ptr(), v)?;
            }
            return Ok(());
        }
    };

    if status != RELATE_OK {
        let msg = lib.take_owned_error(err);
        bail!("relate_params_put_* 失败: {}", msg);
    }
    if !err.is_null() {
        (lib.syms.relate_free_string)(err);
    }
    Ok(())
}

unsafe fn read_owned_bytes_str(ptr: *const c_char, len: usize) -> String {
    if ptr.is_null() || len == 0 {
        return String::new();
    }
    let slice = std::slice::from_raw_parts(ptr as *const u8, len);
    String::from_utf8_lossy(slice).into_owned()
}

unsafe fn read_cstr_opt(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        None
    } else {
        Some(CStr::from_ptr(ptr).to_string_lossy().into_owned())
    }
}

unsafe fn read_i64_field(
    getter: unsafe extern "C" fn(RelateValuePtr, *mut i64) -> c_int,
    v: RelateValuePtr,
) -> i64 {
    let mut out = 0i64;
    if getter(v, &mut out) == RELATE_OK {
        out
    } else {
        0
    }
}

unsafe fn read_map_properties(lib: &RelateFfiLib, v: RelateValuePtr) -> Map<String, Value> {
    let len = (lib.syms.relate_value_map_len)(v);
    if len <= 0 {
        return Map::new();
    }
    let mut map = Map::with_capacity(len as usize);
    for i in 0..len {
        let mut key_len: usize = 0;
        let key_ptr = (lib.syms.relate_value_map_key)(v, i, &mut key_len);
        if key_ptr.is_null() {
            continue;
        }
        let key = read_owned_bytes_str(key_ptr, key_len);
        if let Ok(ckey) = CString::new(key.as_str()) {
            let child = (lib.syms.relate_value_map_get)(v, ckey.as_ptr());
            let val = value_to_json(lib, child);
            map.insert(key, val);
        }
    }
    map
}

/// 将 `relate_value` 完整转换为 `serde_json::Value`
pub unsafe fn value_to_json(lib: &RelateFfiLib, v: RelateValuePtr) -> Value {
    if v.is_null() {
        return Value::Null;
    }
    let kind_raw = (lib.syms.relate_value_kind)(v);
    let Some(kind) = RelateValueKind::from_raw(kind_raw) else {
        return Value::Null;
    };

    match kind {
        RelateValueKind::Null => Value::Null,
        RelateValueKind::Bool => {
            let mut out: u8 = 0;
            if (lib.syms.relate_value_bool)(v, &mut out) == RELATE_OK {
                Value::Bool(out != 0)
            } else {
                Value::Null
            }
        }
        RelateValueKind::Int => {
            let mut out: i64 = 0;
            if (lib.syms.relate_value_int)(v, &mut out) == RELATE_OK {
                Value::Number(Number::from(out))
            } else {
                Value::Null
            }
        }
        RelateValueKind::Float => {
            let mut out: f64 = 0.0;
            if (lib.syms.relate_value_float)(v, &mut out) == RELATE_OK {
                Number::from_f64(out)
                    .map(Value::Number)
                    .unwrap_or_else(|| Value::String(out.to_string()))
            } else {
                Value::Null
            }
        }
        RelateValueKind::String => {
            let mut len: usize = 0;
            let p = (lib.syms.relate_value_string)(v, &mut len);
            Value::String(read_owned_bytes_str(p, len))
        }
        RelateValueKind::Node => {
            let element_id = read_cstr_opt((lib.syms.relate_value_element_id)(v));
            let label_len = (lib.syms.relate_value_list_len)(v).max(0);
            let mut labels = Vec::with_capacity(label_len as usize);
            for i in 0..label_len {
                let item = (lib.syms.relate_value_list_get)(v, i);
                labels.push(value_to_json(lib, item));
            }
            let props = read_map_properties(lib, v);
            let mut obj = Map::new();
            obj.insert(
                "element_id".to_string(),
                element_id.map(Value::String).unwrap_or(Value::Null),
            );
            obj.insert("labels".to_string(), Value::Array(labels));
            obj.insert("properties".to_string(), Value::Object(props));
            Value::Object(obj)
        }
        RelateValueKind::Relationship => {
            let element_id = read_cstr_opt((lib.syms.relate_value_element_id)(v));
            let start_id = read_cstr_opt((lib.syms.relate_value_start_element_id)(v));
            let end_id = read_cstr_opt((lib.syms.relate_value_end_element_id)(v));
            let mut rtype_len: usize = 0;
            let rtype_ptr = (lib.syms.relate_value_string)(v, &mut rtype_len);
            let rel_type = read_owned_bytes_str(rtype_ptr, rtype_len);
            let props = read_map_properties(lib, v);

            let mut obj = Map::new();
            obj.insert(
                "element_id".to_string(),
                element_id.map(Value::String).unwrap_or(Value::Null),
            );
            obj.insert("type".to_string(), Value::String(rel_type));
            obj.insert(
                "start_element_id".to_string(),
                start_id.map(Value::String).unwrap_or(Value::Null),
            );
            obj.insert(
                "end_element_id".to_string(),
                end_id.map(Value::String).unwrap_or(Value::Null),
            );
            obj.insert("properties".to_string(), Value::Object(props));
            Value::Object(obj)
        }
        RelateValueKind::Path => {
            let len = (lib.syms.relate_value_list_len)(v).max(0);
            let mut elements = Vec::with_capacity(len as usize);
            for i in 0..len {
                let item = (lib.syms.relate_value_list_get)(v, i);
                elements.push(value_to_json(lib, item));
            }
            let mut obj = Map::new();
            obj.insert("elements".to_string(), Value::Array(elements));
            Value::Object(obj)
        }
        RelateValueKind::List => {
            let len = (lib.syms.relate_value_list_len)(v).max(0);
            let mut arr = Vec::with_capacity(len as usize);
            for i in 0..len {
                let item = (lib.syms.relate_value_list_get)(v, i);
                arr.push(value_to_json(lib, item));
            }
            Value::Array(arr)
        }
        RelateValueKind::Map => Value::Object(read_map_properties(lib, v)),
        RelateValueKind::Point => {
            let (mut srid, mut dim, mut x, mut y, mut z) = (0i32, 0i32, 0.0f64, 0.0f64, 0.0f64);
            if (lib.syms.relate_value_point)(v, &mut srid, &mut dim, &mut x, &mut y, &mut z)
                == RELATE_OK
            {
                let mut obj = Map::new();
                obj.insert("srid".to_string(), Value::Number(Number::from(srid)));
                obj.insert(
                    "x".to_string(),
                    Number::from_f64(x).map(Value::Number).unwrap_or(Value::Null),
                );
                obj.insert(
                    "y".to_string(),
                    Number::from_f64(y).map(Value::Number).unwrap_or(Value::Null),
                );
                if dim == 3 {
                    obj.insert(
                        "z".to_string(),
                        Number::from_f64(z).map(Value::Number).unwrap_or(Value::Null),
                    );
                }
                Value::Object(obj)
            } else {
                Value::Null
            }
        }
        RelateValueKind::Date => {
            let y = read_i64_field(lib.syms.relate_value_year, v);
            let m = read_i64_field(lib.syms.relate_value_month, v);
            let d = read_i64_field(lib.syms.relate_value_day, v);
            Value::String(format!("{:04}-{:02}-{:02}", y, m, d))
        }
        RelateValueKind::Time | RelateValueKind::LocalTime => {
            let h = read_i64_field(lib.syms.relate_value_hour, v);
            let min = read_i64_field(lib.syms.relate_value_minute, v);
            let s = read_i64_field(lib.syms.relate_value_second, v);
            let ns = read_i64_field(lib.syms.relate_value_nanosecond, v);
            if kind == RelateValueKind::Time {
                let off = read_i64_field(lib.syms.relate_value_offset_seconds, v);
                Value::String(format!("{:02}:{:02}:{:02}.{:09}{:+05}", h, min, s, ns, off))
            } else {
                Value::String(format!("{:02}:{:02}:{:02}.{:09}", h, min, s, ns))
            }
        }
        RelateValueKind::DateTime | RelateValueKind::LocalDateTime => {
            let y = read_i64_field(lib.syms.relate_value_year, v);
            let mo = read_i64_field(lib.syms.relate_value_month, v);
            let d = read_i64_field(lib.syms.relate_value_day, v);
            let h = read_i64_field(lib.syms.relate_value_hour, v);
            let min = read_i64_field(lib.syms.relate_value_minute, v);
            let s = read_i64_field(lib.syms.relate_value_second, v);
            let ns = read_i64_field(lib.syms.relate_value_nanosecond, v);
            if kind == RelateValueKind::DateTime {
                let off = read_i64_field(lib.syms.relate_value_offset_seconds, v);
                let zone = read_cstr_opt((lib.syms.relate_value_zone)(v)).unwrap_or_default();
                if zone.is_empty() {
                    Value::String(format!(
                        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}{:+}",
                        y, mo, d, h, min, s, ns, off
                    ))
                } else {
                    Value::String(format!(
                        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}[{}]",
                        y, mo, d, h, min, s, ns, zone
                    ))
                }
            } else {
                Value::String(format!(
                    "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}",
                    y, mo, d, h, min, s, ns
                ))
            }
        }
        RelateValueKind::Duration => {
            let (mut mo, mut d, mut s, mut ns) = (0i64, 0i64, 0i64, 0i64);
            if (lib.syms.relate_value_duration)(v, &mut mo, &mut d, &mut s, &mut ns) == RELATE_OK {
                let mut obj = Map::new();
                obj.insert("months".to_string(), Value::Number(Number::from(mo)));
                obj.insert("days".to_string(), Value::Number(Number::from(d)));
                obj.insert("seconds".to_string(), Value::Number(Number::from(s)));
                obj.insert("nanoseconds".to_string(), Value::Number(Number::from(ns)));
                Value::Object(obj)
            } else {
                Value::Null
            }
        }
        RelateValueKind::Vector => {
            let dtype = read_cstr_opt((lib.syms.relate_value_vector_dtype)(v)).unwrap_or_default();
            let dim = (lib.syms.relate_value_vector_dim)(v).max(0);
            let mut arr = Vec::with_capacity(dim as usize);
            if dtype.starts_with("INTEGER") {
                for i in 0..dim {
                    let mut out = 0i64;
                    if (lib.syms.relate_value_vector_int)(v, i, &mut out) == RELATE_OK {
                        arr.push(Value::Number(Number::from(out)));
                    }
                }
            } else {
                for i in 0..dim {
                    let mut out = 0.0f64;
                    if (lib.syms.relate_value_vector_float)(v, i, &mut out) == RELATE_OK {
                        arr.push(
                            Number::from_f64(out)
                                .map(Value::Number)
                                .unwrap_or(Value::Null),
                        );
                    }
                }
            }
            Value::Array(arr)
        }
    }
}
