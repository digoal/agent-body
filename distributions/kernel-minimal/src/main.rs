//! `agent-body-kernel-minimal` — 0 `contrib` 依赖纯微内核最小发行版入口

mod demo_ext;

use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_body_kernel::{
    HookChain, HttpRequest, KernelRuntime, KernelWaitRegistry, MemoryTableAm, RrfMergeExecutor,
    SearchQuery, SqliteCoreTableAm,
};
use agent_body_sdk::{
    CallerIdentity, ClientAuthContext, EvidenceTuple, IndexHit, RowFilterPredicate,
    TableAmRoutine, TransportKind,
};
use anyhow::{bail, Context};
use clap::{Parser, Subcommand};

use demo_ext::{DemoIndexChannel, KernelDemoExtension};

#[derive(Parser)]
#[command(
    name = "agent-body-kernel-minimal",
    version,
    about = "Agent Body 纯微内核最小发行版 (0 contrib)"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 输出纯微内核运行态状态摘要
    Status,
    /// 列出系统目录已安装扩展与四端派生矩阵
    Catalog,
    /// 启动 stdio MCP JSON-RPC 2.0 派发循环
    Mcp,
    /// 执行纯微内核自检（基表/ScopedSql/RRF/四端派发/Hook闭环）
    SelfCheck,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Status => cmd_status(),
        Commands::Catalog => cmd_catalog(),
        Commands::Mcp => cmd_mcp(),
        Commands::SelfCheck => cmd_self_check(),
    }
}

fn loopback_auth(transport: TransportKind, method: &str, path: &str, read_only: bool) -> ClientAuthContext {
    ClientAuthContext {
        transport,
        cert: None,
        bearer_token: None,
        method: method.to_string(),
        path: path.to_string(),
        is_loopback: true,
        is_read_only: read_only,
    }
}

fn cmd_status() -> anyhow::Result<()> {
    let runtime = KernelRuntime::open_sqlite_in_memory(None)?;
    let mem_am = MemoryTableAm::new();
    mem_am.init_catalog_and_base_tables()?;

    let status = serde_json::json!({
        "distribution": "kernel-minimal",
        "contrib_count": 0,
        "sqlite_am": runtime.table_am().am_id(),
        "memory_am": mem_am.am_id(),
        "wait_events_count": runtime.waits().snapshot_all().len(),
    });
    println!("{}", serde_json::to_string_pretty(&status)?);
    Ok(())
}

fn cmd_catalog() -> anyhow::Result<()> {
    let mut runtime = KernelRuntime::open_sqlite_in_memory(None)?;
    let demo = KernelDemoExtension::new();
    runtime.install_extensions(&[Box::new(demo)])?;

    let parity = runtime.dispatcher().parity_matrix();
    let payload = serde_json::json!({
        "extensions": runtime.installed_metas(),
        "parity_matrix": parity,
    });
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

fn cmd_mcp() -> anyhow::Result<()> {
    let mut runtime = KernelRuntime::open_sqlite_in_memory(None)?;
    runtime.install_extensions(&[Box::new(KernelDemoExtension::new())])?;

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line_res in stdin.lock().lines() {
        let line = line_res?;
        if line.trim().is_empty() {
            continue;
        }
        let req_val: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(err) => {
                let err_resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 0,
                    "error": { "code": -32700, "message": format!("parse error: {err}") }
                });
                writeln!(stdout, "{}", serde_json::to_string(&err_resp)?)?;
                stdout.flush()?;
                continue;
            }
        };
        let auth = loopback_auth(TransportKind::StdioMcp, "POST", "/mcp", false);
        let resp = runtime.handle_mcp(&req_val, &auth);
        writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
        stdout.flush()?;
    }
    Ok(())
}

fn cmd_self_check() -> anyhow::Result<()> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temp_dir = std::env::temp_dir().join(format!(
        "agent_body_kernel_minimal_check_{}_{}",
        std::process::id(),
        nanos
    ));
    std::fs::create_dir_all(&temp_dir)
        .with_context(|| format!("创建自检临时目录失败: {}", temp_dir.display()))?;
    let db_path = temp_dir.join("kernel.db");

    let check_res = run_self_check_inner(&db_path);
    let _cleanup = std::fs::remove_dir_all(&temp_dir);
    check_res
}

