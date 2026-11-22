use std::sync::Arc;
use std::thread;
use agent_body::storage::{CoordinationLock, Storage};
use agent_body::coordination::{CoordinationEngine, LockRequest, LockResult, LockScope, ResourceType};
use agent_body::api::hooks::AgentHookEngine;

#[test]
fn test_concurrent_port_lock_exclusion() {
    let storage = Storage::open_in_memory().expect("open storage");
    let engine = Arc::new(CoordinationEngine::new(storage.provider().clone()));

    let mut handles = Vec::new();
    for i in 1..=2 {
        let engine_clone = engine.clone();
        let handle = thread::spawn(move || {
            let req = LockRequest {
                resource_type: ResourceType::Port,
                resource_id: "8088".to_string(),
                scope: LockScope::Exclusive,
                holder_agent: format!("agent-{}", i),
                holder_pid: Some(1000 + i as u32),
                ttl_seconds: 60,
                metadata: None,
            };
            engine_clone.try_acquire(&req).expect("try acquire")
        });
        handles.push(handle);
    }

    let mut granted_count = 0;
    let mut denied_count = 0;

    for h in handles {
        match h.join().expect("thread join") {
            LockResult::Granted { .. } => granted_count += 1,
            LockResult::Denied { .. } => denied_count += 1,
            LockResult::Deadlock { suggestion, .. } => panic!("Unexpected deadlock: {}", suggestion),
            LockResult::Timeout { suggestion, .. } => panic!("Unexpected timeout: {}", suggestion),
            LockResult::Error { message } => panic!("Unexpected error: {}", message),
        }
    }

    assert_eq!(granted_count, 1, "两个并发竞争排他锁的 Agent 必须恰好 1 个获得锁");
    assert_eq!(denied_count, 1, "未获得排他锁的 Agent 必须收到 Denied");
}

#[test]
fn test_concurrent_directory_lock_shared() {
    let storage = Storage::open_in_memory().expect("open storage");
    let engine = Arc::new(CoordinationEngine::new(storage.provider().clone()));

    let mut handles = Vec::new();
    for i in 1..=3 {
        let engine_clone = engine.clone();
        let handle = thread::spawn(move || {
            let req = LockRequest {
                resource_type: ResourceType::Directory,
                resource_id: "/root/new/shared_data/".to_string(),
                scope: LockScope::Shared,
                holder_agent: format!("reader-agent-{}", i),
                holder_pid: None,
                ttl_seconds: 60,
                metadata: None,
            };
            engine_clone.try_acquire(&req).expect("try acquire shared")
        });
        handles.push(handle);
    }

    let mut granted_count = 0;
    for h in handles {
        if let LockResult::Granted { .. } = h.join().expect("join") {
            granted_count += 1;
        }
    }

    assert_eq!(granted_count, 3, "所有申请共享锁的 Agent 均应成功获取");
}

#[test]
fn test_lock_then_hook_integration() {
    let storage = Storage::open_in_memory().expect("open storage");
    let engine = CoordinationEngine::new(storage.provider().clone());

    let req = LockRequest {
        resource_type: ResourceType::Port,
        resource_id: "8089".to_string(),
        scope: LockScope::Exclusive,
        holder_agent: "agent-owner".to_string(),
        holder_pid: None,
        ttl_seconds: 120,
        metadata: None,
    };
    let acq = engine.try_acquire(&req).expect("acquire");
    assert!(matches!(acq, LockResult::Granted { .. }));

    // 模拟第三方 Agent 调用工具前触发 PreHook
    let hook_engine = AgentHookEngine::new(storage);
    let pre_res = hook_engine.pre_tool_call("bind_port", &serde_json::json!({
        "port": 8089
    }));

    assert!(pre_res.has_warnings, "已加锁资源在 pre-hook 阶段必须产生告警");
    assert!(pre_res.warnings.iter().any(|w| w.contains("agent-owner")), "告警信息必须清晰呈现持有者 Agent 标识");
}

#[test]
fn test_ttl_expiry_and_reacquire() {
    let storage = Storage::open_in_memory().expect("open storage");
    let engine = CoordinationEngine::new(storage.provider().clone());

    let req1 = LockRequest {
        resource_type: ResourceType::Service,
        resource_id: "test-microservice.service".to_string(),
        scope: LockScope::Exclusive,
        holder_agent: "agent-first".to_string(),
        holder_pid: None,
        ttl_seconds: 5, // 最小 5 秒
        metadata: None,
    };
    let res1 = engine.try_acquire(&req1).expect("acquire 1");
    let lock_id = match res1 {
        LockResult::Granted { lock_id, .. } => lock_id,
        _ => panic!("acquire 1 failed"),
    };

    // 人工将该锁的 expires_at 篡改为过去的时间戳，模拟 TTL 到期
    let expired_lock = CoordinationLock {
        lock_id,
        resource_type: "service".to_string(),
        resource_id: "test-microservice.service".to_string(),
        scope: "exclusive".to_string(),
        holder_agent: "agent-first".to_string(),
        holder_pid: None,
        acquired_at: "2020-01-01T00:00:00Z".to_string(),
        expires_at: "2020-01-01T00:00:01Z".to_string(),
        metadata: None,
    };
    let _ = storage.insert_coordination_lock(&expired_lock);

    // Agent B 尝试获取同一资源锁，应成功顶替/获取
    let req2 = LockRequest {
        resource_type: ResourceType::Service,
        resource_id: "test-microservice.service".to_string(),
        scope: LockScope::Exclusive,
        holder_agent: "agent-second".to_string(),
        holder_pid: None,
        ttl_seconds: 60,
        metadata: None,
    };
    let res2 = engine.try_acquire(&req2).expect("acquire 2");
    assert!(matches!(res2, LockResult::Granted { .. }), "过期后其他 Agent 必须能够获取该资源锁");
}

