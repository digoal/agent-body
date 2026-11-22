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
    pub hooks: Vec<crate::hooks::HookRule>,

    #[serde(default)]
    pub pipelines: Vec<crate::hooks::HookPipeline>,
}

/// 可移植宿主机工作区与规范目录配置 (Phase 55)
/// 若宿主机存在 `~/new`（历史约定工作台），自动将其作为默认基准目录以保持零回归；
/// 在全新开源宿主机上自动采用便携式 `~/agent-body-workspace` 与系统临时目录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    /// 宿主机工作区根目录 (默认: $AGENT_BODY_WORKSPACE_BASE 或 ~/new 或 ~/agent-body-workspace)
    #[serde(default = "default_workspace_base")]
    pub base_dir: PathBuf,

    /// 代码仓库与工程根目录列表 (用于 Fact 编目、Daemon 默认监听与拓扑深度归一化)
    #[serde(default = "default_workspace_roots")]
    pub workspace_roots: Vec<PathBuf>,

    /// 受管临时验证与导出安全目录 (默认: $AGENT_BODY_TMP_DIR 或 {base_dir}/tmp 或 OS temp_dir)
    #[serde(default = "default_temp_dir")]
    pub temp_dir: PathBuf,

    /// SKILL 生态扫描目录列表
    #[serde(default = "default_skills_dirs")]
    pub skills_dirs: Vec<PathBuf>,

    /// 笔记与权威资料目录列表
    #[serde(default = "default_notes_dirs")]
    pub notes_dirs: Vec<PathBuf>,

    /// 对外开源/镜像 Git 仓库目录列表 (供 GC 冗余缓存扫描)
    #[serde(default = "default_shared_git_dirs")]
    pub shared_git_dirs: Vec<PathBuf>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            base_dir: default_workspace_base(),
            workspace_roots: default_workspace_roots(),
            temp_dir: default_temp_dir(),
            skills_dirs: default_skills_dirs(),
            notes_dirs: default_notes_dirs(),
            shared_git_dirs: default_shared_git_dirs(),
        }
    }
}

/// 长期记忆引擎提供者配置 (`[memory]`, 驱动 MemoryFactory)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// 提供者类型: "builtin" | "powercontext" | "mem0" | "letta"
    #[serde(default = "default_memory_provider")]
    pub provider: String,

    #[serde(default)]
    pub powercontext_url: Option<String>,

    #[serde(default)]
    pub powercontext_scope_id: Option<String>,

    #[serde(default)]
    pub mem0_url: Option<String>,

    /// 仅填环境变量名 (例如 "MEM0_API_KEY")，不在 config.toml 中明文落盘凭据
    #[serde(default)]
    pub mem0_api_key_env: Option<String>,

    #[serde(default)]
    pub mem0_user_id: Option<String>,

    #[serde(default)]
    pub letta_url: Option<String>,

    #[serde(default)]
    pub letta_agent_id: Option<String>,

    /// 仅填环境变量名 (例如 "LETTA_API_TOKEN")
    #[serde(default)]
    pub letta_token_env: Option<String>,
}

fn default_memory_provider() -> String {
    "builtin".to_string()
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            provider: default_memory_provider(),
            powercontext_url: None,
            powercontext_scope_id: None,
            mem0_url: None,
            mem0_api_key_env: None,
            mem0_user_id: None,
            letta_url: None,
            letta_agent_id: None,
            letta_token_env: None,
        }
    }
}

/// 资产语义元数据 Ollama 自动标注配置 (Phase 23)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationConfig {
    /// 是否启用本地 Ollama 小模型语义标注精炼
    #[serde(default = "default_annotation_enabled")]
    pub enabled: bool,

    /// 本地或内网 Ollama 服务地址
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,

    /// 用于生成资产结构化元数据的轻量小模型（推荐 qwen3.5:0.8b）
    #[serde(default = "default_annotation_model")]
    pub model: String,

    /// 单文件标注请求超时毫秒
    #[serde(default = "default_annotation_timeout_ms")]
    pub timeout_ms: u64,

    /// 送入小模型的文件头部最大摘录字符数
    #[serde(default = "default_annotation_max_snippet_chars")]
    pub max_snippet_chars: usize,

    /// 守护进程每次巡检自动精炼的待标注资产批量上限
    #[serde(default = "default_auto_enrich_batch_size")]
    pub auto_enrich_batch_size: usize,
}

fn default_annotation_enabled() -> bool {
    true
}

fn default_annotation_model() -> String {
    "qwen3.5:0.8b".to_string()
}

fn default_annotation_timeout_ms() -> u64 {
    15000
}

fn default_annotation_max_snippet_chars() -> usize {
    1200
}

fn default_auto_enrich_batch_size() -> usize {
    10
}

impl Default for AnnotationConfig {
    fn default() -> Self {
        Self {
            enabled: default_annotation_enabled(),
            ollama_url: default_ollama_url(),
            model: default_annotation_model(),
            timeout_ms: default_annotation_timeout_ms(),
            max_snippet_chars: default_annotation_max_snippet_chars(),
            auto_enrich_batch_size: default_auto_enrich_batch_size(),
        }
    }
}

