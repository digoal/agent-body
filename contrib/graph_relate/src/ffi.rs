//! `librelate_ffi` v2 动态加载与安全 Rust 封装（ADR-0502）
//!
//! 采用运行期 `dlopen` + `dlsym` 绑定 `/usr/local/lib/librelate_ffi.so`，
//! 避免将上游 20+ Rust crate 直接耦合进 `agent-body` 主编译图；
//! 当宿主机未安装共享库时编译零阻断、运行期可平滑降级。

use std::collections::HashMap;
use std::ffi::{c_void, CStr, CString};
use std::os::raw::{c_char, c_double, c_int, c_longlong};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{Map, Number, Value};

pub const EXPECTED_ABI_VERSION: i32 = 2;
pub const RELATE_OK: c_int = 0;
pub const RELATE_ERROR: c_int = 1;

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelateValueKind {
    Null = 0,
    Bool = 1,
    Int = 2,
    Float = 3,
    String = 4,
    Node = 5,
    Relationship = 6,
    Path = 7,
    List = 8,
    Map = 9,
    Point = 10,
    Date = 11,
    Time = 12,
    LocalTime = 13,
    DateTime = 14,
    LocalDateTime = 15,
    Duration = 16,
    Vector = 17,
}

impl RelateValueKind {
    fn from_raw(raw: c_int) -> Option<Self> {
        match raw {
            0 => Some(Self::Null),
            1 => Some(Self::Bool),
            2 => Some(Self::Int),
            3 => Some(Self::Float),
            4 => Some(Self::String),
            5 => Some(Self::Node),
            6 => Some(Self::Relationship),
            7 => Some(Self::Path),
            8 => Some(Self::List),
            9 => Some(Self::Map),
            10 => Some(Self::Point),
            11 => Some(Self::Date),
            12 => Some(Self::Time),
            13 => Some(Self::LocalTime),
            14 => Some(Self::DateTime),
            15 => Some(Self::LocalDateTime),
            16 => Some(Self::Duration),
            17 => Some(Self::Vector),
            _ => None,
        }
    }
}

