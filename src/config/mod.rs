use std::path::{Path, PathBuf};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use anyhow::{Context, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,

    #[serde(default = "default_db_path")]
    pub db_path: PathBuf,

    #[serde(default)]
    pub workspace: WorkspaceConfig,

    #[serde(default)]
    pub memory: MemoryConfig,

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
    pub mesh: MeshConfig,

    #[serde(default)]
    pub annotation: AnnotationConfig,

    #[serde(default)]
    pub runtime: RuntimeConfig,

    #[serde(default)]
    pub knowledge_graph: KnowledgeGraphConfig,

    #[serde(default)]
    pub hooks: Vec<crate::hooks::HookRule>,

    #[serde(default)]
    pub pipelines: Vec<crate::hooks::HookPipeline>,
}


mod types;
pub use types::*;
/// 跨平台读取当前用户 Home 目录
pub fn home_dir() -> PathBuf {
    if let Ok(h) = std::env::var("HOME") {
        if !h.trim().is_empty() {
            return PathBuf::from(h);
        }
    }
    if let Ok(h) = std::env::var("USERPROFILE") {
        if !h.trim().is_empty() {
            return PathBuf::from(h);
        }
    }
    std::env::temp_dir()
}

pub fn default_data_dir() -> PathBuf {
    home_dir().join(".agent-body")
}

pub fn default_db_path() -> PathBuf {
    default_data_dir().join("body.db")
}

/// 宿主机默认工作区基准目录：优先 `$AGENT_BODY_WORKSPACE_BASE` -> `$HOME/new` (若存在) -> `$HOME/agent-body-workspace`
pub fn default_workspace_base() -> PathBuf {
    if let Ok(v) = std::env::var("AGENT_BODY_WORKSPACE_BASE") {
        let t = v.trim();
        if !t.is_empty() {
            return PathBuf::from(t);
        }
    }
    let home = home_dir();
    let legacy_new = home.join("new");
    if legacy_new.is_dir() {
        legacy_new
    } else {
        home.join("agent-body-workspace")
    }
}

/// 宿主机默认安全临时目录：优先 `$AGENT_BODY_TMP_DIR` -> `{base}/tmp` (若存在) -> OS temp_dir/agent-body
pub fn default_temp_dir() -> PathBuf {
    if let Ok(v) = std::env::var("AGENT_BODY_TMP_DIR") {
        let t = v.trim();
        if !t.is_empty() {
            return PathBuf::from(t);
        }
    }
    let base_tmp = default_workspace_base().join("tmp");
    if base_tmp.is_dir() {
        base_tmp
    } else {
        std::env::temp_dir().join("agent-body")
    }
}

pub fn default_workspace_roots() -> Vec<PathBuf> {
    let base = default_workspace_base();
    vec![
        base.join("work"),
        base.join("shared_proj"),
        base.join("src"),
        base.join("shared_git"),
    ]
}

pub fn default_skills_dirs() -> Vec<PathBuf> {
    let home = home_dir();
    let candidates = [
        default_workspace_base().join("skills"),
        home.join(".agent-body").join("skills"),
        home.join(".gemini/antigravity-cli/skills"),
        home.join(".gemini/antigravity-cli/builtin/skills"),
    ];
    let mut out = Vec::new();
    for c in candidates {
        if !out.contains(&c) {
            out.push(c);
        }
    }
    out
}

pub fn default_notes_dirs() -> Vec<PathBuf> {
    vec![default_workspace_base().join("note")]
}

pub fn default_shared_git_dirs() -> Vec<PathBuf> {
    vec![default_workspace_base().join("shared_git")]
}

pub(super) fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}

pub(super) fn default_model() -> String {
    "nomic-embed-text".to_string()
}

pub(super) fn default_dimensions() -> usize {
    768
}

pub(super) fn default_fallback_fts_only() -> bool {
    true
}

/// 从当前环境变量推导可移植默认归属主体 (`user:<username>`，缺省 `user:local`)
pub fn default_owner() -> String {
    if let Ok(u) = std::env::var("USER") {
        let t = u.trim();
        if !t.is_empty() {
            return format!("user:{}", t);
        }
    }
    if let Ok(u) = std::env::var("USERNAME") {
        let t = u.trim();
        if !t.is_empty() {
            return format!("user:{}", t);
        }
    }
    "user:local".to_string()
}

