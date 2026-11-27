//! 微内核统一运行时引擎 (`KernelRuntime`)：目录 + TableAM + HookChain + RRF + Dispatcher + BgWorker 宿主机

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use agent_body_sdk::{
    AuditEnvelope, BgWorkerRoutine, BodyExtension, CallerIdentity, CancellationToken,
    ClientAuthContext, ClientAuthHook, EvidenceTuple, ExtensionMeta, ExtensionRegistrar,
    FdwRoutine, FdwSourceHealth, HookRegistration, KernelObservabilitySnapshot,
    ObservabilityProvider, ProbeContext, TableAmRoutine, WorkerTickOutcome, WorkerTrigger,
    SDK_API_VERSION,
};
use anyhow::{anyhow, bail, Context};
use chrono::Utc;
use rusqlite::Connection;
use uuid::Uuid;

use crate::catalog::{
    compute_control_hash, init_catalog_schema, topo_sort_extensions, CatalogStore, ExtensionRecord,
    ExtensionState,
};
use crate::dispatcher::{DispatchError, DispatchSuccess, HttpRequest, HttpResponse, KernelDispatcher};
use crate::hook_chain::{HookChain, LoopbackOrTokenAuthHook};
use crate::mem_am::MemoryTableAm;
use crate::observability::KernelObservability;
use crate::rrf_executor::{RrfMergeExecutor, RrfSearchResult, SearchQuery};
use crate::sqlite_am::SqliteCoreTableAm;
use crate::wait_events::{KernelWaitEventId, KernelWaitRegistry};

/// 微内核统一运行时门面
pub struct KernelRuntime {
    catalog_conn: Arc<Mutex<Connection>>,
    table_am: Arc<dyn TableAmRoutine>,
    hooks: HookChain,
    rrf: RrfMergeExecutor,
    dispatcher: KernelDispatcher,
    waits: Arc<KernelWaitRegistry>,
    obs: Arc<KernelObservability>,
    fdws: BTreeMap<String, Arc<dyn FdwRoutine>>,
    bgworkers: Vec<Arc<dyn BgWorkerRoutine>>,
    extensions: Vec<ExtensionMeta>,
}

impl KernelRuntime {
    /// 基于 SQLite 存储初始化微内核运行时
    pub fn open_sqlite(
        path: impl AsRef<Path>,
        auth_hook: Option<Arc<dyn ClientAuthHook>>,
    ) -> anyhow::Result<Self> {
        let am = SqliteCoreTableAm::open(path)?;
        let shared = am.shared_conn();
        Self::from_parts(shared, Arc::new(am), auth_hook)
    }

    /// 基于内存 SQLite 初始化微内核运行时
    pub fn open_sqlite_in_memory(auth_hook: Option<Arc<dyn ClientAuthHook>>) -> anyhow::Result<Self> {
        let am = SqliteCoreTableAm::open_in_memory()?;
        let shared = am.shared_conn();
        Self::from_parts(shared, Arc::new(am), auth_hook)
    }

    /// 基于纯内存无盘 `MemoryTableAm` 初始化微内核运行时（目录表使用内存 SQLite）
    pub fn new_memory(auth_hook: Option<Arc<dyn ClientAuthHook>>) -> anyhow::Result<Self> {
        let cat_conn = Connection::open_in_memory().context("创建内存系统目录库失败")?;
        cat_conn.pragma_update(None, "foreign_keys", "ON")?;
        let am = MemoryTableAm::new();
        Self::from_parts(Arc::new(Mutex::new(cat_conn)), Arc::new(am), auth_hook)
    }

    fn from_parts(
        catalog_conn: Arc<Mutex<Connection>>,
        table_am: Arc<dyn TableAmRoutine>,
        auth_hook: Option<Arc<dyn ClientAuthHook>>,
    ) -> anyhow::Result<Self> {
        let mut hooks = HookChain::new();
        let default_auth: Arc<dyn ClientAuthHook> = auth_hook
            .unwrap_or_else(|| Arc::new(LoopbackOrTokenAuthHook::new(false, None)));
        hooks.register(HookRegistration::ClientAuth(default_auth));

        let waits = Arc::new(KernelWaitRegistry::new());
        let obs = Arc::new(KernelObservability::new(Arc::clone(&waits)));
        let runtime = Self {
            catalog_conn,
            table_am,
            hooks,
            rrf: RrfMergeExecutor::default(),
            dispatcher: KernelDispatcher::new(),
            waits,
            obs,
            fdws: BTreeMap::new(),
            bgworkers: Vec::new(),
            extensions: Vec::new(),
        };
        runtime.init()?;
        Ok(runtime)
    }

