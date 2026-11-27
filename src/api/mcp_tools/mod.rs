use serde_json::Value;
use anyhow::{bail, Result};
use crate::storage::Storage;


mod body_handlers;
mod core_handlers;
mod infra_handlers;
#[derive(Clone)]
pub struct McpToolRegistry {
    storage: Storage,
    profile: crate::config::McpToolProfile,
}

impl McpToolRegistry {
    pub fn new(storage: Storage) -> Self {
        Self::with_profile(storage, crate::config::McpToolProfile::Full)
    }

    pub fn with_profile(storage: Storage, profile: crate::config::McpToolProfile) -> Self {
        Self { storage, profile }
    }

    /// 列出所有按当前 Profile 暴露的 MCP 工具及其标准 JSON Schema
    pub fn list_tools(&self) -> Vec<Value> {
        crate::api::mcp_schemas::tool_schemas_for_profile(self.profile)
    }


    /// 执行具体工具调用（带生产效能存证与双域审计自动记账）
    pub fn call_tool(&self, name: &str, args: Value) -> Result<String> {
        let t0 = std::time::Instant::now();
        let args_preview = args.clone();
        let res = self.call_tool_inner(name, args);
        let elapsed_us = t0.elapsed().as_micros() as i64;
        if let Ok(ref out) = res {
            crate::benchmark::ledger::EffectivenessLedger::record_mcp_call(
                self.storage.provider().as_ref(),
                name,
                out,
                elapsed_us,
            );
        }
        if name != "body_benchmark" && name != "body_demo_scenario" {
            let outcome = if res.is_ok() { "success" } else { "failed" };
            let target_id = args_preview
                .get("path")
                .or_else(|| args_preview.get("id"))
                .or_else(|| args_preview.get("query"))
                .or_else(|| args_preview.get("resource_id"))
                .or_else(|| args_preview.get("port"))
                .map(|v| v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string()))
                .unwrap_or_default();
            crate::audit::AuditLogger::record_best_effort(
                &self.storage,
                crate::audit::AuditEventInput::in_agent("mcp", "tool_call", name)
                    .with_target("mcp_arg", target_id)
                    .with_outcome(outcome)
                    .with_elapsed_us(elapsed_us)
                    .with_delta(serde_json::json!({
                        "args": args_preview,
                        "ok": res.is_ok(),
                        "error": res.as_ref().err().map(|e| e.to_string()),
                    })),
            );
        }
        res
    }

    fn call_tool_inner(&self, name: &str, args: Value) -> Result<String> {
        if let Some(res) = self.call_core_tool(name, &args) {
            return res;
        }
        if let Some(res) = self.call_infra_tool(name, &args) {
            return res;
        }
        if let Some(res) = self.call_body_tool(name, &args) {
            return res;
        }
        bail!("Unknown tool: {}", name)
    }
}