pub type RelateDatabasePtr = *mut c_void;
pub type RelateConnectionPtr = *mut c_void;
pub type RelateQueryResultPtr = *mut c_void;
pub type RelateParamsPtr = *mut c_void;
pub type RelateValuePtr = *mut c_void;

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct RelateSymbolTable {
    relate_abi_version: unsafe extern "C" fn() -> c_int,
    relate_free_string: unsafe extern "C" fn(*mut c_char),
    relate_database_open:
        unsafe extern "C" fn(*const c_char, *mut RelateDatabasePtr, *mut *mut c_char) -> c_int,
    relate_database_open_read_only:
        unsafe extern "C" fn(*const c_char, *mut RelateDatabasePtr, *mut *mut c_char) -> c_int,
    relate_database_close: unsafe extern "C" fn(RelateDatabasePtr) -> c_int,
    relate_connect_namespace: unsafe extern "C" fn(
        RelateDatabasePtr,
        *const c_char,
        *mut RelateConnectionPtr,
        *mut *mut c_char,
    ) -> c_int,
    relate_conn_set_namespace:
        unsafe extern "C" fn(RelateConnectionPtr, *const c_char, *mut *mut c_char) -> c_int,
    relate_conn_set_statement_timeout:
        unsafe extern "C" fn(RelateConnectionPtr, u64, *mut *mut c_char) -> c_int,
    relate_disconnect: unsafe extern "C" fn(RelateConnectionPtr) -> c_int,
    relate_query_params: unsafe extern "C" fn(
        RelateConnectionPtr,
        *const c_char,
        RelateParamsPtr,
        *mut RelateQueryResultPtr,
        *mut *mut c_char,
    ) -> c_int,
    relate_conn_begin: unsafe extern "C" fn(RelateConnectionPtr, *mut *mut c_char) -> c_int,
    relate_conn_commit:
        unsafe extern "C" fn(RelateConnectionPtr, *mut *mut c_char, *mut *mut c_char) -> c_int,
    relate_conn_rollback: unsafe extern "C" fn(RelateConnectionPtr, *mut *mut c_char) -> c_int,
    relate_checkpoint: unsafe extern "C" fn(RelateDatabasePtr, *mut *mut c_char) -> c_int,
    relate_destroy_result: unsafe extern "C" fn(RelateQueryResultPtr),

    relate_result_is_error: unsafe extern "C" fn(RelateQueryResultPtr) -> u8,
    relate_result_error_json: unsafe extern "C" fn(RelateQueryResultPtr) -> *const c_char,
    relate_result_bookmark: unsafe extern "C" fn(RelateQueryResultPtr) -> *const c_char,
    relate_result_stats: unsafe extern "C" fn(RelateQueryResultPtr) -> *const c_char,
    relate_result_notifications: unsafe extern "C" fn(RelateQueryResultPtr) -> *const c_char,
    relate_result_elapsed_ms: unsafe extern "C" fn(RelateQueryResultPtr) -> c_double,
    relate_column_count: unsafe extern "C" fn(RelateQueryResultPtr) -> c_longlong,
    relate_column_name: unsafe extern "C" fn(RelateQueryResultPtr, c_longlong) -> *const c_char,
    relate_row_count: unsafe extern "C" fn(RelateQueryResultPtr) -> c_longlong,

    relate_value_at:
        unsafe extern "C" fn(RelateQueryResultPtr, c_longlong, c_longlong) -> RelateValuePtr,
    relate_value_kind: unsafe extern "C" fn(RelateValuePtr) -> c_int,
    relate_value_bool: unsafe extern "C" fn(RelateValuePtr, *mut u8) -> c_int,
    relate_value_int: unsafe extern "C" fn(RelateValuePtr, *mut i64) -> c_int,
    relate_value_float: unsafe extern "C" fn(RelateValuePtr, *mut c_double) -> c_int,
    relate_value_is_nan: unsafe extern "C" fn(RelateValuePtr, *mut u8) -> c_int,
    relate_value_is_inf: unsafe extern "C" fn(RelateValuePtr, *mut u8) -> c_int,
    relate_value_string: unsafe extern "C" fn(RelateValuePtr, *mut usize) -> *const c_char,
    relate_value_element_id: unsafe extern "C" fn(RelateValuePtr) -> *const c_char,
    relate_value_start_element_id: unsafe extern "C" fn(RelateValuePtr) -> *const c_char,
    relate_value_end_element_id: unsafe extern "C" fn(RelateValuePtr) -> *const c_char,
    relate_value_list_len: unsafe extern "C" fn(RelateValuePtr) -> c_longlong,
    relate_value_list_get: unsafe extern "C" fn(RelateValuePtr, c_longlong) -> RelateValuePtr,
    relate_value_map_len: unsafe extern "C" fn(RelateValuePtr) -> c_longlong,
    relate_value_map_key:
        unsafe extern "C" fn(RelateValuePtr, c_longlong, *mut usize) -> *const c_char,
    relate_value_map_get: unsafe extern "C" fn(RelateValuePtr, *const c_char) -> RelateValuePtr,
    relate_value_year: unsafe extern "C" fn(RelateValuePtr, *mut i64) -> c_int,
    relate_value_month: unsafe extern "C" fn(RelateValuePtr, *mut i64) -> c_int,
    relate_value_day: unsafe extern "C" fn(RelateValuePtr, *mut i64) -> c_int,
    relate_value_hour: unsafe extern "C" fn(RelateValuePtr, *mut i64) -> c_int,
    relate_value_minute: unsafe extern "C" fn(RelateValuePtr, *mut i64) -> c_int,
    relate_value_second: unsafe extern "C" fn(RelateValuePtr, *mut i64) -> c_int,
    relate_value_nanosecond: unsafe extern "C" fn(RelateValuePtr, *mut i64) -> c_int,
    relate_value_offset_seconds: unsafe extern "C" fn(RelateValuePtr, *mut i64) -> c_int,
    relate_value_zone: unsafe extern "C" fn(RelateValuePtr) -> *const c_char,
    relate_value_duration:
        unsafe extern "C" fn(RelateValuePtr, *mut i64, *mut i64, *mut i64, *mut i64) -> c_int,
    relate_value_point: unsafe extern "C" fn(
        RelateValuePtr,
        *mut i32,
        *mut i32,
        *mut c_double,
        *mut c_double,
        *mut c_double,
    ) -> c_int,
    relate_value_vector_dtype: unsafe extern "C" fn(RelateValuePtr) -> *const c_char,
    relate_value_vector_dim: unsafe extern "C" fn(RelateValuePtr) -> c_longlong,
    relate_value_vector_float:
        unsafe extern "C" fn(RelateValuePtr, c_longlong, *mut c_double) -> c_int,
    relate_value_vector_int: unsafe extern "C" fn(RelateValuePtr, c_longlong, *mut i64) -> c_int,

    relate_params_new: unsafe extern "C" fn() -> RelateParamsPtr,
    relate_params_free: unsafe extern "C" fn(RelateParamsPtr),
    relate_params_put_null:
        unsafe extern "C" fn(RelateParamsPtr, *const c_char, *mut *mut c_char) -> c_int,
    relate_params_put_bool:
        unsafe extern "C" fn(RelateParamsPtr, *const c_char, u8, *mut *mut c_char) -> c_int,
    relate_params_put_int:
        unsafe extern "C" fn(RelateParamsPtr, *const c_char, i64, *mut *mut c_char) -> c_int,
    relate_params_put_float:
        unsafe extern "C" fn(RelateParamsPtr, *const c_char, c_double, *mut *mut c_char) -> c_int,
    relate_params_put_string: unsafe extern "C" fn(
        RelateParamsPtr,
        *const c_char,
        *const c_char,
        usize,
        *mut *mut c_char,
    ) -> c_int,
    relate_params_put_list: unsafe extern "C" fn(
        RelateParamsPtr,
        *const c_char,
        *mut RelateParamsPtr,
        *mut *mut c_char,
    ) -> c_int,
    relate_params_put_map: unsafe extern "C" fn(
        RelateParamsPtr,
        *const c_char,
        *mut RelateParamsPtr,
        *mut *mut c_char,
    ) -> c_int,
    relate_params_put_vector_float: unsafe extern "C" fn(
        RelateParamsPtr,
        *const c_char,
        *const c_char,
        *const c_double,
        usize,
        *mut *mut c_char,
    ) -> c_int,
}

