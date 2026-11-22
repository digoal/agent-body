use serde::{Deserialize, Serialize};
use crate::storage::StorageProvider;
use crate::storage::CoordinationLock;
use crate::web::http::{HttpRequest, HttpResponse, StatusCode};
use crate::web::api::WebResponse;

/// Web 展示的活跃锁数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockViewItem {
    pub lock_id: String,
    pub resource_type: String,
    pub resource_id: String,
    pub holder_agent: String,
    pub scope: String,
    pub acquired_at: String,
    pub expires_at: String,
    pub remaining_ttl_secs: i64,
}

impl From<CoordinationLock> for LockViewItem {
    fn from(rec: CoordinationLock) -> Self {
        let remaining = if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(&rec.expires_at) {
            let now = chrono::Utc::now();
            (expires.with_timezone(&chrono::Utc) - now).num_seconds().max(0)
        } else {
            0
        };

        Self {
            lock_id: rec.lock_id,
            resource_type: rec.resource_type,
            resource_id: rec.resource_id,
            holder_agent: rec.holder_agent,
            scope: rec.scope,
            acquired_at: rec.acquired_at,
            expires_at: rec.expires_at,
            remaining_ttl_secs: remaining,
        }
    }
}

/// 释放锁请求体
#[derive(Debug, Deserialize)]
pub struct ReleaseLockRequest {
    pub resource_type: String,
    pub resource_id: String,
    pub agent_id: Option<String>,
}

/// 处理 GET /api/v1/locks
pub fn handle_locks_list(storage: &dyn StorageProvider) -> HttpResponse {
    match storage.list_coordination_locks(None) {
        Ok(locks) => {
            let items: Vec<LockViewItem> = locks.into_iter().map(LockViewItem::from).collect();
            WebResponse::success(items)
        }
        Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to list locks: {}", e)),
    }
}

/// 处理 GET /api/v1/locks/history (从 changes 表检索锁相关审计历史)
pub fn handle_locks_history(storage: &dyn StorageProvider) -> HttpResponse {
    match storage.list_changes(150) {
        Ok(changes) => {
            let lock_changes: Vec<_> = changes
                .into_iter()
                .filter(|c| {
                    let t = c.change_type.to_lowercase();
                    let target = c.target.to_lowercase();
                    t.contains("lock")
                        || t.contains("coordination")
                        || target.contains("lock:")
                        || target.contains("coordination")
                })
                .take(50)
                .collect();
            WebResponse::success(lock_changes)
        }
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to query locks history: {}", e),
        ),
    }
}

/// 处理 GET /api/v1/locks/wait-queue (Phase 17)
pub fn handle_locks_wait_queue(storage: &dyn StorageProvider) -> HttpResponse {
    match storage.list_lock_wait_queue(None, None) {
        Ok(items) => WebResponse::success(items),
        Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to list wait queue: {}", e)),
    }
}

/// 处理 GET /api/v1/locks/deadlocks (Phase 17)
pub fn handle_locks_deadlocks(storage: &dyn StorageProvider) -> HttpResponse {
    let locks = match storage.list_coordination_locks(None) {
        Ok(l) => l,
        Err(e) => return WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to list locks: {}", e)),
    };
    let waits = match storage.list_lock_wait_queue(None, None) {
        Ok(w) => w,
        Err(e) => return WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to list wait queue: {}", e)),
    };
    let report = crate::coordination::DeadlockDetector::detect(&locks, &waits, None);
    WebResponse::success(report)
}

/// 处理 POST /api/v1/locks/release
pub fn handle_lock_release(
    req: &HttpRequest,
    storage: &dyn StorageProvider,
    auth_token: &Option<String>,
) -> HttpResponse {
    // 若配置了 auth_token，则使用常量时间比较校验请求头 Authorization
    if let Some(expected_token) = auth_token.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let auth_header = req.headers.get("authorization").cloned().unwrap_or_default();
        let token = auth_header.strip_prefix("Bearer ").unwrap_or(&auth_header);
        if !crate::mesh::auth::constant_time_eq(token, expected_token) {
            return WebResponse::error(StatusCode::UNAUTHORIZED, "Unauthorized: invalid auth token");
        }
    }

    let payload: ReleaseLockRequest = match req.json() {
        Ok(p) => p,
        Err(e) => return WebResponse::error(StatusCode::BAD_REQUEST, &format!("Invalid JSON body: {}", e)),
    };

    match storage.get_coordination_lock(&payload.resource_type, &payload.resource_id) {
        Ok(Some(lock)) => {
            if let Some(ref agent) = payload.agent_id {
                if &lock.holder_agent != agent && agent != "admin" && agent != "admin_web" {
                    return WebResponse::error(StatusCode::FORBIDDEN, "Agent mismatch: lock held by another agent");
                }
            }
            match storage.delete_coordination_lock(&lock.lock_id) {
                Ok(true) => WebResponse::success(serde_json::json!({
                    "released": true,
                    "lock_id": lock.lock_id,
                    "resource_type": payload.resource_type,
                    "resource_id": payload.resource_id
                })),
                Ok(false) => WebResponse::error(StatusCode::NOT_FOUND, "Failed to release lock"),
                Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to delete lock: {}", e)),
            }
        }
        Ok(None) => WebResponse::error(StatusCode::NOT_FOUND, "Lock not found"),
        Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Error checking lock: {}", e)),
    }
}
