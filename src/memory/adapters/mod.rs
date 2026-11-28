//! 成熟开源生态记忆适配器套件
//!
//! 本模块提供三大生产级适配器与统一工厂函数：
//!
//! | 适配器 | 引擎 | 场景 |
//! |---|---|---|
//! | `PowerContextAdapter` | 本地 PowerContext 服务（端口 8000） | 宿主机优先，混合 FTS+向量检索 |
//! | `Mem0Adapter` | Mem0 云端/私有化 REST API | 云端高级语义记忆 |
//! | `LettaAdapter` | Letta/MemGPT 本地 Agent 服务 | 自主 Agent 记忆管理 |
//!
//! ## 工厂接入示例
//! ```rust,ignore
//! use agent_body::memory::adapters::MemoryFactory;
//!
//! // 由 config.toml 中的 provider 字段驱动
//! let provider = MemoryFactory::from_config("powercontext", &config)?;
//! let results = provider.search("rust agent", 10, None)?;
//! ```

pub mod letta;
pub mod mem0;
pub mod pg_tx;
pub mod powercontext;

pub use letta::LettaAdapter;
pub use mem0::Mem0Adapter;
pub use pg_tx::{PgTxAdapterConfig, PgTxMemoryAdapter};
pub use powercontext::PowerContextAdapter;

use anyhow::{anyhow, Result};
use std::sync::Arc;

use crate::memory::spi::{MemoryProvider, TxMemoryProvider};
use crate::memory::builtin_provider::BuiltinMemoryProvider;
use crate::storage::Storage;

/// 适配器配置参数（从 config.toml 映射）
#[derive(Debug, Clone, Default)]
pub struct AdapterConfig {
    // PostgreSQL 事务记忆 (Phase 76)
    pub pg_url: String,
    pub pg_pool_size: usize,
    pub pg_tenant_scope: String,
    pub vector_backend: String,
    pub fallback_to_sqlite: bool,
    // PowerContext
    pub powercontext_url: String,
    pub powercontext_scope_id: String,
    // Mem0
    pub mem0_url: String,
    pub mem0_api_key: String,
    pub mem0_user_id: String,
    // Letta
    pub letta_url: String,
    pub letta_agent_id: String,
    pub letta_token: String,
}

impl AdapterConfig {
    /// 应用默认值（允许 toml 配置部分省略）
    pub fn with_defaults(mut self) -> Self {
        if self.pg_pool_size == 0 {
            self.pg_pool_size = 8;
        }
        if self.pg_tenant_scope.is_empty() {
            self.pg_tenant_scope = "default".to_string();
        }
        if self.vector_backend.is_empty() {
            self.vector_backend = "none".to_string();
        }
        if self.powercontext_url.is_empty() {
            self.powercontext_url = "http://127.0.0.1:8000".to_string();
        }
        if self.mem0_url.is_empty() {
            self.mem0_url = "https://api.mem0.ai".to_string();
        }
        if self.mem0_user_id.is_empty() {
            self.mem0_user_id = "agent-body".to_string();
        }
        if self.letta_url.is_empty() {
            self.letta_url = "http://127.0.0.1:8283".to_string();
        }
        self
    }
}

/// 记忆提供者工厂
///
/// 根据 `provider` 字符串（来自 `config.toml` 的 `[memory].provider`）
/// 动态创建对应的 `MemoryProvider` 实例。
///
/// 若目标引擎不可达，自动降级为内置 SQLite 引擎并打印警告。
pub struct MemoryFactory;

impl MemoryFactory {
    /// 构建支持双时态 MVCC 事务契约的 `TxMemoryProvider`（用于 `pg_tx` 模式及双时态查询）
    pub fn create_tx(
        config: &AdapterConfig,
        storage: Storage,
    ) -> Result<Arc<dyn TxMemoryProvider>> {
        let cfg = config.clone().with_defaults();
        let adapter = PgTxMemoryAdapter::connect_or_fallback(
            PgTxAdapterConfig {
                pg_url: cfg.pg_url,
                pool_size: cfg.pg_pool_size,
                tenant_scope: cfg.pg_tenant_scope,
                vector_backend: cfg.vector_backend,
                fallback_to_sqlite: cfg.fallback_to_sqlite,
                mirror_writes_to_sqlite: true,
            },
            storage,
        )?;
        Ok(Arc::new(adapter))
    }

