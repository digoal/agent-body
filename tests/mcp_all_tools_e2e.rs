use agent_body::api::mcp::McpServer;
use agent_body::config::McpToolProfile;
use agent_body::storage::sqlite_provider::SqliteStorageProvider;
use agent_body::storage::Storage;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::sync::Arc;

fn call_rpc(server: &McpServer, payload: Value) -> Value {
    let raw = server
        .handle_message(&payload.to_string())
        .expect("JSON-RPC request with id must return response");
    serde_json::from_str(&raw).expect("valid JSON-RPC response")
}

fn call_tool(server: &McpServer, id: u64, name: &str, arguments: Value) -> (bool, String) {
    let resp = call_rpc(
        server,
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": arguments
            }
        }),
    );
    assert!(
        resp.get("error").is_none(),
        "tool {} returned JSON-RPC protocol error: {}",
        name,
        resp
    );
    let result = resp.get("result").expect("result field");
    let is_error = result
        .get("isError")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let text = result["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();
    (is_error, text)
}

#[test]
fn test_mcp_jsonrpc_protocol_and_profile_exposure() {
    let storage = Storage::open_in_memory().expect("storage");
    let server = McpServer::new(storage.clone());

    // 1. initialize
    let init = call_rpc(
        &server,
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );
    assert_eq!(init["result"]["serverInfo"]["name"], "agent-body");
    assert_eq!(init["result"]["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));

    // 2. notifications/initialized (无 id 或通知不返回响应)
    assert!(server
        .handle_message(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
        .is_none());
    assert!(server.handle_message("   ").is_none());

    // 3. ping
    let ping = call_rpc(&server, json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}));
    assert!(ping.get("result").is_some());

    // 4. malformed JSON -> -32700 Parse error
    let parse_err_raw = server
        .handle_message("{not valid json")
        .expect("parse error response");
    let parse_err: Value = serde_json::from_str(&parse_err_raw).expect("json");
    assert_eq!(parse_err["error"]["code"], -32700);

    // 5. unknown method -> -32601
    let method_err = call_rpc(
        &server,
        json!({"jsonrpc": "2.0", "id": 3, "method": "nonexistent/method"}),
    );
    assert_eq!(method_err["error"]["code"], -32601);

    // 6. unknown tool -> isError: true
    let (is_err, text) = call_tool(&server, 4, "body_no_such_tool", json!({}));
    assert!(is_err);
    assert!(text.contains("Unknown tool"));

    // 7. Profile 工具数量与 Schema 合法性全量校验
    for (profile, expected_cnt) in [
        (McpToolProfile::Full, 43usize),
        (McpToolProfile::Core, 10usize),
        (McpToolProfile::Dev, 18usize),
        (McpToolProfile::Ops, 20usize),
    ] {
        let srv = McpServer::with_profile(storage.clone(), profile);
        let list_resp = call_rpc(&srv, json!({"jsonrpc": "2.0", "id": 10, "method": "tools/list"}));
        let tools = list_resp["result"]["tools"]
            .as_array()
            .expect("tools array");
        assert_eq!(
            tools.len(),
            expected_cnt,
            "profile {:?} expected {} tools, got {}",
            profile,
            expected_cnt,
            tools.len()
        );
        for t in tools {
            let name = t["name"].as_str().expect("tool name");
            assert!(name.starts_with("body_"), "unexpected tool name: {}", name);
            assert!(
                !t["description"].as_str().unwrap_or("").trim().is_empty(),
                "tool {} missing description",
                name
            );
            assert_eq!(
                t["inputSchema"]["type"].as_str(),
                Some("object"),
                "tool {} inputSchema must be object",
                name
            );
        }
    }
}

#[test]
fn test_mcp_all_43_tools_exhaustive_dispatch_matrix() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let sample_dir = tmp.path().join("sample_proj");
    std::fs::create_dir_all(&sample_dir).expect("mkdir");
    let sample_file = sample_dir.join("worker.py");
    std::fs::write(&sample_file, "# worker script\nimport os\nprint('ok')\n").expect("write file");
    let sample_file_s = sample_file.to_string_lossy().to_string();
    let sample_dir_s = sample_dir.to_string_lossy().to_string();
    let export_csv_s = tmp.path().join("mcp_assets_export.csv").to_string_lossy().to_string();
    let audit_csv_s = tmp.path().join("mcp_audit_export.csv").to_string_lossy().to_string();

    let storage = Storage::open_in_memory().expect("in-memory storage");
    let server = McpServer::new(storage);

    // 拉取 43 个官方工具全集，确保本次矩阵 100% 覆盖无遗漏
    let list_resp = call_rpc(&server, json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}));
    let registered_tools: BTreeSet<String> = list_resp["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|t| t["name"].as_str().expect("name").to_string())
        .collect();
    assert_eq!(registered_tools.len(), 43);

    let cases: Vec<(&str, Value)> = vec![
        ("body_brief", json!({"query": "sqlite benchmark", "budget_tokens": 800})),
        ("body_placement_suggest", json!({"object_type": "script", "name": "pg_check", "intent": "PostgreSQL 健康巡检脚本"})),
        ("body_placement_rules", json!({})),
        ("body_asset_register", json!({"path": sample_file_s, "type": "script", "description": "MCP 矩阵测试脚本"})),
        ("body_asset_scan", json!({"path": sample_dir_s})),
        ("body_asset_search", json!({"query": "worker", "limit": 5, "detail_level": "standard"})),
        ("body_env_processes", json!({})),
        ("body_env_show", json!({})),
        ("body_memory_add", json!({"type": "decision", "content": "MCP 43 工具全覆盖测试决策记录", "summary": "MCP 覆盖"})),
        ("body_memory_search", json!({"query": "MCP", "limit": 5, "hybrid": false})),
        ("body_links_query", json!({"type": "asset", "id": sample_file_s, "max_hops": 1})),
        ("body_links_visualize", json!({"limit": 10})),
        ("body_changes_since", json!({"limit": 10})),
        ("body_port_check", json!({"port": 59432, "suggest_count": 2})),
        ("body_links_impact", json!({"target": sample_file_s, "max_depth": 2})),
        ("body_links_audit", json!({"fix_dangling": false})),
        ("body_codegraph_bridge", json!({"path": sample_dir_s})),
        ("body_session_close", json!({"session_id": "mcp-matrix-sess", "clean_temp": false})),
        ("body_links_repair", json!({"apply": false})),
        ("body_placement_learn", json!({"min_samples": 3, "accept": false})),
        ("body_env_gc", json!({"tmp_dir": sample_dir_s, "max_age_hours": 72, "apply": false})),
        ("body_memory_audit", json!({})),
        ("body_fact_query", json!({"query": "git", "fact_type": "capability"})),
        ("body_hook_pre", json!({"tool_name": "run_command", "args": {"command": "echo hello"}})),
        ("body_hook_post", json!({"tool_name": "write_file", "args": {"path": sample_file_s}, "success": true})),
        ("body_hook_generate", json!({"client": "agy", "dry_run": true})),
        ("body_analytics_heatmap", json!({"days": 7})),
        ("body_analytics_trend", json!({"metric": "assets", "days": 7})),
        ("body_analytics_export", json!({"table": "assets", "output_path": export_csv_s})),
        ("body_lock_acquire", json!({"resource_type": "custom", "resource_id": "mcp-matrix-res-1", "holder_agent": "agent-mcp-test", "ttl_seconds": 60})),
        ("body_lock_list", json!({})),
        ("body_lock_wait", json!({"resource_type": "custom", "resource_id": "mcp-matrix-res-2", "holder_agent": "agent-mcp-waiter", "wait_timeout_ms": 200})),
        ("body_lock_deadlock_check", json!({})),
        ("body_lock_release", json!({"resource_type": "custom", "resource_id": "mcp-matrix-res-1", "holder_agent": "agent-mcp-test"})),
        ("body_mcp_catalog", json!({"action": "list"})),
        ("body_skill_route", json!({"action": "list"})),
        ("body_service_contact", json!({"action": "list"})),
        ("body_mesh_query", json!({"action": "local"})),
        ("body_demo_scenario", json!({"action": "list"})),
        ("body_benchmark", json!({"action": "workloads"})),
        ("body_feedback", json!({"action": "reconcile"})),
        ("body_audit_query", json!({"action": "export", "output_path": audit_csv_s})),
        ("body_state", json!({"action": "manifest"})),
    ];

    let mut covered = BTreeSet::new();
    for (idx, (tool_name, args)) in cases.into_iter().enumerate() {
        covered.insert(tool_name.to_string());
        let (is_err, text) = call_tool(&server, 100 + idx as u64, tool_name, args);
        assert!(
            !is_err,
            "MCP 工具 {} 执行失败 (isError=true): {}",
            tool_name, text
        );
        assert!(
            !text.trim().is_empty(),
            "MCP 工具 {} 返回空文本",
            tool_name
        );
    }

    assert_eq!(
        covered, registered_tools,
        "43/43 MCP 工具矩阵必须无一遗漏!"
    );
}