    /// 初始化 8 张 `body_cat_*` 系统目录表与 5 张内核通用基表
    pub fn init(&self) -> anyhow::Result<()> {
        {
            let _guard = self.waits.enter(KernelWaitEventId::CatalogWrite);
            let conn = self.lock_catalog()?;
            init_catalog_schema(&conn)?;
        }
        let _guard = self.waits.enter(KernelWaitEventId::TableAmWrite);
        self.table_am.init_catalog_and_base_tables()
    }

    fn lock_catalog(&self) -> anyhow::Result<MutexGuard<'_, Connection>> {
        self.catalog_conn
            .lock()
            .map_err(|_| anyhow!("系统目录连接锁已中毒"))
    }

    /// 按依赖 DAG 拓扑排序并安装激活扩展（校验 SDK 主版本、迁移沙盒前缀、同步 Catalog）
    pub fn install_extensions(
        &mut self,
        extensions: &[Box<dyn BodyExtension>],
    ) -> anyhow::Result<Vec<String>> {
        let mut by_id: std::collections::BTreeMap<String, (&Box<dyn BodyExtension>, ExtensionMeta)> =
            std::collections::BTreeMap::new();
        let mut metas = Vec::with_capacity(extensions.len());

        for ext in extensions {
            let meta = ext.meta();
            check_sdk_major_compat(&meta)?;
            metas.push(meta.clone());
            by_id.insert(meta.ext_id.clone(), (ext, meta));
        }

        let order = topo_sort_extensions(&metas)?;
        for ext_id in &order {
            let Some((ext, meta)) = by_id.get(ext_id) else {
                bail!("拓扑排序后未找到扩展 '{ext_id}'");
            };
            let mut reg = ExtensionRegistrar::new(meta.clone());
            ext.register(&mut reg)
                .with_context(|| format!("调用扩展 '{ext_id}' register() 失败"))?;

            let fingerprint = format!("{}:{}:{}", meta.ext_id, meta.version, meta.table_prefix);
            let hash = compute_control_hash(&fingerprint);
            {
                let _guard = self.waits.enter(KernelWaitEventId::CatalogWrite);
                let conn = self.lock_catalog()?;
                CatalogStore::sync_registrar(&conn, &reg, &hash)?;
                CatalogStore::transition_state(&conn, ext_id, ExtensionState::Verified)?;
            }

            for mig in &reg.migrations {
                let _guard = self.waits.enter(KernelWaitEventId::TableAmWrite);
                self.table_am.exec_scoped_migration(
                    &meta.ext_id,
                    &meta.table_prefix,
                    &mig.version,
                    &mig.sql,
                )?;
            }

            {
                let _guard = self.waits.enter(KernelWaitEventId::CatalogWrite);
                let conn = self.lock_catalog()?;
                CatalogStore::transition_state(&conn, ext_id, ExtensionState::Installed)?;
            }

            self.dispatcher.register_from_registrar(&reg)?;
            for hook in reg.hooks {
                self.hooks.register(hook);
            }
            for idx_am in reg.index_ams {
                self.rrf.register_channel(idx_am);
            }
            for fdw in reg.fdws {
                self.fdws.insert(fdw.fdw_id().to_string(), fdw);
            }
            for worker in reg.bgworkers {
                self.bgworkers.push(worker);
            }

            {
                let _guard = self.waits.enter(KernelWaitEventId::CatalogWrite);
                let conn = self.lock_catalog()?;
                CatalogStore::transition_state(&conn, ext_id, ExtensionState::Enabled)?;
            }
            self.extensions.push(meta.clone());
        }
        Ok(order)
    }

    /// 手动注册额外 Hook
    pub fn register_hook(&mut self, hook: HookRegistration) {
        self.hooks.register(hook);
    }

    /// 手动注册外部 FDW（如轨 B `OutOfProcessFdwAdapter`）
    pub fn register_fdw(&mut self, fdw: Arc<dyn FdwRoutine>) {
        self.fdws.insert(fdw.fdw_id().to_string(), fdw);
    }

    /// 检查已注册 FDW 数据源健康度与权限前提
    pub fn explain_fdw(
        &self,
        fdw_id: &str,
        ctx: &ProbeContext,
    ) -> anyhow::Result<FdwSourceHealth> {
        crate::engine_fdw::explain_fdw_inner(&self.fdws, fdw_id, ctx, &self.waits)
    }

    /// 托管执行 FDW 扫描（全链路 `ClientAuth` + `ObjectAccess(RLS)` + `PlannerRewrite` + `TupleRedact` + `EmitAudit`）
    pub fn scan_fdw(
        &self,
        fdw_id: &str,
        target: Option<&str>,
        ctx: &mut ProbeContext,
        auth_ctx: &ClientAuthContext,
    ) -> anyhow::Result<Vec<EvidenceTuple>> {
        crate::engine_fdw::scan_fdw_inner(
            &self.fdws,
            &self.hooks,
            &self.waits,
            Some(&self.obs),
            fdw_id,
            target,
            ctx,
            auth_ctx,
        )
    }

    /// 列出已注册 FDW ID
    pub fn registered_fdw_ids(&self) -> Vec<String> {
        self.fdws.keys().cloned().collect()
    }

    /// 单轮串行执行所有已挂载 `BgWorkerRoutine`（含 WaitEvent 计时与 `OUT_OF_AGENT` 审计留痕）
    pub fn run_bgworkers_once(
        &self,
        cancel: &CancellationToken,
    ) -> anyhow::Result<Vec<(String, WorkerTickOutcome)>> {
        let mut outcomes = Vec::with_capacity(self.bgworkers.len());
        let worker_caller = CallerIdentity {
            caller_id: "kernel-bgworker".to_string(),
            roles: vec!["bgworker".to_string()],
            auth_method: "internal".to_string(),
            is_loopback: true,
            ..Default::default()
        };
        let obs_provider: Arc<dyn ObservabilityProvider> =
            Arc::clone(&self.obs) as Arc<dyn ObservabilityProvider>;
        for worker in &self.bgworkers {
            if cancel.is_cancelled() {
                break;
            }
            let wid = worker.worker_id();
            let trace_id = Uuid::new_v4().to_string();
            let started = Instant::now();
            let _in_flight = self.obs.enter_op(&trace_id, "bgworker", wid, &worker_caller.caller_id);
            let ctx = agent_body_sdk::KernelServices::new(
                worker_caller.clone(),
                trace_id.clone(),
                Arc::clone(&self.table_am),
            )
            .with_observability(Arc::clone(&obs_provider));
            let tick_res = {
                let _guard = self.waits.enter(KernelWaitEventId::BgWorkerTick);
                worker.run_tick(&ctx, cancel)
            };
            match tick_res {
                Ok(outcome) => {
                    let _audit_errs = self.hooks.run_emit_audit(&AuditEnvelope {
                        trace_id: trace_id.clone(),
                        caller: worker_caller.clone(),
                        domain: "OUT_OF_AGENT".to_string(),
                        operation: "bgworker_tick".to_string(),
                        target: wid.to_string(),
                        outcome: "ok".to_string(),
                        before_after_delta: serde_json::to_value(&outcome).ok(),
                        evidence_sha256: String::new(),
                        recorded_at: Utc::now().to_rfc3339(),
                    });
                    self.obs.record_outcome(
                        &trace_id,
                        "bgworker",
                        wid,
                        &worker_caller.caller_id,
                        "ok",
                        started.elapsed().as_micros() as u64,
                        outcome.note.clone(),
                    );
                    outcomes.push((wid.to_string(), outcome));
                }
                Err(err) => {
                    let err_msg = err.to_string();
                    let _audit_errs = self.hooks.run_emit_audit(&AuditEnvelope {
                        trace_id: trace_id.clone(),
                        caller: worker_caller.clone(),
                        domain: "OUT_OF_AGENT".to_string(),
                        operation: "bgworker_tick".to_string(),
                        target: wid.to_string(),
                        outcome: "error".to_string(),
                        before_after_delta: Some(serde_json::json!({ "error": err_msg })),
                        evidence_sha256: String::new(),
                        recorded_at: Utc::now().to_rfc3339(),
                    });
                    self.obs.record_outcome(
                        &trace_id,
                        "bgworker",
                        wid,
                        &worker_caller.caller_id,
                        "error",
                        started.elapsed().as_micros() as u64,
                        Some(err_msg),
                    );
                    return Err(err);
                }
            }
        }
        Ok(outcomes)
    }

    /// 启动托管后台协程线程（仅启动 `WorkerTrigger::Interval` 类型协程，支持 `CancellationToken` 优雅停机）
    pub fn spawn_bgworkers(&self, cancel: CancellationToken) -> Vec<thread::JoinHandle<()>> {
        let mut handles = Vec::new();
        for worker in &self.bgworkers {
            let interval = match worker.trigger() {
                WorkerTrigger::Interval(d) => d.max(Duration::from_millis(50)),
                WorkerTrigger::EventTopics(_) => continue,
            };
            let worker_ref = Arc::clone(worker);
            let table_am = Arc::clone(&self.table_am);
            let waits = Arc::clone(&self.waits);
            let obs = Arc::clone(&self.obs);
            let token = cancel.clone();
            let handle = thread::spawn(move || {
                let caller = CallerIdentity {
                    caller_id: format!("bgworker:{}", worker_ref.worker_id()),
                    roles: vec!["bgworker".to_string()],
                    auth_method: "internal".to_string(),
                    is_loopback: true,
                    ..Default::default()
                };
                let obs_provider: Arc<dyn ObservabilityProvider> =
                    Arc::clone(&obs) as Arc<dyn ObservabilityProvider>;
                while !token.is_cancelled() {
                    let wid = worker_ref.worker_id();
                    let trace_id = Uuid::new_v4().to_string();
                    let started = Instant::now();
                    let _in_flight = obs.enter_op(&trace_id, "bgworker", wid, &caller.caller_id);
                    let ctx = agent_body_sdk::KernelServices::new(
                        caller.clone(),
                        trace_id.clone(),
                        Arc::clone(&table_am),
                    )
                    .with_observability(Arc::clone(&obs_provider));
                    let res = {
                        let _guard = waits.enter(KernelWaitEventId::BgWorkerTick);
                        worker_ref.run_tick(&ctx, &token)
                    };
                    let elapsed_us = started.elapsed().as_micros() as u64;
                    match res {
                        Ok(out) => obs.record_outcome(
                            &trace_id,
                            "bgworker",
                            wid,
                            &caller.caller_id,
                            "ok",
                            elapsed_us,
                            out.note,
                        ),
                        Err(e) => obs.record_outcome(
                            &trace_id,
                            "bgworker",
                            wid,
                            &caller.caller_id,
                            "error",
                            elapsed_us,
                            Some(e.to_string()),
                        ),
                    }
                    drop(_in_flight);
                    let mut slept = Duration::ZERO;
                    let step = Duration::from_millis(50);
                    while slept < interval && !token.is_cancelled() {
                        let chunk = step.min(interval.saturating_sub(slept));
                        thread::sleep(chunk);
                        slept += chunk;
                    }
                }
            });
            handles.push(handle);
        }
        handles
    }

    pub fn handle_mcp(
        &self,
        raw_req: &serde_json::Value,
        auth_ctx: &ClientAuthContext,
    ) -> serde_json::Value {
        self.dispatcher.dispatch_mcp_jsonrpc_with_obs(
            raw_req,
            auth_ctx,
            &self.hooks,
            &self.table_am,
            &self.waits,
            Some(&self.obs),
        )
    }

    pub fn handle_cli(
        &self,
        args: &[String],
        json_args: Option<serde_json::Value>,
        auth_ctx: &ClientAuthContext,
    ) -> Result<DispatchSuccess, DispatchError> {
        self.dispatcher.dispatch_cli_with_obs(
            args,
            json_args,
            auth_ctx,
            &self.hooks,
            &self.table_am,
            &self.waits,
            Some(&self.obs),
        )
    }

    pub fn handle_http(&self, req: &HttpRequest) -> HttpResponse {
        self.dispatcher.dispatch_http_with_obs(
            req,
            Some(self.catalog_conn.as_ref()),
            &self.hooks,
            &self.table_am,
            &self.waits,
            Some(&self.obs),
        )
    }

    pub fn search_rrf(
        &self,
        query: &SearchQuery,
        caller: &CallerIdentity,
    ) -> anyhow::Result<RrfSearchResult> {
        self.rrf
            .search(query, caller, &self.hooks, self.table_am.as_ref(), &self.waits)
    }

    pub fn get_extension_record(&self, ext_id: &str) -> anyhow::Result<Option<ExtensionRecord>> {
        let _guard = self.waits.enter(KernelWaitEventId::CatalogRead);
        let conn = self.lock_catalog()?;
        CatalogStore::get_extension(&conn, ext_id)
    }

    pub fn table_am(&self) -> &Arc<dyn TableAmRoutine> {
        &self.table_am
    }

    pub fn hooks(&self) -> &HookChain {
        &self.hooks
    }

    pub fn rrf(&self) -> &RrfMergeExecutor {
        &self.rrf
    }

    pub fn dispatcher(&self) -> &KernelDispatcher {
        &self.dispatcher
    }

    pub fn waits(&self) -> &Arc<KernelWaitRegistry> {
        &self.waits
    }

    pub fn observability(&self) -> &Arc<KernelObservability> {
        &self.obs
    }

    pub fn observability_snapshot(&self) -> KernelObservabilitySnapshot {
        self.obs.snapshot()
    }

    pub fn installed_metas(&self) -> &[ExtensionMeta] {
        &self.extensions
    }
}

fn check_sdk_major_compat(meta: &ExtensionMeta) -> anyhow::Result<()> {
    let kernel_major = SDK_API_VERSION.split('.').next().unwrap_or("");
    let ext_major = meta.min_sdk_version.split('.').next().unwrap_or("");
    if kernel_major != ext_major {
        bail!(
            "扩展 '{}' 要求的 SDK 版本 '{}' 与内核 SDK '{}' 主版本不兼容",
            meta.ext_id,
            meta.min_sdk_version,
            SDK_API_VERSION
        );
    }
    Ok(())
}