unsafe fn load_sym<T: Copy>(handle: *mut c_void, name: &str) -> Result<T> {
    let cname = CString::new(name)?;
    let _ = libc::dlerror();
    let sym = libc::dlsym(handle, cname.as_ptr());
    if sym.is_null() {
        let err_ptr = libc::dlerror();
        let err_msg = if err_ptr.is_null() {
            "symbol not found".to_string()
        } else {
            CStr::from_ptr(err_ptr).to_string_lossy().into_owned()
        };
        bail!("dlsym({}) 失败: {}", name, err_msg);
    }
    Ok(std::mem::transmute_copy::<*mut c_void, T>(&sym))
}

impl RelateSymbolTable {
    unsafe fn load(handle: *mut c_void) -> Result<Self> {
        Ok(Self {
            relate_abi_version: load_sym(handle, "relate_abi_version")?,
            relate_free_string: load_sym(handle, "relate_free_string")?,
            relate_database_open: load_sym(handle, "relate_database_open")?,
            relate_database_open_read_only: load_sym(handle, "relate_database_open_read_only")?,
            relate_database_close: load_sym(handle, "relate_database_close")?,
            relate_connect_namespace: load_sym(handle, "relate_connect_namespace")?,
            relate_conn_set_namespace: load_sym(handle, "relate_conn_set_namespace")?,
            relate_conn_set_statement_timeout: load_sym(
                handle,
                "relate_conn_set_statement_timeout",
            )?,
            relate_disconnect: load_sym(handle, "relate_disconnect")?,
            relate_query_params: load_sym(handle, "relate_query_params")?,
            relate_conn_begin: load_sym(handle, "relate_conn_begin")?,
            relate_conn_commit: load_sym(handle, "relate_conn_commit")?,
            relate_conn_rollback: load_sym(handle, "relate_conn_rollback")?,
            relate_checkpoint: load_sym(handle, "relate_checkpoint")?,
            relate_destroy_result: load_sym(handle, "relate_destroy_result")?,

            relate_result_is_error: load_sym(handle, "relate_result_is_error")?,
            relate_result_error_json: load_sym(handle, "relate_result_error_json")?,
            relate_result_bookmark: load_sym(handle, "relate_result_bookmark")?,
            relate_result_stats: load_sym(handle, "relate_result_stats")?,
            relate_result_notifications: load_sym(handle, "relate_result_notifications")?,
            relate_result_elapsed_ms: load_sym(handle, "relate_result_elapsed_ms")?,
            relate_column_count: load_sym(handle, "relate_column_count")?,
            relate_column_name: load_sym(handle, "relate_column_name")?,
            relate_row_count: load_sym(handle, "relate_row_count")?,

            relate_value_at: load_sym(handle, "relate_value_at")?,
            relate_value_kind: load_sym(handle, "relate_value_kind")?,
            relate_value_bool: load_sym(handle, "relate_value_bool")?,
            relate_value_int: load_sym(handle, "relate_value_int")?,
            relate_value_float: load_sym(handle, "relate_value_float")?,
            relate_value_is_nan: load_sym(handle, "relate_value_is_nan")?,
            relate_value_is_inf: load_sym(handle, "relate_value_is_inf")?,
            relate_value_string: load_sym(handle, "relate_value_string")?,
            relate_value_element_id: load_sym(handle, "relate_value_element_id")?,
            relate_value_start_element_id: load_sym(handle, "relate_value_start_element_id")?,
            relate_value_end_element_id: load_sym(handle, "relate_value_end_element_id")?,
            relate_value_list_len: load_sym(handle, "relate_value_list_len")?,
            relate_value_list_get: load_sym(handle, "relate_value_list_get")?,
            relate_value_map_len: load_sym(handle, "relate_value_map_len")?,
            relate_value_map_key: load_sym(handle, "relate_value_map_key")?,
            relate_value_map_get: load_sym(handle, "relate_value_map_get")?,
            relate_value_year: load_sym(handle, "relate_value_year")?,
            relate_value_month: load_sym(handle, "relate_value_month")?,
            relate_value_day: load_sym(handle, "relate_value_day")?,
            relate_value_hour: load_sym(handle, "relate_value_hour")?,
            relate_value_minute: load_sym(handle, "relate_value_minute")?,
            relate_value_second: load_sym(handle, "relate_value_second")?,
            relate_value_nanosecond: load_sym(handle, "relate_value_nanosecond")?,
            relate_value_offset_seconds: load_sym(handle, "relate_value_offset_seconds")?,
            relate_value_zone: load_sym(handle, "relate_value_zone")?,
            relate_value_duration: load_sym(handle, "relate_value_duration")?,
            relate_value_point: load_sym(handle, "relate_value_point")?,
            relate_value_vector_dtype: load_sym(handle, "relate_value_vector_dtype")?,
            relate_value_vector_dim: load_sym(handle, "relate_value_vector_dim")?,
            relate_value_vector_float: load_sym(handle, "relate_value_vector_float")?,
            relate_value_vector_int: load_sym(handle, "relate_value_vector_int")?,

            relate_params_new: load_sym(handle, "relate_params_new")?,
            relate_params_free: load_sym(handle, "relate_params_free")?,
            relate_params_put_null: load_sym(handle, "relate_params_put_null")?,
            relate_params_put_bool: load_sym(handle, "relate_params_put_bool")?,
            relate_params_put_int: load_sym(handle, "relate_params_put_int")?,
            relate_params_put_float: load_sym(handle, "relate_params_put_float")?,
            relate_params_put_string: load_sym(handle, "relate_params_put_string")?,
            relate_params_put_list: load_sym(handle, "relate_params_put_list")?,
            relate_params_put_map: load_sym(handle, "relate_params_put_map")?,
            relate_params_put_vector_float: load_sym(handle, "relate_params_put_vector_float")?,
        })
    }
}

