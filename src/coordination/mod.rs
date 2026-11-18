pub mod lock_registry;

use std::sync::Arc;
use anyhow::Result;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use lock_registry::{LockRegistry, LockScope, ResourceType};
use crate::storage::{CoordinationLock, StorageProvider};

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
                let _ = self.storage.insert_coordination_lock(&updated_lock)?;
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
                let _ = bus.emit(crate::hooks::StewardEvent::LockAcquired {
                    resource_type: rt_str.to_string(),
                    resource_id: norm_id.clone(),
                    holder_agent: req.holder_agent.clone(),
                    ttl_seconds: ttl,
                    timestamp: now.to_rfc3339(),
                });
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
            if let (Some(l), Some(ref bus)) = (target_lock, &self.event_bus) {
                let _ = bus.emit(crate::hooks::StewardEvent::LockReleased {
                    resource_type: l.resource_type,
                    resource_id: l.resource_id,
                    holder_agent: l.holder_agent,
                    timestamp: Utc::now().to_rfc3339(),
                });
            }
        }
        Ok(res)
    }

    /// 强制释放锁（管理员/运维 CLI 操作）
    pub fn force_release(&self, lock_id: &str) -> Result<bool> {
        let locks = self.storage.list_coordination_locks(None)?;
        let target_lock = locks.into_iter().find(|x| x.lock_id == lock_id);
        let res = self.storage.delete_coordination_lock(lock_id)?;
        if res {
            if let (Some(l), Some(ref bus)) = (target_lock, &self.event_bus) {
                let _ = bus.emit(crate::hooks::StewardEvent::LockReleased {
                    resource_type: l.resource_type,
                    resource_id: l.resource_id,
                    holder_agent: l.holder_agent,
                    timestamp: Utc::now().to_rfc3339(),
                });
            }
        }
        Ok(res)
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

    /// 清理所有过期锁
    pub fn gc_expired_locks(&self) -> Result<usize> {
        self.storage.gc_expired_coordination_locks()
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
}
