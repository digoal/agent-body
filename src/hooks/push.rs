use std::sync::Arc;
use std::time::Duration;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use super::StewardEvent;
use crate::api::response::StewardResponse;

/// Webhook 投递结果报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDeliveryReport {
    pub url: String,
    pub attempts: usize,
    pub success: bool,
    pub status_code: Option<u16>,
    pub error_message: Option<String>,
}

/// HTTP Webhook 重试推送通道 (支持指数退避与规约 #1 封套)
pub struct WebhookPusher {
    client: reqwest::Client,
    max_retries: usize,
    base_backoff_ms: u64,
}

impl Default for WebhookPusher {
    fn default() -> Self {
        Self::new(3, 100)
    }
}

impl WebhookPusher {
    pub fn new(max_retries: usize, base_backoff_ms: u64) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();
        Self {
            client,
            max_retries,
            base_backoff_ms,
        }
    }

    /// 发送 Webhook 事件 (带指数退避重试)
    pub async fn send_event(
        &self,
        url: &str,
        event: &StewardEvent,
        timeout_ms: u64,
    ) -> WebhookDeliveryReport {
        let start = std::time::Instant::now();
        let envelope = StewardResponse::ok(
            "EVENT_DISPATCHED",
            "Steward Hook Event Notification",
            event.clone(),
            "hook_pusher",
            start,
        );
        let mut attempts = 0;
        let mut last_error = None;
        let mut status_code = None;

        for attempt in 0..=self.max_retries {
            attempts += 1;
            let resp_result = self.client.post(url)
                .timeout(Duration::from_millis(timeout_ms))
                .json(&envelope)
                .send()
                .await;

            match resp_result {
                Ok(resp) => {
                    let code = resp.status().as_u16();
                    status_code = Some(code);
                    if resp.status().is_success() {
                        return WebhookDeliveryReport {
                            url: url.to_string(),
                            attempts,
                            success: true,
                            status_code,
                            error_message: None,
                        };
                    } else {
                        last_error = Some(format!("HTTP 状态异常: {}", code));
                    }
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                }
            }

            if attempt < self.max_retries {
                let delay = Duration::from_millis(self.base_backoff_ms * (1 << attempt));
                tokio::time::sleep(delay).await;
            }
        }

        WebhookDeliveryReport {
            url: url.to_string(),
            attempts,
            success: false,
            status_code,
            error_message: last_error,
        }
    }
}

/// 本地 Unix Domain Socket 低延迟事件流广播服务
#[cfg(unix)]
pub struct UdsBroadcaster {
    socket_path: std::path::PathBuf,
    event_tx: broadcast::Sender<String>,
}

#[cfg(unix)]
impl UdsBroadcaster {
    pub fn new(socket_path: std::path::PathBuf) -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self {
            socket_path,
            event_tx,
        }
    }

    /// 广播事件给所有 UDS 客户端
    pub fn broadcast(&self, event: &StewardEvent) -> Result<usize> {
        let json = serde_json::to_string(event)?;
        self.event_tx.send(json).map_err(|e| anyhow::anyhow!("广播失败: {}", e))
    }

    /// 启动 Unix Domain Socket 服务端监听循环
    pub async fn run_server(&self) -> Result<()> {
        if self.socket_path.exists() {
            if let Err(e) = std::fs::remove_file(&self.socket_path) {
                eprintln!("[Hooks][WARN] 清理旧 UDS Socket {:?} 失败: {}", self.socket_path, e);
            }
        }

        let listener = tokio::net::UnixListener::bind(&self.socket_path)
            .with_context(|| format!("绑定 UDS Socket 失败: {:?}", self.socket_path))?;

        let tx = self.event_tx.clone();

        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                use tokio::io::AsyncWriteExt;
                let mut rx = tx.subscribe();
                tokio::spawn(async move {
                    while let Ok(msg) = rx.recv().await {
                        let line = format!("{}\n", msg);
                        if stream.write_all(line.as_bytes()).await.is_err() {
                            break;
                        }
                    }
                });
            }
        });

        Ok(())
    }
}

/// 将 EventBus 事件流异步桥接至 WebServer SSE 发送器
pub fn bridge_event_bus_to_sse(
    mut event_rx: broadcast::Receiver<Arc<StewardEvent>>,
    sse_tx: broadcast::Sender<String>,
) {
    tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            if let Ok(json_str) = serde_json::to_string(&*event) {
                if sse_tx.receiver_count() > 0 && sse_tx.send(json_str).is_err() {
                    eprintln!("[Hooks][WARN] EventBus -> SSE 广播投递失败");
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_webhook_pusher_retry_on_unreachable_endpoint() {
        let pusher = WebhookPusher::new(2, 10);
        let event = StewardEvent::FileCreated {
            path: "/root/new/work/test.rs".to_string(),
            source: "test".to_string(),
            timestamp: "2026-11-18T22:00:00Z".to_string(),
        };

        // 投递至物理上无服务的端口，应自动重试 2 次并最终报告失败（无 Panic）
        let rep = pusher.send_event("http://127.0.0.1:59999/dummy_webhook", &event, 50).await;
        assert_eq!(rep.attempts, 3); // 1 初始 + 2 次重试
        assert!(!rep.success);
        assert!(rep.error_message.is_some());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_uds_broadcaster_lifecycle() {
        use tempfile::tempdir;
        use tokio::io::AsyncBufReadExt;

        let dir = tempdir().unwrap();
        let sock_path = dir.path().join("steward_test.sock");
        let broadcaster = UdsBroadcaster::new(sock_path.clone());

        broadcaster.run_server().await.expect("run uds server");

        // 连接客户端
        let stream = tokio::net::UnixStream::connect(&sock_path).await.expect("connect uds");
        let mut reader = tokio::io::BufReader::new(stream);

        // 触发广播
        let event = StewardEvent::AssetRegistered {
            path: "/root/new/work/test.rs".to_string(),
            object_type: "script".to_string(),
            owner: "user:digoal".to_string(),
            security_level: "internal".to_string(),
            timestamp: "2026-11-18T22:00:00Z".to_string(),
        };

        // 短暂等待客户端连接建立
        tokio::time::sleep(Duration::from_millis(50)).await;
        broadcaster.broadcast(&event).expect("broadcast");

        let mut line = String::new();
        reader.read_line(&mut line).await.expect("read line");
        assert!(line.contains("AssetRegistered"));
        assert!(line.contains("/root/new/work/test.rs"));
    }
}