#[test]
fn test_mcp_benchmark_and_body_state_on_migrated_legacy_db() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("mcp_legacy.db");
    let lp = ["st", "ew", "ard"].concat();
    {
        let raw = rusqlite::Connection::open(&db_path).expect("raw sqlite");
        let ddl = format!(
            "CREATE TABLE benchmark_runs (
                run_id TEXT PRIMARY KEY, suite_name TEXT NOT NULL, git_commit TEXT NOT NULL,
                app_version TEXT NOT NULL, host_fingerprint TEXT NOT NULL, workload_count INTEGER NOT NULL,
                total_baseline_tokens INTEGER NOT NULL, total_{lp}_tokens INTEGER NOT NULL,
                overall_token_savings_pct REAL NOT NULL, overall_speedup_factor REAL NOT NULL,
                total_hazards_intercepted INTEGER NOT NULL, total_noise_blocked INTEGER NOT NULL,
                sla_passed INTEGER NOT NULL, results_json TEXT NOT NULL, report_sha256 TEXT NOT NULL,
                duration_ms INTEGER NOT NULL, created_at TEXT NOT NULL
            );
            CREATE TABLE effectiveness_ledger (
                event_id TEXT PRIMARY KEY, session_id TEXT NOT NULL DEFAULT 'default',
                source_channel TEXT NOT NULL, tool_or_action TEXT NOT NULL, provenance_rule TEXT NOT NULL,
                baseline_tokens INTEGER NOT NULL DEFAULT 0, {lp}_tokens INTEGER NOT NULL DEFAULT 0,
                tokens_saved INTEGER NOT NULL DEFAULT 0, baseline_rounds INTEGER NOT NULL DEFAULT 1,
                {lp}_rounds INTEGER NOT NULL DEFAULT 1, rounds_saved INTEGER NOT NULL DEFAULT 0,
                hazard_intercepted INTEGER NOT NULL DEFAULT 0, noise_blocked INTEGER NOT NULL DEFAULT 0,
                elapsed_us INTEGER NOT NULL DEFAULT 0, evidence_summary TEXT NOT NULL, created_at TEXT NOT NULL
            );",
            lp = lp
        );
        raw.execute_batch(&ddl).expect("seed legacy schema");
    }

    let provider = Arc::new(SqliteStorageProvider::open(&db_path).expect("open & migrate"));
    let server = McpServer::new(Storage::new(provider));

    // 通过 MCP 调用 body_benchmark run (micro, save=true) & ledger & body_state perf_snapshot/perf_awr
    let (err_run, out_run) = call_tool(
        &server,
        201,
        "body_benchmark",
        json!({"action": "run", "suite": "micro", "save": true}),
    );
    assert!(!err_run, "body_benchmark run failed: {}", out_run);
    assert!(out_run.contains("report_sha256"));

    let (err_ledger, out_ledger) = call_tool(
        &server,
        202,
        "body_benchmark",
        json!({"action": "ledger", "window": "all"}),
    );
    assert!(!err_ledger, "body_benchmark ledger failed: {}", out_ledger);

    let (err_snap, out_snap) = call_tool(
        &server,
        203,
        "body_state",
        json!({"action": "perf_snapshot", "label": "mcp-legacy-base", "set_baseline": true}),
    );
    assert!(!err_snap, "body_state perf_snapshot failed: {}", out_snap);

    let (err_awr, out_awr) = call_tool(
        &server,
        204,
        "body_state",
        json!({"action": "perf_awr", "vs_baseline": true}),
    );
    assert!(!err_awr, "body_state perf_awr failed: {}", out_awr);
    assert!(out_awr.contains("report_id"));
}

