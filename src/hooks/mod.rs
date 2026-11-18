use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod event_bus;
pub mod dispatcher;
pub mod auto_sync;
pub mod client_gen;
pub mod push;
pub mod pipeline;

pub use event_bus::{EventBus, EventBusHandle};
pub use dispatcher::{HookDispatcher, HookRule, HookAction, HookFilter};
pub use auto_sync::{AutoSyncEngine, AutoSyncResult};
pub use client_gen::{ClientHookGenerator, ClientType, GeneratedHookConfig};
pub use push::{WebhookPusher, WebhookDeliveryReport, bridge_event_bus_to_sse};
#[cfg(unix)]
pub use push::UdsBroadcaster;
pub use pipeline::{HookPipeline, HookStep, HookPipelineEngine, HookAuditRecord, OnFailureStrategy, PipelineExecutionStatus};

/// Steward 系统生命周期与宿主物理事实事件枚举 (零捏造，规约 #5)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload")]
pub enum StewardEvent {
    /// 物理文件创建 (inotify / agent 产出)
    FileCreated {
        path: String,
        source: String,
        timestamp: String,
    },
    /// 物理文件修改
    FileModified {
        path: String,
        source: String,
        timestamp: String,
    },
    /// 物理文件删除
    FileDeleted {
        path: String,
        source: String,
        timestamp: String,
    },
    /// 资产正式入库登记
    AssetRegistered {
        path: String,
        object_type: String,
        owner: String,
        security_level: String,
        timestamp: String,
    },
    /// 长期记忆写入
    MemoryAdded {
        memory_id: String,
        memory_type: String,
        summary: Option<String>,
        tags: Vec<String>,
        timestamp: String,
    },
    /// 长期记忆更新
    MemoryUpdated {
        memory_id: String,
        memory_type: String,
        tags: Vec<String>,
        timestamp: String,
    },
    /// 宿主应用服务状态变更 (systemd 等)
    ServiceStatusChanged {
        service_name: String,
        old_status: String,
        new_status: String,
        timestamp: String,
    },
    /// 宿主端口状态变更
    PortChanged {
        port: u16,
        status: String,
        process_name: Option<String>,
        pid: Option<u32>,
        timestamp: String,
    },
    /// 多 Agent 协作锁成功持有
    LockAcquired {
        resource_type: String,
        resource_id: String,
        holder_agent: String,
        ttl_seconds: u64,
        timestamp: String,
    },
    /// 多 Agent 协作锁释放或失效
    LockReleased {
        resource_type: String,
        resource_id: String,
        holder_agent: String,
        timestamp: String,
    },
    /// Agent 会话开工
    SessionStarted {
        session_id: String,
        agent_id: Option<String>,
        timestamp: String,
    },
    /// Agent 会话结清归档
    SessionClosed {
        session_id: String,
        cleaned_files_count: usize,
        saved_memory_id: Option<String>,
        timestamp: String,
    },
}

impl StewardEvent {
    /// 获取事件类型字符串标识
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::FileCreated { .. } => "on_file_created",
            Self::FileModified { .. } => "on_file_modified",
            Self::FileDeleted { .. } => "on_file_deleted",
            Self::AssetRegistered { .. } => "on_asset_registered",
            Self::MemoryAdded { .. } => "on_memory_added",
            Self::MemoryUpdated { .. } => "on_memory_updated",
            Self::ServiceStatusChanged { .. } => "on_service_status_changed",
            Self::PortChanged { .. } => "on_port_changed",
            Self::LockAcquired { .. } => "on_lock_acquired",
            Self::LockReleased { .. } => "on_lock_released",
            Self::SessionStarted { .. } => "on_session_started",
            Self::SessionClosed { .. } => "on_session_closed",
        }
    }

    /// 提取事件涉及的主要路径 (若有)
    pub fn target_path(&self) -> Option<&str> {
        match self {
            Self::FileCreated { path, .. }
            | Self::FileModified { path, .. }
            | Self::FileDeleted { path, .. }
            | Self::AssetRegistered { path, .. } => Some(path.as_str()),
            _ => None,
        }
    }

    /// 转换为 JSON Value 用于通知或序列化
    pub fn to_json_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}