/// 轻量多机联邦 (Peer Mesh) 配置 (Phase 18 -> Phase 41 Wire Protocol v1.0)
/// 鉴权支持节点独立 `[mesh.peer_keys]`、全局 `[mesh].shared_secret` 或回退 `[web].auth_token`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshConfig {
    /// 是否启用联邦客户端能力（服务端 /api/v1/mesh/* 随 Web 服务常驻）
    #[serde(default)]
    pub enabled: bool,

    /// 本机节点标识（缺省自动取系统 hostname）
    #[serde(default)]
    pub node_id: Option<String>,

    /// 对等节点 Web 基地址列表，如 ["http://192.168.31.161:8756"]
    #[serde(default)]
    pub peers: Vec<String>,

    /// 单次对等节点请求超时毫秒
    #[serde(default = "default_mesh_timeout_ms")]
    pub timeout_ms: u64,

    /// Mesh 共享 HMAC 签名密钥（缺省回退至 [web].auth_token）
    #[serde(default)]
    pub shared_secret: Option<String>,

    /// 按节点 ID 指定的独立共享密钥 (node_id -> secret)
    #[serde(default)]
    pub peer_keys: std::collections::HashMap<String, String>,

    /// 按节点 ID 授权的作用域白名单 (node_id -> ["read:facts", "write:locks", "exchange:dialogue", "transfer:artifact"])
    #[serde(default)]
    pub peer_scopes: std::collections::HashMap<String, Vec<String>>,

    /// 信封防重放时间窗（秒，默认 ±300s）
    #[serde(default = "default_mesh_replay_window_secs")]
    pub replay_window_secs: u64,

    /// 跨机文件投递单文件体积上限（字节，默认 10MB）
    #[serde(default = "default_mesh_max_transfer_bytes")]
    pub max_transfer_bytes: usize,

    /// 跨机接收文件隔离沙盒收件箱路径（默认 ~/.agent-body/mesh_inbox）
    #[serde(default)]
    pub inbox_dir: Option<String>,
}

fn default_mesh_timeout_ms() -> u64 {
    1500
}

fn default_mesh_replay_window_secs() -> u64 {
    300
}

fn default_mesh_max_transfer_bytes() -> usize {
    10 * 1024 * 1024
}

impl Default for MeshConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            node_id: None,
            peers: vec![],
            timeout_ms: default_mesh_timeout_ms(),
            shared_secret: None,
            peer_keys: std::collections::HashMap::new(),
            peer_scopes: std::collections::HashMap::new(),
            replay_window_secs: default_mesh_replay_window_secs(),
            max_transfer_bytes: default_mesh_max_transfer_bytes(),
            inbox_dir: None,
        }
    }
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

fn default_security_level() -> String {
    "internal".to_string()
}

fn default_ignore_patterns() -> Vec<String> {
    vec![
        ".git".to_string(),
        "node_modules".to_string(),
        "__pycache__".to_string(),
        ".agent-body".to_string(),
        "target".to_string(),
        ".venv".to_string(),
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
            hooks: vec![],
            pipelines: vec![],
        }
    }
}

/// 纯函数解析配置文件查找优先级：`custom_path` > `AGENT_BODY_CONFIG` > `STEWARD_CONFIG` > `~/.agent-body/config.toml`
pub fn resolve_config_path_with(
    custom_path: Option<&Path>,
    agent_body_env: Option<&str>,
    steward_env: Option<&str>,
) -> PathBuf {
    if let Some(p) = custom_path {
        return p.to_path_buf();
    }
    if let Some(v) = agent_body_env.map(str::trim).filter(|s| !s.is_empty()) {
        return PathBuf::from(v);
    }
    if let Some(v) = steward_env.map(str::trim).filter(|s| !s.is_empty()) {
        return PathBuf::from(v);
    }
    default_data_dir().join("config.toml")
}

impl Config {
    /// 加载配置，若不存在则创建默认配置
    pub fn load_or_default(custom_path: Option<&Path>) -> Result<Self> {
        let ab_env = std::env::var("AGENT_BODY_CONFIG").ok();
        let st_env = std::env::var("STEWARD_CONFIG").ok();
        let config_path = resolve_config_path_with(
            custom_path,
            ab_env.as_deref(),
            st_env.as_deref(),
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
    pub fn load_effective_or_default() -> Self {
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

        let adapter_cfg = crate::memory::adapters::AdapterConfig {
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
        match self.memory.provider.trim() {
            "builtin" | "powercontext" | "mem0" | "letta" => {}
            other => anyhow::bail!(
                "未知 memory.provider: '{}'，仅支持 'builtin' / 'powercontext' / 'mem0' / 'letta'",
                other
            ),
        }
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
        Ok(())
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
                let _ = std::fs::set_permissions(path, perms);
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (path, is_dir);
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
            resolve_config_path_with(Some(explicit), Some("/tmp/ab.toml"), Some("/tmp/st.toml")),
            explicit
        );
        assert_eq!(
            resolve_config_path_with(None, Some("/tmp/ab.toml"), Some("/tmp/st.toml")),
            PathBuf::from("/tmp/ab.toml")
        );
        assert_eq!(
            resolve_config_path_with(None, Some(""), Some("/tmp/st.toml")),
            PathBuf::from("/tmp/st.toml")
        );
        assert_eq!(
            resolve_config_path_with(None, None, None),
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

        cfg.storage.pg_url = Some("host=localhost dbname=steward".to_string());
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
}
