//! 单点四端自动派发外壳 (`KernelDispatcher`)：MCP JSON-RPC 2.0 / CLI / HTTP REST + 全链路 7 Hook 包裹

mod pipeline;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use agent_body_protocol::{RpcError, RpcId, RpcRequest, RpcResponse, JSONRPC_VERSION};
use agent_body_sdk::{
    derive_aopcc_mode, AccessOp, AopccMode, AutonomyLevel, ClientAuthContext, ExtensionRegistrar,
    ProcRoutine, ProcSpec, ProcVolatility, TableAmRoutine, TransportKind,
};
use anyhow::bail;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::hook_chain::HookChain;
use crate::wait_events::KernelWaitRegistry;
use pipeline::{
    emit_kernel_audit, execute_proc_pipeline, parse_cli_tail_args, query_catalog_extensions,
    run_auth_gate, run_object_access_gate,
};

/// HTTP REST 请求入参抽象（与具体 HTTP 传输框架解耦）
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub bearer_token: Option<String>,
    pub is_loopback: bool,
    pub body: serde_json::Value,
}

/// HTTP REST 响应输出抽象
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status_code: u16,
    pub body: serde_json::Value,
}

impl HttpResponse {
    pub fn json(status_code: u16, body: serde_json::Value) -> Self {
        Self { status_code, body }
    }
}

/// `/api/v1/parity/matrix` 四端 1:1:1 对齐矩阵单条记录
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParityEntry {
    pub proc_name: String,
    pub cli_path: Vec<String>,
    pub rest_method: String,
    pub rest_path: String,
    pub profiles: Vec<String>,
    pub volatility: ProcVolatility,
    pub autonomy_level: AutonomyLevel,
    pub aopcc_mode: AopccMode,
    pub mcp_enabled: bool,
    pub cli_enabled: bool,
    pub http_enabled: bool,
}

/// 算子派发成功回执
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DispatchSuccess {
    pub proc_name: String,
    pub trace_id: String,
    pub aopcc_mode: AopccMode,
    pub data: serde_json::Value,
}

/// 算子派发失败分类
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DispatchError {
    AuthDenied { status_code: u16, reason: String },
    Forbidden { reason: String },
    RequireConfirm { prompt: String },
    NotFound { message: String },
    InvalidRequest { message: String },
    ExecutionFailed {
        message: String,
        recovery: Option<Box<agent_body_sdk::RecoveryPrescription>>,
        hook_errors: Vec<String>,
    },
}

impl DispatchError {
    pub fn status_code(&self) -> u16 {
        match self {
            Self::AuthDenied { status_code, .. } => *status_code,
            Self::Forbidden { .. } => 403,
            Self::RequireConfirm { .. } => 409,
            Self::NotFound { .. } => 404,
            Self::InvalidRequest { .. } => 400,
            Self::ExecutionFailed { .. } => 500,
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::AuthDenied { reason, .. } | Self::Forbidden { reason } => reason.clone(),
            Self::RequireConfirm { prompt } => prompt.clone(),
            Self::NotFound { message }
            | Self::InvalidRequest { message }
            | Self::ExecutionFailed { message, .. } => message.clone(),
        }
    }
}

/// 基于 `ExtensionRegistrar` / `body_cat_proc` 的单点四端自动派发器
#[derive(Default, Clone)]
pub struct KernelDispatcher {
    procs: BTreeMap<String, Arc<dyn ProcRoutine>>,
}