/// 进程内共享库句柄缓存（同一路径只 `dlopen` 一次）
static LIB_CACHE: OnceLock<Mutex<HashMap<String, Arc<RelateFfiLib>>>> = OnceLock::new();

/// 按既定优先级生成 `librelate_ffi.so` 候选路径
pub fn resolve_library_candidates(explicit: Option<&Path>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(p) = explicit {
        if !p.as_os_str().is_empty() {
            candidates.push(p.to_path_buf());
        }
    }
    if let Ok(env_lib) = std::env::var("RELATE_LIB") {
        let trimmed = env_lib.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed);
            if p.is_dir() {
                candidates.push(p.join("librelate_ffi.so"));
            } else {
                candidates.push(p);
            }
        }
    }
    candidates.push(PathBuf::from("/usr/local/lib/librelate_ffi.so"));
    candidates.push(PathBuf::from("/usr/lib/librelate_ffi.so"));
    candidates.push(PathBuf::from("librelate_ffi.so"));
    candidates
}

pub struct RelateFfiLib {
    dl_handle: *mut c_void,
    syms: RelateSymbolTable,
    resolved_path: String,
    abi_version: i32,
}

unsafe impl Send for RelateFfiLib {}
unsafe impl Sync for RelateFfiLib {}

impl Drop for RelateFfiLib {
    fn drop(&mut self) {
        if !self.dl_handle.is_null() {
            unsafe {
                libc::dlclose(self.dl_handle);
            }
            self.dl_handle = ptr::null_mut();
        }
    }
}

