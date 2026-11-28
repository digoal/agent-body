use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use super::*;

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
    /// 提供者类型: "builtin" | "pg_tx" | "powercontext" | "mem0" | "letta"
    #[serde(default = "default_memory_provider")]
    pub provider: String,

    /// Phase 76: PostgreSQL 事务级记忆连接串（留空则回退 `[storage].pg_url` 或 `AGENT_BODY_MEMORY_PG_URL`）
    #[serde(default)]
    pub pg_url: Option<String>,

    /// Phase 76: PostgreSQL 事务记忆连接池最大连接数 (1..=64，默认 8)
    #[serde(default = "default_memory_pg_pool_size")]
    pub pg_pool_size: usize,

    /// Phase 76: 事务记忆租户/项目隔离域 (默认 "default")
    #[serde(default = "default_memory_tenant_scope")]
    pub pg_tenant_scope: String,

    /// Phase 76: 解耦向量索引后端: "none" | "pgvector" | "pg_vectorsearch" | "external_lsm"
    #[serde(default = "default_memory_vector_backend")]
    pub vector_backend: String,

    /// Phase 76: 当 PostgreSQL 不可达时是否自动无损降级为本地内置 SQLite 引擎 (默认 true)
    #[serde(default = "default_true_flag")]
    pub fallback_to_sqlite: bool,

    /// Phase 77: 是否在 L1_AUTONOMIC 空闲窗口期启用睡眠整理 (`SleepTimeCompactor`)
    #[serde(default = "default_true_flag")]
    pub sleep_compaction_enabled: bool,

    /// Phase 77: 空闲窗口 CPU 利用率/归一化负载阈值 (%)，超过则延后睡眠整理 (5.0..=95.0，默认 45.0)
    #[serde(default = "default_sleep_idle_cpu_pct")]
    pub sleep_idle_cpu_pct: f32,

    /// Phase 77: 触发单轮聚类压缩的最小事件数 (>= 2，默认 4)
    #[serde(default = "default_sleep_min_events")]
    pub sleep_min_events: usize,

    /// Phase 77: 单轮睡眠整理最大扫描事件批大小 (>= sleep_min_events，默认 64)
    #[serde(default = "default_sleep_max_batch")]
    pub sleep_max_batch: usize,

    /// Phase 78 (方案 B / M3): 是否启用旁路快权重血缘控制面与神经 Compaction
    #[serde(default)]
    pub fast_weight_enabled: bool,

    /// Phase 78 (方案 B / M3): 外部旁路 Model Runtime JSON-RPC 2.0 端点 URL（留空仅管快照元数据，不内置训练/推理）
    #[serde(default)]
    pub fast_weight_runtime_url: String,

    /// Phase 78 (方案 B / M3): 快权重范数增量比防雪崩上限 (默认 1.50)
    #[serde(default = "default_fast_weight_max_norm_ratio")]
    pub fast_weight_max_norm_ratio: f32,

    /// Phase 78 (方案 B / M3): 通用基准回归允许下限 (默认 -0.03，即衰减 < 3%)
    #[serde(default = "default_fast_weight_min_glue_delta")]
    pub fast_weight_min_glue_delta: f32,

    /// Phase 78 (方案 B / M3): 定向遗忘墓碑诱导召回泄漏率上限 (默认 0.001，即 < 0.1%)
    #[serde(default = "default_fast_weight_max_unlearn_leak")]
    pub fast_weight_max_unlearn_leak: f32,

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

pub(super) fn default_memory_provider() -> String {
    "builtin".to_string()
}

pub(super) fn default_memory_pg_pool_size() -> usize {
    8
}

pub(super) fn default_memory_tenant_scope() -> String {
    "default".to_string()
}

pub(super) fn default_memory_vector_backend() -> String {
    "none".to_string()
}

pub(super) fn default_sleep_idle_cpu_pct() -> f32 {
    45.0
}

pub(super) fn default_sleep_min_events() -> usize {
    4
}

pub(super) fn default_sleep_max_batch() -> usize {
    64
}

pub(super) fn default_fast_weight_max_norm_ratio() -> f32 {
    1.50
}

pub(super) fn default_fast_weight_min_glue_delta() -> f32 {
    -0.03
}

