use std::sync::Arc;
use std::time::Duration;
use agent_body::api::mcp_tools::McpToolRegistry;
use agent_body::coordination::{
    CoordinationEngine, LockRequest, LockResult, LockScope, ResourceType,
};
use agent_body::hooks::{EventBus, BodyEvent};
use agent_body::storage::Storage;
use serde_json::json;

#[test]
fn test_wait_and_acquire_push_wakeup() {
    let storage = Storage::open_in_memory().expect("open");
    let bus = EventBus::new();
    let mut rx = bus.subscribe();
    let engine = Arc::new(CoordinationEngine::with_event_bus(
        storage.provider().clone(),
        bus.handle(),
    ));

    // Agent-A 先持有 port:9100
    let req_a = LockRequest {
        resource_type: ResourceType::Port,
        resource_id: "9100".to_string(),
        scope: LockScope::Exclusive,
        holder_agent: "agent-a".to_string(),
        holder_pid: None,
        ttl_seconds: 60,
        metadata: None,
    };
    let lock_id_a = match engine.try_acquire(&req_a).expect("acquire a") {
        LockResult::Granted { lock_id, .. } => lock_id,
        other => panic!("expected Granted, got {:?}", other),
    };

    let eng_a = engine.clone();
    let releaser = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(80));
        eng_a.release(&lock_id_a, "agent-a").expect("release a");
    });

    // Agent-B 排队等待并拿锁
    let req_b = LockRequest {
        resource_type: ResourceType::Port,
        resource_id: "9100".to_string(),
        scope: LockScope::Exclusive,
        holder_agent: "agent-b".to_string(),
        holder_pid: None,
        ttl_seconds: 60,
        metadata: None,
    };
    let res_b = engine
        .wait_and_acquire(&req_b, 2000, 10)
        .expect("wait_and_acquire b");
    assert!(
        matches!(res_b, LockResult::Granted { .. }),
        "Agent-A 释放后 Agent-B 应被唤醒并拿到锁: {:?}",
        res_b
    );

    releaser.join().expect("join");

    // 验证 EventBus 确实广播了 LockWaiterNotified 唤醒事件
    let mut got_notified = false;
    while let Ok(ev) = rx.try_recv() {
        if let BodyEvent::LockWaiterNotified { notified_agent, .. } = ev.as_ref() {
            if notified_agent == "agent-b" {
                got_notified = true;
            }
        }
    }
    assert!(got_notified, "释放锁后应向 EventBus 发射 LockWaiterNotified");
}

#[test]
fn test_wait_and_acquire_deadlock_interception_and_37_mcp_tools() {
    let storage = Storage::open_in_memory().expect("open");
    let engine = Arc::new(CoordinationEngine::new(storage.provider().clone()));

    // 验证 43 个 MCP 工具
    let registry = McpToolRegistry::new(storage.clone());
    assert_eq!(registry.list_tools().len(), 43);

    // A 持 9201，B 持 9202
    let _ = engine
        .try_acquire(&LockRequest {
            resource_type: ResourceType::Port,
            resource_id: "9201".to_string(),
            scope: LockScope::Exclusive,
            holder_agent: "agent-a".to_string(),
            holder_pid: None,
            ttl_seconds: 60,
            metadata: None,
        })
        .expect("a 9201");
    let _ = engine
        .try_acquire(&LockRequest {
            resource_type: ResourceType::Port,
            resource_id: "9202".to_string(),
            scope: LockScope::Exclusive,
            holder_agent: "agent-b".to_string(),
            holder_pid: None,
            ttl_seconds: 60,
            metadata: None,
        })
        .expect("b 9202");

    // 模拟 A 正在等 9202
    let _ = storage.enqueue_lock_wait(&agent_body::storage::models::LockWaitEntry {
        wait_id: "w-a-9202".to_string(),
        resource_type: "port".to_string(),
        resource_id: "9202".to_string(),
        scope: "exclusive".to_string(),
        waiting_agent: "agent-a".to_string(),
        priority: 100,
        enqueued_at: "2026-11-18T00:00:00Z".to_string(),
        timeout_at: "2099-01-01T00:00:00Z".to_string(),
        status: "waiting".to_string(),
    });

    // B 申请等 9201 -> 立即构成 A->B->A 死锁环路并被拦截
    let res = engine
        .wait_and_acquire(
            &LockRequest {
                resource_type: ResourceType::Port,
                resource_id: "9201".to_string(),
                scope: LockScope::Exclusive,
                holder_agent: "agent-b".to_string(),
                holder_pid: None,
                ttl_seconds: 60,
                metadata: None,
            },
            2000,
            100,
        )
        .expect("wait_and_acquire");

    match res {
        LockResult::Deadlock { cycle_agents, .. } => {
            assert!(cycle_agents.contains(&"agent-a".to_string()));
            assert!(cycle_agents.contains(&"agent-b".to_string()));
        }
        other => panic!("必须拦截并返回 LockResult::Deadlock，实际: {:?}", other),
    }

    // 验证 MCP body_lock_deadlock_check 返回环路报告
    let out = registry
        .call_tool("body_lock_deadlock_check", json!({}))
        .expect("deadlock check");
    assert!(out.contains("wait_queue_count"));
}