impl RelateFfiLib {
    /// 按候选顺序加载 `librelate_ffi.so` 并验证 `relate_abi_version() == 2`
    pub fn load_or_cached(explicit: Option<&Path>) -> Result<Arc<Self>> {
        let candidates = resolve_library_candidates(explicit);
        let cache = LIB_CACHE.get_or_init(|| Mutex::new(HashMap::new()));

        let mut errors = Vec::new();
        for cand in &candidates {
            let cand_str = cand.to_string_lossy().into_owned();
            // 若路径明确存在或为 bare soname，先查缓存
            {
                let guard = cache
                    .lock()
                    .map_err(|_| anyhow!("Relate FFI 库缓存锁已中毒"))?;
                if let Some(cached) = guard.get(&cand_str) {
                    return Ok(Arc::clone(cached));
                }
            }

            // 如果是绝对路径且文件不存在，直接跳过避免无谓 dlopen
            if cand.is_absolute() && !cand.exists() {
                errors.push(format!("{}: 文件不存在", cand_str));
                continue;
            }

            match Self::try_load_path(&cand_str) {
                Ok(lib) => {
                    let arc = Arc::new(lib);
                    let mut guard = cache
                        .lock()
                        .map_err(|_| anyhow!("Relate FFI 库缓存锁已中毒"))?;
                    guard.insert(cand_str, Arc::clone(&arc));
                    return Ok(arc);
                }
                Err(e) => {
                    errors.push(format!("{}: {}", cand_str, e));
                }
            }
        }

        bail!(
            "未能加载 librelate_ffi.so（请确认已构建并安装至 /usr/local/lib/librelate_ffi.so 或设置 RELATE_LIB）: {}",
            errors.join(" | ")
        )
    }

    fn try_load_path(path: &str) -> Result<Self> {
        let cpath = CString::new(path)?;
        let dl_handle = unsafe {
            let _ = libc::dlerror();
            libc::dlopen(cpath.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL)
        };
        if dl_handle.is_null() {
            let err_msg = unsafe {
                let p = libc::dlerror();
                if p.is_null() {
                    "dlopen returned null".to_string()
                } else {
                    CStr::from_ptr(p).to_string_lossy().into_owned()
                }
            };
            bail!("{}", err_msg);
        }

        let syms = match unsafe { RelateSymbolTable::load(dl_handle) } {
            Ok(s) => s,
            Err(e) => {
                unsafe {
                    libc::dlclose(dl_handle);
                }
                return Err(e);
            }
        };

        let abi = unsafe { (syms.relate_abi_version)() };
        if abi != EXPECTED_ABI_VERSION {
            unsafe {
                libc::dlclose(dl_handle);
            }
            bail!(
                "librelate_ffi ABI 版本不匹配: 期望 {}, 实际 {}",
                EXPECTED_ABI_VERSION,
                abi
            );
        }

        Ok(Self {
            dl_handle,
            syms,
            resolved_path: path.to_string(),
            abi_version: abi,
        })
    }

    pub fn resolved_path(&self) -> &str {
        &self.resolved_path
    }

    pub fn abi_version(&self) -> i32 {
        self.abi_version
    }

    /// 消费并释放 FFI 返回的 caller-owned `char *` 字符串
    unsafe fn take_owned_error(&self, err_ptr: *mut c_char) -> String {
        if err_ptr.is_null() {
            return "unknown relate ffi error".to_string();
        }
        let s = CStr::from_ptr(err_ptr).to_string_lossy().into_owned();
        (self.syms.relate_free_string)(err_ptr);
        s
    }

    unsafe fn take_owned_string(&self, ptr: *mut c_char) -> Option<String> {
        if ptr.is_null() {
            return None;
        }
        let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();
        (self.syms.relate_free_string)(ptr);
        Some(s)
    }
}

pub struct RelateDbHandle {
    lib: Arc<RelateFfiLib>,
    raw: RelateDatabasePtr,
}

unsafe impl Send for RelateDbHandle {}
unsafe impl Sync for RelateDbHandle {}