#[test]
fn test_renew_extends_ttl() {
    let storage = Storage::open_in_memory().expect("open storage");
    let engine = CoordinationEngine::new(storage.provider().clone());

    let req = LockRequest {
        resource_type: ResourceType::Custom,
        resource_id: "deploy-pipeline-step".to_string(),
        scope: LockScope::Exclusive,
        holder_agent: "ci-agent".to_string(),
        holder_pid: None,
        ttl_seconds: 30,
        metadata: None,
    };
    let res = engine.try_acquire(&req).expect("acquire");
    let lock_id = match res {
        LockResult::Granted { lock_id, expires_at, .. } => {
            assert!(!expires_at.is_empty());
            lock_id
        }
        _ => panic!("acquire should succeed"),
    };

    let ok = engine.renew(&lock_id, "ci-agent", 120).expect("renew");
    assert!(ok, "持有者续约必须成功");
}

#[test]
fn test_force_release_by_admin() {
    let storage = Storage::open_in_memory().expect("open storage");
    let engine = CoordinationEngine::new(storage.provider().clone());

    let req = LockRequest {
        resource_type: ResourceType::Directory,
        resource_id: "/etc/kubernetes/manifests/".to_string(),
        scope: LockScope::Exclusive,
        holder_agent: "troubled-agent".to_string(),
        holder_pid: None,
        ttl_seconds: 300,
        metadata: None,
    };
    let res = engine.try_acquire(&req).expect("acquire");
    let lock_id = match res {
        LockResult::Granted { lock_id, .. } => lock_id,
        _ => panic!("acquire should succeed"),
    };

    // 管理员执行强制解除
    let force_ok = engine.force_release(&lock_id).expect("force release");
    assert!(force_ok);

    // 确认已释放
    let check = engine.check_resource(&ResourceType::Directory, "/etc/kubernetes/manifests/").expect("check");
    assert!(check.is_none());
}

#[test]
fn test_gc_cleans_expired_only() {
    let storage = Storage::open_in_memory().expect("open storage");
    let engine = CoordinationEngine::new(storage.provider().clone());

    // 1. 插入一个过去时间已过期的锁
    let expired = CoordinationLock {
        lock_id: "lock-expired-1".to_string(),
        resource_type: "port".to_string(),
        resource_id: "7001".to_string(),
        scope: "exclusive".to_string(),
        holder_agent: "zombie-agent".to_string(),
        holder_pid: None,
        acquired_at: "2020-01-01T00:00:00Z".to_string(),
        expires_at: "2020-01-01T00:00:05Z".to_string(),
        metadata: None,
    };
    storage.insert_coordination_lock(&expired).expect("insert expired");

    // 2. 插入一个有效的活跃锁
    let active_req = LockRequest {
        resource_type: ResourceType::Port,
        resource_id: "7002".to_string(),
        scope: LockScope::Exclusive,
        holder_agent: "live-agent".to_string(),
        holder_pid: None,
        ttl_seconds: 300,
        metadata: None,
    };
    let _ = engine.try_acquire(&active_req).expect("acquire active");

    // 3. 执行 GC
    let cleaned = engine.gc_expired_locks().expect("gc");
    assert!(cleaned >= 1, "GC 应至少清理掉 1 个已过期的僵尸锁");

    // 4. 验证活跃锁依然存在
    let active_check = engine.check_resource(&ResourceType::Port, "7002").expect("check");
    assert!(active_check.is_some(), "活跃锁在 GC 后必须安全保留");
}

#[test]
fn test_pg_advisory_lock_cross_connection() {
    use agent_body::storage::PgStorageProvider;
    const PG_TEST_URL: &str = "postgresql://postgres@127.0.0.1:5432/agent_body_test";

    let p1 = match PgStorageProvider::connect(PG_TEST_URL) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("本地 PostgreSQL 未就绪，跳过跨连接 Advisory Lock 测试");
            return;
        }
    };
    let p2 = match PgStorageProvider::connect(PG_TEST_URL) {
        Ok(p) => p,
        Err(_) => return,
    };

    let lock_key: i64 = 0x1234_5678_9ABC;

    // 连接 1 获取 advisory lock
    let g1 = p1.try_advisory_lock(lock_key).expect("p1 lock");
    assert!(g1, "连接 1 应成功获取 Advisory Lock");

    // 连接 2 竞争同一 lock_key，应返回 false (互斥)
    let g2 = p2.try_advisory_lock(lock_key).expect("p2 lock");
    assert!(!g2, "连接 2 竞争同一 Advisory Lock 应被拒绝");

    // 连接 1 释放 lock
    let u1 = p1.advisory_unlock(lock_key).expect("p1 unlock");
    assert!(u1, "连接 1 应成功解锁");

    // 连接 2 重新尝试获取，应成功
    let g2_retry = p2.try_advisory_lock(lock_key).expect("p2 retry");
    assert!(g2_retry, "连接 1 释放后，连接 2 应能够获取该锁");

    // 连接 2 解锁清理
    let _ = p2.advisory_unlock(lock_key);
}