pub(super) fn default_fast_weight_max_unlearn_leak() -> f32 {
    0.001
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            provider: default_memory_provider(),
            pg_url: None,
            pg_pool_size: default_memory_pg_pool_size(),
            pg_tenant_scope: default_memory_tenant_scope(),
            vector_backend: default_memory_vector_backend(),
            fallback_to_sqlite: true,
            sleep_compaction_enabled: true,
            sleep_idle_cpu_pct: default_sleep_idle_cpu_pct(),
            sleep_min_events: default_sleep_min_events(),
            sleep_max_batch: default_sleep_max_batch(),
            fast_weight_enabled: false,
            fast_weight_runtime_url: String::new(),
            fast_weight_max_norm_ratio: default_fast_weight_max_norm_ratio(),
            fast_weight_min_glue_delta: default_fast_weight_min_glue_delta(),
            fast_weight_max_unlearn_leak: default_fast_weight_max_unlearn_leak(),
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

impl MemoryConfig {
    /// 校验长期记忆引擎、空闲睡眠整理与 Phase 78 快权重门禁配置参数
    pub fn validate(&self, storage_pg_url: Option<&str>) -> anyhow::Result<()> {
        match self.provider.trim() {
            "builtin" | "pg_tx" | "powercontext" | "mem0" | "letta" => {}
            other => anyhow::bail!(
                "未知 memory.provider: '{}'，仅支持 'builtin' / 'pg_tx' / 'powercontext' / 'mem0' / 'letta'",
                other
            ),
        }
        if self.pg_pool_size == 0 || self.pg_pool_size > 64 {
            anyhow::bail!("memory.pg_pool_size ({}) 必须位于 1..=64 之间", self.pg_pool_size);
        }
        match self.vector_backend.trim().to_ascii_lowercase().as_str() {
            "none" | "pgvector" | "pg_vectorsearch" | "external_lsm" => {}
            other => anyhow::bail!(
                "未知 memory.vector_backend: '{}'，仅支持 'none' / 'pgvector' / 'pg_vectorsearch' / 'external_lsm'",
                other
            ),
        }
        if self.provider.trim() == "pg_tx"
            && !self.fallback_to_sqlite
            && self.pg_url.as_deref().map(str::trim).filter(|s| !s.is_empty()).is_none()
            && storage_pg_url.map(str::trim).filter(|s| !s.is_empty()).is_none()
        {
            anyhow::bail!(
                "当 memory.provider='pg_tx' 且关闭 fallback_to_sqlite 时，必须配置 memory.pg_url 或 storage.pg_url"
            );
        }
        if !(5.0..=95.0).contains(&self.sleep_idle_cpu_pct) {
            anyhow::bail!(
                "memory.sleep_idle_cpu_pct ({}) 必须位于 5.0..=95.0 之间",
                self.sleep_idle_cpu_pct
            );
        }
        if self.sleep_min_events < 2 {
            anyhow::bail!("memory.sleep_min_events ({}) 不能小于 2", self.sleep_min_events);
        }
        if self.sleep_max_batch < self.sleep_min_events {
            anyhow::bail!(
                "memory.sleep_max_batch ({}) 不能小于 memory.sleep_min_events ({})",
                self.sleep_max_batch,
                self.sleep_min_events
            );
        }
        if !self.fast_weight_max_norm_ratio.is_finite() || self.fast_weight_max_norm_ratio <= 1.0 {
            anyhow::bail!(
                "memory.fast_weight_max_norm_ratio ({}) 必须为大于 1.0 的有限浮点数",
                self.fast_weight_max_norm_ratio
            );
        }
        if !self.fast_weight_min_glue_delta.is_finite()
            || self.fast_weight_min_glue_delta < -0.5
            || self.fast_weight_min_glue_delta > 0.5
        {
            anyhow::bail!(
                "memory.fast_weight_min_glue_delta ({}) 必须位于 -0.5..=0.5 之间",
                self.fast_weight_min_glue_delta
            );
        }
        if !self.fast_weight_max_unlearn_leak.is_finite()
            || self.fast_weight_max_unlearn_leak <= 0.0
            || self.fast_weight_max_unlearn_leak > 0.1
        {
            anyhow::bail!(
                "memory.fast_weight_max_unlearn_leak ({}) 必须位于 (0.0, 0.1] 之间",
                self.fast_weight_max_unlearn_leak
            );
        }
        let fw_url = self.fast_weight_runtime_url.trim();
        if !fw_url.is_empty() && !(fw_url.starts_with("http://") || fw_url.starts_with("https://")) {
            anyhow::bail!(
                "memory.fast_weight_runtime_url ('{}') 必须以 http:// 或 https:// 开头",
                fw_url
            );
        }
        Ok(())
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

    /// 是否对图片资产启用 Ollama Vision 视觉模型增强理解 (ISSUE #42)
    #[serde(default = "default_annotation_vision_enabled")]
    pub vision_enabled: bool,

    /// 本地视觉理解模型名称 (推荐轻量多模态模型 qwen3.5:0.8b，与文本标注共享常驻模型)
    #[serde(default = "default_annotation_vision_model")]
    pub vision_model: String,

    /// 送入本地视觉模型的单张图片最大字节数（默认 4MB，防止大图 OOM/超时）
    #[serde(default = "default_annotation_vision_max_image_bytes")]
    pub vision_max_image_bytes: usize,

    /// 是否允许多模态元数据提取按有界超时调用宿主机只读工具 (pdfinfo/pdftotext/ffprobe)
    #[serde(default = "default_true_flag")]
    pub multimodal_tools_enabled: bool,

    /// 宿主机多模态只读工具探针超时上限 (ms，默认 500ms)
    #[serde(default = "default_multimodal_tool_timeout_ms")]
    pub multimodal_tool_timeout_ms: u64,
}

/// 三态运行模式 (ISSUE #43): 默认仅用本体 / 旁路直通 / 真实使用 A/B 对照
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMode {
    BodyOnly,
    Bypass,
    ShadowAb,
}

impl RuntimeMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BodyOnly => "body_only",
            Self::Bypass => "bypass",
            Self::ShadowAb => "shadow_ab",
        }
    }

    pub fn parse_str(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "body_only" | "body" | "default" => Some(Self::BodyOnly),
            "bypass" | "off" | "passthrough" => Some(Self::Bypass),
            "shadow_ab" | "shadow" | "ab" | "both" => Some(Self::ShadowAb),
            _ => None,
        }
    }
}

