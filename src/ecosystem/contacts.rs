use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::config::Config;
use crate::storage::Storage;
use crate::storage::models::ExternalServiceRecord;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceHealthReport {
    pub service_name: String,
    pub endpoint: String,
    pub status: String,
    pub auth_env_var: Option<String>,
    pub auth_env_ready: bool,
    pub latency_ms: u64,
    pub detail: String,
}

/// 外部可用服务通讯录管理器（类似 Agent 的“外部联系人名单”，严禁存储明文密钥）
pub struct ExternalContactsManager {
    storage: Storage,
}

impl ExternalContactsManager {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    /// 校验字段中是否包含疑似硬编码明文凭证（规约：绝不充当 Vault 存明文密钥）
    pub fn validate_no_secret_leak(text: &str) -> Result<()> {
        let lower = text.to_lowercase();
        let forbidden_prefixes = ["sk-", "ghp_", "gho_", "xoxb-", "xoxp-", "aiza"];
        for prefix in forbidden_prefixes {
            if lower.contains(prefix) {
                anyhow::bail!(
                    "安全拦截：检测到疑似明文密钥前缀 '{}'！external_services 仅允许通过 auth_env_var 引用环境变量名，严禁存储明文密钥。",
                    prefix
                );
            }
        }
        if lower.contains("password=") && !lower.contains("${") {
            anyhow::bail!("安全拦截：检测到明文 password= 参数，请改用环境变量引用。");
        }
        Ok(())
    }

    /// 登记或更新一条外部服务名片
    pub fn register_service(&self, rec: &ExternalServiceRecord) -> Result<()> {
        Self::validate_no_secret_leak(&rec.endpoint)?;
        if let Some(ref ex) = rec.usage_example {
            Self::validate_no_secret_leak(ex)?;
        }
        if let Some(ref env_var) = rec.auth_env_var {
            Self::validate_no_secret_leak(env_var)?;
        }
        self.storage.upsert_external_service(rec)
    }

    /// 从当前 Config 冷启动自动推导宿主机关联的外部服务（如 Ollama / PostgreSQL）
    pub fn bootstrap_from_config(&self, cfg: &Config) -> Result<usize> {
        let mut added = 0usize;

        // 1. Ollama 嵌入服务
        let ollama_url = cfg.embedding.ollama_url.trim_end_matches('/').to_string();
        let ollama = ExternalServiceRecord {
            id: "svc:ollama".to_string(),
            service_name: "ollama-embedding".to_string(),
            category: "embedding".to_string(),
            endpoint: ollama_url.clone(),
            protocol: "http".to_string(),
            capabilities: format!("本地文本向量嵌入与大模型推理 (模型: {})", cfg.embedding.model),
            auth_type: "none".to_string(),
            auth_env_var: None,
            health_endpoint: Some(format!("{}/api/tags", ollama_url)),
            usage_example: Some(format!("curl -s {}/api/tags", ollama_url)),
            status: "unknown".to_string(),
            last_checked_at: None,
        };
        self.register_service(&ollama)?;
        added += 1;

        // 2. 若配置了 PostgreSQL URL，则脱敏凭证后登记
        if let Some(ref pg_url) = cfg.storage.pg_url {
            let sanitized_endpoint = Self::sanitize_pg_endpoint(pg_url);
            let pg = ExternalServiceRecord {
                id: "svc:postgres".to_string(),
                service_name: "postgres-shared-store".to_string(),
                category: "database".to_string(),
                endpoint: sanitized_endpoint,
                protocol: "postgres".to_string(),
                capabilities: "PostgreSQL + pgvector 向量与关系共享存储、Advisory Lock 分布式锁".to_string(),
                auth_type: "env".to_string(),
                auth_env_var: Some("AGENT_BODY_PG_URL".to_string()),
                health_endpoint: None,
                usage_example: Some("psql \"$AGENT_BODY_PG_URL\" -c 'SELECT version();'".to_string()),
                status: "unknown".to_string(),
                last_checked_at: None,
            };
            self.register_service(&pg)?;
            added += 1;
        }

        Ok(added)
    }

