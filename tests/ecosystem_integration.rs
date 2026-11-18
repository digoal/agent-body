use agent_steward::api::mcp_tools::McpToolRegistry;
use agent_steward::config::Config;
use agent_steward::ecosystem::{
    ContextBudgetRouter, ContextRouteRequest, ExternalContactsManager, McpConfigScanner,
    SkillCatalogScanner,
};
use agent_steward::storage::models::ExternalServiceRecord;
use agent_steward::storage::Storage;
use serde_json::json;

#[test]
fn test_ecosystem_end_to_end_and_35_mcp_tools() {
    let storage = Storage::open_in_memory().expect("open_in_memory");

    // 1. 验证 MCP 工具注册总数已达 35 个
    let registry = McpToolRegistry::new(storage.clone());
    let tools = registry.list_tools();
    assert_eq!(tools.len(), 35, "Phase 16 后原生 MCP 工具应为 35 个");

    // 2. 构造临时目录测试 SKILL 与 MCP 扫描
    let tmp = std::env::temp_dir().join(format!("steward_eco_e2e_{}", uuid::Uuid::new_v4()));
    let skill_a = tmp.join("skills/postgres-tuning");
    let skill_b = tmp.join("skills/video-editor");
    std::fs::create_dir_all(&skill_a).expect("mkdir a");
    std::fs::create_dir_all(&skill_b).expect("mkdir b");

    std::fs::write(
        skill_a.join("SKILL.md"),
        "---\nname: postgres-tuning\ndescription: PostgreSQL vacuum checkpoint WAL tuning\n---\n# Detail\n...",
    )
    .expect("write skill a");
    std::fs::write(
        skill_b.join("SKILL.md"),
        "---\nname: video-editor\ndescription: ffmpeg clip subtitle transcode\n---\n# Detail\n...",
    )
    .expect("write skill b");

    let mcp_cfg = tmp.join("mcp.json");
    std::fs::write(
        &mcp_cfg,
        r#"{
  "mcpServers": {
    "demo-search": {
      "command": "npx",
      "args": ["-y", "demo-search"],
      "env": { "SEARCH_TOKEN": "sk-must-not-be-persisted" }
    }
  }
}"#,
    )
    .expect("write mcp");

    let mcp_scanner = McpConfigScanner::new(storage.clone());
    let (mcp_cnt, sanitized) = mcp_scanner.scan_config_file("cursor", &mcp_cfg).expect("scan mcp");
    assert_eq!(mcp_cnt, 1);
    assert_eq!(sanitized, 1);

    let mcps = storage.list_mcp_catalog().expect("list mcp");
    assert_eq!(mcps.len(), 1);
    let dumped = serde_json::to_string(&mcps).expect("json");
    assert!(!dumped.contains("sk-must-not-be-persisted"), "MCP 扫描必须脱敏明文密钥");

    let skill_scanner = SkillCatalogScanner::new(storage.clone());
    let (sk_cnt, _) = skill_scanner.scan_roots(&[tmp.join("skills")]).expect("scan skills");
    assert_eq!(sk_cnt, 2);

    // 3. 外部服务通讯录：防明文密钥拦截 + Config 冷启动
    let contacts = ExternalContactsManager::new(storage.clone());
    let bad_svc = ExternalServiceRecord {
        id: "svc:bad".to_string(),
        service_name: "bad".to_string(),
        category: "llm".to_string(),
        endpoint: "https://api.example.com".to_string(),
        protocol: "https".to_string(),
        capabilities: "llm".to_string(),
        auth_type: "bearer_env".to_string(),
        auth_env_var: Some("API_KEY".to_string()),
        health_endpoint: None,
        usage_example: Some("Bearer sk-leak-secret".to_string()),
        status: "unknown".to_string(),
        last_checked_at: None,
    };
    assert!(contacts.register_service(&bad_svc).is_err());

    let cfg = Config::default();
    assert!(contacts.bootstrap_from_config(&cfg).expect("bootstrap") >= 1);

    // 4. Context 防爆路由器与 MCP 端点联动
    let router = ContextBudgetRouter::new(storage.clone());
    let plan = router
        .route(&ContextRouteRequest {
            intent: "PostgreSQL WAL tuning".to_string(),
            budget_tokens: 500,
            max_skills: 1,
            max_mcps: 1,
            max_services: 1,
        })
        .expect("route");
    assert_eq!(plan.skills.len(), 1);
    assert_eq!(plan.skills[0].skill_name, "postgres-tuning");
    assert!(plan.route_tokens_used <= 550);

    let mcp_out = registry
        .call_tool("steward_skill_route", json!({ "action": "route", "intent": "PostgreSQL", "budget_tokens": 500 }))
        .expect("call steward_skill_route");
    assert!(mcp_out.contains("postgres-tuning"));

    let _ = std::fs::remove_dir_all(&tmp);
}
