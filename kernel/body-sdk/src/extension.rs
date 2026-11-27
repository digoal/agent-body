//! 扩展元信息、注册器 (`ExtensionRegistrar`) 与 `BodyExtension` Trait

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::hooks::{
    ClientAuthHook, EmitAuditHook, ExecutorEndHook, ExecutorStartHook, FailPolicy,
    ObjectAccessHook, PlannerRewriteHook, TupleRedactHook,
};
use crate::routines::{
    BgWorkerRoutine, FdwRoutine, IndexAmRoutine, OperatorRoutine, ProcRoutine, TableAmRoutine,
    TypeRoutine,
};

/// 扩展交付形态分类
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionKind {
    BuiltinCore,
    CompiledCrate,
    OutOfProcess,
}

impl ExtensionKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::BuiltinCore => "builtin_core",
            Self::CompiledCrate => "compiled_crate",
            Self::OutOfProcess => "out_of_process",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "builtin_core" => Some(Self::BuiltinCore),
            "compiled_crate" => Some(Self::CompiledCrate),
            "out_of_process" => Some(Self::OutOfProcess),
            _ => None,
        }
    }
}

/// 扩展静态描述元信息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionMeta {
    pub ext_id: String,
    pub version: String,
    pub kind: ExtensionKind,
    pub min_sdk_version: String,
    pub table_prefix: String,
    pub requires: Vec<String>,
}

/// 扩展私有表 SQL 迁移脚本描述符
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationScript {
    pub version: String,
    pub upgrade_from: Option<String>,
    pub sql: String,
}

/// 7 大 Hook 注册条目统一枚举
#[derive(Clone)]
pub enum HookRegistration {
    ClientAuth(Arc<dyn ClientAuthHook>),
    ObjectAccess(Arc<dyn ObjectAccessHook>),
    PlannerRewrite(Arc<dyn PlannerRewriteHook>),
    ExecutorStart(Arc<dyn ExecutorStartHook>),
    ExecutorEnd(Arc<dyn ExecutorEndHook>),
    TupleRedact(Arc<dyn TupleRedactHook>),
    EmitAudit(Arc<dyn EmitAuditHook>),
}

impl HookRegistration {
    pub fn hook_point(&self) -> &'static str {
        match self {
            Self::ClientAuth(_) => "client_auth",
            Self::ObjectAccess(_) => "object_access",
            Self::PlannerRewrite(_) => "planner_rewrite",
            Self::ExecutorStart(_) => "executor_start",
            Self::ExecutorEnd(_) => "executor_end",
            Self::TupleRedact(_) => "tuple_redact",
            Self::EmitAudit(_) => "emit_audit",
        }
    }

    pub fn hook_id(&self) -> &'static str {
        match self {
            Self::ClientAuth(h) => h.hook_id(),
            Self::ObjectAccess(h) => h.hook_id(),
            Self::PlannerRewrite(h) => h.hook_id(),
            Self::ExecutorStart(h) => h.hook_id(),
            Self::ExecutorEnd(h) => h.hook_id(),
            Self::TupleRedact(h) => h.hook_id(),
            Self::EmitAudit(h) => h.hook_id(),
        }
    }

    pub fn priority(&self) -> i32 {
        match self {
            Self::ClientAuth(h) => h.priority(),
            Self::ObjectAccess(h) => h.priority(),
            Self::PlannerRewrite(h) => h.priority(),
            Self::ExecutorStart(h) => h.priority(),
            Self::ExecutorEnd(h) => h.priority(),
            Self::TupleRedact(h) => h.priority(),
            Self::EmitAudit(h) => h.priority(),
        }
    }

    pub fn fail_policy(&self) -> FailPolicy {
        match self {
            Self::ClientAuth(h) => h.fail_policy(),
            Self::ObjectAccess(h) => h.fail_policy(),
            Self::PlannerRewrite(h) => h.fail_policy(),
            Self::ExecutorStart(h) => h.fail_policy(),
            Self::ExecutorEnd(h) => h.fail_policy(),
            Self::TupleRedact(h) => h.fail_policy(),
            Self::EmitAudit(h) => h.fail_policy(),
        }
    }
}