    /// 脱敏 PostgreSQL 连接串中的密码字段
    fn sanitize_pg_endpoint(raw: &str) -> String {
        if let Some(at_idx) = raw.find('@') {
            if let Some(scheme_idx) = raw.find("://") {
                return format!("{}://***@{}", &raw[..scheme_idx], &raw[at_idx + 1..]);
            }
        }
        raw.split_whitespace()
            .filter(|part| !part.to_lowercase().starts_with("password="))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// 检查所需鉴权环境变量是否已在当前进程环境就绪（不读取具体值）
    pub fn is_auth_env_ready(auth_env_var: Option<&str>) -> bool {
        match auth_env_var {
            None | Some("") => true,
            Some(var_name) => std::env::var_os(var_name).is_some(),
        }
    }

    /// 轻量真实探活（HTTP GET 或 TCP Connect，1.5s 超时，零捏造规约 #5）
    pub fn check_health(&self, service_name: &str) -> Result<ServiceHealthReport> {
        let all = self.storage.list_external_services(None)?;
        let mut rec = all
            .into_iter()
            .find(|s| s.service_name == service_name)
            .ok_or_else(|| anyhow::anyhow!("未在外部服务通讯录中找到服务: {}", service_name))?;

        let start = std::time::Instant::now();
        let probe_target = rec.health_endpoint.as_deref().unwrap_or(&rec.endpoint);
        let (status, detail) = if probe_target.starts_with("http://") || probe_target.starts_with("https://") {
            let target_url = probe_target.to_string();
            let http_probe = move || -> Result<(String, String)> {
                let client = reqwest::blocking::Client::builder()
                    .timeout(Duration::from_millis(1500))
                    .build()?;
                match client.get(&target_url).send() {
                    Ok(resp) if resp.status().is_success() => {
                        Ok(("healthy".to_string(), format!("HTTP {}", resp.status().as_u16())))
                    }
                    Ok(resp) => Ok((
                        "degraded".to_string(),
                        format!("HTTP {} 非 2xx 响应", resp.status().as_u16()),
                    )),
                    Err(e) => Ok(("offline".to_string(), format!("HTTP 探测不可达: {}", e))),
                }
            };
            if tokio::runtime::Handle::try_current().is_ok() {
                std::thread::scope(|s| match s.spawn(http_probe).join() {
                    Ok(res) => res,
                    Err(_) => Ok(("offline".to_string(), "HTTP 探测线程异常终止".to_string())),
                })?
            } else {
                http_probe()?
            }
        } else {
            // 尝试解析 host:port 进行 TCP 探活
            let host_port = probe_target
                .trim_start_matches("postgres://")
                .trim_start_matches("postgresql://")
                .trim_start_matches("redis://")
                .split('@')
                .next_back()
                .unwrap_or(probe_target)
                .split('/')
                .next()
                .unwrap_or(probe_target);

            match host_port.to_socket_addrs() {
                Ok(mut addrs) => {
                    if let Some(addr) = addrs.next() {
                        match TcpStream::connect_timeout(&addr, Duration::from_millis(1200)) {
                            Ok(_) => ("healthy".to_string(), format!("TCP {} 连通", addr)),
                            Err(e) => ("offline".to_string(), format!("TCP 连接失败: {}", e)),
                        }
                    } else {
                        ("unknown".to_string(), "未解析出可用 socket 地址".to_string())
                    }
                }
                Err(e) => ("unknown".to_string(), format!("地址不可用于直接 TCP 探活: {}", e)),
            }
        };

        let latency_ms = start.elapsed().as_millis() as u64;
        let now = chrono::Utc::now().to_rfc3339();
        rec.status = status.clone();
        rec.last_checked_at = Some(now);
        self.storage.upsert_external_service(&rec)?;

        let auth_env_ready = Self::is_auth_env_ready(rec.auth_env_var.as_deref());
        Ok(ServiceHealthReport {
            service_name: rec.service_name,
            endpoint: rec.endpoint,
            status,
            auth_env_var: rec.auth_env_var,
            auth_env_ready,
            latency_ms,
            detail,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reject_plaintext_secrets() {
        let storage = Storage::open_in_memory().expect("open");
        let mgr = ExternalContactsManager::new(storage);
        let bad = ExternalServiceRecord {
            id: "svc-bad".to_string(),
            service_name: "bad-openai".to_string(),
            category: "llm".to_string(),
            endpoint: "https://api.openai.com/v1".to_string(),
            protocol: "https".to_string(),
            capabilities: "chat".to_string(),
            auth_type: "bearer_env".to_string(),
            auth_env_var: Some("OPENAI_API_KEY".to_string()),
            health_endpoint: None,
            usage_example: Some("curl -H 'Authorization: Bearer sk-1234567890abcdef'".to_string()),
            status: "unknown".to_string(),
            last_checked_at: None,
        };
        assert!(mgr.register_service(&bad).is_err(), "包含 sk- 明文密钥必须被拦截");
    }

    #[tokio::test]
    async fn test_check_health_http_inside_tokio_runtime_no_panic() {
        let storage = Storage::open_in_memory().expect("open");
        let mgr = ExternalContactsManager::new(storage);
        let svc = ExternalServiceRecord {
            id: "svc-tokio".to_string(),
            service_name: "tokio-http-svc".to_string(),
            category: "api".to_string(),
            endpoint: "http://127.0.0.1:59996/health".to_string(),
            protocol: "http".to_string(),
            capabilities: "health".to_string(),
            auth_type: "none".to_string(),
            auth_env_var: None,
            health_endpoint: None,
            usage_example: None,
            status: "unknown".to_string(),
            last_checked_at: None,
        };
        mgr.register_service(&svc).expect("register");
        let rep = mgr.check_health("tokio-http-svc").expect("check_health inside tokio");
        assert_eq!(rep.status, "offline");
    }

    #[test]
    fn test_bootstrap_from_config_and_sanitize_pg() {
        let storage = Storage::open_in_memory().expect("open");
        let mgr = ExternalContactsManager::new(storage.clone());
        let mut cfg = Config::default();
        cfg.storage.pg_url = Some("postgresql://agent_body:supersecret@127.0.0.1:5432/agent_body".to_string());

        let count = mgr.bootstrap_from_config(&cfg).expect("bootstrap");
        assert_eq!(count, 2);
        let svcs = storage.list_external_services(None).expect("list");
        let pg = svcs.iter().find(|s| s.service_name == "postgres-shared-store").expect("pg");
        assert!(!pg.endpoint.contains("supersecret"), "PG 密码必须脱敏");
    }
}