    /// 根据 provider 名称与配置创建适配器
    ///
    /// - `provider`: "builtin" | "pg_tx" | "powercontext" | "mem0" | "letta"
    /// - `config`: 适配器配置（从 config.toml 解析）
    /// - `storage`: 内置引擎所需的 Storage 实例（供降级使用）
    pub fn create(
        provider: &str,
        config: &AdapterConfig,
        storage: Storage,
    ) -> Result<Arc<dyn MemoryProvider>> {
        let cfg = config.clone().with_defaults();

        match provider {
            "builtin" => {
                let p = BuiltinMemoryProvider::new(storage);
                Ok(Arc::new(p))
            }
            "pg_tx" => {
                let adapter = PgTxMemoryAdapter::connect_or_fallback(
                    PgTxAdapterConfig {
                        pg_url: cfg.pg_url,
                        pool_size: cfg.pg_pool_size,
                        tenant_scope: cfg.pg_tenant_scope,
                        vector_backend: cfg.vector_backend,
                        fallback_to_sqlite: cfg.fallback_to_sqlite,
                        mirror_writes_to_sqlite: true,
                    },
                    storage,
                )?;
                Ok(Arc::new(adapter))
            }
            "powercontext" => {
                match PowerContextAdapter::new(&cfg.powercontext_url, &cfg.powercontext_scope_id) {
                    Ok(p) => {
                        eprintln!("[agent-body] 记忆引擎 → PowerContext ({})", cfg.powercontext_url);
                        Ok(Arc::new(p))
                    }
                    Err(e) => {
                        eprintln!(
                            "[agent-body] ⚠️  PowerContext 不可达，降级为内置引擎: {}",
                            e
                        );
                        let p = BuiltinMemoryProvider::new(storage);
                        Ok(Arc::new(p))
                    }
                }
            }
            "mem0" => {
                let p = Mem0Adapter::new(&cfg.mem0_url, &cfg.mem0_api_key, &cfg.mem0_user_id)?;
                eprintln!("[agent-body] 记忆引擎 → Mem0 ({})", cfg.mem0_url);
                Ok(Arc::new(p))
            }
            "letta" => {
                match LettaAdapter::new(&cfg.letta_url, &cfg.letta_agent_id, &cfg.letta_token) {
                    Ok(p) => {
                        eprintln!("[agent-body] 记忆引擎 → Letta ({})", cfg.letta_url);
                        Ok(Arc::new(p))
                    }
                    Err(e) => {
                        eprintln!(
                            "[agent-body] ⚠️  Letta 配置错误，降级为内置引擎: {}",
                            e
                        );
                        let p = BuiltinMemoryProvider::new(storage);
                        Ok(Arc::new(p))
                    }
                }
            }
            unknown => Err(anyhow!(
                "未知的记忆提供者: '{}'. 支持: builtin | pg_tx | powercontext | mem0 | letta",
                unknown
            )),
        }
    }
}

// ─── 单元测试 ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Storage;

    fn default_cfg() -> AdapterConfig {
        AdapterConfig::default().with_defaults()
    }

    #[test]
    fn test_factory_builtin_provider() {
        let storage = Storage::open_in_memory().expect("in-memory storage");
        let provider = MemoryFactory::create("builtin", &default_cfg(), storage)
            .expect("builtin 应创建成功");
        assert_eq!(provider.provider_name(), "builtin");
    }

    #[test]
    fn test_factory_unknown_provider_returns_error() {
        let storage = Storage::open_in_memory().expect("in-memory storage");
        let result = MemoryFactory::create("nonexistent", &default_cfg(), storage);
        assert!(result.is_err());
        let msg = result.err().unwrap().to_string();
        assert!(msg.contains("未知的记忆提供者"), "实际错误: {}", msg);
    }

    #[test]
    fn test_factory_powercontext_fallback_to_builtin() {
        // 使用不可达的端口，期望自动降级为内置引擎
        let storage = Storage::open_in_memory().expect("in-memory storage");
        let cfg = AdapterConfig {
            powercontext_url: "http://127.0.0.1:19998".to_string(), // 不可达
            ..AdapterConfig::default()
        };
        let provider = MemoryFactory::create("powercontext", &cfg, storage)
            .expect("PowerContext 降级应成功");
        // 降级后应为内置引擎
        assert_eq!(provider.provider_name(), "builtin");
    }

    #[test]
    fn test_factory_letta_empty_agent_id_fallback() {
        // agent_id 为空应降级为内置引擎
        let storage = Storage::open_in_memory().expect("in-memory storage");
        let cfg = AdapterConfig {
            letta_url: "http://127.0.0.1:8283".to_string(),
            letta_agent_id: "".to_string(), // 触发 LettaAdapter::new 的 Err
            ..AdapterConfig::default()
        };
        let provider = MemoryFactory::create("letta", &cfg, storage)
            .expect("Letta 配置错误应降级");
        assert_eq!(provider.provider_name(), "builtin");
    }

    #[test]
    fn test_adapter_config_defaults() {
        let cfg = AdapterConfig::default().with_defaults();
        assert_eq!(cfg.powercontext_url, "http://127.0.0.1:8000");
        assert_eq!(cfg.mem0_url, "https://api.mem0.ai");
        assert_eq!(cfg.mem0_user_id, "agent-body");
        assert_eq!(cfg.letta_url, "http://127.0.0.1:8283");
    }
}
