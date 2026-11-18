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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    #[serde(default = "default_storage_provider")]
    pub provider: String,

    #[serde(default = "default_pg_url")]
    pub pg_url: Option<String>,
}

fn default_storage_provider() -> String {
    "sqlite".to_string()
}

fn default_pg_url() -> Option<String> {
    None
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            provider: default_storage_provider(),
            pg_url: default_pg_url(),
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
            Ok(cfg)
        }
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
}