/// MCP 工具暴露档位：按场景裁剪 tools/list Schema，大幅压降每轮上下文工具税
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpToolProfile {
    Core,
    Dev,
    Ops,
    Full,
}

impl McpToolProfile {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Dev => "dev",
            Self::Ops => "ops",
            Self::Full => "full",
        }
    }

    pub fn parse_str(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "core" | "minimal" => Some(Self::Core),
            "dev" | "developer" => Some(Self::Dev),
            "ops" | "governance" => Some(Self::Ops),
            "full" | "all" => Some(Self::Full),
            _ => None,
        }
    }
}

/// 运行模式与真实 A/B 对照配置 (`[runtime]`, Phase 58 / ISSUE #43)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// 运行模式: "body_only" (默认) | "bypass" (旁路直通) | "shadow_ab" (实时搜索与本体双轨对照)
    #[serde(default = "default_runtime_mode")]
    pub mode: String,

    /// MCP tools/list 工具暴露档位: "full" (默认43工具) | "core" (10核心工具) | "dev" (18工具) | "ops" (20工具)
    #[serde(default = "default_mcp_profile")]
    pub mcp_profile: String,

    /// shadow_ab 模式下优先返回给 Agent 的结果侧: "live" (默认，真实对照基准) | "body"
    #[serde(default = "default_shadow_primary")]
    pub shadow_primary: String,

    /// shadow_ab / bypass 模式下实时工作区扫描的时间预算上限（毫秒，默认 800ms）
    #[serde(default = "default_shadow_max_live_ms")]
    pub shadow_max_live_ms: u64,

    /// shadow_ab 采样率 (0.0..=1.0，默认 1.0 全量采集)
    #[serde(default = "default_ab_sample_rate")]
    pub ab_sample_rate: f64,

    /// shadow_ab 检测到 H1 漏召回 / H2 幽灵陈旧命中时是否写穿自愈索引 (Phase 63)
    #[serde(default = "default_true_flag")]
    pub auto_heal_on_hindering: bool,
}

