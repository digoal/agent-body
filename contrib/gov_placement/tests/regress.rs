//! `ext-gov-placement` 集成回归测试：安装、私有表创建、四端派生与非 tmp 临时脚本拦截

use std::path::PathBuf;

use agent_body_kernel::{DispatchError, HttpRequest, KernelRuntime};
use agent_body_sdk::{ClientAuthContext, TransportKind};
use ext_gov_placement::{
    resolve_workspace_base, to_snake_case_name, GovPlacementExtension, PlacementConfig,
};
use tempfile::TempDir;

#[test]
fn snake_case_and_dynamic_workspace_base_have_zero_hardcoded_host_paths() {
    assert_eq!(to_snake_case_name("MyTempTool.py"), "my_temp_tool.py");
    assert_eq!(to_snake_case_name("sec-scraper"), "sec_scraper");
    assert_eq!(to_snake_case_name("Report Draft v2.md"), "report_draft_v2.md");

    let explicit = resolve_workspace_base(Some("/custom/ws"), None);
    assert_eq!(explicit, PathBuf::from("/custom/ws"));

    let fallback = resolve_workspace_base(None, None);
    let fallback_str = fallback.to_string_lossy();
    assert!(!fallback_str.contains("/root/new"));
    assert!(!fallback_str.contains("user:digoal"));
}

#[test]
fn runtime_mounts_proc_to_mcp_cli_rest_parity_and_enforces_temp_guard_and_private_tables() {
    let ws = TempDir::new().unwrap();
    let mut rt = KernelRuntime::open_sqlite_in_memory(None).unwrap();
    let ext = GovPlacementExtension::new(PlacementConfig {
        base_dir: Some(ws.path().to_path_buf()),
    });
    let order = rt.install_extensions(&[Box::new(ext)]).unwrap();
    assert_eq!(order, vec!["gov_placement"]);

    // 1. 验证 ext_gov_placement__rules 与 ext_gov_placement__history 私有表已创建并可写入
    rt.table_am()
        .exec_scoped_migration(
            "gov_placement",
            "ext_gov_placement__",
            "0.1.1",
            "INSERT INTO ext_gov_placement__history (history_id, caller_id, category, raw_name, suggested_path, verdict, created_at)
             VALUES ('h-1', 'tester', 'script', 'MyTool.py', '/ws/shared_proj/my_tool.py', 'allow', '2026-11-27T16:00:00+08:00');",
        )
        .unwrap();

    // 验证 Parity Matrix 自动包含 ext_placement_suggest
    let parity_resp = rt.handle_http(&HttpRequest {
        method: "GET".to_string(),
        path: "/api/v1/parity/matrix".to_string(),
        bearer_token: None,
        is_loopback: true,
        body: serde_json::Value::Null,
    });
    assert_eq!(parity_resp.status_code, 200);
    assert_eq!(parity_resp.body["total"], 1);
    assert_eq!(
        parity_resp.body["matrix"][0]["proc_name"],
        "ext_placement_suggest"
    );

    // 2. MCP tools/list & tools/call
    let mcp_auth = ClientAuthContext {
        transport: TransportKind::StdioMcp,
        cert: None,
        bearer_token: None,
        method: "POST".to_string(),
        path: "/mcp".to_string(),
        is_loopback: true,
        is_read_only: false,
    };
    let mcp_list = rt.handle_mcp(
        &serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}),
        &mcp_auth,
    );
    assert_eq!(
        mcp_list["result"]["tools"][0]["name"],
        "ext_placement_suggest"
    );

    let mcp_call = rt.handle_mcp(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "ext_placement_suggest",
                "arguments": { "category": "project", "name": "OrderBillingService" }
            }
        }),
        &mcp_auth,
    );
    assert_eq!(
        mcp_call["result"]["structuredContent"]["sanitized_name"],
        "order_billing_service"
    );
    let expected_proj = ws.path().join("work").join("order_billing_service");
    assert_eq!(
        mcp_call["result"]["structuredContent"]["suggested_path"],
        expected_proj.to_string_lossy().as_ref()
    );

    // 3. CLI 路由派发 (script / document / temp)
    let cli_auth = ClientAuthContext {
        transport: TransportKind::Cli,
        cert: None,
        bearer_token: None,
        method: "EXEC".to_string(),
        path: "gov/placement/suggest".to_string(),
        is_loopback: true,
        is_read_only: false,
    };
    let cli_doc = rt
        .handle_cli(
            &[
                "gov".to_string(),
                "placement".to_string(),
                "suggest".to_string(),
                "--category".to_string(),
                "document".to_string(),
                "--name".to_string(),
                "DesignNotes.md".to_string(),
            ],
            None,
            &cli_auth,
        )
        .unwrap();
    assert_eq!(
        cli_doc.data["suggested_path"],
        ws.path()
            .join("note")
            .join("design_notes.md")
            .to_string_lossy()
            .as_ref()
    );

    // 4. HTTP REST 派发 (temp)
    let rest_resp = rt.handle_http(&HttpRequest {
        method: "POST".to_string(),
        path: "/api/v1/gov/placement/suggest".to_string(),
        bearer_token: None,
        is_loopback: true,
        body: serde_json::json!({
            "category": "temp",
            "name": "QuickScratch.sh"
        }),
    });
    assert_eq!(rest_resp.status_code, 200);
    assert_eq!(
        rest_resp.body["data"]["suggested_path"],
        ws.path()
            .join("tmp")
            .join("quick_scratch.sh")
            .to_string_lossy()
            .as_ref()
    );

    // 5. ExecutorStartHook 规则拦截：非 tmp 目录临时脚本被阻断 (403)
    let non_tmp_path = ws.path().join("work").join("tmp_probe.py");
    let blocked_http = rt.handle_http(&HttpRequest {
        method: "POST".to_string(),
        path: "/api/v1/gov/placement/suggest".to_string(),
        bearer_token: None,
        is_loopback: true,
        body: serde_json::json!({
            "category": "script",
            "name": "tmp_probe.py",
            "intent": "临时抓包调试脚本",
            "target_path": non_tmp_path.to_string_lossy()
        }),
    });
    assert_eq!(blocked_http.status_code, 403);
    assert!(blocked_http.body["error"]
        .as_str()
        .unwrap_or("")
        .contains("tmp"));

    // 6. warn_only=true 时给出确认风险提示 (RequireConfirm)
    let warn_err = rt
        .handle_cli(
            &[
                "gov".to_string(),
                "placement".to_string(),
                "suggest".to_string(),
            ],
            Some(serde_json::json!({
                "category": "temp",
                "name": "tmp_probe.sh",
                "target_path": non_tmp_path.to_string_lossy(),
                "warn_only": true
            })),
            &cli_auth,
        )
        .unwrap_err();
    assert!(matches!(warn_err, DispatchError::RequireConfirm { .. }));

    // 7. 若路径在 tmp/ 下则放行
    let safe_tmp_path = ws.path().join("tmp").join("tmp_probe.py");
    let allowed_http = rt.handle_http(&HttpRequest {
        method: "POST".to_string(),
        path: "/api/v1/gov/placement/suggest".to_string(),
        bearer_token: None,
        is_loopback: true,
        body: serde_json::json!({
            "category": "temp",
            "name": "tmp_probe.py",
            "target_path": safe_tmp_path.to_string_lossy()
        }),
    });
    assert_eq!(allowed_http.status_code, 200);
}