impl KernelDispatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_from_registrar(&mut self, reg: &ExtensionRegistrar) -> anyhow::Result<()> {
        for proc_routine in &reg.procs {
            self.register_proc(Arc::clone(proc_routine))?;
        }
        Ok(())
    }

    pub fn register_proc(&mut self, proc_routine: Arc<dyn ProcRoutine>) -> anyhow::Result<()> {
        let spec = proc_routine.spec();
        if self.procs.contains_key(spec.proc_name) {
            bail!("算子 '{}' 重复注册", spec.proc_name);
        }
        for existing in self.procs.values() {
            let es = existing.spec();
            if es.rest_route.0.eq_ignore_ascii_case(spec.rest_route.0)
                && es.rest_route.1 == spec.rest_route.1
            {
                bail!(
                    "HTTP 路由 '{} {}' 与已注册算子 '{}' 冲突",
                    spec.rest_route.0,
                    spec.rest_route.1,
                    es.proc_name
                );
            }
        }
        self.procs.insert(spec.proc_name.to_string(), proc_routine);
        Ok(())
    }

    pub fn list_procs(&self) -> Vec<ProcSpec> {
        self.procs.values().map(|p| p.spec()).collect()
    }

    /// 核对内存注册算子与 `body_cat_proc` 目录一致性
    pub fn catalog_proc_names(&self, conn: &Connection) -> anyhow::Result<Vec<String>> {
        let mut stmt = conn.prepare("SELECT proc_name FROM body_cat_proc ORDER BY proc_name ASC")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn parity_matrix(&self) -> Vec<ParityEntry> {
        self.procs
            .values()
            .map(|p| {
                let s = p.spec();
                ParityEntry {
                    proc_name: s.proc_name.to_string(),
                    cli_path: s.cli_path.iter().map(|x| (*x).to_string()).collect(),
                    rest_method: s.rest_route.0.to_ascii_uppercase(),
                    rest_path: s.rest_route.1.to_string(),
                    profiles: s.profiles.iter().map(|x| (*x).to_string()).collect(),
                    volatility: s.volatility,
                    autonomy_level: s.autonomy_level,
                    aopcc_mode: derive_aopcc_mode(s.volatility, s.autonomy_level),
                    mcp_enabled: true,
                    cli_enabled: !s.cli_path.is_empty(),
                    http_enabled: !s.rest_route.1.is_empty(),
                }
            })
            .collect()
    }

    pub fn mcp_tools_list(&self) -> serde_json::Value {
        let tools: Vec<serde_json::Value> = self
            .procs
            .values()
            .map(|p| {
                let s = p.spec();
                serde_json::json!({
                    "name": s.proc_name,
                    "description": format!(
                        "Kernel Proc '{}' ({}, {}, CLI: {})",
                        s.proc_name,
                        s.volatility.as_str(),
                        s.autonomy_level.as_str(),
                        s.cli_path.join(" ")
                    ),
                    "inputSchema": s.input_schema,
                })
            })
            .collect();
        serde_json::json!({ "tools": tools })
    }

    /// 端 1：MCP JSON-RPC 2.0 派发 (`initialize`, `ping`, `tools/list`, `tools/call`)
    pub fn dispatch_mcp_jsonrpc(
        &self,
        raw_req: &serde_json::Value,
        auth_ctx: &ClientAuthContext,
        hooks: &HookChain,
        table_am: &Arc<dyn TableAmRoutine>,
        waits: &KernelWaitRegistry,
    ) -> serde_json::Value {
        let req: RpcRequest = match serde_json::from_value(raw_req.clone()) {
            Ok(r) => r,
            Err(err) => {
                let resp = RpcResponse::err(
                    RpcId::Number(0),
                    RpcError::new(-32600, format!("invalid JSON-RPC request: {err}")),
                );
                return serde_json::to_value(resp).unwrap_or_default();
            }
        };
        if req.jsonrpc != JSONRPC_VERSION {
            let resp = RpcResponse::err(
                req.id,
                RpcError::new(-32600, format!("unsupported jsonrpc version '{}'", req.jsonrpc)),
            );
            return serde_json::to_value(resp).unwrap_or_default();
        }

        let resp = match req.method.as_str() {
            "initialize" => match run_auth_gate(auth_ctx, hooks, waits) {
                Ok(_) => RpcResponse::ok(
                    req.id,
                    serde_json::json!({
                        "protocolVersion": "2024-11-05",
                        "capabilities": { "tools": {} },
                        "serverInfo": {
                            "name": "agent-body-kernel",
                            "version": env!("CARGO_PKG_VERSION"),
                        }
                    }),
                ),
                Err(err) => RpcResponse::err(req.id, RpcError::new(-32001, err.message())),
            },
            "ping" => match run_auth_gate(auth_ctx, hooks, waits) {
                Ok(_) => RpcResponse::ok(req.id, serde_json::json!({})),
                Err(err) => RpcResponse::err(req.id, RpcError::new(-32001, err.message())),
            },
            "tools/list" => match run_auth_gate(auth_ctx, hooks, waits) {
                Ok(caller) => {
                    emit_kernel_audit(
                        hooks,
                        waits,
                        &caller,
                        "mcp_tools_list",
                        "body_cat_proc",
                        "ok",
                        None,
                        "",
                    );
                    RpcResponse::ok(req.id, self.mcp_tools_list())
                }
                Err(err) => RpcResponse::err(req.id, RpcError::new(-32001, err.message())),
            },
            "tools/call" => {
                self.handle_mcp_tool_call(req.id, &req.params, auth_ctx, hooks, table_am, waits)
            }
            other => RpcResponse::err(
                req.id,
                RpcError::new(-32601, format!("method '{other}' not found")),
            ),
        };
        serde_json::to_value(resp).unwrap_or_default()
    }

    fn handle_mcp_tool_call(
        &self,
        id: RpcId,
        params: &serde_json::Value,
        auth_ctx: &ClientAuthContext,
        hooks: &HookChain,
        table_am: &Arc<dyn TableAmRoutine>,
        waits: &KernelWaitRegistry,
    ) -> RpcResponse {
        let Some(name) = params.get("name").and_then(|v| v.as_str()) else {
            return RpcResponse::err(id, RpcError::new(-32602, "missing params.name"));
        };
        let Some(routine) = self.procs.get(name) else {
            return RpcResponse::err(id, RpcError::new(-32601, format!("unknown tool '{name}'")));
        };
        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        match execute_proc_pipeline(routine.as_ref(), args, auth_ctx, hooks, table_am, waits) {
            Ok(ok) => {
                let text = serde_json::to_string(&ok.data).unwrap_or_else(|_| "{}".to_string());
                RpcResponse::ok(
                    id,
                    serde_json::json!({
                        "content": [{ "type": "text", "text": text }],
                        "structuredContent": ok.data,
                        "isError": false
                    }),
                )
            }
            Err(err) => RpcResponse::err(
                id,
                RpcError::new(-(i64::from(err.status_code())), err.message()),
            ),
        }
    }

    /// 端 2：CLI 参数路由匹配派发
    pub fn dispatch_cli(
        &self,
        cli_args: &[String],
        json_args: Option<serde_json::Value>,
        auth_ctx: &ClientAuthContext,
        hooks: &HookChain,
        table_am: &Arc<dyn TableAmRoutine>,
        waits: &KernelWaitRegistry,
    ) -> Result<DispatchSuccess, DispatchError> {
        let mut best_match: Option<(&Arc<dyn ProcRoutine>, usize)> = None;
        for routine in self.procs.values() {
            let spec = routine.spec();
            if spec.cli_path.is_empty() || cli_args.len() < spec.cli_path.len() {
                continue;
            }
            let matched = spec
                .cli_path
                .iter()
                .zip(cli_args.iter())
                .all(|(expected, actual)| expected == actual);
            if matched {
                let len = spec.cli_path.len();
                if best_match.map(|(_, best_len)| len > best_len).unwrap_or(true) {
                    best_match = Some((routine, len));
                }
            }
        }
        let Some((routine, matched_len)) = best_match else {
            return Err(DispatchError::NotFound {
                message: format!("未匹配到 CLI 命令路由: {}", cli_args.join(" ")),
            });
        };

        let tail = &cli_args[matched_len..];
        let mut merged_args = parse_cli_tail_args(tail).map_err(|e| DispatchError::InvalidRequest {
            message: e.to_string(),
        })?;
        if let Some(serde_json::Value::Object(extra)) = json_args {
            if let Some(obj) = merged_args.as_object_mut() {
                for (k, v) in extra {
                    obj.insert(k, v);
                }
            }
        }
        execute_proc_pipeline(
            routine.as_ref(),
            merged_args,
            auth_ctx,
            hooks,
            table_am,
            waits,
        )
    }

    /// 端 3：HTTP REST 路由分发（内置 health / catalog / parity + 已注册 `ProcSpec.rest_route`）
    pub fn dispatch_http(
        &self,
        req: &HttpRequest,
        catalog_conn: Option<&Mutex<Connection>>,
        hooks: &HookChain,
        table_am: &Arc<dyn TableAmRoutine>,
        waits: &KernelWaitRegistry,
    ) -> HttpResponse {
        let is_read_only =
            req.method.eq_ignore_ascii_case("GET") || req.method.eq_ignore_ascii_case("HEAD");
        let auth_ctx = ClientAuthContext {
            transport: TransportKind::HttpRest,
            cert: None,
            bearer_token: req.bearer_token.clone(),
            method: req.method.to_ascii_uppercase(),
            path: req.path.clone(),
            is_loopback: req.is_loopback,
            is_read_only,
        };

        if req.method.eq_ignore_ascii_case("GET") && req.path == "/api/v1/health" {
            return match run_auth_gate(&auth_ctx, hooks, waits) {
                Ok(caller) => {
                    emit_kernel_audit(hooks, waits, &caller, "http_health", "/api/v1/health", "ok", None, "");
                    HttpResponse::json(
                        200,
                        serde_json::json!({
                            "status": "ok",
                            "kernel": "agent-body-kernel",
                            "proc_count": self.procs.len(),
                        }),
                    )
                }
                Err(err) => {
                    HttpResponse::json(err.status_code(), serde_json::json!({ "error": err.message() }))
                }
            };
        }

        if req.method.eq_ignore_ascii_case("GET") && req.path == "/api/v1/catalog/extensions" {
            return match run_auth_gate(&auth_ctx, hooks, waits) {
                Ok(caller) => {
                    if let Err(err) = run_object_access_gate(
                        &caller,
                        AccessOp::Read,
                        "body_cat_extension",
                        None,
                        hooks,
                        waits,
                    ) {
                        return HttpResponse::json(
                            err.status_code(),
                            serde_json::json!({ "error": err.message() }),
                        );
                    }
                    let extensions = query_catalog_extensions(catalog_conn);
                    emit_kernel_audit(
                        hooks,
                        waits,
                        &caller,
                        "http_catalog_extensions",
                        "/api/v1/catalog/extensions",
                        "ok",
                        None,
                        "",
                    );
                    HttpResponse::json(200, serde_json::json!({ "extensions": extensions }))
                }
                Err(err) => {
                    HttpResponse::json(err.status_code(), serde_json::json!({ "error": err.message() }))
                }
            };
        }

        if req.method.eq_ignore_ascii_case("GET") && req.path == "/api/v1/parity/matrix" {
            return match run_auth_gate(&auth_ctx, hooks, waits) {
                Ok(caller) => {
                    if let Err(err) = run_object_access_gate(
                        &caller,
                        AccessOp::Read,
                        "body_cat_proc",
                        None,
                        hooks,
                        waits,
                    ) {
                        return HttpResponse::json(
                            err.status_code(),
                            serde_json::json!({ "error": err.message() }),
                        );
                    }
                    let matrix = self.parity_matrix();
                    emit_kernel_audit(
                        hooks,
                        waits,
                        &caller,
                        "http_parity_matrix",
                        "/api/v1/parity/matrix",
                        "ok",
                        None,
                        "",
                    );
                    HttpResponse::json(
                        200,
                        serde_json::json!({
                            "total": matrix.len(),
                            "matrix": matrix,
                        }),
                    )
                }
                Err(err) => {
                    HttpResponse::json(err.status_code(), serde_json::json!({ "error": err.message() }))
                }
            };
        }

        for routine in self.procs.values() {
            let spec = routine.spec();
            if spec.rest_route.0.eq_ignore_ascii_case(&req.method) && spec.rest_route.1 == req.path
            {
                return match execute_proc_pipeline(
                    routine.as_ref(),
                    req.body.clone(),
                    &auth_ctx,
                    hooks,
                    table_am,
                    waits,
                ) {
                    Ok(ok) => HttpResponse::json(
                        200,
                        serde_json::json!({
                            "proc_name": ok.proc_name,
                            "trace_id": ok.trace_id,
                            "aopcc_mode": ok.aopcc_mode,
                            "data": ok.data,
                        }),
                    ),
                    Err(err) => HttpResponse::json(
                        err.status_code(),
                        serde_json::json!({ "error": err.message() }),
                    ),
                };
            }
        }

        HttpResponse::json(
            404,
            serde_json::json!({ "error": format!("route '{} {}' not found", req.method, req.path) }),
        )
    }
}