pub(super) fn default_security_level() -> String {
    "internal".to_string()
}

pub(super) fn default_ignore_patterns() -> Vec<String> {
    vec![
        ".git".to_string(),
        "node_modules".to_string(),
        "__pycache__".to_string(),
        ".agent-body".to_string(),
        "target".to_string(),
        ".venv".to_string(),
        "dist".to_string(),
        "build".to_string(),
        ".next".to_string(),
        ".nuxt".to_string(),
        ".cache".to_string(),
        "coverage".to_string(),
        ".pytest_cache".to_string(),
        ".mypy_cache".to_string(),
        ".ruff_cache".to_string(),
    ]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            db_path: default_db_path(),
            workspace: WorkspaceConfig::default(),
            memory: MemoryConfig::default(),
            embedding: EmbeddingConfig::default(),
            assets: AssetsConfig::default(),
            placement: PlacementConfig::default(),
            storage: StorageConfig::default(),
            web: WebConfig::default(),
            mesh: MeshConfig::default(),
            annotation: AnnotationConfig::default(),
            runtime: RuntimeConfig::default(),
            knowledge_graph: KnowledgeGraphConfig::default(),
            hooks: vec![],
            pipelines: vec![],
        }
    }
}

/// 纯函数解析配置文件查找优先级：`custom_path` > `AGENT_BODY_CONFIG` > `~/.agent-body/config.toml`
pub fn resolve_config_path_with(
    custom_path: Option<&Path>,
    agent_body_env: Option<&str>,
) -> PathBuf {
    if let Some(p) = custom_path {
        return p.to_path_buf();
    }
    if let Some(v) = agent_body_env.map(str::trim).filter(|s| !s.is_empty()) {
        return PathBuf::from(v);
    }
    default_data_dir().join("config.toml")
}