impl Drop for RelateDbHandle {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe {
                let _ = (self.lib.syms.relate_database_close)(self.raw);
            }
            self.raw = ptr::null_mut();
        }
    }
}

impl RelateDbHandle {
    pub fn open(lib: Arc<RelateFfiLib>, uri: &str, read_only: bool) -> Result<Arc<Self>> {
        let curi = CString::new(uri).context("Relate database URI 不能包含 NUL 字节")?;
        let mut out: RelateDatabasePtr = ptr::null_mut();
        let mut err: *mut c_char = ptr::null_mut();

        let status = unsafe {
            if read_only {
                (lib.syms.relate_database_open_read_only)(curi.as_ptr(), &mut out, &mut err)
            } else {
                (lib.syms.relate_database_open)(curi.as_ptr(), &mut out, &mut err)
            }
        };
        if status != RELATE_OK || out.is_null() {
            let msg = unsafe { lib.take_owned_error(err) };
            bail!("打开 Relate 数据库 '{}' 失败: {}", uri, msg);
        }
        if !err.is_null() {
            unsafe { (lib.syms.relate_free_string)(err) };
        }
        Ok(Arc::new(Self { lib, raw: out }))
    }

    pub fn checkpoint(&self) -> Result<()> {
        let mut err: *mut c_char = ptr::null_mut();
        let status = unsafe { (self.lib.syms.relate_checkpoint)(self.raw, &mut err) };
        if status != RELATE_OK {
            let msg = unsafe { self.lib.take_owned_error(err) };
            bail!("Relate checkpoint 失败: {}", msg);
        }
        if !err.is_null() {
            unsafe { (self.lib.syms.relate_free_string)(err) };
        }
        Ok(())
    }

    pub fn lib(&self) -> &RelateFfiLib {
        &self.lib
    }
}

struct RawConn(RelateConnectionPtr);
unsafe impl Send for RawConn {}

pub struct RelateConnHandle {
    db: Arc<RelateDbHandle>,
    conn: Mutex<RawConn>,
}

impl Drop for RelateConnHandle {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.conn.lock() {
            if !guard.0.is_null() {
                unsafe {
                    let _ = (self.db.lib.syms.relate_disconnect)(guard.0);
                }
                guard.0 = ptr::null_mut();
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct RawQueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Map<String, Value>>,
    pub elapsed_ms: f64,
    pub bookmark: Option<String>,
    pub stats: Option<Value>,
    pub notifications: Option<Value>,
}

impl RelateConnHandle {
    pub fn connect(db: Arc<RelateDbHandle>, namespace: Option<&str>) -> Result<Self> {
        let cname = match namespace {
            Some(ns) if !ns.trim().is_empty() => Some(CString::new(ns.trim())?),
            _ => None,
        };
        let ns_ptr = cname.as_ref().map_or(ptr::null(), |s| s.as_ptr());
        let mut out: RelateConnectionPtr = ptr::null_mut();
        let mut err: *mut c_char = ptr::null_mut();

        let status = unsafe {
            (db.lib.syms.relate_connect_namespace)(db.raw, ns_ptr, &mut out, &mut err)
        };
        if status != RELATE_OK || out.is_null() {
            let msg = unsafe { db.lib.take_owned_error(err) };
            bail!("连接 Relate namespace {:?} 失败: {}", namespace, msg);
        }
        if !err.is_null() {
            unsafe { (db.lib.syms.relate_free_string)(err) };
        }
        Ok(Self {
            db,
            conn: Mutex::new(RawConn(out)),
        })
    }

    pub fn set_statement_timeout(&self, timeout_ms: u64) -> Result<()> {
        let guard = self
            .conn
            .lock()
            .map_err(|_| anyhow!("Relate 连接互斥锁已中毒"))?;
        let mut err: *mut c_char = ptr::null_mut();
        let status = unsafe {
            (self.db.lib.syms.relate_conn_set_statement_timeout)(guard.0, timeout_ms, &mut err)
        };
        if status != RELATE_OK {
            let msg = unsafe { self.db.lib.take_owned_error(err) };
            bail!("设置 Relate 查询超时失败: {}", msg);
        }
        if !err.is_null() {
            unsafe { (self.db.lib.syms.relate_free_string)(err) };
        }
        Ok(())
    }

