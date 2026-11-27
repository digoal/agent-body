//! 内置纯机制演示扩展（仅用于验证 `body_cat_proc` 四端派生、`HookChain` 与 `RrfMergeExecutor` 闭环）

use std::sync::{Arc, Mutex};

use agent_body_sdk::{
    AuditEnvelope, AutonomyLevel, BodyExtension, CallerIdentity, EmitAuditHook, EvidenceTuple,
    ExtensionKind, ExtensionMeta, ExtensionRegistrar, FieldPathContext, FunctionCallInfo,
    HookRegistration, IndexAmRoutine, IndexCostEstimate, IndexHit, IndexScanDesc, KernelServices,
    MigrationScript, ProcRoutine, ProcSpec, ProcVolatility, RedactVerdict, TableAmRoutine,
    TupleRedactHook, SDK_API_VERSION,
};

/// 仅用于自检的纯微内核演示扩展（0 任何周边业务词）
pub struct KernelDemoExtension {
    pub audit_log: Arc<Mutex<Vec<AuditEnvelope>>>,
}

impl KernelDemoExtension {
    pub fn new() -> Self {
        Self {
            audit_log: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for KernelDemoExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl BodyExtension for KernelDemoExtension {
    fn meta(&self) -> ExtensionMeta {
        ExtensionMeta {
            ext_id: "kernel_demo".to_string(),
            version: "0.1.0".to_string(),
            kind: ExtensionKind::BuiltinCore,
            min_sdk_version: SDK_API_VERSION.to_string(),
            table_prefix: "ext_kernel_demo__".to_string(),
            requires: vec![],
        }
    }

    fn register(&self, reg: &mut ExtensionRegistrar) -> anyhow::Result<()> {
        reg.register_migration(MigrationScript {
            version: "0.1.0".to_string(),
            upgrade_from: None,
            sql: "CREATE TABLE IF NOT EXISTS ext_kernel_demo__kv_items (
                    item_key TEXT PRIMARY KEY,
                    item_val TEXT NOT NULL
                  );"
            .to_string(),
        });
        reg.register_proc(Arc::new(DemoEchoProc));
        reg.register_hook(HookRegistration::TupleRedact(Arc::new(DemoRedactHook)));
        reg.register_hook(HookRegistration::EmitAudit(Arc::new(DemoAuditSinkHook {
            log: Arc::clone(&self.audit_log),
        })));
        Ok(())
    }
}

struct DemoEchoProc;

impl ProcRoutine for DemoEchoProc {
    fn spec(&self) -> ProcSpec {
        ProcSpec {
            proc_name: "kernel_demo_echo",
            cli_path: &["demo", "echo"],
            rest_route: ("POST", "/api/v1/demo/echo"),
            profiles: &["minimal"],
            volatility: ProcVolatility::Immutable,
            autonomy_level: AutonomyLevel::L0Observe,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "msg": { "type": "string" },
                    "secret": { "type": "string" }
                }
            }),
            provenance_rule: None,
        }
    }

    fn invoke(
        &self,
        fcinfo: FunctionCallInfo,
        ctx: &KernelServices,
    ) -> anyhow::Result<serde_json::Value> {
        let msg = fcinfo
            .args
            .get("msg")
            .and_then(|v| v.as_str())
            .unwrap_or("pong")
            .to_string();
        let secret = fcinfo
            .args
            .get("secret")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        ctx.table_am()
            .kv_put("kernel_demo", "last_trace", &serde_json::json!(fcinfo.trace_id))?;

        let mut out = serde_json::Map::new();
        out.insert("echo".to_string(), serde_json::Value::String(msg));
        out.insert(
            "caller".to_string(),
            serde_json::Value::String(fcinfo.caller.caller_id),
        );
        if let Some(sec) = secret {
            out.insert("secret".to_string(), serde_json::Value::String(sec));
        }
        Ok(serde_json::Value::Object(out))
    }
}

struct DemoRedactHook;

impl TupleRedactHook for DemoRedactHook {
    fn hook_id(&self) -> &'static str {
        "kernel_demo:redact_secret"
    }

    fn redact(
        &self,
        value: &mut serde_json::Value,
        _path_ctx: &FieldPathContext,
        _caller: &CallerIdentity,
    ) -> anyhow::Result<RedactVerdict> {
        if let Some(obj) = value.as_object_mut() {
            if obj.contains_key("secret") {
                obj.insert("secret".to_string(), serde_json::json!("***"));
                return Ok(RedactVerdict::Redacted);
            }
        }
        Ok(RedactVerdict::Unchanged)
    }
}

struct DemoAuditSinkHook {
    log: Arc<Mutex<Vec<AuditEnvelope>>>,
}

impl EmitAuditHook for DemoAuditSinkHook {
    fn hook_id(&self) -> &'static str {
        "kernel_demo:audit_sink"
    }

    fn emit(&self, envelope: &AuditEnvelope) -> anyhow::Result<()> {
        if let Ok(mut guard) = self.log.lock() {
            guard.push(envelope.clone());
        }
        Ok(())
    }
}

/// 纯机制演示索引通道，用于验证 RRF 权重重归一化
pub struct DemoIndexChannel {
    pub id: &'static str,
    pub weight: f64,
    pub available: bool,
    pub hits: Vec<IndexHit>,
}

impl IndexAmRoutine for DemoIndexChannel {
    fn am_id(&self) -> &'static str {
        self.id
    }

    fn default_weight(&self) -> f64 {
        self.weight
    }

    fn amcostestimate(&self, _scan: &IndexScanDesc) -> IndexCostEstimate {
        IndexCostEstimate {
            available: self.available,
            est_latency_us: 50,
            est_selectivity: 0.2,
        }
    }

    fn amgetmulti(
        &self,
        _scan: &IndexScanDesc,
        _am: &dyn TableAmRoutine,
    ) -> anyhow::Result<Vec<IndexHit>> {
        Ok(self.hits.clone())
    }

    fn aminsert(&self, _entity: &EvidenceTuple) -> anyhow::Result<()> {
        Ok(())
    }

    fn amdelete_by_source(&self, _source_key: &str) -> anyhow::Result<usize> {
        Ok(0)
    }

    fn amvacuum(&self) -> anyhow::Result<usize> {
        Ok(0)
    }
}
