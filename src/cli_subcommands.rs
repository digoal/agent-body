use std::path::PathBuf;
use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum MeshCommands {
    #[command(about = "查看本节点 mesh 配置与对等节点列表")]
    Peers,
    #[command(about = "采集并输出本节点事实（对等节点将收到的同款数据）")]
    Local,
    #[command(about = "探测所有对等节点连通性与延迟")]
    Ping,
    #[command(about = "拉取所有对等节点事实")]
    Facts,
    #[command(about = "拉取所有对等节点活跃协作锁")]
    Locks,
    #[command(about = "联邦端口查询：集群哪台机器占用了某端口")]
    Port {
        port: u16,
    },
    #[command(about = "向属主对等节点申请跨节点协作锁 (HTTP 租约)")]
    Acquire {
        #[arg(long, help = "目标对等节点基地址，如 http://192.168.31.161:8756")]
        peer: String,
        #[arg(short = 't', long)]
        resource_type: String,
        #[arg(short = 'i', long)]
        resource_id: String,
        #[arg(short, long, default_value = "exclusive")]
        scope: String,
        #[arg(short = 'a', long, default_value = "cli-agent")]
        holder_agent: String,
        #[arg(long, default_value_t = 300)]
        ttl_seconds: u64,
    },
    #[command(about = "跨节点图谱联邦探查")]
    Graph {
        #[arg(short, long, default_value = "")]
        query: String,
        #[arg(long, default_value_t = 2)]
        hops: usize,
    },
    #[command(about = "向对等节点发送 HMAC 签名结构化 Agent 对话")]
    DialogueSend {
        #[arg(long)]
        peer: String,
        #[arg(long, default_value = "")]
        thread_id: String,
        #[arg(long, default_value = "ASK_CONTEXT")]
        intent: String,
        #[arg(long)]
        content: String,
        #[arg(long, default_value = "cli-agent")]
        from_agent: String,
        #[arg(long, default_value = "")]
        to_agent: String,
    },
    #[command(about = "查看本机 Mesh 收到的跨机对话收件箱")]
    DialogueInbox {
        #[arg(short, long, default_value_t = 20)]
        limit: usize,
    },
    #[command(about = "确认已读一条 Mesh 对话")]
    DialogueAck {
        dialogue_id: String,
    },
    #[command(about = "向对等节点投递受控文件 (<=10MB, HMAC 签名 + 沙盒隔离)")]
    TransferSend {
        #[arg(long)]
        peer: String,
        #[arg(long)]
        file: String,
        #[arg(long, default_value = "cli-agent")]
        from_agent: String,
    },
    #[command(about = "查看本机 ~/.agent-body/mesh_inbox/ 隔离收件箱")]
    TransferInbox {
        #[arg(short, long, default_value_t = 20)]
        limit: usize,
    },
    #[command(about = "显式放行隔离收件箱中的文件到目标目录")]
    TransferAccept {
        transfer_id: String,
        #[arg(long)]
        dest_dir: String,
    },
    #[command(about = "拒绝并清除隔离收件箱中的文件")]
    TransferReject {
        transfer_id: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum PlatformCommands {
    #[command(about = "显示当前平台探针名称与 OS")]
    Info,
    #[command(about = "用当前平台探针枚举监听端口")]
    Ports,
    #[command(about = "用当前平台探针枚举自动化服务/计划任务")]
    Services,
}

#[derive(Subcommand)]
pub(crate) enum McpCatalogCommands {
    #[command(about = "扫描宿主机客户端 (Hermes/AGY/Claude/Cursor) MCP 配置并脱敏入库")]
    Scan,
    #[command(about = "列出已入库的 MCP Server 目录")]
    List,
    #[command(about = "按任务意图检索匹配 MCP")]
    Route {
        #[arg(help = "查询关键词或意图")]
        query: String,
        #[arg(short, long, default_value_t = 5)]
        limit: usize,
    },
}

#[derive(Subcommand)]
pub(crate) enum SkillCommands {
    #[command(about = "扫描宿主机全局与本地 SKILL.md 摘要目录")]
    Scan,
    #[command(about = "列出已索引的 SKILL 清单")]
    List,
    #[command(about = "按 Token 预算按需路由推荐 Top-K SKILL（防止 Context 被打爆）")]
    Route {
        #[arg(help = "任务意图")]
        intent: String,
        #[arg(short, long, default_value_t = 600)]
        budget_tokens: usize,
        #[arg(short, long, default_value_t = 3)]
        max_skills: usize,
    },
}

#[derive(Subcommand)]
pub(crate) enum ServiceCommands {
    #[command(about = "登记外部服务联系人（仅存环境变量名，严禁明文密钥）")]
    Register {
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "api")]
        category: String,
        #[arg(long)]
        endpoint: String,
        #[arg(long, default_value = "https")]
        protocol: String,
        #[arg(long, default_value = "")]
        capabilities: String,
        #[arg(long, default_value = "none")]
        auth_type: String,
        #[arg(long)]
        auth_env_var: Option<String>,
        #[arg(long)]
        health_endpoint: Option<String>,
        #[arg(long)]
        usage_example: Option<String>,
    },
    #[command(about = "列出外部服务通讯录")]
    List {
        #[arg(short, long)]
        category: Option<String>,
    },
    #[command(about = "按能力关键词检索外部服务")]
    Query {
        #[arg(help = "关键词")]
        query: String,
    },
    #[command(about = "对指定外部服务执行轻量实时探活")]
    Check {
        #[arg(help = "服务名称")]
        name: String,
    },
    #[command(about = "从 config.toml 自动推导并初始化外部服务名片")]
    Bootstrap,
}