    pub fn begin(&self) -> Result<()> {
        let guard = self
            .conn
            .lock()
            .map_err(|_| anyhow!("Relate 连接互斥锁已中毒"))?;
        let mut err: *mut c_char = ptr::null_mut();
        let status = unsafe { (self.db.lib.syms.relate_conn_begin)(guard.0, &mut err) };
        if status != RELATE_OK {
            let msg = unsafe { self.db.lib.take_owned_error(err) };
            bail!("Relate BEGIN 失败: {}", msg);
        }
        if !err.is_null() {
            unsafe { (self.db.lib.syms.relate_free_string)(err) };
        }
        Ok(())
    }

    pub fn commit(&self) -> Result<Option<String>> {
        let guard = self
            .conn
            .lock()
            .map_err(|_| anyhow!("Relate 连接互斥锁已中毒"))?;
        let mut bookmark_ptr: *mut c_char = ptr::null_mut();
        let mut err: *mut c_char = ptr::null_mut();
        let status = unsafe {
            (self.db.lib.syms.relate_conn_commit)(guard.0, &mut bookmark_ptr, &mut err)
        };
        if status != RELATE_OK {
            if !bookmark_ptr.is_null() {
                unsafe { (self.db.lib.syms.relate_free_string)(bookmark_ptr) };
            }
            let msg = unsafe { self.db.lib.take_owned_error(err) };
            bail!("Relate COMMIT 失败: {}", msg);
        }
        if !err.is_null() {
            unsafe { (self.db.lib.syms.relate_free_string)(err) };
        }
        let bookmark = unsafe { self.db.lib.take_owned_string(bookmark_ptr) };
        Ok(bookmark)
    }

    pub fn rollback(&self) -> Result<()> {
        let guard = self
            .conn
            .lock()
            .map_err(|_| anyhow!("Relate 连接互斥锁已中毒"))?;
        let mut err: *mut c_char = ptr::null_mut();
        let status = unsafe { (self.db.lib.syms.relate_conn_rollback)(guard.0, &mut err) };
        if status != RELATE_OK {
            let msg = unsafe { self.db.lib.take_owned_error(err) };
            bail!("Relate ROLLBACK 失败: {}", msg);
        }
        if !err.is_null() {
            unsafe { (self.db.lib.syms.relate_free_string)(err) };
        }
        Ok(())
    }