/// 行业 Know-How 知识网络与 Omni-RRF 多路融合检索配置 (`[knowledge_graph]`, Phase 61 / ISSUE #44)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraphConfig {
    /// 是否启用 L1 确定性知识网络与图扩展检索
    #[serde(default = "default_kg_enabled")]
    pub enabled: bool,

    /// Heading-Aware 章节语义切片的最大字符数
    #[serde(default = "default_kg_max_section_chars")]
    pub max_section_chars: usize,

    /// 跨章节 PMI 点互信息共现边阈值
    #[serde(default = "default_kg_pmi_threshold")]
    pub pmi_threshold: f64,

    /// 是否在 Omni-RRF 中启用纯 Rust Fuzzy 模糊查询通道
    #[serde(default = "default_kg_fuzzy_enabled")]
    pub fuzzy_enabled: bool,

    /// 层级结构通道基础权重
    #[serde(default = "default_kg_hierarchy_boost")]
    pub hierarchy_boost: f64,

    /// 文档间互链通道基础权重
    #[serde(default = "default_kg_doc_link_boost")]
    pub doc_link_boost: f64,

    /// 是否允许本地 Ollama 进行章节级原子 Know-How 因果抽取 (Phase 62)
    #[serde(default)]
    pub l2_extract_enabled: bool,

    /// 检索默认图扩展跳数 (0 | 1 | 2)
    #[serde(default = "default_kg_default_search_hops")]
    pub default_search_hops: usize,

    /// 是否启用数据库侧三元组模糊索引加速 (ISSUE #56: SQLite FTS5 trigram / PG pg_trgm)
    #[serde(default = "default_true_flag")]
    pub db_fuzzy_index: bool,

    /// 单文件入库索引正文 content_text 最大字节上限 (默认 128KB，大文件保留首尾)
    #[serde(default = "default_kg_max_index_content_bytes")]
    pub max_index_content_bytes: usize,

    /// 语义向量召回最低余弦相似度门禁 (默认 0.58，防止弱相似无关资产充当多余噪音)
    #[serde(default = "default_kg_min_vec_similarity")]
    pub min_vec_similarity: f32,

    /// 对严格代码标识符查询启用精准门禁 (已命中精确/模糊/FTS 时不再填充纯语义弱相关噪声)
    #[serde(default = "default_true_flag")]
    pub strict_identifier_vec_gate: bool,

    /// 自定义行业术语别名归一化映射
    #[serde(default)]
    pub aliases: std::collections::HashMap<String, String>,

    /// 是否启用 Relate 嵌入式 openCypher 图谱引擎 (contrib/graph_relate + librelate_ffi.so)
    #[serde(default)]
    pub relate_enabled: bool,

    /// Relate 图库落盘路径（默认 None -> {data_dir}/graph.rl）
    #[serde(default)]
    pub relate_db_path: Option<PathBuf>,

    /// 可选显式指定 `librelate_ffi.so` 路径
    #[serde(default)]
    pub relate_lib_path: Option<PathBuf>,

    /// Relate 单条 Cypher 语句超时毫秒数（默认 5000）
    #[serde(default = "default_kg_relate_timeout_ms")]
    pub relate_timeout_ms: u64,

    /// Relate 查询最大返回行数（默认 200）
    #[serde(default = "default_kg_relate_max_rows")]
    pub relate_max_rows: usize,

    /// 当 Relate 共享库不可用或未同步时是否自动无损回退 SQLite/PG（默认 true）
    #[serde(default = "default_true_flag")]
    pub relate_fallback_to_sqlite: bool,
}

pub(super) fn default_kg_enabled() -> bool {
    true
}

pub(super) fn default_kg_max_section_chars() -> usize {
    900
}

pub(super) fn default_kg_pmi_threshold() -> f64 {
    1.2
}

pub(super) fn default_kg_fuzzy_enabled() -> bool {
    true
}

pub(super) fn default_kg_hierarchy_boost() -> f64 {
    0.60
}

pub(super) fn default_kg_doc_link_boost() -> f64 {
    0.65
}

pub(super) fn default_kg_default_search_hops() -> usize {
    1
}