#[derive(Subcommand)]
pub(crate) enum AnalyticsCommands {
    #[command(about = "统计近期各目录文件变更热力图")]
    Heatmap {
        #[arg(short, long, default_value_t = 30, help = "回溯天数")]
        days: usize,
    },
    #[command(about = "统计资产增长、记忆衰减与事实图谱趋势")]
    Trend {
        #[arg(short, long, default_value = "assets", help = "指标类型 (assets / memories / changes)")]
        metric: String,

        #[arg(short, long, default_value_t = 30, help = "回溯天数")]
        days: usize,
    },
    #[command(about = "将指定数据表导出为 Parquet/CSV 文件")]
    Export {
        #[arg(short, long, help = "数据表名 (assets / changes / memories / fact_nodes / fact_edges / links / active_processes)")]
        table: String,

        #[arg(short, long, help = "目标输出文件绝对路径")]
        output_path: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum HooksCommands {
    #[command(about = "自动生成指定 Agent 客户端的 Pre/Post Hook 集成配置")]
    Generate {
        #[arg(short, long, help = "目标客户端类型 (agy, cursor, claude, codex, hermes, git)")]
        client: String,

        #[arg(short, long, help = "写入目标根目录 (若未指定则输出到标准输出)")]
        out_dir: Option<PathBuf>,

        #[arg(long, help = "仅预览生成内容，不实际写入磁盘", default_value_t = false)]
        dry_run: bool,
    },
    #[command(about = "执行前置防撞检查门禁 (端口/文件/协作锁冲突)")]
    Pre {
        #[arg(short, long, help = "即将调用的工具名 (如 write_file, bind_port)")]
        tool_name: String,

        #[arg(short, long, default_value = "{}", help = "工具入参 JSON 字符串")]
        args: String,
    },
    #[command(about = "执行后置变更通知、自动事实同步与 AOPCC 碰壁自愈")]
    Post {
        #[arg(short, long, help = "刚执行完成的工具名")]
        tool_name: String,

        #[arg(short, long, default_value = "{}", help = "工具入参 JSON 字符串")]
        args: String,

        #[arg(
            long,
            default_value_t = true,
            num_args = 0..=1,
            default_missing_value = "true",
            action = clap::ArgAction::Set,
            help = "该工具调用是否成功 (兼容 --success 与 --success=false)"
        )]
        success: bool,

        #[arg(short = 'e', long, help = "可选失败错误输出（触发 AOPCC 秒级自愈处方）")]
        error_output: Option<String>,
    },
    #[command(about = "原生 Rust Hook 桥接器（替代 Python 桥接，读取 stdin JSON 并输出客户端协议）")]
    Bridge {
        #[arg(help = "Hook 事件名: session_start | pre_tool | post_tool")]
        event: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum CoordinationCommands {
    #[command(about = "原子申请资源协作互斥/共享锁 (端口/目录/服务/文件/自定义)")]
    Acquire {
        #[arg(short = 't', long, help = "资源类型: port, directory, service, file, custom")]
        resource_type: String,

        #[arg(short = 'i', long, help = "资源标识 (例如 8080, /path/to/workspace)")]
        resource_id: String,

        #[arg(short, long, default_value = "exclusive", help = "锁模式: exclusive 或 shared")]
        scope: String,

        #[arg(short = 'a', long, default_value = "cli-agent", help = "持有者 Agent 标识")]
        holder_agent: String,

        #[arg(long, help = "持有者进程 PID (可选)")]
        holder_pid: Option<u32>,

        #[arg(long, default_value_t = 300, help = "锁有效期秒数 (5~3600)")]
        ttl_seconds: u64,
    },
    #[command(about = "列出当前所有生效中的活跃资源协作锁")]
    List {
        #[arg(short, long, help = "按资源类别过滤: port, directory, service, file, custom")]
        resource_type: Option<String>,
    },
    #[command(about = "手动释放指定资源锁 (支持 --lock-id 或 -t/-i 资源匹配，--force=false 校验持有者)")]
    Release {
        #[arg(long, help = "锁的唯一标识 lock_id（可与 -t/-i 二选一）")]
        lock_id: Option<String>,

        #[arg(short = 't', long, help = "按资源类型匹配释放: port, directory, service, file, custom")]
        resource_type: Option<String>,

        #[arg(short = 'i', long, help = "按资源标识匹配释放")]
        resource_id: Option<String>,

        #[arg(short = 'a', long, help = "持有者 Agent 标识（当 --force=false 时用于权限校验）")]
        holder_agent: Option<String>,

        #[arg(long, help = "是否强制释放", default_value_t = true, action = clap::ArgAction::Set, num_args = 0..=1, default_missing_value = "true")]
        force: bool,
    },
    #[command(about = "手动回收所有已过期的死锁")]
    Gc,
    #[command(about = "排队等待并获取资源协作锁（含死锁环路预检）")]
    Wait {
        #[arg(short = 't', long)]
        resource_type: String,
        #[arg(short = 'i', long)]
        resource_id: String,
        #[arg(short, long, default_value = "exclusive")]
        scope: String,
        #[arg(short = 'a', long, default_value = "cli-agent")]
        holder_agent: String,
        #[arg(long, default_value_t = 300)]
        ttl_seconds: u64,
        #[arg(long, default_value_t = 5000)]
        wait_timeout_ms: u64,
        #[arg(long, default_value_t = 100)]
        priority: i32,
    },
    #[command(about = "执行 Wait-For Graph (WFG) 全局死锁环路与等待队列检测")]
    Deadlock,
}

#[derive(Subcommand)]
pub(crate) enum StorageCommands {
    #[command(about = "在不同的存储后端之间执行全量数据热迁移")]
    Migrate {
        #[arg(long, help = "源存储类型 (sqlite / postgres / duckdb)", default_value = "sqlite")]
        from: String,

        #[arg(long, help = "源端连接串或文件路径 (默认使用当前配置)")]
        from_url: Option<String>,

        #[arg(long, help = "目标存储类型 (sqlite / postgres / duckdb)", default_value = "postgres")]
        to: String,

        #[arg(long, help = "目标端连接串或文件路径 (默认 postgresql://postgres@127.0.0.1:5432/agent_body)")]
        to_url: Option<String>,
    },
    #[command(about = "执行主存储到分析副本 (DuckDB) 的增量或全量数据同步")]
    Sync {
        #[arg(long, help = "目标分析引擎类型 (duckdb)", default_value = "duckdb")]
        to: String,

        #[arg(long, help = "目标端数据库文件路径 (默认 ~/.agent-body/body.duckdb)")]
        to_path: Option<String>,
    },
    #[command(about = "查看当前生效的存储后端技术栈与配置")]
    Info,
}

#[derive(Subcommand)]
pub(crate) enum LinksCommands {
    #[command(about = "查询指定实体的关联拓扑（含 L1/L2/L3 属性图谱边）")]
    Query {
        #[arg(short, long, help = "实体类型: asset, service, project, tool, memory", default_value = "asset")]
        r#type: String,

        #[arg(help = "实体唯一标识或路径")]
        id: String,

        #[arg(long, default_value_t = 2, help = "属性图谱最大跳数")]
        max_hops: usize,

        #[arg(long, default_value_t = 0.0, help = "最低边权重过滤 (0.0~1.0)")]
        min_weight: f32,

        #[arg(long, default_value_t = true, action = clap::ArgAction::Set, help = "是否包含 L3_SEMANTIC 隐式语义边")]
        include_semantic: bool,
    },

    #[command(about = "导出当前关联依赖的 Mermaid 架构图")]
    Graph {
        #[arg(short, long, default_value_t = 20)]
        limit: usize,

        #[arg(long, help = "导出前先触发 L2 结构上卷与 L3 语义挖掘")]
        discover_semantic: bool,
    },

    #[command(about = "分析指定资产变更或删除的爆炸半径与加权上下游依赖")]
    Impact {
        #[arg(help = "被评估资产绝对路径、项目或服务标识")]
        target: String,

        #[arg(short, long, default_value_t = 3, help = "依赖多跳推演最大深度")]
        max_depth: usize,

        #[arg(long, default_value_t = 0.50, help = "最低有效权重阈值 (0.0~1.0)")]
        min_weight: f32,

        #[arg(long, help = "是否包含 L3_SEMANTIC 隐式语义边")]
        include_semantic: bool,
    },

    #[command(about = "主动审计依赖拓扑中的断链、悬空文件与离线服务")]
    Audit {
        #[arg(long, help = "是否自动清理已确认失效的死链")]
        fix: bool,
    },

    #[command(about = "智能推演断链修复建议并可一键应用补丁")]
    Repair {
        #[arg(long, help = "是否直接应用修复修改源文件与关联表")]
        apply: bool,
    },

    #[command(about = "跨层联动 CodeGraph 微观代码图")]
    Codegraph {
        #[arg(help = "目标项目工程路径或代码文件")]
        path: String,

        #[arg(help = "待查询的微观符号/函数/类名称 (可选)")]
        symbol: Option<String>,
    },
}

#[derive(Subcommand)]
pub(crate) enum PlaceCommands {
    #[command(about = "获取新对象规范存放位置建议")]
    Suggest {
        #[arg(short, long, help = "对象类别 (project/script/skill/note/tmp/src)")]
        r#type: String,

        #[arg(short, long, help = "对象名称")]
        name: String,

        #[arg(short, long, help = "对象用途与预期生命周期")]
        intent: String,

        #[arg(short, long, help = "保密级别 (public/internal/confidential/secret)", default_value = "internal")]
        security: String,
    },

    #[command(about = "列出当前生效的宿主机目录放置规范")]
    Rules,

    #[command(about = "从历史资产分布中自动挖掘自学习放置规则候选")]
    Learn {
        #[arg(short, long, default_value_t = 3, help = "聚类最小样本数")]
        min_samples: usize,

        #[arg(long, help = "自动采纳并入库所有发现的规则候选")]
        accept: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum AssetCommands {
    #[command(about = "执行白黑名单分级扫描（含 L1 确定性元数据提取，可选 --enrich 触发本地 Ollama 语义标注）")]
    Scan {
        #[arg(help = "目标目录路径")]
        path: PathBuf,

        #[arg(long, help = "调用本地 Ollama 小模型进行 L2 语义标注精炼")]
        enrich: bool,

        #[arg(long, default_value_t = 10, help = "Ollama 单次最多精炼资产数")]
        enrich_limit: usize,

        #[arg(long, help = "扫描后同步构建 L1 行业 Know-How 知识网络（章节切片 + 别名归一 + PMI 共现图）")]
        mine_knowledge: bool,

        #[arg(long, help = "调用本地 Ollama 小模型 (think=false) 抽取章节级 L2 Know-How 因果与边界边（带原文锚定防幻觉）")]
        extract_knowhow: bool,

        #[arg(long, help = "幂等升级存量 other 误判或旧版 schema 资产的多模态元数据")]
        backfill_multimodal: bool,
    },

    #[command(about = "全文与语义搜索资产（支持多模态、领域与 KV 过滤及 Omni-RRF 多路融合检索）")]
    Search {
        #[arg(help = "搜索关键词（传空字符串时可纯按过滤器检索）")]
        query: String,

        #[arg(short, long, default_value_t = 10)]
        limit: usize,

        #[arg(long, default_value = "standard", help = "披露粒度: micro | standard | full")]
        detail: String,

        #[arg(long, help = "按多模态类型过滤: image | audio | video | pdf | office | dataset | code | document | config")]
        modality: Option<String>,

        #[arg(long, help = "按资产类别过滤: script | document | dataset | config | media | other")]
        asset_type: Option<String>,

        #[arg(long, help = "按业务领域过滤 (如 web_dashboard, storage_engine)")]
        domain: Option<String>,

        #[arg(long, help = "要求 desc_kv 中存在指定键 (如 table_names, columns, dimensions)")]
        kv_key: Option<String>,

        #[arg(long, help = "要求 desc_kv（或 --kv-key 对应值）匹配子串")]
        kv_value: Option<String>,

        #[arg(long, help = "多谓词结构化过滤表达式 (如 \"importance_score>=0.7,modality=document;has:doc_title\")")]
        kv_filter: Option<String>,

        #[arg(long, help = "禁用 Ollama 向量 RRF 混合检索，仅使用确定性 Omni-RRF 与结构化过滤")]
        no_hybrid: bool,

        #[arg(long, default_value_t = 1, help = "知识网络扩散跳数 (0 | 1 | 2)")]
        graph_hops: usize,

        #[arg(long, help = "禁用纯 Rust 模糊容错通道")]
        no_fuzzy: bool,

        #[arg(long, help = "禁用 links 文档互链扩展通道")]
        no_links: bool,
    },

    #[command(about = "手动向Agent 本体登记一个文件或目录资产")]
    Register {
        #[arg(short, long, help = "文件或目录绝对路径")]
        path: String,

        #[arg(short = 't', long, default_value = "other", help = "资产类型 (directory/project/script/document/dataset/config/media/other)")]
        r#type: String,

        #[arg(short, long, help = "归属主体 (缺省使用 [placement].default_owner 配置)")]
        owner: Option<String>,

        #[arg(short, long, default_value = "internal", help = "安全分级 (public/internal/confidential/secret)")]
        security_level: String,

        #[arg(short, long, help = "文字描述")]
        description: Option<String>,

        #[arg(long, default_value = "{}", help = "结构化键值 JSON 字符串")]
        desc_kv: String,
    },

    #[command(about = "列出最近索引的资产")]
    List {
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },

    #[command(about = "断点续扫状态机任务管控 (create, run, pause, list, status)")]
    Job {
        #[arg(short, long, default_value = "list", help = "操作动作: create, run, pause, list, status")]
        action: String,

        #[arg(help = "目标目录路径或 Job ID")]
        target: Option<String>,

        #[arg(long, help = "单次推进步数限制 (用于调试)")]
        limit: Option<usize>,
    },
}

#[derive(Subcommand)]
pub(crate) enum EnvCommands {
    #[command(about = "查看操作系统与硬件规格")]
    Show,

    #[command(about = "查看活跃动态任务与普通进程 (过滤系统底噪)")]
    Processes,

    #[command(about = "端口防撞预警与空闲端口推荐")]
    Port {
        #[arg(help = "拟占用的网络端口号")]
        port: u16,

        #[arg(short, long, default_value_t = 3, help = "推荐备用空闲端口个数")]
        suggest_count: usize,
    },
}

#[derive(Subcommand)]
pub(crate) enum MemoryCommands {
    #[command(about = "写入一条新记忆（支持 pg_tx 双时态 fact_key 与 expected_epoch）")]
    Add {
        #[arg(short, long, help = "记忆类型 (decision/task/experience/fact/note)", default_value = "decision")]
        r#type: String,

        #[arg(short, long, help = "记忆正文内容")]
        content: String,

        #[arg(short, long, help = "一句话摘要")]
        summary: Option<String>,

        #[arg(long, help = "可选逻辑主键（用于双时态版本链与写穿更新）")]
        fact_key: Option<String>,

        #[arg(long, default_value = "L3_SEMANTIC", help = "证据分级 (L1_EXPLICIT/L2_STRUCTURAL/L3_SEMANTIC)")]
        evidence_tier: String,

        #[arg(long, help = "可选 CAS 期望逻辑纪元 (tx_epoch)")]
        expected_epoch: Option<i64>,

        #[arg(long, help = "可选项目/租户隔离域")]
        tenant_scope: Option<String>,
    },

    #[command(about = "搜索记忆 (支持 FTS5/向量 RRF 与 pg_tx 双时态快照回溯)")]
    Search {
        #[arg(help = "搜索关键词或语义描述")]
        query: String,

        #[arg(short, long, default_value_t = 10)]
        limit: usize,

        #[arg(long, default_value_t = true, action = clap::ArgAction::Set, num_args = 0..=1, default_missing_value = "true", help = "是否启用 Ollama 向量混合 RRF 检索")]
        hybrid: bool,

        #[arg(long, help = "可选逻辑纪元时间旅行 (tx_epoch <= as_of_epoch)")]
        as_of_epoch: Option<i64>,

        #[arg(long, default_value_t = false, help = "是否包含历史已关闭版本")]
        include_history: bool,

        #[arg(long, help = "可选项目/租户隔离域")]
        tenant_scope: Option<String>,
    },

    #[command(about = "全面体检长期记忆健康度 (含冲突/衰减检测、PG 事务健康态与事实墓碑擦除)")]
    Audit {
        #[arg(long, help = "若存在冲突，指定归档淘汰的旧记忆 ID")]
        archive_id: Option<String>,

        #[arg(long, default_value_t = false, help = "查看 PostgreSQL 事务级记忆健康状态与当前 tx_epoch")]
        tx_status: bool,

        #[arg(long, help = "将指定 fact_key 墓碑化（0% 僵尸召回）")]
        tombstone_fact: Option<String>,
    },
}
