use std::sync::Arc;
use tokio::sync::broadcast;
use anyhow::Result;
use super::BodyEvent;

/// 异步事件总线句柄 (克隆开销低，用于在多模块间共享注入)
#[derive(Clone)]
pub struct EventBusHandle {
    sender: broadcast::Sender<Arc<BodyEvent>>,
}

impl EventBusHandle {
    /// 广播发射事实事件
    /// 
    /// 无论是否有活跃的接收端，发射操作均保证快速非阻塞 (< 100μs)
    pub fn emit(&self, event: BodyEvent) -> Result<usize> {
        let _wait = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::IpcEventBus);
        // 如果当前没有订阅者，broadcast::send 会返回 Err(SendError)
        // 在总线语义下，无订阅者属于正常现象，静默返回 0 接收者
        match self.sender.send(Arc::new(event)) {
            Ok(receivers) => Ok(receivers),
            Err(_) => Ok(0),
        }
    }

    /// 订阅事件流
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<BodyEvent>> {
        self.sender.subscribe()
    }
}

/// 集中式事件总线
pub struct EventBus {
    sender: broadcast::Sender<Arc<BodyEvent>>,
}

impl EventBus {
    /// 创建默认容量的事件总线 (默认容量 2048 事件)
    pub fn new() -> Self {
        Self::with_capacity(2048)
    }

    /// 创建指定容量的事件总线
    pub fn with_capacity(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// 获取可在任务间自由传递克隆的 Handle
    pub fn handle(&self) -> EventBusHandle {
        EventBusHandle {
            sender: self.sender.clone(),
        }
    }

    /// 订阅事件广播
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<BodyEvent>> {
        self.sender.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;

    #[tokio::test]
    async fn test_event_bus_broadcast_multiple_subscribers() {
        let bus = EventBus::new();
        let handle = bus.handle();

        let mut rx1 = handle.subscribe();
        let mut rx2 = handle.subscribe();

        let event = BodyEvent::FileCreated {
            path: "/root/new/work/demo.rs".to_string(),
            source: "agent".to_string(),
            timestamp: Local::now().to_rfc3339(),
        };

        let sent = handle.emit(event.clone()).expect("emit failed");
        assert_eq!(sent, 2, "应有两个活跃订阅者");

        let rec1 = rx1.recv().await.expect("rx1 recv");
        let rec2 = rx2.recv().await.expect("rx2 recv");

        assert_eq!(*rec1, event);
        assert_eq!(*rec2, event);
    }

    #[test]
    fn test_event_bus_emit_without_subscribers_ok() {
        let bus = EventBus::new();
        let handle = bus.handle();

        let event = BodyEvent::PortChanged {
            port: 8080,
            status: "occupied".to_string(),
            process_name: Some("python3".to_string()),
            pid: Some(12345),
            timestamp: Local::now().to_rfc3339(),
        };

        let sent = handle.emit(event).expect("emit without receivers should succeed");
        assert_eq!(sent, 0);
    }
}