impl Config {
    /// 加载配置，若不存在则创建默认配置
    pub fn load_or_default(custom_path: Option<&Path>) -> Result<Self> {
        let ab_env = std::env::var("AGENT_BODY_CONFIG").ok();
        let config_path = resolve_config_path_with(
            custom_path,
            ab_env.as_deref(),
        );

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
                restrict_private_permissions(parent, true);
            }
            let toml_str = toml::to_string_pretty(&cfg)
                .context("序列化默认配置失败")?;
            std::fs::write(&config_path, toml_str)
                .with_context(|| format!("写入默认配置文件失败: {:?}", config_path))?;
            restrict_private_permissions(&config_path, false);
            cfg.validate()?;
            Ok(cfg)
        }
    }

    /// 无异常读取当前生效配置；若加载失败则安全回退到 `Config::default()`
    /// 单元测试模式下除非显式指定 `AGENT_BODY_CONFIG`，否则隔离宿主机真实配置以保持零外部污染
    pub fn load_effective_or_default() -> Self {
        if cfg!(test) && std::env::var_os("AGENT_BODY_CONFIG").is_none() {
            return Self::default();
        }
        Self::load_or_default(None).unwrap_or_default()
    }

    /// 获取有效工作区基准目录
    pub fn effective_workspace_base(&self) -> PathBuf {
        self.workspace.base_dir.clone()
    }

    /// 获取有效工作区代码根目录列表
    pub fn effective_workspace_roots(&self) -> Vec<PathBuf> {
        if self.workspace.workspace_roots.is_empty() {
            default_workspace_roots()
        } else {
            self.workspace.workspace_roots.clone()
        }
    }

    /// 获取有效临时工作目录
    pub fn effective_temp_dir(&self) -> PathBuf {
        self.workspace.temp_dir.clone()
    }

    /// 解析当前生效的运行模式（`AGENT_BODY_MODE` > override 文件 > `[runtime].mode`）
    pub fn effective_runtime_mode(&self) -> RuntimeMode {
        if let Ok(env_mode) = std::env::var("AGENT_BODY_MODE") {
            if let Some(m) = RuntimeMode::parse_str(&env_mode) {
                return m;
            }
        }
        if !cfg!(test) {
            if let Some(m) = Self::read_mode_override_in(&self.data_dir) {
                return m;
            }
        }
        RuntimeMode::parse_str(&self.runtime.mode).unwrap_or(RuntimeMode::BodyOnly)
    }

    pub fn read_mode_override_in(dir: &Path) -> Option<RuntimeMode> {
        for name in ["runtime_mode.override", "execution_mode.override"] {
            if let Ok(raw) = std::fs::read_to_string(dir.join(name)) {
                if let Some(m) = RuntimeMode::parse_str(&raw) {
                    return Some(m);
                }
            }
        }
        None
    }

    /// 解析当前生效的 MCP 工具暴露档位（环境变量 `AGENT_BODY_MCP_PROFILE` 优先于 `[runtime].mcp_profile`）
    pub fn effective_mcp_profile(&self) -> McpToolProfile {
        if let Ok(env_prof) = std::env::var("AGENT_BODY_MCP_PROFILE") {
            if let Some(p) = McpToolProfile::parse_str(&env_prof) {
                return p;
            }
        }
        McpToolProfile::parse_str(&self.runtime.mcp_profile).unwrap_or(McpToolProfile::Full)
    }

    /// 持久化更新 `[runtime]` 配置并写回配置文件（保持 0600 安全权限）
    pub fn update_runtime_mode(
        custom_path: Option<&Path>,
        mode: RuntimeMode,
        shadow_primary: Option<&str>,
        mcp_profile: Option<McpToolProfile>,
    ) -> Result<(PathBuf, Self)> {
        let ab_env = std::env::var("AGENT_BODY_CONFIG").ok();
        let config_path = resolve_config_path_with(
            custom_path,
            ab_env.as_deref(),
        );
        let mut cfg = Self::load_or_default(custom_path)?;
        cfg.runtime.mode = mode.as_str().to_string();
        if let Some(primary) = shadow_primary {
            cfg.runtime.shadow_primary = primary.trim().to_ascii_lowercase();
        }
        if let Some(profile) = mcp_profile {
            cfg.runtime.mcp_profile = profile.as_str().to_string();
        }
        cfg.validate()?;
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("创建配置目录失败: {:?}", parent))?;
            restrict_private_permissions(parent, true);
        }
        let toml_str = toml::to_string_pretty(&cfg).context("序列化配置失败")?;
        std::fs::write(&config_path, toml_str)
            .with_context(|| format!("写入配置文件失败: {:?}", config_path))?;
        restrict_private_permissions(&config_path, false);
        Ok((config_path, cfg))
    }

    /// 获取有效 SKILL 目录列表
    pub fn effective_skills_dirs(&self) -> Vec<PathBuf> {
        if self.workspace.skills_dirs.is_empty() {
            default_skills_dirs()
        } else {
            self.workspace.skills_dirs.clone()
        }
    }

    /// 获取有效笔记资料目录列表
    pub fn effective_notes_dirs(&self) -> Vec<PathBuf> {
        if self.workspace.notes_dirs.is_empty() {
            default_notes_dirs()
        } else {
            self.workspace.notes_dirs.clone()
        }
    }

    /// 获取有效共享/开源仓库目录列表
    pub fn effective_shared_git_dirs(&self) -> Vec<PathBuf> {
        if self.workspace.shared_git_dirs.is_empty() {
            default_shared_git_dirs()
        } else {
            self.workspace.shared_git_dirs.clone()
        }
    }

    /// 获取有效默认归属主体
    pub fn effective_default_owner(&self) -> String {
        let trimmed = self.placement.default_owner.trim();
        if trimmed.is_empty() {
            default_owner()
        } else {
            trimmed.to_string()
        }
    }

    /// 返回允许导出/GC/沙盒直写的安全临时根目录白名单
    pub fn allowed_temp_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        for cand in [
            self.effective_temp_dir(),
            PathBuf::from("/tmp"),
            std::env::temp_dir(),
        ] {
            if !roots.contains(&cand) {
                roots.push(cand);
            }
        }
        roots
    }

    /// 校验路径是否安全位于受允许临时目录白名单内（拦截目录穿越与软链接逃逸）
    pub fn is_allowed_temp_path(&self, path: &Path) -> bool {
        if !path.is_absolute() {
            return false;
        }
        if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return false;
        }
        let allowed = self.allowed_temp_roots();
        if !allowed.iter().any(|r| path.starts_with(r)) {
            return false;
        }
        if let Some(parent) = path.parent() {
            if parent.exists() {
                if let Ok(canon_parent) = std::fs::canonicalize(parent) {
                    return allowed.iter().any(|r| {
                        std::fs::canonicalize(r)
                            .map(|cr| canon_parent.starts_with(&cr))
                            .unwrap_or_else(|_| canon_parent.starts_with(r))
                    });
                }
                return false;
            }
        }
        true
    }

    /// 根据 `[memory]` 配置经由 `MemoryFactory` 构建 `MemoryProvider`（外部适配器失败时安全降级）
    pub fn build_memory_provider(
        &self,
        storage: crate::storage::Storage,
    ) -> Arc<dyn crate::memory::MemoryProvider> {
        let provider_name = self.memory.provider.trim();
        if provider_name.is_empty() || provider_name.eq_ignore_ascii_case("builtin") {
            return Arc::new(crate::memory::builtin_provider::BuiltinMemoryProvider::new(
                storage,
            ));
        }

        let mem0_api_key = self
            .memory
            .mem0_api_key_env
            .as_deref()
            .and_then(|env_name| std::env::var(env_name.trim()).ok())
            .unwrap_or_default();
        let letta_token = self
            .memory
            .letta_token_env
            .as_deref()
            .and_then(|env_name| std::env::var(env_name.trim()).ok())
            .unwrap_or_default();

        let resolved_pg_url = self
            .memory
            .pg_url
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| self.storage.pg_url.clone().filter(|s| !s.trim().is_empty()))
            .or_else(|| {
                std::env::var("AGENT_BODY_MEMORY_PG_URL")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
            })
            .unwrap_or_default();

        let adapter_cfg = crate::memory::adapters::AdapterConfig {
            pg_url: resolved_pg_url,
            pg_pool_size: self.memory.pg_pool_size,
            pg_tenant_scope: self.memory.pg_tenant_scope.clone(),
            vector_backend: self.memory.vector_backend.clone(),
            fallback_to_sqlite: self.memory.fallback_to_sqlite,
            powercontext_url: self.memory.powercontext_url.clone().unwrap_or_default(),
            powercontext_scope_id: self.memory.powercontext_scope_id.clone().unwrap_or_default(),
            mem0_url: self.memory.mem0_url.clone().unwrap_or_default(),
            mem0_api_key,
            mem0_user_id: self.memory.mem0_user_id.clone().unwrap_or_default(),
            letta_url: self.memory.letta_url.clone().unwrap_or_default(),
            letta_agent_id: self.memory.letta_agent_id.clone().unwrap_or_default(),
            letta_token,
        };

        match crate::memory::adapters::MemoryFactory::create(
            provider_name,
            &adapter_cfg,
            storage.clone(),
        ) {
            Ok(p) => p,
            Err(e) => {
                eprintln!(
                    "[Config][WARN] 构建记忆后端 '{}' 失败，安全降级为内置 SQLite 引擎: {}",
                    provider_name, e
                );
                Arc::new(crate::memory::builtin_provider::BuiltinMemoryProvider::new(
                    storage,
                ))
            }
        }
    }

    /// 构建支持双时态 MVCC 契约的 `TxMemoryProvider`（未启用 `pg_tx` 或 PG 不可达时自动降级本地 SQLite）
    pub fn build_tx_memory_provider(
        &self,
        storage: crate::storage::Storage,
    ) -> Result<Arc<dyn crate::memory::TxMemoryProvider>> {
        let resolved_pg_url = if self.memory.provider.trim().eq_ignore_ascii_case("pg_tx")
            || std::env::var("AGENT_BODY_MEMORY_PG_URL")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .is_some()
        {
            self.memory
                .pg_url
                .clone()
                .filter(|s| !s.trim().is_empty())
                .or_else(|| self.storage.pg_url.clone().filter(|s| !s.trim().is_empty()))
                .or_else(|| {
                    std::env::var("AGENT_BODY_MEMORY_PG_URL")
                        .ok()
                        .filter(|s| !s.trim().is_empty())
                })
                .unwrap_or_default()
        } else {
            String::new()
        };

        let adapter_cfg = crate::memory::adapters::AdapterConfig {
            pg_url: resolved_pg_url,
            pg_pool_size: self.memory.pg_pool_size,
            pg_tenant_scope: self.memory.pg_tenant_scope.clone(),
            vector_backend: self.memory.vector_backend.clone(),
            fallback_to_sqlite: self.memory.fallback_to_sqlite,
            ..crate::memory::adapters::AdapterConfig::default()
        };
        crate::memory::adapters::MemoryFactory::create_tx(&adapter_cfg, storage)
    }

    /// 校验配置参数合法性与依赖关系
    pub fn validate(&self) -> Result<()> {
        if self.web.enabled && self.web.port < 1024 {
            anyhow::bail!("web.port ({}) 不能小于 1024 (特权端口限制)", self.web.port);
        }
        if self.embedding.dimensions == 0 {
            anyhow::bail!("embedding.dimensions 不能为 0");
        }
        match self.storage.provider.as_str() {
            "sqlite" | "postgres" | "duckdb" => {}
            other => anyhow::bail!(
                "未知存储后端: '{}'，仅支持 'sqlite' / 'postgres' / 'duckdb'",
                other
            ),
        }
        if self.storage.provider == "postgres" && self.storage.pg_url.is_none() {
            anyhow::bail!("当 storage.provider 设置为 'postgres' 时，必须配置 storage.pg_url 连接串");
        }
        self.memory.validate(self.storage.pg_url.as_deref())?;
        for peer in &self.mesh.peers {
            if !(peer.starts_with("http://") || peer.starts_with("https://")) {
                anyhow::bail!("mesh.peers 中的 '{}' 必须以 http:// 或 https:// 开头", peer);
            }
        }
        if self.mesh.enabled {
            if self.mesh.timeout_ms < 100 || self.mesh.timeout_ms > 60_000 {
                anyhow::bail!(
                    "mesh.timeout_ms ({}) 必须位于 100..=60000 毫秒之间",
                    self.mesh.timeout_ms
                );
            }
            if self.mesh.replay_window_secs == 0 || self.mesh.replay_window_secs > 3600 {
                anyhow::bail!(
                    "mesh.replay_window_secs ({}) 必须位于 1..=3600 秒之间",
                    self.mesh.replay_window_secs
                );
            }
            if self.mesh.max_transfer_bytes == 0 || self.mesh.max_transfer_bytes > 50 * 1024 * 1024 {
                anyhow::bail!(
                    "mesh.max_transfer_bytes ({}) 必须位于 1..=52428800 (50MB) 之间",
                    self.mesh.max_transfer_bytes
                );
            }
            let has_web_token = self
                .web
                .auth_token
                .as_deref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            let has_shared_secret = self
                .mesh
                .shared_secret
                .as_deref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            let has_peer_keys = !self.mesh.peer_keys.is_empty();
            if !self.mesh.peers.is_empty() && !has_web_token && !has_shared_secret && !has_peer_keys {
                anyhow::bail!(
                    "启用 mesh 联邦且配置了 peers 时，必须设置 [mesh].shared_secret、[mesh.peer_keys] 或 [web].auth_token 作为签名密钥"
                );
            }
        }
        if self.annotation.enabled && self.annotation.timeout_ms < 500 {
            anyhow::bail!(
                "annotation.timeout_ms ({}) 不能小于 500ms",
                self.annotation.timeout_ms
            );
        }
        if self.annotation.enabled && self.annotation.max_snippet_chars < 200 {
            anyhow::bail!(
                "annotation.max_snippet_chars ({}) 不能小于 200",
                self.annotation.max_snippet_chars
            );
        }
        if self.annotation.enabled && self.annotation.vision_enabled {
            if self.annotation.vision_model.trim().is_empty() {
                anyhow::bail!("启用 annotation.vision_enabled 时，annotation.vision_model 不能为空");
            }
            if self.annotation.vision_max_image_bytes < 1024
                || self.annotation.vision_max_image_bytes > 50 * 1024 * 1024
            {
                anyhow::bail!(
                    "annotation.vision_max_image_bytes ({}) 必须位于 1024..=52428800 (50MB) 之间",
                    self.annotation.vision_max_image_bytes
                );
            }
        }
        if RuntimeMode::parse_str(&self.runtime.mode).is_none() {
            anyhow::bail!(
                "未知 runtime.mode: '{}'，仅支持 'body_only' / 'bypass' / 'shadow_ab'",
                self.runtime.mode
            );
        }
        if McpToolProfile::parse_str(&self.runtime.mcp_profile).is_none() {
            anyhow::bail!(
                "未知 runtime.mcp_profile: '{}'，仅支持 'core' / 'dev' / 'ops' / 'full'",
                self.runtime.mcp_profile
            );
        }
        if !matches!(self.runtime.shadow_primary.trim(), "live" | "body") {
            anyhow::bail!(
                "未知 runtime.shadow_primary: '{}'，仅支持 'live' / 'body'",
                self.runtime.shadow_primary
            );
        }
        if self.runtime.shadow_max_live_ms < 50 || self.runtime.shadow_max_live_ms > 30_000 {
            anyhow::bail!(
                "runtime.shadow_max_live_ms ({}) 必须位于 50..=30000 毫秒之间",
                self.runtime.shadow_max_live_ms
            );
        }
        if !(0.0..=1.0).contains(&self.runtime.ab_sample_rate) || self.runtime.ab_sample_rate.is_nan() {
            anyhow::bail!(
                "runtime.ab_sample_rate ({}) 必须位于 0.0..=1.0 之间",
                self.runtime.ab_sample_rate
            );
        }
        if self.runtime.token_guard_max_full_read_bytes < 4096 {
            anyhow::bail!(
                "runtime.token_guard_max_full_read_bytes ({}) 不能小于 4096",
                self.runtime.token_guard_max_full_read_bytes
            );
        }
        if self.runtime.token_guard_max_full_read_lines < 50 {
            anyhow::bail!(
                "runtime.token_guard_max_full_read_lines ({}) 不能小于 50",
                self.runtime.token_guard_max_full_read_lines
            );
        }
        if self.runtime.token_guard_auto_slice_lines < 20 {
            anyhow::bail!(
                "runtime.token_guard_auto_slice_lines ({}) 不能小于 20",
                self.runtime.token_guard_auto_slice_lines
            );
        }
        if self.runtime.token_guard_search_max_lines < 20 {
            anyhow::bail!(
                "runtime.token_guard_search_max_lines ({}) 不能小于 20",
                self.runtime.token_guard_search_max_lines
            );
        }
        if self.knowledge_graph.max_section_chars < 200 || self.knowledge_graph.max_section_chars > 8000 {
            anyhow::bail!(
                "knowledge_graph.max_section_chars ({}) 必须位于 200..=8000 之间",
                self.knowledge_graph.max_section_chars
            );
        }
        if self.knowledge_graph.default_search_hops > 2 {
            anyhow::bail!(
                "knowledge_graph.default_search_hops ({}) 必须位于 0..=2 之间",
                self.knowledge_graph.default_search_hops
            );
        }
        if self.knowledge_graph.pmi_threshold < 0.0 || self.knowledge_graph.pmi_threshold.is_nan() {
            anyhow::bail!(
                "knowledge_graph.pmi_threshold ({}) 不能为负数",
                self.knowledge_graph.pmi_threshold
            );
        }
        if self.knowledge_graph.relate_timeout_ms == 0 {
            anyhow::bail!("knowledge_graph.relate_timeout_ms 必须大于 0");
        }
        if self.knowledge_graph.relate_max_rows == 0 {
            anyhow::bail!("knowledge_graph.relate_max_rows 必须大于 0");
        }
        Ok(())
    }

    /// 解析 Relate 图谱扩展运行配置
    pub fn resolve_relate_config(&self) -> ext_graph_relate::RelateGraphConfig {
        let db_path = self
            .knowledge_graph
            .relate_db_path
            .clone()
            .unwrap_or_else(|| self.data_dir.join("graph.rl"));
        ext_graph_relate::RelateGraphConfig {
            db_path: db_path.to_string_lossy().into_owned(),
            lib_path: self.knowledge_graph.relate_lib_path.clone(),
            read_only: false,
            timeout_ms: self.knowledge_graph.relate_timeout_ms,
            max_rows: self.knowledge_graph.relate_max_rows,
            namespace: "neo4j".to_string(),
        }
    }

    /// 按配置尝试打开 Relate 图谱引擎；`relate_enabled=false` 时返回 `Ok(None)`，
    /// 打开失败且 `relate_fallback_to_sqlite=true` 时优雅降级返回 `Ok(None)`
    pub fn try_open_relate_graph(&self) -> Result<Option<ext_graph_relate::RelateGraphEngine>> {
        if !self.knowledge_graph.relate_enabled {
            return Ok(None);
        }
        let cfg = self.resolve_relate_config();
        match ext_graph_relate::RelateGraphEngine::open(cfg) {
            Ok(eng) => Ok(Some(eng)),
            Err(e) => {
                if self.knowledge_graph.relate_fallback_to_sqlite {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }
    }
}

/// 在 Unix 平台将私密目录收敛为 0700、私密文件（config.toml / body.db）收敛为 0600
pub fn restrict_private_permissions(path: &Path, is_dir: bool) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if is_dir { 0o700 } else { 0o600 };
        if let Ok(meta) = std::fs::metadata(path) {
            let mut perms = meta.permissions();
            if (perms.mode() & 0o777) != mode {
                perms.set_mode(mode);
                if let Err(e) = std::fs::set_permissions(path, perms) {
                    eprintln!("[Config][WARN] 收敛文件权限 {:?} 失败: {}", path, e);
                }
            }
        }
    }
    #[cfg(not(unix))]
    {
        let (_p, _d) = (path, is_dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.web.port, 8765);
        assert!(cfg.placement.default_owner.starts_with("user:"));
        assert_eq!(cfg.placement.default_security_level, "internal");
        assert!(cfg.embedding.fallback_fts_only);
        assert_eq!(cfg.memory.provider, "builtin");
        assert!(!cfg.workspace.workspace_roots.is_empty());
    }

    #[test]
    fn test_toml_roundtrip() {
        let cfg = Config::default();
        let toml_str = toml::to_string(&cfg).expect("to_string");
        let parsed: Config = toml::from_str(&toml_str).expect("from_str");
        assert_eq!(parsed.web.port, cfg.web.port);
        assert_eq!(parsed.workspace.temp_dir, cfg.workspace.temp_dir);
    }

    #[test]
    fn test_legacy_config_ignores_removed_api_port_bind() {
        let legacy_toml = r#"
data_dir = "/tmp/ab-data"
db_path = "/tmp/ab-data/body.db"
api_port = 9527
api_bind = "127.0.0.1"

[placement]
default_owner = "user:digoal"
"#;
        let parsed: Config = toml::from_str(legacy_toml).expect("legacy toml parse");
        parsed.validate().expect("legacy validate");
        assert_eq!(parsed.placement.default_owner, "user:digoal");
    }

    #[test]
    fn test_agent_body_config_env_takes_precedence() {
        let explicit = Path::new("/tmp/explicit.toml");
        assert_eq!(
            resolve_config_path_with(Some(explicit), Some("/tmp/ab.toml")),
            explicit
        );
        assert_eq!(
            resolve_config_path_with(None, Some("/tmp/ab.toml")),
            PathBuf::from("/tmp/ab.toml")
        );
        assert_eq!(
            resolve_config_path_with(None, Some("")),
            default_data_dir().join("config.toml")
        );
        assert_eq!(
            resolve_config_path_with(None, None),
            default_data_dir().join("config.toml")
        );
    }

    #[test]
    fn test_config_validation() {
        let mut cfg = Config::default();
        assert!(cfg.validate().is_ok());

        cfg.web.port = 80;
        assert!(cfg.validate().is_err());

        cfg.web.port = 8765;
        cfg.embedding.dimensions = 0;
        assert!(cfg.validate().is_err());

        cfg.embedding.dimensions = 768;
        cfg.storage.provider = "oracle".to_string();
        assert!(cfg.validate().is_err());

        cfg.storage.provider = "postgres".to_string();
        cfg.storage.pg_url = None;
        assert!(cfg.validate().is_err());

        cfg.storage.pg_url = Some("host=localhost dbname=agent_body".to_string());
        assert!(cfg.validate().is_ok());

        cfg.memory.provider = "unknown_mem".to_string();
        assert!(cfg.validate().is_err());
        cfg.memory.provider = "powercontext".to_string();
        assert!(cfg.validate().is_ok());

        cfg.annotation.timeout_ms = 100;
        assert!(cfg.validate().is_err());
        cfg.annotation.timeout_ms = 8000;
        cfg.annotation.max_snippet_chars = 50;
        assert!(cfg.validate().is_err());
        cfg.annotation.max_snippet_chars = 1200;
        assert!(cfg.validate().is_ok());

        cfg.annotation.vision_enabled = true;
        cfg.annotation.vision_max_image_bytes = 512;
        assert!(cfg.validate().is_err());
        cfg.annotation.vision_max_image_bytes = 4 * 1024 * 1024;
        assert!(cfg.validate().is_ok());

        cfg.knowledge_graph.max_section_chars = 100;
        assert!(cfg.validate().is_err());
        cfg.knowledge_graph.max_section_chars = 900;
        cfg.knowledge_graph.default_search_hops = 3;
        assert!(cfg.validate().is_err());
        cfg.knowledge_graph.default_search_hops = 1;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_mesh_validation_accepts_shared_secret_and_checks_bounds() {
        let mut cfg = Config::default();
        cfg.mesh.enabled = true;
        cfg.mesh.peers = vec!["http://127.0.0.1:8765".to_string()];
        // 未配置任何密钥应报错
        assert!(cfg.validate().is_err());

        // 仅配置 mesh.shared_secret 应通过
        cfg.mesh.shared_secret = Some("mesh-secret-xyz".to_string());
        assert!(cfg.validate().is_ok());

        // replay_window 越界检查
        cfg.mesh.replay_window_secs = 0;
        assert!(cfg.validate().is_err());
        cfg.mesh.replay_window_secs = 300;

        // timeout_ms 越界检查
        cfg.mesh.timeout_ms = 20;
        assert!(cfg.validate().is_err());
        cfg.mesh.timeout_ms = 1500;

        // max_transfer_bytes 越界检查 (>50MB)
        cfg.mesh.max_transfer_bytes = 60 * 1024 * 1024;
        assert!(cfg.validate().is_err());
        cfg.mesh.max_transfer_bytes = 10 * 1024 * 1024;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_memory_config_provider_validation_and_factory() {
        let mut cfg = Config::default();
        cfg.memory.provider = "powercontext".to_string();
        cfg.memory.powercontext_url = Some("http://127.0.0.1:19999".to_string());
        let st = crate::storage::Storage::open_in_memory().expect("open_in_memory");
        let p = cfg.build_memory_provider(st);
        // PowerContext 不可达时应安全降级至 builtin
        assert_eq!(p.provider_name(), "builtin");
    }

    #[test]
    fn test_pipelines_config_deserialization() {
        let toml_str = r#"
[[pipelines]]
id = "pipe-1"
name = "测试编排"
event_pattern = "on_file_created"
debounce_ms = 250

[[pipelines.steps]]
name = "step-1"
action = { action = "exec", command = "echo ok" }
on_failure = "continue"
timeout_ms = 1000
"#;
        let parsed: Config = toml::from_str(toml_str).expect("parse pipelines");
        assert_eq!(parsed.pipelines.len(), 1);
        assert_eq!(parsed.pipelines[0].id, "pipe-1");
        assert_eq!(parsed.pipelines[0].steps.len(), 1);
    }

    #[test]
    fn test_runtime_config_modes_profiles_and_persist() {
        let mut cfg = Config::default();
        assert_eq!(cfg.effective_runtime_mode(), RuntimeMode::BodyOnly);
        assert_eq!(cfg.effective_mcp_profile(), McpToolProfile::Full);

        cfg.runtime.mode = "shadow_ab".to_string();
        cfg.runtime.mcp_profile = "core".to_string();
        cfg.runtime.shadow_primary = "live".to_string();
        assert!(cfg.validate().is_ok());

        cfg.runtime.mode = "invalid_mode".to_string();
        assert!(cfg.validate().is_err());
        cfg.runtime.mode = "bypass".to_string();
        cfg.runtime.mcp_profile = "unknown".to_string();
        assert!(cfg.validate().is_err());
        cfg.runtime.mcp_profile = "dev".to_string();
        cfg.runtime.shadow_primary = "other".to_string();
        assert!(cfg.validate().is_err());
        cfg.runtime.shadow_primary = "body".to_string();
        assert!(cfg.validate().is_ok());

        let tmp_dir = tempfile::tempdir().expect("tempdir");
        let cfg_path = tmp_dir.path().join("config.toml");
        let (written, updated) = Config::update_runtime_mode(
            Some(&cfg_path),
            RuntimeMode::ShadowAb,
            Some("live"),
            Some(McpToolProfile::Core),
        )
        .expect("update_runtime_mode");
        assert_eq!(written, cfg_path);
        assert_eq!(updated.runtime.mode, "shadow_ab");
        assert_eq!(updated.runtime.mcp_profile, "core");

        let reloaded = Config::load_or_default(Some(&cfg_path)).expect("reload");
        assert_eq!(reloaded.runtime.mode, "shadow_ab");
        assert_eq!(reloaded.runtime.mcp_profile, "core");
        assert_eq!(reloaded.runtime.shadow_primary, "live");
    }
}