fn run_self_check_inner(db_path: &std::path::Path) -> anyhow::Result<()> {
    // 1. SQLite 8 body_cat_* + 5 基表 + MemoryTableAm 初始化
    let mut runtime = KernelRuntime::open_sqlite(db_path, None)?;
    let mem_am = MemoryTableAm::new();
    mem_am.init_catalog_and_base_tables()?;

    let demo = KernelDemoExtension::new();
    let audit_log = Arc::clone(&demo.audit_log);
    let installed = runtime.install_extensions(&[Box::new(demo)])?;
    if installed != vec!["kernel_demo"] {
        bail!("扩展拓扑安装结果异常: {installed:?}");
    }

    // 2. 验证 ScopedSqlExecutor 前缀拦截
    let bad_sql = runtime.table_am().exec_scoped_migration(
        "kernel_demo",
        "ext_kernel_demo__",
        "0.1.1",
        "DROP TABLE body_cat_proc;",
    );
    if bad_sql.is_ok() {
        bail!("ScopedSqlExecutor 未拦截非法跨表 DDL");
    }

    // 3. 验证 RrfMergeExecutor 通道摘除与权重重归一化
    verify_rrf_normalization()?;

    // 4. 验证 body_cat_proc 四端派发 + HookChain 闭环
    let mcp_list = runtime.handle_mcp(
        &serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}),
        &loopback_auth(TransportKind::StdioMcp, "POST", "/mcp", true),
    );
    let tools = mcp_list["result"]["tools"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("MCP tools/list 返回格式异常"))?;
    if !tools.iter().any(|t| t["name"] == "kernel_demo_echo") {
        bail!("MCP tools/list 缺少 kernel_demo_echo");
    }

    let cli_ok = runtime
        .handle_cli(
            &[
                "demo".to_string(),
                "echo".to_string(),
                "--msg".to_string(),
                "hello-kernel".to_string(),
                "--secret".to_string(),
                "raw-secret".to_string(),
            ],
            None,
            &loopback_auth(TransportKind::Cli, "EXEC", "demo/echo", false),
        )
        .map_err(|e| anyhow::anyhow!("CLI 派发失败: {}", e.message()))?;
    if cli_ok.data["secret"] != "***" || cli_ok.data["echo"] != "hello-kernel" {
        bail!("CLI 脱敏或响应异常: {}", cli_ok.data);
    }

    let http_parity = runtime.handle_http(&HttpRequest {
        method: "GET".to_string(),
        path: "/api/v1/parity/matrix".to_string(),
        bearer_token: None,
        is_loopback: true,
        body: serde_json::Value::Null,
    });
    if http_parity.status_code != 200 || http_parity.body["total"] != 1 {
        bail!("HTTP /api/v1/parity/matrix 校验失败: {:?}", http_parity);
    }

    let http_proc = runtime.handle_http(&HttpRequest {
        method: "POST".to_string(),
        path: "/api/v1/demo/echo".to_string(),
        bearer_token: None,
        is_loopback: true,
        body: serde_json::json!({"msg": "via-http"}),
    });
    if http_proc.status_code != 200 || http_proc.body["data"]["echo"] != "via-http" {
        bail!("HTTP ProcRoute 派发失败: {:?}", http_proc);
    }

    let audit_count = audit_log.lock().map(|g| g.len()).unwrap_or(0);
    if audit_count < 3 {
        bail!("EmitAuditHook 留痕数量不足: {audit_count}");
    }

    // 校验 SQLiteCoreTableAm 独立实例化也正常
    let _sqlite_check = SqliteCoreTableAm::open_in_memory()?;
    println!("✅ kernel-minimal self-check passed (catalog=8, base=5, parity=1, audit={audit_count})");
    Ok(())
}

fn verify_rrf_normalization() -> anyhow::Result<()> {
    let mem = MemoryTableAm::new();
    mem.init_catalog_and_base_tables()?;

    let mut labels = BTreeMap::new();
    labels.insert("tenant_id".to_string(), "t1".to_string());
    let tuple = EvidenceTuple {
        evidence_id: "ev-demo-1".to_string(),
        host_id: "local".to_string(),
        host_labels: labels,
        entity_type: "entity:item".to_string(),
        entity_id: "k1".to_string(),
        source_ext: "kernel_demo".to_string(),
        evidence_tier: "L1_EXPLICIT".to_string(),
        collected_at: "2026-11-27T16:00:00+08:00".to_string(),
        ttl_sec: 0,
        payload: serde_json::json!({"title": "pure-kernel"}),
        redacted: false,
        evidence_sha256: String::new(),
    }
    .with_computed_sha256();
    mem.entity_upsert(&tuple)?;

    let mut rrf = RrfMergeExecutor::default();
    rrf.register_channel(Arc::new(DemoIndexChannel {
        id: "ch_off",
        weight: 0.6,
        available: false,
        hits: vec![],
    }));
    rrf.register_channel(Arc::new(DemoIndexChannel {
        id: "ch_a",
        weight: 0.3,
        available: true,
        hits: vec![IndexHit {
            entity_id: "k1".to_string(),
            entity_type: "entity:item".to_string(),
            score: 1.0,
            channel: "ch_a".to_string(),
            reasoning_path: None,
            tuple: Some(tuple.clone()),
        }],
    }));
    rrf.register_channel(Arc::new(DemoIndexChannel {
        id: "ch_b",
        weight: 0.1,
        available: true,
        hits: vec![IndexHit {
            entity_id: "k1".to_string(),
            entity_type: "entity:item".to_string(),
            score: 0.8,
            channel: "ch_b".to_string(),
            reasoning_path: None,
            tuple: Some(tuple),
        }],
    }));

    let query = SearchQuery {
        query_text: "pure".to_string(),
        rls_filter: Some(RowFilterPredicate {
            tenant_id: Some("t1".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let res = rrf.search(
        &query,
        &CallerIdentity::default(),
        &HookChain::new(),
        &mem,
        KernelWaitRegistry::global(),
    )?;
    let sum_w: f64 = res.normalized_weights.values().sum();
    if (sum_w - 1.0).abs() > 1e-6 {
        bail!("RRF 重归一化权重之和不为 1.0: {sum_w}");
    }
    Ok(())
}