    pub fn query(&self, cypher: &str, params: &Map<String, Value>) -> Result<RawQueryResult> {
        let c_cypher = CString::new(cypher).context("Cypher 语句不能包含 NUL 字节")?;
        let lib = &self.db.lib;
        let params_guard = ParamsBuilderGuard::from_json_map(lib, params)?;

        let guard = self
            .conn
            .lock()
            .map_err(|_| anyhow!("Relate 连接互斥锁已中毒"))?;

        let mut res_ptr: RelateQueryResultPtr = ptr::null_mut();
        let mut err: *mut c_char = ptr::null_mut();
        let status = unsafe {
            (lib.syms.relate_query_params)(
                guard.0,
                c_cypher.as_ptr(),
                params_guard.as_ptr(),
                &mut res_ptr,
                &mut err,
            )
        };

        if status != RELATE_OK || res_ptr.is_null() {
            let msg = unsafe { lib.take_owned_error(err) };
            bail!("Relate 连接级查询失败: {}", msg);
        }
        if !err.is_null() {
            unsafe { (lib.syms.relate_free_string)(err) };
        }

        let _res_guard = ResultDestroyGuard {
            lib: Arc::clone(lib),
            ptr: res_ptr,
        };

        let is_err = unsafe { (lib.syms.relate_result_is_error)(res_ptr) };
        if is_err != 0 {
            let err_json_ptr = unsafe { (lib.syms.relate_result_error_json)(res_ptr) };
            let err_json = if err_json_ptr.is_null() {
                "{\"message\":\"statement failed\"}".to_string()
            } else {
                unsafe { CStr::from_ptr(err_json_ptr) }
                    .to_string_lossy()
                    .into_owned()
            };
            bail!("Cypher 语句执行失败: {}", err_json);
        }

        let col_count = unsafe { (lib.syms.relate_column_count)(res_ptr) }.max(0) as usize;
        let row_count = unsafe { (lib.syms.relate_row_count)(res_ptr) }.max(0) as usize;
        let elapsed_ms = unsafe { (lib.syms.relate_result_elapsed_ms)(res_ptr) };

        let bookmark = unsafe {
            let p = (lib.syms.relate_result_bookmark)(res_ptr);
            if p.is_null() {
                None
            } else {
                Some(CStr::from_ptr(p).to_string_lossy().into_owned())
            }
        };
        let stats = unsafe {
            let p = (lib.syms.relate_result_stats)(res_ptr);
            if p.is_null() {
                None
            } else {
                serde_json::from_str::<Value>(&CStr::from_ptr(p).to_string_lossy()).ok()
            }
        };
        let notifications = unsafe {
            let p = (lib.syms.relate_result_notifications)(res_ptr);
            if p.is_null() {
                None
            } else {
                serde_json::from_str::<Value>(&CStr::from_ptr(p).to_string_lossy()).ok()
            }
        };

        let mut columns = Vec::with_capacity(col_count);
        for c in 0..col_count {
            let name = unsafe {
                let p = (lib.syms.relate_column_name)(res_ptr, c as c_longlong);
                if p.is_null() {
                    format!("col_{}", c)
                } else {
                    CStr::from_ptr(p).to_string_lossy().into_owned()
                }
            };
            columns.push(name);
        }

        let mut rows = Vec::with_capacity(row_count);
        for r in 0..row_count {
            let mut row_map = Map::with_capacity(col_count);
            for (c, col_name) in columns.iter().enumerate() {
                let val_ptr =
                    unsafe { (lib.syms.relate_value_at)(res_ptr, r as c_longlong, c as c_longlong) };
                let json_val = unsafe { value_to_json(lib, val_ptr) };
                row_map.insert(col_name.clone(), json_val);
            }
            rows.push(row_map);
        }

        Ok(RawQueryResult {
            columns,
            rows,
            elapsed_ms,
            bookmark,
            stats,
            notifications,
        })
    }
}

struct ResultDestroyGuard {
    lib: Arc<RelateFfiLib>,
    ptr: RelateQueryResultPtr,
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

struct ParamsBuilderGuard {
    lib: Arc<RelateFfiLib>,
    root: RelateParamsPtr,
}

impl ParamsBuilderGuard {
    fn from_json_map(lib: &Arc<RelateFfiLib>, map: &Map<String, Value>) -> Result<Self> {
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

    fn as_ptr(&self) -> RelateParamsPtr {
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
                // relate.h: LIST 容器追加元素必须传 NULL 作为 name
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
            let (mut y, mut m, mut d) = (0i64, 0i64, 0i64);
            let _ = (lib.syms.relate_value_year)(v, &mut y);
            let _ = (lib.syms.relate_value_month)(v, &mut m);
            let _ = (lib.syms.relate_value_day)(v, &mut d);
            Value::String(format!("{:04}-{:02}-{:02}", y, m, d))
        }
        RelateValueKind::Time | RelateValueKind::LocalTime => {
            let (mut h, mut min, mut s, mut ns, mut off) = (0i64, 0i64, 0i64, 0i64, 0i64);
            let _ = (lib.syms.relate_value_hour)(v, &mut h);
            let _ = (lib.syms.relate_value_minute)(v, &mut min);
            let _ = (lib.syms.relate_value_second)(v, &mut s);
            let _ = (lib.syms.relate_value_nanosecond)(v, &mut ns);
            if kind == RelateValueKind::Time {
                let _ = (lib.syms.relate_value_offset_seconds)(v, &mut off);
                Value::String(format!("{:02}:{:02}:{:02}.{:09}{:+05}", h, min, s, ns, off))
            } else {
                Value::String(format!("{:02}:{:02}:{:02}.{:09}", h, min, s, ns))
            }
        }
        RelateValueKind::DateTime | RelateValueKind::LocalDateTime => {
            let (mut y, mut mo, mut d) = (0i64, 0i64, 0i64);
            let (mut h, mut min, mut s, mut ns) = (0i64, 0i64, 0i64, 0i64);
            let _ = (lib.syms.relate_value_year)(v, &mut y);
            let _ = (lib.syms.relate_value_month)(v, &mut mo);
            let _ = (lib.syms.relate_value_day)(v, &mut d);
            let _ = (lib.syms.relate_value_hour)(v, &mut h);
            let _ = (lib.syms.relate_value_minute)(v, &mut min);
            let _ = (lib.syms.relate_value_second)(v, &mut s);
            let _ = (lib.syms.relate_value_nanosecond)(v, &mut ns);
            if kind == RelateValueKind::DateTime {
                let mut off = 0i64;
                let _ = (lib.syms.relate_value_offset_seconds)(v, &mut off);
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
