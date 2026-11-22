use std::sync::Arc;

use agent_body::api::hooks::AgentHookEngine;
use agent_body::hooks::{
    AutoSyncEngine, ClientHookGenerator, ClientType, EventBus, HookAction, HookDispatcher,
    HookFilter, HookPipeline, HookPipelineEngine, HookRule, HookStep, OnFailureStrategy,
    PipelineExecutionStatus, StewardEvent,
};
use agent_body::storage::Storage;

#[test]
fn test_hook_engine_pre_guard_and_post_auto_sync() {
    let storage = Storage::open_in_memory().expect("open storage");
    let engine = AgentHookEngine::new(storage.clone());

    // Destructive command must be flagged & upgraded to pessimistic_lock by pre_tool_call
    let danger = engine.pre_tool_call("run_command", &serde_json::json!({"command": "rm -rf /*"}));
    assert!(danger.has_warnings, "高危破坏命令必须触发 PreHook 警告");
    assert_eq!(danger.concurrency_mode, "PESSIMISTIC_LOCK");
    assert!(danger.warnings.iter().any(|w| w.contains("rm -rf")));

    // Create real file in tempdir and verify post hook registers asset
    let tmp = tempfile::tempdir().expect("tempdir");
    let script_path = tmp.path().join("collector.py");
    std::fs::write(&script_path, "# integration test script\nprint('ok')\n").expect("write script");

    let post = engine.post_tool_call_with_error(
        "write_file",
        &serde_json::json!({ "path": script_path.to_string_lossy() }),
        true,
        None,
    );
    assert!(post.recorded);

    let asset = storage
        .get_asset_by_path(&script_path.to_string_lossy())
        .expect("query asset");
    assert!(asset.is_some(), "PostHook 应触发 AutoSyncEngine 完成资产自动入库");

    // Verify AutoSyncEngine directly on Markdown note creates memory
    let md_path = tmp.path().join("architecture_note.md");
    std::fs::write(&md_path, "# 架构备忘\nAgent Body 双域审计与 AWR 基线闭环完成。\n").expect("write md");
    let sync_res = AutoSyncEngine::new(storage.clone())
        .sync_file_output(&md_path.to_string_lossy(), "write_file")
        .expect("auto sync");
    assert!(sync_res.asset_registered);
    assert!(sync_res.topology_updated);
}

#[test]
fn test_client_hook_generator_all_supported_clients() {
    let tmp = tempfile::tempdir().expect("tempdir");
    for client in [ClientType::Agy, ClientType::Claude, ClientType::Cursor, ClientType::Hermes, ClientType::Codex] {
        let gen = ClientHookGenerator::generate(client, Some(tmp.path()), false)
            .expect("generate client hook config");
        assert!(!gen.content.trim().is_empty());
        assert!(!gen.relative_path.trim().is_empty());
        if matches!(client, ClientType::Agy | ClientType::Claude | ClientType::Hermes) {
            assert!(!gen.companion_files.is_empty());
        }
    }
}

#[tokio::test]
async fn test_event_bus_dispatcher_and_pipeline_debounce() {
    let storage = Storage::open_in_memory().expect("open storage");
    let bus = EventBus::new();
    let mut rx = bus.subscribe();

    let ev = StewardEvent::FileCreated {
        path: "/tmp/demo_pipeline.rs".to_string(),
        source: "integration_test".to_string(),
        timestamp: "2026-11-23T00:00:00+08:00".to_string(),
    };
    let count = bus.handle().emit(ev.clone()).expect("emit");
    assert_eq!(count, 1);
    let received = rx.recv().await.expect("recv");
    assert_eq!(received.event_type(), "on_file_created");

    // Verify HookDispatcher blacklist pruning
    let dispatcher = HookDispatcher::new(
        vec![HookRule {
            event: "on_file_created".to_string(),
            filter: Some(HookFilter {
                extensions: Some(vec![".rs".to_string()]),
                ..Default::default()
            }),
            action: HookAction::Webhook {
                url: "http://127.0.0.1:9/noop".to_string(),
                timeout_ms: 50,
            },
        }],
        vec!["target".to_string(), ".git".to_string()],
    );
    let skipped = dispatcher
        .dispatch(Arc::new(StewardEvent::FileCreated {
            path: "/app/target/debug/out.rs".to_string(),
            source: "test".to_string(),
            timestamp: "2026-11-23T00:00:00+08:00".to_string(),
        }))
        .await;
    assert!(skipped.is_empty(), "黑名单路径下的文件变更严禁触发外部 Hook");

    // Verify HookPipelineEngine debouncing
    let pipeline_engine = HookPipelineEngine::new(
        vec![HookPipeline {
            id: "pipe-debounce-1".to_string(),
            name: "Debounce Test Pipeline".to_string(),
            event_pattern: "on_file_created".to_string(),
            debounce_ms: 5000,
            steps: vec![HookStep {
                name: "step-true".to_string(),
                action: HookAction::Exec {
                    command: "true".to_string(),
                },
                filter: None,
                timeout_ms: 5000,
                on_failure: OnFailureStrategy::Abort,
            }],
        }],
        Some(storage),
    );
    let first = pipeline_engine.execute(Arc::new(ev.clone())).await;
    assert_eq!(first[0].status, PipelineExecutionStatus::Success);
    let second = pipeline_engine.execute(Arc::new(ev)).await;
    assert_eq!(second[0].status, PipelineExecutionStatus::SkippedDebounced);
}

#[test]
fn test_hook_pre_critical_path_marks_critical_warning() {
    let storage = Storage::open_in_memory().expect("open storage");
    let engine = AgentHookEngine::new(storage);
    let res = engine.pre_tool_call(
        "write_file",
        &serde_json::json!({ "path": "/etc/shadow" }),
    );
    assert!(res.has_warnings);
    assert!(res.warnings.iter().any(|w| w.contains("CRITICAL")));
}

