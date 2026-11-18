use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use anyhow::{Context, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,

    #[serde(default = "default_db_path")]
    pub db_path: PathBuf,

    #[serde(default = "default_api_port")]
    pub api_port: u16,

    #[serde(default = "default_api_bind")]
    pub api_bind: String,

    #[serde(default)]
    pub embedding: EmbeddingConfig,

    #[serde(default)]
    pub assets: AssetsConfig,

    #[serde(default)]
    pub placement: PlacementConfig,

    #[serde(default)]
    pub storage: StorageConfig,

    #[serde(default)]
    pub web: WebConfig,

    #[serde(default)]
    pub hooks: Vec<crate::hooks::HookRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    #[serde(default = "default_web_enabled")]
    pub enabled: bool,

    #[serde(default = "default_web_host")]
    pub host: String,

    #[serde(default = "default_web_port")]
    pub port: u16,

    #[serde(default)]
    pub auth_token: Option<String>,
}

fn default_web_enabled() -> bool {
    true
}

fn default_web_host() -> String {
    "127.0.0.1".to_string()
}

fn default_web_port() -> u16 {
    8765
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            enabled: default_web_enabled(),
            host: default_web_host(),
            port: default_web_port(),
            auth_token: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    #[serde(default = "default_storage_provider")]
    pub provider: String,

    #[serde(default = "default_pg_url")]
    pub pg_url: Option<String>,

    #[serde(default = "default_duckdb_path")]
    pub duckdb_path: Option<PathBuf>,
}

fn default_storage_provider() -> String {
    "sqlite".to_string()
}

fn default_pg_url() -> Option<String> {
    None
}

fn default_duckdb_path() -> Option<PathBuf> {
    None
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            provider: default_storage_provider(),
            pg_url: default_pg_url(),
            duckdb_path: default_duckdb_path(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,

    #[serde(default = "default_model")]
    pub model: String,

    #[serde(default = "default_dimensions")]
    pub dimensions: usize,

    #[serde(default = "default_fallback_fts_only")]
    pub fallback_fts_only: bool,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            ollama_url: default_ollama_url(),
            model: default_model(),
            dimensions: default_dimensions(),
            fallback_fts_only: default_fallback_fts_only(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetsConfig {
    #[serde(default)]
    pub auto_scan_dirs: Vec<PathBuf>,

    #[serde(default = "default_ignore_patterns")]
    pub ignore_patterns: Vec<String>,
}

impl Default for AssetsConfig {
    fn default() -> Self {
        Self {
            auto_scan_dirs: vec![],
            ignore_patterns: default_ignore_patterns(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementConfig {
    #[serde(default = "default_owner")]
    pub default_owner: String,

    #[serde(default = "default_security_level")]
    pub default_security_level: String,
}

impl Default for PlacementConfig {
    fn default() -> Self {
        Self {
            default_owner: default_owner(),
            default_security_level: default_security_level(),
        }
    }
}

fn default_data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".steward")
}

fn default_db_path() -> PathBuf {
    default_data_dir().join("steward.db")
}

fn default_api_port() -> u16 {
    9527
}

fn default_api_bind() -> String {
    "127.0.0.1".to_string()
}

fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}

fn default_model() -> String {
    "nomic-embed-text".to_string()
}

fn default_dimensions() -> usize {
    768
}

fn default_fallback_fts_only() -> bool {
    true
}

fn default_owner() -> String {
    "user:digoal".to_string()
}

fn default_security_level() -> String {
    "internal".to_string()
}

fn default_ignore_patterns() -> Vec<String> {
    vec![
        ".git".to_string(),
        "node_modules".to_string(),
        "__pycache__".to_string(),
        ".steward".to_string(),
        "target".to_string(),
        ".venv".to_string(),
    ]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            db_path: default_db_path(),
            api_port: default_api_port(),
            api_bind: default_api_bind(),
            embedding: EmbeddingConfig::default(),
            assets: AssetsConfig::default(),
            placement: PlacementConfig::default(),
            storage: StorageConfig::default(),
            web: WebConfig::default(),
            hooks: vec![],
        }
    }
}

impl Config {
    /// 加载配置，若不存在则创建默认配置
    pub fn load_or_default(custom_path: Option<&Path>) -> Result<Self> {
        let config_path = match custom_path {
            Some(p) => p.to_path_buf(),
            None => {
                if let Ok(env_path) = std::env::var("STEWARD_CONFIG") {
                    PathBuf::from(env_path)
                } else {
                    default_data_dir().join("config.toml")
                }
            }
        };

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .with_context(|| format!("读取配置文件失败: {:?}", config_path))?;
            let cfg: Config = toml::from_str(&content)
                .with_context(|| format!("解析配置文件 TOML 失败: {:?}", config_path))?;
            cfg.validate()
                .with_context(|| format!("配置文件校验未通过: {:?}", config_path))?;
            Ok(cfg)
        } else {
            let cfg = Config::default();
            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("创建配置目录失败: {:?}", parent))?;
            }
            let toml_str = toml::to_string_pretty(&cfg)
                .context("序列化默认配置失败")?;
            std::fs::write(&config_path, toml_str)
                .with_context(|| format!("写入默认配置文件失败: {:?}", config_path))?;
            cfg.validate()?;
            Ok(cfg)
        }
    }

    /// 校验配置参数合法性与依赖关系
    pub fn validate(&self) -> Result<()> {
        if self.api_port < 1024 {
            anyhow::bail!("api_port ({}) 不能小于 1024 (特权端口限制)", self.api_port);
        }
        if self.web.enabled && self.web.port < 1024 {
            anyhow::bail!("web.port ({}) 不能小于 1024 (特权端口限制)", self.web.port);
        }
        if self.embedding.dimensions == 0 {
            anyhow::bail!("embedding.dimensions 不能为 0");
        }
        match self.storage.provider.as_str() {
            "sqlite" | "postgres" | "duckdb" => {}
            other => anyhow::bail!("未知存储后端: '{}'，仅支持 'sqlite' / 'postgres' / 'duckdb'", other),
        }
        if self.storage.provider == "postgres" && self.storage.pg_url.is_none() {
            anyhow::bail!("当 storage.provider 设置为 'postgres' 时，必须配置 storage.pg_url 连接串");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.api_port, 9527);
        assert_eq!(cfg.placement.default_owner, "user:digoal");
        assert_eq!(cfg.placement.default_security_level, "internal");
        assert!(cfg.embedding.fallback_fts_only);
    }

    #[test]
    fn test_toml_roundtrip() {
        let cfg = Config::default();
        let toml_str = toml::to_string(&cfg).expect("to_string");
        let parsed: Config = toml::from_str(&toml_str).expect("from_str");
        assert_eq!(parsed.api_port, cfg.api_port);
    }

    #[test]
    fn test_config_validation() {
        let mut cfg = Config::default();
        assert!(cfg.validate().is_ok());

        cfg.api_port = 80;
        assert!(cfg.validate().is_err());

        cfg.api_port = 9527;
        cfg.embedding.dimensions = 0;
        assert!(cfg.validate().is_err());

        cfg.embedding.dimensions = 768;
        cfg.storage.provider = "oracle".to_string();
        assert!(cfg.validate().is_err());

        cfg.storage.provider = "postgres".to_string();
        cfg.storage.pg_url = None;
        assert!(cfg.validate().is_err());

        cfg.storage.pg_url = Some("host=localhost dbname=steward".to_string());
        assert!(cfg.validate().is_ok());
    }
}