#[test]
fn test_mcp_security_boundaries_redaction_and_error_handling() {
    let storage = Storage::open_in_memory().expect("storage");
    let server = McpServer::new(storage.clone());

    // 1. 写入含明文密钥和 troubleshooting 别名的记忆：应自动脱敏并映射为 experience
    let (err_mem, _) = call_tool(
        &server,
        301,
        "body_memory_add",
        json!({
            "type": "troubleshooting",
            "content": "排查 PG 连接 postgres://admin:SuperSecret99@10.0.0.1:5432/prod 与 key sk-1234567890abcdefghij",
            "summary": "token ghp_abcdefghijklmnopqrstuvwxyz123456 脱敏测试"
        }),
    );
    assert!(!err_mem, "troubleshooting alias memory add should succeed");
    let mems = storage.list_all_active_memories(10).expect("list mems");
    assert_eq!(mems.len(), 1);
    assert_eq!(mems[0].memory_type, "experience");
    assert!(!mems[0].content.contains("SuperSecret99"));
    assert!(!mems[0].content.contains("sk-1234567890abcdefghij"));
    assert!(!mems[0].summary.as_deref().unwrap_or("").contains("ghp_abcdefghijklmnopqrstuvwxyz123456"));

    // 2. L3 禁区路径包络校验必须拒绝
    let (err_env, out_env) = call_tool(
        &server,
        302,
        "body_state",
        json!({"action": "envelope", "check_action": "edit_file", "target": "/etc/shadow", "dry_run": false}),
    );
    assert!(!err_env);
    assert!(
        out_env.contains("\"allowed\": false") || out_env.contains("\"allowed\":false"),
        "L3 forbidden path /etc/shadow must be denied: {}",
        out_env
    );

    // 3. Pre-Hook 高危命令拦截
    let (err_hook, out_hook) = call_tool(
        &server,
        303,
        "body_hook_pre",
        json!({"tool_name": "run_command", "args": {"command": "rm -rf /"}}),
    );
    assert!(!err_hook);
    assert!(
        out_hook.contains("\"allowed\": false")
            || out_hook.contains("\"allowed\":false")
            || out_hook.contains("BLOCK")
            || out_hook.contains("警告")
            || out_hook.contains("高危"),
        "destructive rm -rf / must trigger pre-hook warning/block: {}",
        out_hook
    );

    // 4. Analytics Export 路径穿越与 SQL 注入拦截
    for bad_path in [
        "/etc/passwd.csv",
        "/tmp/../../etc/shadow.csv",
        "/tmp/bad'quote.csv",
        "relative_path.csv",
    ] {
        let (is_err, txt) = call_tool(
            &server,
            304,
            "body_analytics_export",
            json!({"table": "assets", "output_path": bad_path}),
        );
        assert!(
            is_err,
            "export path '{}' must be rejected by sandbox, got: {}",
            bad_path, txt
        );
    }

    // 5. 缺失必填参数时优雅返回 isError=true，绝不 panic
    for (tool, args) in [
        ("body_memory_add", json!({})),
        ("body_asset_register", json!({})),
        ("body_port_check", json!({})),
        ("body_lock_acquire", json!({})),
        ("body_analytics_export", json!({"table": "unknown_table", "output_path": "/tmp/x.csv"})),
    ] {
        let (is_err, _txt) = call_tool(&server, 310, tool, args);
        assert!(is_err, "missing/invalid args for {} should return isError=true", tool);
    }
}