pub(super) fn default_kg_max_index_content_bytes() -> usize {
    131_072
}

pub(super) fn default_kg_min_vec_similarity() -> f32 {
    0.58
}

pub(super) fn default_kg_relate_timeout_ms() -> u64 {
    5000
}

pub(super) fn default_kg_relate_max_rows() -> usize {
    200
}

impl Default for KnowledgeGraphConfig {
    fn default() -> Self {
        Self {
            enabled: default_kg_enabled(),
            max_section_chars: default_kg_max_section_chars(),
            pmi_threshold: default_kg_pmi_threshold(),
            fuzzy_enabled: default_kg_fuzzy_enabled(),
            hierarchy_boost: default_kg_hierarchy_boost(),
            doc_link_boost: default_kg_doc_link_boost(),
            l2_extract_enabled: false,
            default_search_hops: default_kg_default_search_hops(),
            db_fuzzy_index: default_true_flag(),
            max_index_content_bytes: default_kg_max_index_content_bytes(),
            min_vec_similarity: default_kg_min_vec_similarity(),
            strict_identifier_vec_gate: default_true_flag(),
            aliases: std::collections::HashMap::new(),
            relate_enabled: false,
            relate_db_path: None,
            relate_lib_path: None,
            relate_timeout_ms: default_kg_relate_timeout_ms(),
            relate_max_rows: default_kg_relate_max_rows(),
            relate_fallback_to_sqlite: true,
        }
    }
}

pub(super) fn default_runtime_mode() -> String {
    "body_only".to_string()
}

pub(super) fn default_mcp_profile() -> String {
    "full".to_string()
}

pub(super) fn default_shadow_primary() -> String {
    "live".to_string()
}

pub(super) fn default_shadow_max_live_ms() -> u64 {
    800
}

pub(super) fn default_ab_sample_rate() -> f64 {
    1.0
}

pub(super) fn default_true_flag() -> bool {
    true
}

pub(super) fn default_multimodal_tool_timeout_ms() -> u64 {
    500
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            mode: default_runtime_mode(),
            mcp_profile: default_mcp_profile(),
            shadow_primary: default_shadow_primary(),
            shadow_max_live_ms: default_shadow_max_live_ms(),
            ab_sample_rate: default_ab_sample_rate(),
            auto_heal_on_hindering: default_true_flag(),
        }
    }
}

pub(super) fn default_annotation_enabled() -> bool {
    true
}

pub(super) fn default_annotation_model() -> String {
    "qwen3.5:0.8b".to_string()
}

pub(super) fn default_annotation_timeout_ms() -> u64 {
    15000
}

pub(super) fn default_annotation_max_snippet_chars() -> usize {
    1200
}

pub(super) fn default_auto_enrich_batch_size() -> usize {
    10
}

pub(super) fn default_annotation_vision_enabled() -> bool {
    false
}

pub(super) fn default_annotation_vision_model() -> String {
    "qwen3.5:0.8b".to_string()
}

pub(super) fn default_annotation_vision_max_image_bytes() -> usize {
    4 * 1024 * 1024
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
            vision_enabled: default_annotation_vision_enabled(),
            vision_model: default_annotation_vision_model(),
            vision_max_image_bytes: default_annotation_vision_max_image_bytes(),
            multimodal_tools_enabled: default_true_flag(),
            multimodal_tool_timeout_ms: default_multimodal_tool_timeout_ms(),
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

    /// 对等节点 Web 基地址列表，如 ["http://192.168.31.161:8765"]
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

pub(super) fn default_mesh_timeout_ms() -> u64 {
    1500
}

pub(super) fn default_mesh_replay_window_secs() -> u64 {
    300
}

pub(super) fn default_mesh_max_transfer_bytes() -> usize {
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

pub(super) fn default_web_enabled() -> bool {
    true
}

pub(super) fn default_web_host() -> String {
    "127.0.0.1".to_string()
}

pub(super) fn default_web_port() -> u16 {
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

pub(super) fn default_storage_provider() -> String {
    "sqlite".to_string()
}

pub(super) fn default_pg_url() -> Option<String> {
    None
}

pub(super) fn default_duckdb_path() -> Option<PathBuf> {
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