/// 操作符注册条目（绑定至特定 AM 与目标类型）
#[derive(Clone)]
pub struct OperatorRegistration {
    pub am_id: String,
    pub target_type_id: String,
    pub routine: Arc<dyn OperatorRoutine>,
}

/// 扩展向微内核申报能力的统一收集器
pub struct ExtensionRegistrar {
    pub meta: ExtensionMeta,
    pub types: Vec<Arc<dyn TypeRoutine>>,
    pub operators: Vec<OperatorRegistration>,
    pub table_ams: Vec<Arc<dyn TableAmRoutine>>,
    pub index_ams: Vec<Arc<dyn IndexAmRoutine>>,
    pub fdws: Vec<Arc<dyn FdwRoutine>>,
    pub procs: Vec<Arc<dyn ProcRoutine>>,
    pub hooks: Vec<HookRegistration>,
    pub bgworkers: Vec<Arc<dyn BgWorkerRoutine>>,
    pub migrations: Vec<MigrationScript>,
}

impl ExtensionRegistrar {
    pub fn new(meta: ExtensionMeta) -> Self {
        Self {
            meta,
            types: Vec::new(),
            operators: Vec::new(),
            table_ams: Vec::new(),
            index_ams: Vec::new(),
            fdws: Vec::new(),
            procs: Vec::new(),
            hooks: Vec::new(),
            bgworkers: Vec::new(),
            migrations: Vec::new(),
        }
    }

    pub fn register_type(&mut self, routine: Arc<dyn TypeRoutine>) {
        self.types.push(routine);
    }

    pub fn register_operator(
        &mut self,
        am_id: impl Into<String>,
        target_type_id: impl Into<String>,
        routine: Arc<dyn OperatorRoutine>,
    ) {
        self.operators.push(OperatorRegistration {
            am_id: am_id.into(),
            target_type_id: target_type_id.into(),
            routine,
        });
    }

    pub fn register_table_am(&mut self, routine: Arc<dyn TableAmRoutine>) {
        self.table_ams.push(routine);
    }

    pub fn register_index_am(&mut self, routine: Arc<dyn IndexAmRoutine>) {
        self.index_ams.push(routine);
    }

    pub fn register_fdw(&mut self, routine: Arc<dyn FdwRoutine>) {
        self.fdws.push(routine);
    }

    pub fn register_proc(&mut self, routine: Arc<dyn ProcRoutine>) {
        self.procs.push(routine);
    }

    pub fn register_hook(&mut self, hook: HookRegistration) {
        self.hooks.push(hook);
    }

    pub fn register_bgworker(&mut self, worker: Arc<dyn BgWorkerRoutine>) {
        self.bgworkers.push(worker);
    }

    pub fn register_migration(&mut self, migration: MigrationScript) {
        self.migrations.push(migration);
    }
}

/// 所有轨 A 编译期扩展包必须实现的标准入口 Trait
pub trait BodyExtension: Send + Sync {
    fn meta(&self) -> ExtensionMeta;
    fn register(&self, reg: &mut ExtensionRegistrar) -> anyhow::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registrar_collects_metadata_and_migrations() {
        let meta = ExtensionMeta {
            ext_id: "sample_ext".to_string(),
            version: "0.1.0".to_string(),
            kind: ExtensionKind::CompiledCrate,
            min_sdk_version: crate::SDK_API_VERSION.to_string(),
            table_prefix: "ext_sample_ext__".to_string(),
            requires: vec![],
        };
        let mut reg = ExtensionRegistrar::new(meta.clone());
        reg.register_migration(MigrationScript {
            version: "0.1.0".to_string(),
            upgrade_from: None,
            sql: "CREATE TABLE IF NOT EXISTS ext_sample_ext__items (id TEXT PRIMARY KEY);"
                .to_string(),
        });
        assert_eq!(reg.meta.ext_id, "sample_ext");
        assert_eq!(reg.migrations.len(), 1);
    }
}
