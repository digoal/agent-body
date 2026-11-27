pub mod lock_registry;
pub mod deadlock;

use std::sync::Arc;
use anyhow::Result;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use lock_registry::{LockRegistry, LockScope, ResourceType};
pub use deadlock::{DeadlockCycle, DeadlockDetector, DeadlockReport};
use crate::storage::{CoordinationLock, LockWaitEntry, StorageProvider};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockRequest {
    pub resource_type: ResourceType,
    pub resource_id: String,
    #[serde(default = "default_scope")]
    pub scope: LockScope,
    pub holder_agent: String,
    pub holder_pid: Option<u32>,
    #[serde(default = "default_ttl")]
    pub ttl_seconds: u64,
    pub metadata: Option<serde_json::Value>,
}

fn default_scope() -> LockScope {
    LockScope::Exclusive
}

fn default_ttl() -> u64 {
    300
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum LockResult {
    Granted {
        lock_id: String,
        resource_type: String,
        resource_id: String,
        expires_at: String,
    },
    Denied {
        holder_agent: String,
        holder_since: String,
        expires_at: String,
        suggestion: String,
    },
    Deadlock {
        cycle_agents: Vec<String>,
        cycle_resources: Vec<String>,
        suggestion: String,
    },
    Timeout {
        wait_id: String,
        suggestion: String,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockInfo {
    pub lock_id: String,
    pub resource_type: String,
    pub resource_id: String,
    pub scope: String,
    pub holder_agent: String,
    pub holder_pid: Option<u32>,
    pub acquired_at: String,
    pub expires_at: String,
    pub metadata: Option<serde_json::Value>,
}

impl From<CoordinationLock> for LockInfo {
    fn from(c: CoordinationLock) -> Self {
        Self {
            lock_id: c.lock_id,
            resource_type: c.resource_type,
            resource_id: c.resource_id,
            scope: c.scope,
            holder_agent: c.holder_agent,
            holder_pid: c.holder_pid,
            acquired_at: c.acquired_at,
            expires_at: c.expires_at,
            metadata: c.metadata,
        }
    }
}

pub struct CoordinationEngine {
    storage: Arc<dyn StorageProvider>,
    event_bus: Option<crate::hooks::EventBusHandle>,
}

impl CoordinationEngine {
    pub fn new(storage: Arc<dyn StorageProvider>) -> Self {
        Self { storage, event_bus: None }
    }

    pub fn with_event_bus(storage: Arc<dyn StorageProvider>, event_bus: crate::hooks::EventBusHandle) -> Self {
        Self { storage, event_bus: Some(event_bus) }
    }

    /// 尝试获取资源协作互斥/共享锁（非阻塞）
    pub fn try_acquire(&self, req: &LockRequest) -> Result<LockResult> {
        let _wait = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::LockCoordination);
        let norm_id = LockRegistry::normalize_resource_id(&req.resource_type, &req.resource_id);
        let rt_str = req.resource_type.as_str();

        // 1. 查询当前是否已被加锁
        if let Some(existing) = self.storage.get_coordination_lock(rt_str, &norm_id)? {
            // 如果已经被同一 Agent 持有，允许重入并续期
            if existing.holder_agent == req.holder_agent {
                let new_expires = (Utc::now() + Duration::seconds(req.ttl_seconds.min(3600) as i64)).to_rfc3339();
                let updated_lock = CoordinationLock {
                    lock_id: existing.lock_id.clone(),
                    resource_type: rt_str.to_string(),
                    resource_id: norm_id.clone(),
                    scope: req.scope.as_str().to_string(),
                    holder_agent: req.holder_agent.clone(),
                    holder_pid: req.holder_pid,
                    acquired_at: existing.acquired_at,
                    expires_at: new_expires.clone(),
                    metadata: req.metadata.clone(),
                };
                let _renewed = self.storage.insert_coordination_lock(&updated_lock)?;
                return Ok(LockResult::Granted {
                    lock_id: existing.lock_id,
                    resource_type: rt_str.to_string(),
                    resource_id: norm_id,
                    expires_at: new_expires,
                });
            }

            // 排他锁冲突：任意一方为 exclusive 即拒绝
            if req.scope == LockScope::Exclusive || existing.scope == "exclusive" {
                return Ok(LockResult::Denied {
                    holder_agent: existing.holder_agent.clone(),
                    holder_since: existing.acquired_at,
                    expires_at: existing.expires_at.clone(),
                    suggestion: format!(
                        "资源 '{}:{}' 当前被 Agent '{}' 持有至 {}。建议等待或申请其他资源。",
                        rt_str, norm_id, existing.holder_agent, existing.expires_at
                    ),
                });
            }
        }

        // 2. 构造并插入新锁
        let lock_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let ttl = req.ttl_seconds.clamp(5, 3600); // 最小 5 秒，最大 1 小时
        let expires_at = (now + Duration::seconds(ttl as i64)).to_rfc3339();

        let lock = CoordinationLock {
            lock_id: lock_id.clone(),
            resource_type: rt_str.to_string(),
            resource_id: norm_id.clone(),
            scope: req.scope.as_str().to_string(),
            holder_agent: req.holder_agent.clone(),
            holder_pid: req.holder_pid,
            acquired_at: now.to_rfc3339(),
            expires_at: expires_at.clone(),
            metadata: req.metadata.clone(),
        };

        let granted = self.storage.insert_coordination_lock(&lock)?;
        if granted {
            if let Some(ref bus) = self.event_bus {
                if let Err(e) = bus.emit(crate::hooks::BodyEvent::LockAcquired {
                    resource_type: rt_str.to_string(),
                    resource_id: norm_id.clone(),
                    holder_agent: req.holder_agent.clone(),
                    ttl_seconds: ttl,
                    timestamp: now.to_rfc3339(),
                }) {
                    eprintln!("[Coordination][WARN] 广播 LockAcquired 事件失败: {}", e);
                }
            }
            Ok(LockResult::Granted {
                lock_id,
                resource_type: rt_str.to_string(),
                resource_id: norm_id,
                expires_at,
            })
        } else {
            // 并发竞争下被其他 Agent 先手拿走
            if let Some(existing) = self.storage.get_coordination_lock(rt_str, &norm_id)? {
                Ok(LockResult::Denied {
                    holder_agent: existing.holder_agent.clone(),
                    holder_since: existing.acquired_at,
                    expires_at: existing.expires_at.clone(),
                    suggestion: format!("并发竞争中资源被 Agent '{}' 优先获取。", existing.holder_agent),
                })
            } else {
                Ok(LockResult::Error {
                    message: "协作锁写入失败".to_string(),
                })
            }
        }
    }

    /// 释放锁（校验持有者身份）
    pub fn release(&self, lock_id: &str, holder_agent: &str) -> Result<bool> {
        let _wait = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::LockCoordination);
        let locks = self.storage.list_coordination_locks(None)?;
        let mut target_lock = None;
        if let Some(l) = locks.iter().find(|x| x.lock_id == lock_id) {
            if l.holder_agent != holder_agent {
                return Ok(false); // 拒绝非持有者越权释放
            }
            target_lock = Some(l.clone());
        }
        let res = self.storage.delete_coordination_lock(lock_id)?;
        if res {
            if let Some(l) = target_lock {
                if let Some(ref bus) = self.event_bus {
                    if let Err(e) = bus.emit(crate::hooks::BodyEvent::LockReleased {
                        resource_type: l.resource_type.clone(),
                        resource_id: l.resource_id.clone(),
                        holder_agent: l.holder_agent,
                        timestamp: Utc::now().to_rfc3339(),
                    }) {
                        eprintln!("[Coordination][WARN] 广播 LockReleased 事件失败: {}", e);
                    }
                }
                self.notify_next_waiter(&l.resource_type, &l.resource_id);
            }
        }
        Ok(res)
    }

    /// 强制释放锁（管理员/运维 CLI 操作）
    pub fn force_release(&self, lock_id: &str) -> Result<bool> {
        let _wait = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::LockCoordination);
        let locks = self.storage.list_coordination_locks(None)?;
        let target_lock = locks.into_iter().find(|x| x.lock_id == lock_id);
        let res = self.storage.delete_coordination_lock(lock_id)?;
        if res {
            if let Some(l) = target_lock {
                if let Some(ref bus) = self.event_bus {
                    if let Err(e) = bus.emit(crate::hooks::BodyEvent::LockReleased {
                        resource_type: l.resource_type.clone(),
                        resource_id: l.resource_id.clone(),
                        holder_agent: l.holder_agent,
                        timestamp: Utc::now().to_rfc3339(),
                    }) {
                        eprintln!("[Coordination][WARN] 广播 LockReleased 事件失败: {}", e);
                    }
                }
                self.notify_next_waiter(&l.resource_type, &l.resource_id);
            }
        }
        Ok(res)
    }

    /// 统一释放入口：支持按 `lock_id` 或 `(resource_type, resource_id)` 解析目标锁，并按 `force` 决定是否校验 `holder_agent`
    pub fn release_flexible(
        &self,
        lock_id: Option<&str>,
        resource_type: Option<&str>,
        resource_id: Option<&str>,
        holder_agent: Option<&str>,
        force: bool,
    ) -> Result<(Option<String>, bool)> {
        let explicit_id = lock_id.map(str::trim).filter(|s| !s.is_empty());
        let resolved_id = if let Some(id) = explicit_id {
            id.to_string()
        } else if let (Some(rt), Some(rid)) = (
            resource_type.map(str::trim).filter(|s| !s.is_empty()),
            resource_id.map(str::trim).filter(|s| !s.is_empty()),
        ) {
            let rt_enum = ResourceType::from_str_loose(rt);
            let norm_id = LockRegistry::normalize_resource_id(&rt_enum, rid);
            let Some(existing) = self.storage.get_coordination_lock(rt_enum.as_str(), &norm_id)? else {
                return Ok((None, false));
            };
            existing.lock_id
        } else {
            anyhow::bail!("必须提供 lock_id 或 (resource_type + resource_id)");
        };

        let released = if force {
            self.force_release(&resolved_id)?
        } else {
            self.release(&resolved_id, holder_agent.unwrap_or("current-agent"))?
        };
        Ok((Some(resolved_id), released))
    }

    /// 续期
    pub fn renew(&self, lock_id: &str, holder_agent: &str, additional_seconds: u64) -> Result<bool> {
        let locks = self.storage.list_coordination_locks(None)?;
        if let Some(l) = locks.into_iter().find(|x| x.lock_id == lock_id) {
            if l.holder_agent != holder_agent {
                return Ok(false);
            }
            let add_sec = additional_seconds.clamp(5, 3600);
            let current_exp = chrono::DateTime::parse_from_rfc3339(&l.expires_at)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let new_exp = (current_exp + Duration::seconds(add_sec as i64)).to_rfc3339();

            let mut updated = l;
            updated.expires_at = new_exp;
            return self.storage.insert_coordination_lock(&updated);
        }
        Ok(false)
    }

    /// 查询特定资源的活跃锁
    pub fn check_resource(&self, resource_type: &ResourceType, resource_id: &str) -> Result<Option<LockInfo>> {
        let norm_id = LockRegistry::normalize_resource_id(resource_type, resource_id);
        let opt = self.storage.get_coordination_lock(resource_type.as_str(), &norm_id)?;
        Ok(opt.map(LockInfo::from))
    }

    /// 列出所有活跃有效锁
    pub fn list_active_locks(&self, resource_type: Option<&ResourceType>) -> Result<Vec<LockInfo>> {
        let rt_str = resource_type.map(|r| r.as_str());
        let list = self.storage.list_coordination_locks(rt_str)?;
        Ok(list.into_iter().map(LockInfo::from).collect())
    }

    /// 清理所有过期锁并唤醒排队等待者
    pub fn gc_expired_locks(&self) -> Result<usize> {
        if let Err(e) = self.storage.gc_expired_lock_waits() {
            eprintln!("[Coordination][WARN] 清理过期等待队列失败: {}", e);
        }
        let cleaned = self.storage.gc_expired_coordination_locks()?;
        if cleaned > 0 {
            if let Ok(waits) = self.storage.list_lock_wait_queue(None, None) {
                for w in waits {
                    if w.status == "waiting" {
                        if let Ok(None) = self.storage.get_coordination_lock(&w.resource_type, &w.resource_id) {
                            self.notify_next_waiter(&w.resource_type, &w.resource_id);
                        }
                    }
                }
            }
            let st = crate::storage::Storage::new(self.storage.clone());
            crate::audit::AuditLogger::record_best_effort(
                &st,
                crate::audit::AuditEventInput::out_of_agent(
                    "daemon_scheduler",
                    "coordination",
                    "lock_gc",
                    "gc_expired_locks",
                )
                .with_target("table", "coordination_locks")
                .with_outcome("success")
                .with_delta(serde_json::json!({
                    "expired_locks_cleaned": cleaned,
                })),
            );
        }
        Ok(cleaned)
    }

    /// 弹出同资源下一个等待者并推送 EventBus 唤醒信号
    fn notify_next_waiter(&self, resource_type: &str, resource_id: &str) {
        if let Ok(Some(waiter)) = self.storage.pop_next_lock_waiter(resource_type, resource_id) {
            if let Some(ref bus) = self.event_bus {
                if let Err(e) = bus.emit(crate::hooks::BodyEvent::LockWaiterNotified {
                    wait_id: waiter.wait_id,
                    resource_type: waiter.resource_type,
                    resource_id: waiter.resource_id,
                    notified_agent: waiter.waiting_agent,
                    timestamp: Utc::now().to_rfc3339(),
                }) {
                    eprintln!("[Coordination][WARN] 广播 LockWaiterNotified 事件失败: {}", e);
                }
            }
        }
    }

    /// 全局死锁环路检测 (Wait-For Graph)
    pub fn detect_deadlocks(&self) -> Result<DeadlockReport> {
        let active_locks = self.storage.list_coordination_locks(None)?;
        let waits = self.storage.list_lock_wait_queue(None, None)?;
        Ok(DeadlockDetector::detect(&active_locks, &waits, None))
    }

    /// 列出等待队列
    pub fn list_wait_queue(&self, resource_type: Option<&ResourceType>) -> Result<Vec<LockWaitEntry>> {
        self.storage.list_lock_wait_queue(resource_type.map(|r| r.as_str()), None)
    }

    /// 带死锁拦截与等待队列唤醒的协作锁获取原语
    pub fn wait_and_acquire(
        &self,
        req: &LockRequest,
        wait_timeout_ms: u64,
        priority: i32,
    ) -> Result<LockResult> {
        // 1. 先尝试直接拿锁
        match self.try_acquire(req)? {
            granted @ LockResult::Granted { .. } => return Ok(granted),
            LockResult::Error { message } => return Ok(LockResult::Error { message }),
            _ => {}
        }

        let norm_id = LockRegistry::normalize_resource_id(&req.resource_type, &req.resource_id);
        let rt_str = req.resource_type.as_str();

        // 2. 死锁环路预检测
        let active_locks = self.storage.list_coordination_locks(None)?;
        let waits = self.storage.list_lock_wait_queue(None, None)?;
        let dl_rep = DeadlockDetector::detect(
            &active_locks,
            &waits,
            Some((&req.holder_agent, rt_str, &norm_id)),
        );
        if dl_rep.has_deadlock {
            if let Some(first) = dl_rep.cycles.into_iter().next() {
                return Ok(LockResult::Deadlock {
                    cycle_agents: first.cycle_agents,
                    cycle_resources: first.cycle_resources,
                    suggestion: first.suggestion,
                });
            }
        }

        // 3. 入队等待
        let wait_id = format!("wait-{}", Uuid::new_v4());
        let now = Utc::now();
        let timeout_ms = wait_timeout_ms.clamp(50, 60_000);
        let timeout_at = (now + Duration::milliseconds(timeout_ms as i64)).to_rfc3339();
        let entry = LockWaitEntry {
            wait_id: wait_id.clone(),
            resource_type: rt_str.to_string(),
            resource_id: norm_id.clone(),
            scope: req.scope.as_str().to_string(),
            waiting_agent: req.holder_agent.clone(),
            priority,
            enqueued_at: now.to_rfc3339(),
            timeout_at,
            status: "waiting".to_string(),
        };
        self.storage.enqueue_lock_wait(&entry)?;

        // 4. 订阅 EventBus 或短周期检查，直到成功或超时
        let mut rx_opt = self.event_bus.as_ref().map(|b| b.subscribe());
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);

        loop {
            if let LockResult::Granted { lock_id, resource_type, resource_id, expires_at } = self.try_acquire(req)? {
                if let Err(e) = self.storage.update_lock_wait_status(&wait_id, "acquired") {
                    eprintln!("[Coordination][WARN] 更新等待状态为 acquired 失败 ({}): {}", wait_id, e);
                }
                return Ok(LockResult::Granted { lock_id, resource_type, resource_id, expires_at });
            }

            let now_inst = std::time::Instant::now();
            if now_inst >= deadline {
                if let Err(e) = self.storage.update_lock_wait_status(&wait_id, "expired") {
                    eprintln!("[Coordination][WARN] 更新等待状态为 expired 失败 ({}): {}", wait_id, e);
                }
                return Ok(LockResult::Timeout {
                    wait_id,
                    suggestion: format!("等待资源 '{}:{}' 超时 ({}ms)，请稍后重试或协调持锁方。", rt_str, norm_id, timeout_ms),
                });
            }

            let sleep_dur = (deadline - now_inst).min(std::time::Duration::from_millis(25));
            if let (Some(ref mut rx), Ok(handle)) = (&mut rx_opt, tokio::runtime::Handle::try_current()) {
                let recv_res = tokio::task::block_in_place(|| {
                    handle.block_on(tokio::time::timeout(sleep_dur, rx.recv()))
                });
                if let Ok(Ok(ev)) = recv_res {
                    if matches!(ev.as_ref(), crate::hooks::BodyEvent::LockWaiterNotified { .. }) {
                        continue;
                    }
                }
            }
            std::thread::sleep(sleep_dur);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Storage;

    #[test]
    fn test_coordination_engine_acquire_and_release() {
        let storage = Storage::open_in_memory().expect("storage");
        let engine = CoordinationEngine::new(storage.provider().clone());

        let req = LockRequest {
            resource_type: ResourceType::Port,
            resource_id: "8080".to_string(),
            scope: LockScope::Exclusive,
            holder_agent: "agent-1".to_string(),
            holder_pid: Some(1234),
            ttl_seconds: 60,
            metadata: Some(serde_json::json!({"purpose": "web-server"})),
        };

        // 1. 成功加锁
        let res = engine.try_acquire(&req).expect("acquire");
        let lock_id = match res {
            LockResult::Granted { lock_id, resource_id, .. } => {
                assert_eq!(resource_id, "8080");
                lock_id
            }
            _ => panic!("必须成功获取锁"),
        };

        // 2. 检查活跃锁
        let info = engine.check_resource(&ResourceType::Port, "8080").expect("check").expect("exists");
        assert_eq!(info.holder_agent, "agent-1");
        assert_eq!(info.holder_pid, Some(1234));

        // 3. 非持有者释放被拒绝
        let rel_other = engine.release(&lock_id, "agent-intruder").expect("release other");
        assert!(!rel_other, "非持有者不能释放他人锁");

        // 4. 持有者释放成功
        let rel_ok = engine.release(&lock_id, "agent-1").expect("release ok");
        assert!(rel_ok, "持有者可正常释放锁");

        // 5. 再次检查，锁已解除
        let info2 = engine.check_resource(&ResourceType::Port, "8080").expect("check");
        assert!(info2.is_none());
    }

    #[test]
    fn test_coordination_engine_exclusive_conflict() {
        let storage = Storage::open_in_memory().expect("storage");
        let engine = CoordinationEngine::new(storage.provider().clone());

        let req1 = LockRequest {
            resource_type: ResourceType::Directory,
            resource_id: "/etc/nginx".to_string(),
            scope: LockScope::Exclusive,
            holder_agent: "agent-alpha".to_string(),
            holder_pid: None,
            ttl_seconds: 120,
            metadata: None,
        };
        let res1 = engine.try_acquire(&req1).expect("acquire 1");
        assert!(matches!(res1, LockResult::Granted { .. }));

        // 另一 Agent 竞争同一目录排他锁
        let req2 = LockRequest {
            resource_type: ResourceType::Directory,
            resource_id: "/etc/nginx/".to_string(),
            scope: LockScope::Exclusive,
            holder_agent: "agent-beta".to_string(),
            holder_pid: None,
            ttl_seconds: 60,
            metadata: None,
        };
        let res2 = engine.try_acquire(&req2).expect("acquire 2");
        match res2 {
            LockResult::Denied { holder_agent, suggestion, .. } => {
                assert_eq!(holder_agent, "agent-alpha");
                assert!(suggestion.contains("agent-alpha"));
            }
            _ => panic!("排他锁冲突必须返回 Denied"),
        }
    }

    #[test]
    fn test_coordination_engine_reentrant_renewal() {
        let storage = Storage::open_in_memory().expect("storage");
        let engine = CoordinationEngine::new(storage.provider().clone());

        let req = LockRequest {
            resource_type: ResourceType::Service,
            resource_id: "redis".to_string(),
            scope: LockScope::Exclusive,
            holder_agent: "agent-self".to_string(),
            holder_pid: None,
            ttl_seconds: 30,
            metadata: None,
        };
        let res = engine.try_acquire(&req).expect("acquire");
        let lock_id = match res {
            LockResult::Granted { lock_id, .. } => lock_id,
            _ => panic!("granted"),
        };

        // 同一 Agent 续租
        let renewed = engine.renew(&lock_id, "agent-self", 120).expect("renew");
        assert!(renewed);
    }

    #[test]
    fn test_release_flexible_by_id_or_resource_and_holder_guard() {
        let storage = Storage::open_in_memory().expect("storage");
        let engine = CoordinationEngine::new(storage.provider().clone());
        let res = engine
            .try_acquire(&LockRequest {
                resource_type: ResourceType::Port,
                resource_id: "19876".to_string(),
                scope: LockScope::Exclusive,
                holder_agent: "owner-agent".to_string(),
                holder_pid: None,
                ttl_seconds: 60,
                metadata: None,
            })
            .expect("acquire");
        let lock_id = match res {
            LockResult::Granted { lock_id, .. } => lock_id,
            _ => panic!("expected granted"),
        };

        // 1. 错误持有者非强制释放应被拒绝
        let (id1, rel1) = engine
            .release_flexible(None, Some("port"), Some("19876"), Some("other-agent"), false)
            .expect("rel1");
        assert_eq!(id1.as_deref(), Some(lock_id.as_str()));
        assert!(!rel1);

        // 2. 正确持有者按 (resource_type, resource_id) 释放成功
        let (id2, rel2) = engine
            .release_flexible(None, Some("port"), Some("19876"), Some("owner-agent"), false)
            .expect("rel2");
        assert_eq!(id2.as_deref(), Some(lock_id.as_str()));
        assert!(rel2);

        // 3. 按 lock_id + force=true 强制释放
        let res2 = engine
            .try_acquire(&LockRequest {
                resource_type: ResourceType::File,
                resource_id: "/tmp/flex_lock_test.txt".to_string(),
                scope: LockScope::Exclusive,
                holder_agent: "owner-b".to_string(),
                holder_pid: None,
                ttl_seconds: 60,
                metadata: None,
            })
            .expect("acquire 2");
        let lock_id2 = match res2 {
            LockResult::Granted { lock_id, .. } => lock_id,
            _ => panic!("expected granted 2"),
        };
        let (id3, rel3) = engine
            .release_flexible(Some(&lock_id2), None, None, Some("admin"), true)
            .expect("rel3");
        assert_eq!(id3.as_deref(), Some(lock_id2.as_str()));
        assert!(rel3);
    }
}
