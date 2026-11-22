use std::path::PathBuf;
use clap::{Parser, Subcommand};
use anyhow::Result;
use agent_steward::config::Config;
use agent_steward::storage::Storage;
use agent_steward::placement::{PlacementEngine, PlacementRequest};
use agent_steward::scanner::Scanner;
use agent_steward::env::EnvCollector;
use agent_steward::memory::MemoryManager;
use agent_steward::brief::{BriefAssembler, BriefRequest};

#[derive(Parser)]
#[command(name = "steward", version = agent_steward::VERSION, about = "Agent Host Steward — 脑体分离 Agent 本体（宿主机具身底座 · Rust 版）")]
struct Cli {
    #[arg(short, long, help = "自定义配置文件路径")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "查看 Agent 本体运行状态与六器官统计")]
    Status,

    #[command(about = "启动交互式终端全景实时看板")]
    Tui {
        #[arg(short, long, default_value_t = 2, help = "看板刷新间隔秒数")]
        interval: u64,
    },

    #[command(about = "一键获取 Agent 本体开工简报与宿主机上下文 (默认走 BriefAssembler 三位一体组装)")]
    Brief {
        #[arg(short, long, default_value = "", help = "任务查询意图关键词")]
        query: String,

        #[arg(short, long, default_value = "", help = "会话 ID (可选，用于召回本会话关键决策)")]
        session_id: String,

        #[arg(short = 'b', long, default_value_t = 1500, help = "Token 预算上限")]
        budget_tokens: usize,

        #[arg(long, default_value_t = false, help = "使用兼容旧版极简动态扫描输出")]
        legacy: bool,
    },

    #[command(about = "新对象放置与目录位置建议")]
    Place {
        #[command(subcommand)]
        action: PlaceCommands,
    },

    #[command(about = "资产分级纳管与检索")]
    Asset {
        #[command(subcommand)]
        action: AssetCommands,
    },

    #[command(about = "宿主系统环境与动态进程")]
    Env {
        #[command(subcommand)]
        action: EnvCommands,
    },

    #[command(about = "跨会话长期记忆管理")]
    Memory {
        #[command(subcommand)]
        action: MemoryCommands,
    },

    #[command(about = "启动 MCP (Model Context Protocol) stdio 服务端供 Agent 接入")]
    Mcp,

    #[command(about = "启动 Agent 本体常驻后台守护进程 (含 inotify 监听、定时进程巡检与稳态自愈)")]
    Daemon,

    #[command(about = "实体依赖关联与拓扑图")]
    Links {
        #[command(subcommand)]
        action: LinksCommands,
    },

    #[command(about = "宿主环境变更历史与开工回溯")]
    Changes {
        #[arg(short, long, help = "查询自该时间戳以来的变更 (ISO 8601, 如 2026-11-15T00:00:00)")]
        since: Option<String>,

        #[arg(short, long, default_value_t = 20)]
        limit: usize,
    },

    #[command(about = "会话完工结清与环境治理 (清理临时测试资产并归档会话纪要)")]
    Close {
        #[arg(help = "当前会话的唯一标识 (Session ID)")]
        session_id: String,

        #[arg(long, default_value_t = true, help = "是否清理 /root/new/tmp/ 下属于本会话的临时文件")]
        clean_temp: bool,

        #[arg(long, default_value_t = false, help = "仅预览待清理临时文件与归档摘要，不物理删除")]
        dry_run: bool,
    },

    #[command(about = "宿主机孤儿进程与临时垃圾资源安全扫描与回收")]
    Gc {
        #[arg(long, default_value = "/root/new/tmp", help = "待清理的临时目录路径 (强制安全白名单)")]
        tmp_dir: PathBuf,

        #[arg(long, default_value_t = 72, help = "文件超期阈值 (小时)")]
        max_age_hours: u64,

        #[arg(long, help = "是否直接执行清理删除 (默认仅 dry-run 预览)")]
        apply: bool,
    },

    #[command(about = "宿主机客观事实图谱查询 (工具能力/端口/Systemd/Cron/源码资料)")]
    Fact {
        #[arg(help = "查询意图或关键词 (如 '处理 PDF 提取文字', '8080', 'all')")]
        query: Option<String>,

        #[arg(short, long, default_value = "auto", help = "事实类型 (capability, port, automation, impact, repo, doc, all, auto)")]
        fact_type: String,
    },

    #[command(about = "跨存储后端管理与热迁移 (SQLite / PostgreSQL / DuckDB)")]
    Storage {
        #[command(subcommand)]
        action: StorageCommands,
    },

    #[command(about = "多 Agent 协作互斥锁管理与冲突排查")]
    Coordination {
        #[command(subcommand)]
        action: CoordinationCommands,
    },

    #[command(about = "宿主运行环境与依赖完整性预检 (Preflight Check)")]
    Preflight {
        #[arg(short, long, help = "输出原始 JSON 格式以便 CI/CD 或 Agent 机器解析")]
        json: bool,
    },

    #[command(about = "启动基于浏览器的轻量级大管家拓扑图与协作锁监控大屏")]
    Web {
        #[arg(long, help = "监听地址 (默认 127.0.0.1)")]
        host: Option<String>,

        #[arg(short, long, help = "监听端口 (默认 8765)")]
        port: Option<u16>,

        #[arg(long, help = "管理员操作鉴权 Token")]
        auth_token: Option<String>,
    },

    #[command(about = "HOOK 驱动集成与各 Agent 客户端自动化钩子生成")]
    Hooks {
        #[command(subcommand)]
        action: HooksCommands,
    },

    #[command(about = "基于 DuckDB 列存分析引擎的宿主机时序统计与数据导出")]
    Analytics {
        #[command(subcommand)]
        action: AnalyticsCommands,
    },

    #[command(about = "宿主机已配置 MCP Server 目录扫描、列表与路由")]
    McpCatalog {
        #[command(subcommand)]
        action: McpCatalogCommands,
    },

    #[command(about = "百级 SKILL 目录扫描与 Context 防爆按需路由")]
    Skill {
        #[command(subcommand)]
        action: SkillCommands,
    },

    #[command(about = "宿主机外部可用服务通讯录与轻量健康探活")]
    Service {
        #[command(subcommand)]
        action: ServiceCommands,
    },

    #[command(about = "轻量多机联邦 Peer Mesh：对等节点 ping/facts/locks/端口/跨节点锁/图谱/对话/沙盒投递")]
    Mesh {
        #[command(subcommand)]
        action: MeshCommands,
    },

    #[command(about = "跨平台原生探针：查看当前平台探针识别的监听端口与自动化服务")]
    Platform {
        #[command(subcommand)]
        action: PlatformCommands,
    },

    #[command(about = "路演场景演示与黑白屏同步自检引擎 (list/run/roi/parity/reset)")]
    Demo {
        #[command(subcommand)]
        action: DemoCommands,
    },

    #[command(about = "实证 Benchmark 对照评测与生产效能存证账本 (workloads/run/report/ledger/compare/export/reset)")]
    Bench {
        #[command(subcommand)]
        action: BenchCommands,
    },

    #[command(about = "实际使用反馈总账 (user_feedback.md) 查看、追加与沉淀至长期记忆")]
    Feedback {
        #[command(subcommand)]
        action: FeedbackCommands,
    },

    #[command(about = "双域全链路行为审计 (IN_AGENT + OUT_OF_AGENT)、DuckDB OLAP 分析与审计驱动自进化")]
    Audit {
        #[command(subcommand)]
        action: AuditCommands,
    },

    #[command(about = "Agent 本体中枢 (BodyManifest 六器官自省、五维稳态生命体征、四级自治包络与跨大脑连续性接力)")]
    Body {
        #[command(subcommand)]
        action: BodyCommands,
    },
}

#[derive(Subcommand)]
enum BodyCommands {
    #[command(about = "查看本体身份证 (body_id) 与六大器官自省清单")]
    Manifest {
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(about = "评估五维本体稳态生命体征评分")]
    Vitals {
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(about = "查询四级自治权限包络矩阵或评估单个动作权限")]
    Envelope {
        #[arg(long)]
        action: Option<String>,
        #[arg(long)]
        target: Option<String>,
        #[arg(long, default_value_t = true)]
        dry_run: bool,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(about = "保存跨大脑连续性接力胶囊 (<=300 tok, 强制脱敏防投毒)")]
    Handoff {
        #[arg(long)]
        goal: String,
        #[arg(long, default_value = "cli-brain")]
        from_brain: String,
        #[arg(long, default_value = "cli-live")]
        session_id: String,
        #[arg(long)]
        files: Option<String>,
        #[arg(long)]
        facts: Option<String>,
        #[arg(long)]
        blockers: Option<String>,
        #[arg(long, default_value = "")]
        next_step: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(about = "恢复最近激活的跨大脑接力胶囊")]
    Resume {
        #[arg(long, default_value = "cli-brain")]
        by_brain: String,
        #[arg(long)]
        checkpoint_id: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(about = "在 L1_AUTONOMIC 包络内触发可逆稳态反射自愈")]
    Heal {
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(about = "探测跨机多本体联邦名片 (/api/v1/mesh/body_manifest)")]
    Peers {
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum AuditCommands {
    #[command(about = "查询双域行为审计明细")]
    Query {
        #[arg(long)]
        domain: Option<String>,
        #[arg(long)]
        actor_kind: Option<String>,
        #[arg(long)]
        category: Option<String>,
        #[arg(long)]
        outcome: Option<String>,
        #[arg(long)]
        trace_id: Option<String>,
        #[arg(long)]
        target: Option<String>,
        #[arg(long, default_value = "7d")]
        window: String,
        #[arg(long, default_value_t = 30)]
        limit: usize,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(about = "查看双域行为审计汇总与 DuckDB OLAP 热榜")]
    Summary {
        #[arg(long, default_value = "7d")]
        window: String,
        #[arg(long, default_value_t = 7)]
        days: usize,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(about = "按 trace_id 回溯因果审计链路")]
    Trace {
        trace_id: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(about = "运行审计驱动自进化周期 (AOPCC 租约校准 + 摩擦热点沉淀 + 目录规则聚类)")]
    Evolve {
        #[arg(long, default_value_t = false)]
        auto_feedback: bool,
        #[arg(long, default_value_t = 3)]
        min_hits: usize,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(about = "导出 audit_events 审计账本至 Parquet/CSV (安全白名单路径)")]
    Export {
        #[arg(long, default_value = "/root/new/tmp/audit_events_export.csv")]
        output_path: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum FeedbackCommands {
    #[command(about = "列出 user_feedback.md 中的反馈条目")]
    List,
    #[command(about = "追加一条新反馈到 user_feedback.md")]
    Add {
        #[arg(long, default_value = "CON")]
        category: String,
        #[arg(long, default_value = "P1")]
        priority: String,
        #[arg(long)]
        title: String,
        #[arg(long, default_value = "")]
        evidence: String,
        #[arg(long, default_value = "")]
        target: String,
    },
    #[command(about = "将已验证反馈 (✅ Verified) 沉淀入长期记忆库")]
    Sync,
    #[command(about = "执行全生命周期信息新鲜度对账（资产写穿失效/幽灵节点剪枝/记忆去重）")]
    Reconcile {
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(about = "运行 Agent 执行回路五点对比自进化探针 (P1~P5)")]
    Probes {
        #[arg(long, default_value_t = false)]
        auto_heal: bool,
        #[arg(long, default_value_t = false)]
        auto_feedback: bool,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum BenchCommands {
    #[command(about = "列出 15 大标准 A/B 负载与 PR-01~11 推导规则")]
    Workloads,
    #[command(about = "运行宿主机真机 A/B 对照基准 (full/macro/micro/dq/competitors/W1~W15)")]
    Run {
        #[arg(long, default_value = "full")]
        suite: String,
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        save: bool,
    },
    #[command(about = "查看实证基准报告与 SHA-256 存证指纹")]
    Report {
        #[arg(long)]
        run_id: Option<String>,
        #[arg(long, default_value = "table")]
        format: String,
    },
    #[command(about = "审计生产运行态效能存证账本")]
    Ledger {
        #[arg(long, default_value = "7d")]
        window: String,
        #[arg(long)]
        tool: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    #[command(about = "对比两次基准评测批次的回归差异")]
    Compare {
        run_a: String,
        run_b: String,
    },
    #[command(about = "导出带 SHA-256 签名的评测报告")]
    Export {
        #[arg(long, default_value = "markdown")]
        format: String,
        #[arg(long)]
        out: Option<String>,
    },
    #[command(about = "清理 bench:* 隔离测试数据")]
    Reset,
    #[command(about = "运行四大赛道热门竞品横向实证对标 (C1记忆 / C2上下文DB / C3 AgentFS / C4宿主探针)")]
    Competitors {
        #[arg(long, default_value = "all")]
        track: String,
        #[arg(long, default_value = "markdown")]
        format: String,
    },
}

#[derive(Subcommand)]
enum DemoCommands {
    #[command(about = "列出 6 大黄金路演演示剧目")]
    List,
    #[command(about = "用真实宿主机引擎执行指定路演剧目 (scene-1 ~ scene-6)")]
    Run {
        #[arg(help = "剧目 ID，如 scene-1")]
        scenario_id: String,
    },
    #[command(about = "核算宿主机实时商业价值 ROI 指标")]
    Roi,
    #[command(about = "输出黑白屏 41 工具对齐自检列表")]
    Parity,
    #[command(about = "一键清理 demo:* 沙盒隔离锁与等待队列")]
    Reset,
}

#[derive(Subcommand)]
enum MeshCommands {
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
    #[command(about = "查看本机 ~/.steward/mesh_inbox/ 隔离收件箱")]
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
enum PlatformCommands {
    #[command(about = "显示当前平台探针名称与 OS")]
    Info,
    #[command(about = "用当前平台探针枚举监听端口")]
    Ports,
    #[command(about = "用当前平台探针枚举自动化服务/计划任务")]
    Services,
}

#[derive(Subcommand)]
enum McpCatalogCommands {
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
enum SkillCommands {
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
enum ServiceCommands {
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
enum AnalyticsCommands {
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
enum HooksCommands {
    #[command(about = "自动生成指定 Agent 客户端的 Pre/Post Hook 集成配置")]
    Generate {
        #[arg(short, long, help = "目标客户端类型 (agy, cursor, claude, git)")]
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
}

#[derive(Subcommand)]
enum CoordinationCommands {
    #[command(about = "原子申请资源协作互斥/共享锁 (端口/目录/服务/文件/自定义)")]
    Acquire {
        #[arg(short = 't', long, help = "资源类型: port, directory, service, file, custom")]
        resource_type: String,

        #[arg(short = 'i', long, help = "资源标识 (例如 8080, /root/new/work)")]
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
    #[command(about = "手动释放指定资源锁 (管理员运维操作)")]
    Release {
        #[arg(long, help = "锁的唯一标识 lock_id")]
        lock_id: String,

        #[arg(long, help = "是否强制释放", default_value_t = true)]
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
enum StorageCommands {
    #[command(about = "在不同的存储后端之间执行全量数据热迁移")]
    Migrate {
        #[arg(long, help = "源存储类型 (sqlite / postgres / duckdb)", default_value = "sqlite")]
        from: String,

        #[arg(long, help = "源端连接串或文件路径 (默认使用当前配置)")]
        from_url: Option<String>,

        #[arg(long, help = "目标存储类型 (sqlite / postgres / duckdb)", default_value = "postgres")]
        to: String,

        #[arg(long, help = "目标端连接串或文件路径 (默认 postgresql://postgres@127.0.0.1:5432/agent_steward)")]
        to_url: Option<String>,
    },
    #[command(about = "执行主存储到分析副本 (DuckDB) 的增量或全量数据同步")]
    Sync {
        #[arg(long, help = "目标分析引擎类型 (duckdb)", default_value = "duckdb")]
        to: String,

        #[arg(long, help = "目标端数据库文件路径 (默认 ~/.steward/steward.duckdb)")]
        to_path: Option<String>,
    },
    #[command(about = "查看当前生效的存储后端技术栈与配置")]
    Info,
}

#[derive(Subcommand)]
enum LinksCommands {
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
enum PlaceCommands {
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
enum AssetCommands {
    #[command(about = "执行白黑名单分级扫描（含 L1 确定性元数据提取，可选 --enrich 触发本地 Ollama 语义标注）")]
    Scan {
        #[arg(help = "目标目录路径")]
        path: PathBuf,

        #[arg(long, help = "调用本地 Ollama 小模型进行 L2 语义标注精炼")]
        enrich: bool,

        #[arg(long, default_value_t = 10, help = "Ollama 单次最多精炼资产数")]
        enrich_limit: usize,
    },

    #[command(about = "全文与语义搜索资产")]
    Search {
        #[arg(help = "搜索关键词")]
        query: String,

        #[arg(short, long, default_value_t = 10)]
        limit: usize,

        #[arg(long, default_value = "standard", help = "披露粒度: micro | standard | full")]
        detail: String,
    },

    #[command(about = "手动向大管家登记一个文件或目录资产")]
    Register {
        #[arg(short, long, help = "文件或目录绝对路径")]
        path: String,

        #[arg(short = 't', long, default_value = "other", help = "资产类型 (directory/project/script/document/dataset/config/media/other)")]
        r#type: String,

        #[arg(short, long, default_value = "user:digoal", help = "归属主体")]
        owner: String,

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
enum EnvCommands {
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
enum MemoryCommands {
    #[command(about = "写入一条新记忆")]
    Add {
        #[arg(short, long, help = "记忆类型 (decision/task/experience/fact/note)", default_value = "decision")]
        r#type: String,

        #[arg(short, long, help = "记忆正文内容")]
        content: String,

        #[arg(short, long, help = "一句话摘要")]
        summary: Option<String>,
    },

    #[command(about = "搜索记忆 (支持 FTS5 与 Ollama 向量混合 RRF 检索)")]
    Search {
        #[arg(help = "搜索关键词或语义描述")]
        query: String,

        #[arg(short, long, default_value_t = 10)]
        limit: usize,

        #[arg(long, default_value_t = true, help = "是否启用 Ollama 向量混合 RRF 检索")]
        hybrid: bool,
    },

    #[command(about = "全面体检长期记忆健康度 (检测认知冲突矛盾与衰减淘汰候选)")]
    Audit {
        #[arg(long, help = "若存在冲突，指定归档淘汰的旧记忆 ID")]
        archive_id: Option<String>,
    },
}

fn main() -> Result<()> {
    #[cfg(unix)]
    unsafe {
        extern "C" {
            fn signal(sig: i32, handler: usize) -> usize;
        }
        // 恢复 SIGPIPE 默认行为，避免输出被管道下游（如 | head）提前关闭时触发 Broken pipe panic
        signal(13, 0);
    }

    let cli = Cli::parse();
    let cfg = Config::load_or_default(cli.config.as_deref())?;
    let storage = Storage::from_config(&cfg)?;
    let audit_storage = storage.clone();
    let cli_t0 = std::time::Instant::now();
    let cli_cmd_name = cli_command_label(&cli.command);
    let skip_cli_audit = matches!(
        cli.command,
        Commands::Mcp
            | Commands::Daemon
            | Commands::Web { .. }
            | Commands::Tui { .. }
            | Commands::Bench { .. }
            | Commands::Demo { .. }
    );

    let exec_res: Result<()> = (|| -> Result<()> {
        match cli.command {
        Commands::Status => {
            println!("╔═══════════════════════════════════════════════════════╗");
            println!("║      Agent Host Steward (Rust 版) — 系统状态          ║");
            println!("╚═══════════════════════════════════════════════════════╝");
            println!("版本:           {}", agent_steward::version_info());
            println!("数据目录:       {:?}", cfg.data_dir);
            println!("数据库文件:     {:?}", cfg.db_path);
            let assets = storage.list_assets(100_000)?;
            let links = storage.list_all_links(100_000)?;
            let rules = storage.get_placement_rules()?;
            let policies = storage.get_scan_policies()?;
            let procs = storage.list_active_processes()?;
            println!("已纳管资产数:   {}", assets.len());
            println!("依赖关系边数:   {} 条", links.len());
            println!("放置规范规则:   {} 条", rules.len());
            println!("分级扫描策略:   {} 条", policies.len());
            println!("当前动态进程:   {} 个", procs.len());
        }

        Commands::Tui { interval } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let arc_storage = std::sync::Arc::new(storage);
                let dashboard = agent_steward::tui::TuiDashboard::new(arc_storage);
                dashboard.run_loop(interval, None).await
            })?;
        }

        Commands::Brief { query, session_id, budget_tokens, legacy } => {
            let t0 = std::time::Instant::now();
            let out = if legacy {
                let env_collector = EnvCollector::new(storage.clone());
                let summary = env_collector.collect_system_summary();
                let procs = env_collector.collect_and_sync_processes()?;
                let assets = storage.list_assets(5)?;

                let mut buf = String::new();
                buf.push_str("=== Steward Brief 上下文简报 ===\n");
                buf.push_str(&format!("系统: {} | 内核: {} | 内存使用率: {:.1}%\n", summary.os_name, summary.kernel_version, summary.memory_used_pct));
                buf.push_str("活跃动态任务进程 (前 5):\n");
                for p in procs.iter().take(5) {
                    let mem_mb = p.mem_bytes as f64 / (1024.0 * 1024.0);
                    buf.push_str(&format!("  - [PID {}] {} (CPU {:.1}%, 内存 {:.1}MB) -> {}\n", p.pid, p.name, p.cpu_pct, mem_mb, p.command));
                }
                buf.push_str("最近活跃资产:\n");
                for a in assets {
                    buf.push_str(&format!("  - [{}] {} ({})\n", a.asset_type, a.path, a.scan_tier));
                }
                buf
            } else {
                let provider = std::sync::Arc::new(
                    agent_steward::memory::builtin_provider::BuiltinMemoryProvider::new(storage.clone()),
                );
                let assembler = BriefAssembler::new(storage.clone(), Some(provider));
                let output = assembler.assemble(&BriefRequest {
                    query,
                    session_id,
                    budget_tokens,
                })?;
                output.markdown
            };
            print!("{}", out);
            agent_steward::benchmark::ledger::EffectivenessLedger::record_channel_call(
                storage.provider().as_ref(),
                "cli",
                "steward_brief",
                &out,
                t0.elapsed().as_micros() as i64,
            );
        }

        Commands::Place { action } => {
            let engine = PlacementEngine::new(storage.clone());
            match action {
                PlaceCommands::Suggest { r#type, name, intent, security } => {
                    let req = PlacementRequest {
                        object_type: r#type,
                        name,
                        intent,
                        tags: vec![],
                        security_level: security,
                    };
                    let suggestion = engine.suggest(&req)?;
                    println!("╔═══════════════════════════════════════════════════════╗");
                    println!("║             Steward 对象放置与路径建议                ║");
                    println!("╚═══════════════════════════════════════════════════════╝");
                    println!("推荐目标路径:   {:?}", suggestion.suggested_path);
                    println!("命中规范规则:   {}", suggestion.rule_matched);
                    println!("规则理由:       {}", suggestion.reason);
                    println!("命名建议:       {}", suggestion.naming_convention);
                    println!("冲突状态:       {}", if suggestion.conflict_detected { "⚠️ 路径已存在冲突！" } else { "✅ 路径空闲可用" });
                    println!("伴随开发指引:");
                    for g in suggestion.guidelines {
                        println!("  * {}", g);
                    }
                }
                PlaceCommands::Rules => {
                    let rules = storage.get_placement_rules()?;
                    println!("当前生效的宿主机目录放置规范 (共 {} 条):", rules.len());
                    for r in rules {
                        println!("  - [{}] {} -> {} (优先级 {})", r.rule_name, r.pattern_type, r.target_base_dir, r.priority);
                        if let Some(desc) = r.description {
                            println!("    说明: {}", desc);
                        }
                    }
                }
                PlaceCommands::Learn { min_samples, accept } => {
                    let learner = agent_steward::placement::learner::PlacementLearner::new(&storage);
                    let rep = learner.learn_rules(min_samples)?;
                    if rep.candidates.is_empty() {
                        println!("未发现新的聚类放置模式（样本数低于 {} 或已被既有规则完全覆盖）。", min_samples);
                    } else {
                        println!("🔍 分析了 {} 个资产，发现 {} 个潜在的放置规则候选:", rep.total_analyzed_assets, rep.candidate_count);
                        for c in &rep.candidates {
                            println!("\n候选规则: {} ({})", c.rule_name, c.candidate_id);
                            println!("建议基准目录: {}", c.target_base_dir);
                            println!("推荐命名模式: {}", c.naming_pattern);
                            println!("样本数: {}, 置信度: {:.2}", c.sample_count, c.confidence);
                            println!("说明: {}", c.description);
                            println!("样本示例:");
                            for s in &c.sample_paths {
                                println!("  - {}", s);
                            }
                            if accept {
                                let rule = learner.apply_candidate(c)?;
                                println!("  ✅ 已自动采纳并持久化新规则: {}", rule.rule_name);
                            }
                        }
                        if !accept {
                            println!("\n💡 提示: 添加 --accept 参数可一键将上述候选规则持久化到 placement_rules 表中。");
                        }
                    }
                }
            }
        }

        Commands::Asset { action } => {
            match action {
                AssetCommands::Scan { path, enrich, enrich_limit } => {
                    println!("正在执行分级颗粒度扫描: {:?}", path);
                    let scanner = Scanner::new(storage.clone())?;
                    let report = scanner.scan_path(&path)?;
                    println!("扫描完成报告:");
                    println!("  - 扫描目录数:       {}", report.scanned_dirs);
                    println!("  - 深层索引文件数:   {} (已完成 L1 确定性元数据提取)", report.deep_indexed_files);
                    println!("  - 黑名单阻断目录数: {}", report.shallow_blocked_dirs.len());
                    for b in report.shallow_blocked_dirs {
                        println!("    ★ [Shallow Tier 阻断] {} ({})", b.path, b.reason);
                    }
                    if enrich {
                        let prefix = path.to_string_lossy().to_string();
                        println!("\n🤖 正在调用本地 Ollama 小模型进行 L2 语义标注精炼 (limit={})...", enrich_limit);
                        let annotator = agent_steward::scanner::OllamaAnnotator::with_default_config(storage);
                        let er = annotator.enrich_pending_assets(enrich_limit, Some(&prefix))?;
                        println!("  - 模型: {} | 在线: {} | 候选: {} | 成功: {} | 跳过: {} | 失败: {} | 耗时: {}ms",
                            er.model_used, er.ollama_available, er.total_candidates, er.enriched_count, er.skipped_count, er.failed_count, er.elapsed_ms);
                        for s in er.samples {
                            println!("    ✅ [{}] {} -> {} (入口: {})", s.domain, s.path, s.purpose, s.entry_point);
                        }
                        let emb_cnt = annotator.embed_pending_assets(enrich_limit, Some(&prefix)).unwrap_or(0);
                        println!("  - Ollama Embedding 向量化完成: {} 个核心资产已生成向量", emb_cnt);
                    }
                }
                AssetCommands::Search { query, limit, detail } => {
                    let t0 = std::time::Instant::now();
                    let level = agent_steward::scanner::DetailLevel::from_str_opt(Some(&detail));
                    let results = storage.search_assets_fts(&query, limit)?;
                    let mut out = format!("资产搜索匹配结果 (共 {} 条, detail={:?}):\n", results.len(), level);
                    for a in results {
                        out.push_str(&agent_steward::scanner::format_asset_line(&a, level));
                        out.push('\n');
                    }
                    print!("{}", out);
                    agent_steward::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "steward_asset_search",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                }
                AssetCommands::Register { path, r#type, owner, security_level, description, desc_kv } => {
                    let parsed_kv: serde_json::Value = serde_json::from_str(&desc_kv)
                        .unwrap_or_else(|_| serde_json::json!({}));
                    let record = agent_steward::storage::models::AssetRecord {
                        id: uuid::Uuid::new_v4().to_string(),
                        path: path.clone(),
                        asset_type: r#type.clone(),
                        owner,
                        security_level,
                        scan_tier: "deep".to_string(),
                        description,
                        desc_kv: parsed_kv,
                        usage: None,
                        size_bytes: None,
                        file_count: None,
                        tags: vec![],
                        status: "active".to_string(),
                    };
                    storage.upsert_asset(&record)?;
                    println!("✅ 资产登记成功: {} [{}]", path, r#type);
                }
                AssetCommands::List { limit } => {
                    let results = storage.list_assets(limit)?;
                    println!("最近资产清单 (前 {} 条):", results.len());
                    for a in results {
                        println!("  - [{:8}] {:6} | {}", a.asset_type, a.scan_tier, a.path);
                    }
                }
                AssetCommands::Job { action, target, limit } => {
                    let sm = agent_steward::scanner::ScanStateMachine::new(storage.clone())?;
                    match action.as_str() {
                        "create" => {
                            let p = target.ok_or_else(|| anyhow::anyhow!("缺少目标扫描路径"))?;
                            let job = sm.create_job(std::path::Path::new(&p))?;
                            println!("✅ 成功创建断点续扫任务:");
                            println!("  - Job ID:    {}", job.id);
                            println!("  - 根路径:    {}", job.root_path);
                            println!("  - 当前状态:  {}", job.status);
                        }
                        "run" | "resume" => {
                            let jid = target.ok_or_else(|| anyhow::anyhow!("缺少 Job ID"))?;
                            println!("正在启动或恢复断点续扫任务: {}", jid);
                            let job = sm.run_or_resume(&jid, limit)?;
                            println!("任务执行阶段完成:");
                            println!("  - 状态:       {}", job.status);
                            println!("  - 最新断点:   {:?}", job.last_path);
                            println!("  - 扫描目录数: {}", job.scanned_dirs);
                            println!("  - 扫描文件数: {}", job.scanned_files);
                            println!("  - 录入资产数: {}", job.total_assets);
                        }
                        "pause" => {
                            let jid = target.ok_or_else(|| anyhow::anyhow!("缺少 Job ID"))?;
                            let job = sm.pause_job(&jid)?;
                            println!("任务已暂停: {} (状态: {})", job.id, job.status);
                        }
                        "list" => {
                            let jobs = sm.list_jobs(20)?;
                            println!("最近断点续扫任务记录 (共 {} 条):", jobs.len());
                            for j in jobs {
                                println!("  - [{:9}] {} | 根路径: {} | 资产数: {}", j.status, j.id, j.root_path, j.total_assets);
                            }
                        }
                        "status" => {
                            let jid = target.ok_or_else(|| anyhow::anyhow!("缺少 Job ID"))?;
                            if let Some(j) = sm.get_job(&jid)? {
                                println!("断点任务状态详情:");
                                println!("  - Job ID:      {}", j.id);
                                println!("  - 状态:        {}", j.status);
                                println!("  - 根路径:      {}", j.root_path);
                                println!("  - 最新断点:    {:?}", j.last_path);
                                println!("  - 扫描文件数:  {}", j.scanned_files);
                                println!("  - 总资产数:    {}", j.total_assets);
                                println!("  - 创建时间:    {:?}", j.created_at);
                                println!("  - 完成时间:    {:?}", j.completed_at);
                            } else {
                                println!("未找到 Job ID: {}", jid);
                            }
                        }
                        _ => {
                            println!("未知动作: {} (支持: create, run, pause, list, status)", action);
                        }
                    }
                }
            }
        }

        Commands::Env { action } => {
            let collector = EnvCollector::new(storage);
            match action {
                EnvCommands::Show => {
                    let summary = collector.collect_system_summary();
                    println!("=== 系统与硬件规格 ===");
                    println!("OS:       {}", summary.os_name);
                    println!("Kernel:   {}", summary.kernel_version);
                    println!("Host:     {}", summary.host_name);
                    println!("CPU:      {} 核心 ({})", summary.cpu_cores, summary.cpu_brand);
                    println!("Memory:   {} MB 总量 (已用: {} MB, {:.1}%)", summary.total_memory_mb, summary.used_memory_mb, summary.memory_used_pct);
                    println!("Disks:");
                    for d in summary.disks {
                        println!("  - {:<10} 总计 {:.1} GB / 可用 {:.1} GB", d.mount_point, d.total_space_gb, d.available_space_gb);
                    }
                }
                EnvCommands::Processes => {
                    let procs = collector.collect_and_sync_processes()?;
                    println!("活跃动态任务进程 (共 {} 个):", procs.len());
                    println!("{:<8} {:<10} {:<6} {:<10} {:<18} {}", "PID", "NAME", "CPU%", "MEM(MB)", "OWNER", "COMMAND");
                    for p in procs {
                        let mem_mb = p.mem_bytes as f64 / (1024.0 * 1024.0);
                        let cmd_trunc = if p.command.chars().count() > 60 {
                    let t: String = p.command.chars().take(57).collect();
                    format!("{}...", t)
                } else {
                    p.command
                };
                        println!("{:<8} {:<10} {:<6.1} {:<10.1} {:<18} {}", p.pid, p.name, p.cpu_pct, mem_mb, p.owner, cmd_trunc);
                    }
                }
                EnvCommands::Port { port, suggest_count } => {
                    let res = agent_steward::env::port::PortChecker::check_port(port, suggest_count)?;
                    println!("{}", serde_json::to_string_pretty(&res)?);
                }
            }
        }

        Commands::Memory { action } => {
            match action {
                MemoryCommands::Add { r#type, content, summary } => {
                    let mm = MemoryManager::new(storage);
                    let id = mm.add(&r#type, &content, summary.as_deref(), Some("cli"), vec![])?;
                    println!("✅ 记忆已保存, ID: {}", id);
                }
                MemoryCommands::Search { query, limit, hybrid } => {
                    let t0 = std::time::Instant::now();
                    let provider_ref = storage.provider().clone();
                    let mut out = String::new();
                    if hybrid {
                        let arc_storage = std::sync::Arc::new(storage);
                        let results = tokio::runtime::Runtime::new()?.block_on(async {
                            let engine = agent_steward::memory::hybrid::HybridSearchEngine::new(arc_storage);
                            engine.search_hybrid(&query, limit).await
                        })?;
                        out.push_str(&format!("记忆搜索结果 (混合模式 RRF, 共 {} 条):\n", results.len()));
                        for r in results {
                            let sim_str = r.cosine_similarity.map(|s| format!(" (语义相似度: {:.2})", s)).unwrap_or_default();
                            out.push_str(&format!("  - [{}] {} [RRF: {:.4}]{} (ID: {})\n",
                                r.memory.memory_type, r.memory.content, r.rrf_score, sim_str, r.memory.id));
                        }
                    } else {
                        let mm = MemoryManager::new(storage);
                        let results = mm.search(&query, limit)?;
                        out.push_str(&format!("记忆搜索结果 (FTS 纯文本, 共 {} 条):\n", results.len()));
                        for m in results {
                            out.push_str(&format!("  - [{}] {} (ID: {})\n", m.memory_type, m.content, m.id));
                        }
                    }
                    print!("{}", out);
                    agent_steward::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        provider_ref.as_ref(),
                        "cli",
                        "steward_memory_search",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                }
                MemoryCommands::Audit { archive_id } => {
                    let mgr = agent_steward::memory::lifecycle::MemoryLifecycleManager::new(&storage);
                    if let Some(aid) = archive_id {
                        mgr.archive_memory(&aid)?;
                        println!("✅ 已将记忆条目 '{}' 状态归档为 'outdated'。", aid);
                    }
                    let rep = mgr.audit_memories()?;
                    println!("长期记忆体检报告 (总记录: {}, 活跃: {}):", rep.total_memories, rep.active_memories);
                    println!("  - 发现对立认知矛盾: {} 处", rep.conflict_count);
                    println!("  - 发现低热度衰减候选: {} 处", rep.decayed_count);
                    if !rep.conflicts.is_empty() {
                        println!("\n⚠️ 冲突矛盾对详情:");
                        for c in &rep.conflicts {
                            println!("\n  冲突原因: {}", c.conflict_reason);
                            println!("  [条目 A ({})]: {}", c.memory_id_a, c.content_a);
                            println!("  [条目 B ({})]: {}", c.memory_id_b, c.content_b);
                            println!("  仲裁建议: {}", c.suggested_resolution);
                        }
                        println!("\n💡 提示: 可通过 'steward memory audit --archive-id <ID>' 归档旧记录。");
                    }
                }
            }
        }

        Commands::Mcp => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let server = agent_steward::api::mcp::McpServer::new(storage);
                server.run_stdio().await
            })?;
        }

        Commands::Daemon => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let daemon = agent_steward::daemon::StewardDaemon::new(cfg, storage);
                daemon.run().await
            })?;
        }

        Commands::Links { action } => {
            let arc_storage = std::sync::Arc::new(storage);
            let engine = agent_steward::links::LinksEngine::new(arc_storage.clone());
            match action {
                LinksCommands::Query { r#type, id, max_hops, min_weight, include_semantic } => {
                    let links = engine.query_links(&r#type, &id)?;
                    println!("文件级关联拓扑 (共 {} 条):", links.len());
                    for l in &links {
                        println!("  - [{}] {} --({})--> [{}] {} (置信度: {:.2})",
                            l.source_type, l.source_id, l.relation, l.target_type, l.target_id, l.confidence);
                    }
                    let tiers: Vec<&str> = if include_semantic {
                        vec!["L1_EXPLICIT", "L2_STRUCTURAL", "L3_SEMANTIC"]
                    } else {
                        vec!["L1_EXPLICIT", "L2_STRUCTURAL"]
                    };
                    let root_cand = if id.contains(':') { id.clone() } else { format!("{}:{}", r#type, id) };
                    if let Ok(sub) = arc_storage.traverse_fact_graph_filtered(&root_cand, max_hops, min_weight, Some(&tiers)) {
                        if !sub.edges.is_empty() {
                            println!("\n属性图谱分层关系 ({} 跳, 共 {} 条边):", max_hops, sub.edges.len());
                            for e in &sub.edges {
                                println!("  - [{} w={:.2} {}] {} --({})--> {}",
                                    e.evidence_tier, e.weight, e.direction_kind, e.source_id, e.relation, e.target_id);
                            }
                        }
                    }
                }
                LinksCommands::Graph { limit, discover_semantic } => {
                    if discover_semantic {
                        let miner = agent_steward::fact::GraphRelationshipMiner::new(arc_storage.as_ref());
                        let rep = miner.mine_all(true)?;
                        println!("# 关系挖掘完成: L2上卷={}, 服务桥接={}, 工具={}, 衍生={}, L3语义={}",
                            rep.l2_rollup_edges, rep.l2_service_bridge_edges, rep.l2_tool_usage_edges, rep.l2_derivation_edges, rep.l3_semantic_edges);
                    }
                    let mermaid = engine.export_mermaid(limit)?;
                    println!("{}", mermaid);
                }
                LinksCommands::Impact { target, max_depth, min_weight, include_semantic } => {
                    let analyzer = agent_steward::links::impact::ImpactAnalyzer::new(arc_storage);
                    let res = analyzer.analyze_impact_weighted(&target, max_depth, min_weight, include_semantic)?;
                    println!("{}", serde_json::to_string_pretty(&res)?);
                }
                LinksCommands::Audit { fix } => {
                    let t0 = std::time::Instant::now();
                    let auditor = agent_steward::links::audit::LinkAuditor::new(arc_storage.clone());
                    let rep = auditor.audit_links(fix)?;
                    println!("{}", rep.summary);
                    if !rep.issues.is_empty() {
                        println!("\n发现以下异常断链 (共 {} 处):", rep.issues.len());
                        for iss in &rep.issues {
                            println!("  - [{}] {} -> {} (行号: {:?})", iss.severity, iss.source, iss.target, iss.line_number);
                            println!("    详情: {}", iss.description);
                            if let Some(snip) = &iss.snippet {
                                println!("    代码: {}", snip);
                            }
                            println!("    建议: {}", iss.suggested_fix);
                        }
                    }
                    agent_steward::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        arc_storage.provider().as_ref(),
                        "cli",
                        "steward_links_audit",
                        &rep.summary,
                        t0.elapsed().as_micros() as i64,
                    );
                }
                LinksCommands::Repair { apply } => {
                    let repairer = agent_steward::links::repair::LinkRepairer::new(arc_storage);
                    let rep = repairer.repair_links(apply)?;
                    if rep.suggestions.is_empty() {
                        println!("✅ 未发现可修复的断链或拓扑完全健康。");
                    } else {
                        println!("🔍 发现 {} 处缺失目标，找到 {} 处可自动修复候选 (已应用: {}):",
                            rep.total_missing, rep.reparable_count, rep.applied_count);
                        for sug in &rep.suggestions {
                            println!("\n源文件: {} (行号: {:?})", sug.source_file, sug.line_number);
                            println!("失效目标: {}", sug.missing_target);
                            println!("推荐重定向: {} (相似度: {:.2}, 置信度: {})", sug.candidate_target, sug.similarity, sug.confidence);
                            println!("补丁预览:\n{}", sug.patch_diff);
                        }
                        if !apply && rep.reparable_count > 0 {
                            println!("\n💡 提示: 添加 --apply 参数可自动应用补丁并更新关联表。");
                        }
                    }
                }
                LinksCommands::Codegraph { path, symbol } => {
                    let p = std::path::Path::new(&path);
                    if let Some(sym) = symbol {
                        let callers = agent_steward::links::codegraph_bridge::CodeGraphBridge::query_symbol_callers(p, &sym)?;
                        println!("CodeGraph 符号调用方分析: '{}' (共 {} 条):", sym, callers.len());
                        for c in callers {
                            println!("  - [{}] {} ({}:{:?})", c.kind, c.qualified_name, c.file_path, c.line);
                        }
                    } else {
                        let info = agent_steward::links::codegraph_bridge::CodeGraphBridge::inspect_project(p)?;
                        match info {
                            Some(stats) => {
                                println!("CodeGraph 工程概览:");
                                println!("  根目录: {}", stats.root_dir);
                                println!("  数据库: {}", stats.db_path);
                                println!("  节点数: {} (Nodes)", stats.node_count);
                                println!("  调用边: {} (Edges)", stats.edge_count);
                                println!("  文件数: {} (Files)", stats.file_count);
                            }
                            None => println!("未在 '{}' 及其父目录中检测到 .codegraph/ 索引。", path),
                        }
                    }
                }
            }
        }

        Commands::Changes { since, limit } => {
            let arc_storage = std::sync::Arc::new(storage);
            let engine = agent_steward::changes::ChangesEngine::new(arc_storage);
            let summary = engine.generate_summary_markdown(since.as_deref(), limit)?;
            println!("{}", summary);
        }

        Commands::Close { session_id, clean_temp, dry_run } => {
            let arc_storage = std::sync::Arc::new(storage);
            let sm = agent_steward::session::SessionManager::new(arc_storage);
            let res = sm.close_session_with_dry_run(&session_id, clean_temp, dry_run)?;
            println!("{}", serde_json::to_string_pretty(&res)?);
        }

        Commands::Gc { tmp_dir, max_age_hours, apply } => {
            let dry_run = !apply;
            println!("正在扫描宿主垃圾资源 (目录: {:?}, 超期阈值: {} 小时, 模式: {})...",
                tmp_dir, max_age_hours, if dry_run { "预览模式 (dry-run)" } else { "执行清理 (apply)" });
            let rep = agent_steward::env::gc::HostGarbageCollector::run_gc_audited(&storage, &tmp_dir, max_age_hours, dry_run)?;
            println!("\n🧹 临时文件清理结果:");
            println!("  - 扫描临时目录:   {}", rep.tmp_dir_scanned);
            println!("  - 发现超期候选:   {} 个文件 ({:.2} MB)", rep.candidate_files_count, rep.candidate_bytes_reclaimed as f64 / (1024.0 * 1024.0));
            println!("  - 实际删除释放:   {} 个文件 ({:.2} MB)", rep.deleted_files_count, rep.deleted_bytes_reclaimed as f64 / (1024.0 * 1024.0));
            if dry_run && rep.candidate_files_count > 0 {
                println!("  💡 提示: 添加 --apply 参数可真正物理删除上述超期文件。");
            }

            println!("\n👻 孤儿/失控 Agent Worker 进程诊断 (共 {} 个):", rep.orphan_processes.len());
            if rep.orphan_processes.is_empty() {
                println!("  ✅ 未发现孤儿僵尸进程，系统进程状态健康。");
            } else {
                for proc_ in &rep.orphan_processes {
                    println!("  - [PID {}] {} (内存: {} MB, 运行: {} 秒)", proc_.pid, proc_.name, proc_.memory_mb, proc_.running_secs);
                    println!("    命令: {}", proc_.command);
                    println!("    诊断: {}", proc_.reason);
                }
            }
        }

        Commands::Fact { query, fact_type } => {
            let t0 = std::time::Instant::now();
            let q = query.unwrap_or_else(|| "all".to_string());
            let report = agent_steward::fact::FactFacade::query_facts(&fact_type, &q)?;
            println!("{}", report);
            agent_steward::benchmark::ledger::EffectivenessLedger::record_channel_call(
                storage.provider().as_ref(),
                "cli",
                "steward_fact_query",
                &report,
                t0.elapsed().as_micros() as i64,
            );
        }

        Commands::Storage { action } => {
            match action {
                StorageCommands::Info => {
                    println!("当前活跃存储提供者: {}", storage.provider_name());
                    println!("配置数据库文件路径: {:?}", cfg.db_path);
                    if let Some(ref pg) = cfg.storage.pg_url {
                        println!("配置 PostgreSQL URL: {}", pg);
                    }
                }
                StorageCommands::Migrate { from, from_url, to, to_url } => {
                    println!("🚀 正在准备执行跨引擎数据迁移: {} -> {} ...", from, to);
                    let src_storage = match from.to_lowercase().as_str() {
                        "postgres" | "pg" => {
                            let url = from_url.unwrap_or_else(|| "postgresql://postgres@127.0.0.1:5432/agent_steward".to_string());
                            let p = agent_steward::storage::PgStorageProvider::connect(&url)?;
                            Storage::new(std::sync::Arc::new(p))
                        }
                        "duckdb" => {
                            let path = from_url.map(PathBuf::from).unwrap_or_else(|| cfg.data_dir.join("steward.duckdb"));
                            let p = agent_steward::storage::DuckDbStorageProvider::open(&path)?;
                            Storage::new(std::sync::Arc::new(p))
                        }
                        _ => {
                            let path = from_url.map(PathBuf::from).unwrap_or_else(|| cfg.db_path.clone());
                            Storage::open(&path)?
                        }
                    };

                    let dst_storage = match to.to_lowercase().as_str() {
                        "postgres" | "pg" => {
                            let url = to_url.unwrap_or_else(|| "postgresql://postgres@127.0.0.1:5432/agent_steward".to_string());
                            let p = agent_steward::storage::PgStorageProvider::connect(&url)?;
                            Storage::new(std::sync::Arc::new(p))
                        }
                        "duckdb" => {
                            let path = to_url.map(PathBuf::from).unwrap_or_else(|| cfg.data_dir.join("steward.duckdb"));
                            let p = agent_steward::storage::DuckDbStorageProvider::open(&path)?;
                            Storage::new(std::sync::Arc::new(p))
                        }
                        _ => {
                            let path = to_url.map(PathBuf::from).unwrap_or_else(|| cfg.db_path.clone());
                            Storage::open(&path)?
                        }
                    };

                    let report = agent_steward::storage::StorageMigrator::migrate(&src_storage, &dst_storage)?;
                    println!("✅ 数据热迁移完成！耗时: {} ms", report.duration_ms);
                    println!("  - 放置规则迁移: {} 条", report.placement_rules_migrated);
                    println!("  - 扫描策略迁移: {} 条", report.scan_policies_migrated);
                    println!("  - 纳管资产迁移: {} 个", report.assets_migrated);
                    println!("  - 活跃进程迁移: {} 个", report.active_processes_migrated);
                    println!("  - 记忆条目迁移: {} 条", report.memories_migrated);
                    println!("  - 变更记录迁移: {} 条", report.changes_migrated);
                    println!("  - 依赖关系迁移: {} 条", report.links_migrated);
                    println!("  - 扫描作业迁移: {} 个", report.scan_jobs_migrated);
                    println!("  - 属性图节点迁移: {} 个", report.fact_nodes_migrated);
                    println!("  - 属性图边迁移:   {} 条", report.fact_edges_migrated);
                }
                StorageCommands::Sync { to, to_path } => {
                    println!("🔄 正在执行主存储 -> 分析副本 ({}) 同步 ...", to);
                    let target_path = to_path.map(PathBuf::from).unwrap_or_else(|| cfg.data_dir.join("steward.duckdb"));
                    let duckdb_prov = agent_steward::storage::DuckDbStorageProvider::open(&target_path)?;
                    let pipeline = agent_steward::storage::SyncPipeline::new(storage.provider().clone(), std::sync::Arc::new(duckdb_prov));
                    let rep = pipeline.sync_all()?;
                    println!("✅ 分析副本同步完成！耗时: {} ms", rep.duration_ms);
                    println!("  - 纳管资产同步:   {} 个", rep.assets_synced);
                    println!("  - 变更记录同步:   {} 条", rep.changes_synced);
                    println!("  - 长期记忆同步:   {} 条", rep.memories_synced);
                    println!("  - 属性图节点同步: {} 个", rep.fact_nodes_synced);
                    println!("  - 属性图边同步:   {} 条", rep.fact_edges_synced);
                }
            }
        }

        Commands::Coordination { action } => {
            let engine = agent_steward::coordination::CoordinationEngine::new(storage.provider().clone());
            match action {
                CoordinationCommands::Acquire { resource_type, resource_id, scope, holder_agent, holder_pid, ttl_seconds } => {
                    let req = agent_steward::coordination::LockRequest {
                        resource_type: agent_steward::coordination::ResourceType::from_str_loose(&resource_type),
                        resource_id,
                        scope: agent_steward::coordination::LockScope::from_str_loose(&scope),
                        holder_agent,
                        holder_pid,
                        ttl_seconds,
                        metadata: None,
                    };
                    let res = engine.try_acquire(&req)?;
                    println!("{}", serde_json::to_string_pretty(&res)?);
                }
                CoordinationCommands::List { resource_type } => {
                    let rt_opt = resource_type.as_deref().map(agent_steward::coordination::ResourceType::from_str_loose);
                    let locks = engine.list_active_locks(rt_opt.as_ref())?;
                    println!("╔══════════════════════════════════════════════════════════════════════════════════════════╗");
                    println!("║                        多 Agent 协作锁全景监控看板                                       ║");
                    println!("╚══════════════════════════════════════════════════════════════════════════════════════════╝");
                    if locks.is_empty() {
                        println!("当前没有任何活跃的协作锁，所有受控资源均处于可用/空闲状态。");
                    } else {
                        println!("{:<36} {:<10} {:<24} {:<12} {:<10} {:<24}", "LOCK_ID", "TYPE", "RESOURCE_ID", "HOLDER", "SCOPE", "EXPIRES_AT");
                        println!("{:-<120}", "");
                        for l in locks {
                            println!("{:<36} {:<10} {:<24} {:<12} {:<10} {:<24}",
                                l.lock_id,
                                l.resource_type,
                                l.resource_id,
                                l.holder_agent,
                                l.scope,
                                l.expires_at,
                            );
                        }
                    }
                }
                CoordinationCommands::Release { lock_id, force } => {
                    if force {
                        let ok = engine.force_release(&lock_id)?;
                        if ok {
                            println!("✅ 锁 `{}` 已成功强制解除！", lock_id);
                        } else {
                            println!("⚠️ 未找到指定锁 ID `{}` 或该锁已被自动释放。", lock_id);
                        }
                    }
                }
                CoordinationCommands::Gc => {
                    let count = engine.gc_expired_locks()?;
                    println!("🧹 协作锁 GC 完成！已清理释放 {} 个过期锁。", count);
                }
                CoordinationCommands::Wait { resource_type, resource_id, scope, holder_agent, ttl_seconds, wait_timeout_ms, priority } => {
                    let req = agent_steward::coordination::LockRequest {
                        resource_type: agent_steward::coordination::ResourceType::from_str_loose(&resource_type),
                        resource_id,
                        scope: agent_steward::coordination::LockScope::from_str_loose(&scope),
                        holder_agent,
                        holder_pid: None,
                        ttl_seconds,
                        metadata: None,
                    };
                    let res = engine.wait_and_acquire(&req, wait_timeout_ms, priority)?;
                    println!("{}", serde_json::to_string_pretty(&res)?);
                }
                CoordinationCommands::Deadlock => {
                    let rep = engine.detect_deadlocks()?;
                    let waits = engine.list_wait_queue(None)?;
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                        "deadlock_report": rep,
                        "wait_queue_count": waits.len(),
                        "wait_queue": waits,
                    }))?);
                }
            }
        }

        Commands::Preflight { json } => {
            let report = agent_steward::preflight::PreflightChecker::run_all(&cfg.data_dir, Some(&cfg));
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("{}", report.format_table());
            }
            if !report.passed_required {
                std::process::exit(1);
            }
        }

        Commands::Web { host, port, auth_token } => {
            let web_host = host.unwrap_or_else(|| cfg.web.host.clone());
            let web_port = port.unwrap_or(cfg.web.port);
            let token = auth_token.or_else(|| cfg.web.auth_token.clone());

            let web_config = agent_steward::web::WebConfig {
                host: web_host,
                port: web_port,
                auth_token: token,
            };

            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let web_server = agent_steward::web::WebServer::new(web_config, storage.provider().clone());
                web_server.run().await
            })?;
        }

        Commands::Hooks { action } => {
            match action {
                HooksCommands::Generate { client, out_dir, dry_run } => {
                    use agent_steward::hooks::{ClientHookGenerator, ClientType};
                    let client_type = client.parse::<ClientType>()?;
                    let res = ClientHookGenerator::generate(client_type, out_dir.as_deref(), dry_run)?;

                    println!("╔══════════════════════════════════════════════════════════════════════════════════════════╗");
                    println!("║                   Agent 客户端 Hook 自动化生成成功                                         ║");
                    println!("╠══════════════════════════════════════════════════════════════════════════════════════════╣");
                    println!("║ 客户端类型:   {:<75} ║", res.client);
                    println!("║ 推荐相对路径: {:<75} ║", res.relative_path);
                    println!("║ 是否可执行:   {:<75} ║", if res.executable { "是 (0o755)" } else { "否 (普通文本规则)" });
                    println!("║ 执行模式:     {:<75} ║", if res.is_dry_run { "Dry-Run 预览 (未写盘)" } else { "写入已生效" });
                    if let Some(ref path) = res.written_path {
                        println!("║ 落地物理路径: {:<75} ║", path);
                    }
                    for comp in &res.companion_files {
                        let comp_display = comp.written_path.as_deref().unwrap_or(&comp.relative_path);
                        println!("║ 配套脚本路径: {:<75} ║", comp_display);
                    }
                    println!("╚══════════════════════════════════════════════════════════════════════════════════════════╝");
                    println!("\n--- 生成内容预览 ---\n{}", res.content);
                }
                HooksCommands::Pre { tool_name, args } => {
                    let parsed_args: serde_json::Value = serde_json::from_str(&args)
                        .unwrap_or_else(|_| serde_json::json!({}));
                    let engine = agent_steward::api::hooks::AgentHookEngine::new(storage);
                    let res = engine.pre_tool_call(&tool_name, &parsed_args);
                    println!("{}", serde_json::to_string_pretty(&res)?);
                }
                HooksCommands::Post { tool_name, args, success, error_output } => {
                    let parsed_args: serde_json::Value = serde_json::from_str(&args)
                        .unwrap_or_else(|_| serde_json::json!({}));
                    let engine = agent_steward::api::hooks::AgentHookEngine::new(storage);
                    let res = engine.post_tool_call_with_error(&tool_name, &parsed_args, success, error_output.as_deref());
                    println!("{}", serde_json::to_string_pretty(&res)?);
                }
            }
        }

        Commands::Analytics { action } => {
            use agent_steward::storage::{AnalyticsProvider, DuckDbAnalyticsEngine};
            let engine = DuckDbAnalyticsEngine::from_synced_storage(storage.provider().clone())?;
            match action {
                AnalyticsCommands::Heatmap { days } => {
                    let heatmap = engine.changes_heatmap(days)?;
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                        "time_span_days": days,
                        "heatmap": heatmap,
                    }))?);
                }
                AnalyticsCommands::Trend { metric, days } => {
                    let trend = engine.asset_growth_trend(days)?;
                    let decay = engine.memory_decay_curve()?;
                    let stats = engine.fact_graph_stats()?;
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                        "metric": metric,
                        "time_span_days": days,
                        "asset_growth_trend": trend,
                        "memory_decay": decay,
                        "graph_stats": stats,
                    }))?);
                }
                AnalyticsCommands::Export { table, output_path } => {
                    let summary = engine.export_to_parquet(&table, &output_path)?;
                    println!("{}", serde_json::to_string_pretty(&summary)?);
                }
            }
        }

        Commands::McpCatalog { action } => {
            let scanner = agent_steward::ecosystem::McpConfigScanner::new(storage.clone());
            match action {
                McpCatalogCommands::Scan => {
                    let (cnt, sanitized) = scanner.scan_default_clients()?;
                    println!("✅ MCP 目录扫描完成：发现 {} 个 Server（已脱敏 {} 个环境变量密钥）", cnt, sanitized);
                }
                McpCatalogCommands::List => {
                    let list = storage.list_mcp_catalog()?;
                    println!("{}", serde_json::to_string_pretty(&list)?);
                }
                McpCatalogCommands::Route { query, limit } => {
                    let list = storage.search_mcp_catalog(&query, limit)?;
                    println!("{}", serde_json::to_string_pretty(&list)?);
                }
            }
        }

        Commands::Skill { action } => {
            match action {
                SkillCommands::Scan => {
                    let scanner = agent_steward::ecosystem::SkillCatalogScanner::new(storage.clone());
                    let (cnt, tokens) = scanner.scan_default_roots()?;
                    println!("✅ SKILL 目录扫描完成：已编目 {} 个 SKILL（若全量注入约消耗 {} tokens）", cnt, tokens);
                }
                SkillCommands::List => {
                    let list = storage.list_skills_catalog()?;
                    println!("已编目 SKILL 共 {} 个：", list.len());
                    for s in list {
                        println!("  - {} (~{} tok) -> {}", s.skill_name, s.est_tokens, s.skill_path);
                    }
                }
                SkillCommands::Route { intent, budget_tokens, max_skills } => {
                    let router = agent_steward::ecosystem::ContextBudgetRouter::new(storage.clone());
                    let plan = router.route(&agent_steward::ecosystem::ContextRouteRequest {
                        intent,
                        budget_tokens,
                        max_skills,
                        max_mcps: 4,
                        max_services: 3,
                    })?;
                    println!("{}", plan.markdown);
                    println!(
                        "📊 Context 防爆统计：全量 {} 个 SKILL (~{} tok) -> 本次按需路由 {} tok（节省 {:.1}%）",
                        plan.total_skills_catalog_count, plan.total_skills_full_tokens, plan.route_tokens_used, plan.saved_pct
                    );
                }
            }
        }

        Commands::Service { action } => {
            let mgr = agent_steward::ecosystem::ExternalContactsManager::new(storage.clone());
            match action {
                ServiceCommands::Register {
                    name, category, endpoint, protocol, capabilities,
                    auth_type, auth_env_var, health_endpoint, usage_example
                } => {
                    let rec = agent_steward::storage::models::ExternalServiceRecord {
                        id: format!("svc:{}", name),
                        service_name: name.clone(),
                        category,
                        endpoint,
                        protocol,
                        capabilities,
                        auth_type,
                        auth_env_var,
                        health_endpoint,
                        usage_example,
                        status: "unknown".to_string(),
                        last_checked_at: None,
                    };
                    mgr.register_service(&rec)?;
                    println!("✅ 外部服务联系人 '{}' 已安全登记", name);
                }
                ServiceCommands::List { category } => {
                    let list = storage.list_external_services(category.as_deref())?;
                    println!("{}", serde_json::to_string_pretty(&list)?);
                }
                ServiceCommands::Query { query } => {
                    let list = storage.search_external_services(&query, 10)?;
                    println!("{}", serde_json::to_string_pretty(&list)?);
                }
                ServiceCommands::Check { name } => {
                    let rep = mgr.check_health(&name)?;
                    println!("{}", serde_json::to_string_pretty(&rep)?);
                }
                ServiceCommands::Bootstrap => {
                    let cnt = mgr.bootstrap_from_config(&cfg)?;
                    println!("✅ 已从配置自动推导 {} 条外部服务联系人", cnt);
                }
            }
        }

        Commands::Mesh { action } => {
            use agent_steward::mesh::{build_local_facts, MeshClient};
            let client = MeshClient::new(&cfg.mesh, cfg.web.auth_token.clone());
            match action {
                MeshCommands::Peers => {
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                        "enabled": cfg.mesh.enabled,
                        "peers": client.peers(),
                        "timeout_ms": cfg.mesh.timeout_ms,
                    }))?);
                }
                MeshCommands::Local => println!("{}", serde_json::to_string_pretty(&build_local_facts())?),
                MeshCommands::Ping => println!("{}", serde_json::to_string_pretty(&client.ping_all())?),
                MeshCommands::Facts => println!("{}", serde_json::to_string_pretty(&client.facts_all())?),
                MeshCommands::Locks => println!("{}", serde_json::to_string_pretty(&client.locks_all())?),
                MeshCommands::Port { port } => {
                    println!("{}", serde_json::to_string_pretty(&client.federated_port_check(port))?);
                }
                MeshCommands::Acquire { peer, resource_type, resource_id, scope, holder_agent, ttl_seconds } => {
                    let req = agent_steward::coordination::LockRequest {
                        resource_type: agent_steward::coordination::ResourceType::from_str_loose(&resource_type),
                        resource_id,
                        scope: agent_steward::coordination::LockScope::from_str_loose(&scope),
                        holder_agent,
                        holder_pid: None,
                        ttl_seconds,
                        metadata: None,
                    };
                    println!("{}", serde_json::to_string_pretty(&client.acquire_on_peer(&peer, &req)?)?);
                }
                MeshCommands::Graph { query, hops } => {
                    println!("{}", serde_json::to_string_pretty(&client.federated_graph_query(&query, hops))?);
                }
                MeshCommands::DialogueSend { peer, thread_id, intent, content, from_agent, to_agent } => {
                    println!("{}", serde_json::to_string_pretty(&client.send_dialogue(&peer, &thread_id, &intent, &content, &from_agent, &to_agent)?)?);
                }
                MeshCommands::DialogueInbox { limit } => {
                    println!("{}", serde_json::to_string_pretty(&storage.list_mesh_dialogues(limit)?)?);
                }
                MeshCommands::DialogueAck { dialogue_id } => {
                    let ok = storage.update_mesh_dialogue_status(&dialogue_id, "acked")?;
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({"dialogue_id": dialogue_id, "acked": ok}))?);
                }
                MeshCommands::TransferSend { peer, file, from_agent } => {
                    println!("{}", serde_json::to_string_pretty(&client.send_transfer(&peer, std::path::Path::new(&file), &from_agent)?)?);
                }
                MeshCommands::TransferInbox { limit } => {
                    println!("{}", serde_json::to_string_pretty(&storage.list_mesh_transfers(limit)?)?);
                }
                MeshCommands::TransferAccept { transfer_id, dest_dir } => {
                    let rec = agent_steward::mesh::transfer::accept_transfer_to_dir(
                        storage.provider().as_ref(),
                        &transfer_id,
                        std::path::Path::new(&dest_dir),
                    )?;
                    println!("{}", serde_json::to_string_pretty(&rec)?);
                }
                MeshCommands::TransferReject { transfer_id } => {
                    let ok = agent_steward::mesh::transfer::reject_transfer(storage.provider().as_ref(), &transfer_id)?;
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({"transfer_id": transfer_id, "rejected": ok}))?);
                }
            }
        }

        Commands::Platform { action } => {
            let probe = agent_steward::platform::current_probe();
            match action {
                PlatformCommands::Info => {
                    println!("OS: {}  |  平台探针: {}", std::env::consts::OS, probe.name());
                }
                PlatformCommands::Ports => {
                    let ports = probe.listening_ports()?;
                    println!("平台探针 `{}` 识别监听端口 {} 个：", probe.name(), ports.len());
                    for p in ports.iter().filter(|p| !p.is_system_daemon) {
                        println!("  - {}/{} {} pid={:?} {}", p.port, p.protocol, p.bind_ip, p.pid, p.process_name.as_deref().unwrap_or("-"));
                    }
                }
                PlatformCommands::Services => {
                    let svcs = probe.services()?;
                    println!("平台探针 `{}` 识别自动化服务/任务 {} 个：", probe.name(), svcs.len());
                    for s in svcs.iter().take(50) {
                        println!("  - [{}] {} -> {}", s.kind, s.name, s.command);
                    }
                }
            }
        }

        Commands::Demo { action } => {
            use agent_steward::api::mcp_tools::McpToolRegistry;
            use agent_steward::demo::DemoEngine;
            match action {
                DemoCommands::List => {
                    println!("{}", serde_json::to_string_pretty(&DemoEngine::list_scenarios())?);
                }
                DemoCommands::Run { scenario_id } => {
                    let res = DemoEngine::run_scenario(storage.provider().clone(), &scenario_id)?;
                    println!("{}", serde_json::to_string_pretty(&res)?);
                }
                DemoCommands::Roi => {
                    let roi = DemoEngine::compute_roi(storage.provider().as_ref())?;
                    println!("{}", serde_json::to_string_pretty(&roi)?);
                }
                DemoCommands::Parity => {
                    let tools = McpToolRegistry::new(storage.clone()).list_tools();
                    let names: Vec<_> = tools.iter().filter_map(|t| t.get("name").and_then(|v| v.as_str())).collect();
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                        "total_mcp_tools": names.len(),
                        "tools": names,
                    }))?);
                }
                DemoCommands::Reset => {
                    let cleaned = DemoEngine::reset_sandbox(storage.provider().as_ref())?;
                    println!("✅ 已清理 {} 条 demo:* 沙盒状态", cleaned);
                }
            }
        }

        Commands::Bench { action } => {
            use agent_steward::benchmark::ledger::EffectivenessLedger;
            use agent_steward::benchmark::provenance::ProvenanceRule;
            use agent_steward::benchmark::report::{
                render_csv_report, render_markdown_report, BenchmarkReportPayload,
            };
            use agent_steward::benchmark::workloads::list_workload_specs;
            use agent_steward::benchmark::BenchmarkEngine;

            match action {
                BenchCommands::Workloads => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "workloads": list_workload_specs(),
                            "provenance_rules": ProvenanceRule::list_all(),
                        }))?
                    );
                }
                BenchCommands::Run { suite, save } => {
                    let report = BenchmarkEngine::run_suite(
                        storage.provider().clone(),
                        &suite,
                        save,
                        None,
                    )?;
                    println!("{}", render_markdown_report(&report)?);
                }
                BenchCommands::Report { run_id, format } => {
                    let rec_opt = if let Some(id) = run_id {
                        storage.get_benchmark_run(&id)?
                    } else {
                        storage.list_benchmark_runs(1)?.into_iter().next()
                    };
                    let Some(rec) = rec_opt else {
                        println!("尚无基准评测记录，请先运行 `agent-steward bench run`");
                        return Ok(());
                    };
                    let payload = BenchmarkReportPayload::from_record(&rec);
                    match format.as_str() {
                        "json" => println!("{}", serde_json::to_string_pretty(&payload)?),
                        "csv" => println!("{}", render_csv_report(&payload)?),
                        _ => println!("{}", render_markdown_report(&payload)?),
                    }
                }
                BenchCommands::Ledger { window, tool, limit } => {
                    let summary = storage.query_effectiveness_ledger(&window, tool.as_deref(), limit)?;
                    println!("{}", serde_json::to_string_pretty(&summary)?);
                }
                BenchCommands::Compare { run_a, run_b } => {
                    let a = storage.get_benchmark_run(&run_a)?
                        .ok_or_else(|| anyhow::anyhow!("未找到基准批次: {}", run_a))?;
                    let b = storage.get_benchmark_run(&run_b)?
                        .ok_or_else(|| anyhow::anyhow!("未找到基准批次: {}", run_b))?;
                    let diff = EffectivenessLedger::compare_runs(&a, &b);
                    println!("{}", serde_json::to_string_pretty(&diff)?);
                }
                BenchCommands::Export { format, out } => {
                    let rec = storage.list_benchmark_runs(1)?
                        .into_iter()
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("尚无基准评测记录可导出"))?;
                    let payload = BenchmarkReportPayload::from_record(&rec);
                    let content = match format.as_str() {
                        "json" => serde_json::to_string_pretty(&payload)?,
                        "csv" => render_csv_report(&payload)?,
                        _ => render_markdown_report(&payload)?,
                    };
                    if let Some(path) = out {
                        if let Some(parent) = std::path::Path::new(&path).parent() {
                            if !parent.as_os_str().is_empty() {
                                std::fs::create_dir_all(parent)?;
                            }
                        }
                        std::fs::write(&path, &content)?;
                        println!("✅ 已导出实证基准报告至: {} (SHA-256: {})", path, payload.report_sha256);
                    } else {
                        println!("{}", content);
                    }
                }
                BenchCommands::Reset => {
                    let cleaned = BenchmarkEngine::reset_sandbox(storage.provider().as_ref())?;
                    println!("✅ 已清理 {} 条 bench:* 沙盒状态", cleaned);
                }
                BenchCommands::Competitors { track, format } => {
                    let suite = if track.eq_ignore_ascii_case("all") {
                        "competitors".to_string()
                    } else if track.to_ascii_uppercase().starts_with("COMP-") {
                        track
                    } else {
                        format!("COMP-{}", track.to_ascii_uppercase())
                    };
                    let report = BenchmarkEngine::run_suite(
                        storage.provider().clone(),
                        &suite,
                        false,
                        Some("bench:comp"),
                    )?;
                    match format.as_str() {
                        "json" => println!("{}", serde_json::to_string_pretty(&report)?),
                        "csv" => println!("{}", render_csv_report(&report)?),
                        _ => println!("{}", render_markdown_report(&report)?),
                    }
                }
            }
        }

        Commands::Feedback { action } => {
            let t0 = std::time::Instant::now();
            let fb_path = agent_steward::feedback::FeedbackEngine::default_feedback_path();
            match action {
                FeedbackCommands::List => {
                    let items = agent_steward::feedback::FeedbackEngine::list_items(&fb_path)?;
                    println!("📋 反馈总账 ({}, 共 {} 条):", fb_path.display(), items.len());
                    for it in &items {
                        println!("  - [{}][{}] {} ({})", it.id, it.priority, it.title, it.status);
                    }
                }
                FeedbackCommands::Add { category, priority, title, evidence, target } => {
                    let item = agent_steward::feedback::FeedbackEngine::add_item(
                        &fb_path, &category, &priority, &title, &evidence, &target,
                    )?;
                    println!("✅ 已写入反馈 {}: [{}] {}", item.id, item.priority, item.title);
                    agent_steward::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "steward_feedback",
                        &item.title,
                        t0.elapsed().as_micros() as i64,
                    );
                }
                FeedbackCommands::Sync => {
                    let provider_ref = storage.provider().clone();
                    let added = agent_steward::feedback::FeedbackEngine::sync_verified_to_memories(
                        &fb_path, &storage,
                    )?;
                    println!("✅ 已将 {} 条已验证反馈沉淀至长期记忆库 (memories)", added);
                    agent_steward::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        provider_ref.as_ref(),
                        "cli",
                        "steward_feedback",
                        &format!("synced {} items", added),
                        t0.elapsed().as_micros() as i64,
                    );
                }
                FeedbackCommands::Reconcile { json } => {
                    let rep = agent_steward::feedback::probes::FreshnessReconciler::reconcile(&storage)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&rep)?);
                    } else {
                        println!(
                            "🧹 全生命周期新鲜度对账完成 ({} μs): 检查资产 {} | 标记失活 {} | 内容漂移失效写穿 {} | 剪枝幽灵节点 {} | 归档重复记忆 {} | 过期 CodeGraph 仓库 {:?}",
                            rep.elapsed_us,
                            rep.checked_assets,
                            rep.missing_assets_marked,
                            rep.drifted_assets_refreshed,
                            rep.ghost_fact_nodes_pruned,
                            rep.duplicate_memories_archived,
                            rep.stale_codegraph_repos
                        );
                    }
                    agent_steward::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "steward_feedback",
                        &format!(
                            "reconcile missing={} drifted={} ghost={} dup_mem={}",
                            rep.missing_assets_marked,
                            rep.drifted_assets_refreshed,
                            rep.ghost_fact_nodes_pruned,
                            rep.duplicate_memories_archived
                        ),
                        t0.elapsed().as_micros() as i64,
                    );
                }
                FeedbackCommands::Probes { auto_heal, auto_feedback, json } => {
                    let rep = agent_steward::feedback::probes::SelfEvolutionProbeEngine::run_probes(
                        &storage,
                        &fb_path,
                        auto_heal,
                        auto_feedback,
                    )?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&rep)?);
                    } else {
                        println!("🔬 {}", rep.summary);
                        for p in &rep.probes {
                            println!(
                                "  {} [{}] {}: {} (基线: {}) | Δ: {} | 自愈: {} | {}",
                                if p.healthy { "✅" } else { "⚠️" },
                                p.probe_id,
                                p.hook_stage,
                                p.measured_value,
                                p.comparator_baseline,
                                p.delta_summary,
                                p.healed_action,
                                p.recommendation
                            );
                        }
                        if !rep.feedback_items_created.is_empty() {
                            println!("📝 本次自动去重写入反馈: {:?}", rep.feedback_items_created);
                        }
                    }
                    agent_steward::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "steward_feedback",
                        &format!("probes healthy={} auto_heal={}", rep.overall_healthy, auto_heal),
                        t0.elapsed().as_micros() as i64,
                    );
                }
            }
        }

        Commands::Audit { action } => {
            use agent_steward::audit::AuditEvolutionAnalyzer;
            use agent_steward::storage::duckdb_analytics::{AnalyticsProvider, DuckDbAnalyticsEngine};
            use agent_steward::storage::models::AuditQueryFilter;

            match action {
                AuditCommands::Query {
                    domain,
                    actor_kind,
                    category,
                    outcome,
                    trace_id,
                    target,
                    window,
                    limit,
                    json,
                } => {
                    let rows = storage.query_audit_events(&AuditQueryFilter {
                        domain,
                        actor_kind,
                        category,
                        action: None,
                        outcome,
                        trace_id,
                        session_id: None,
                        target_contains: target,
                        window: Some(window),
                        limit,
                    })?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&rows)?);
                    } else {
                        println!("📜 双域行为审计明细 (共 {} 条):", rows.len());
                        for r in &rows {
                            println!(
                                "  [{}] {} | {} | {}.{} | outcome={} | target={} | trace={}",
                                r.created_at,
                                r.domain,
                                r.actor_kind,
                                r.category,
                                r.action,
                                r.outcome,
                                r.target_id,
                                r.trace_id
                            );
                        }
                    }
                }
                AuditCommands::Summary { window, days, json } => {
                    let summary = storage.audit_domain_summary(&window)?;
                    let olap = DuckDbAnalyticsEngine::from_synced_audit_events(storage.provider().clone())
                        .and_then(|eng| eng.audit_analytics_summary(days))
                        .ok();
                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "summary": summary,
                                "olap": olap,
                            }))?
                        );
                    } else {
                        println!(
                            "📊 双域行为审计汇总 ({}): 总计 {} 条 | IN_AGENT={} | OUT_OF_AGENT={} | 异常拦截={} | 自愈={}",
                            summary.window,
                            summary.total_events,
                            summary.in_agent_count,
                            summary.out_of_agent_count,
                            summary.failed_or_blocked_count,
                            summary.recovered_count
                        );
                        if let Some(o) = olap {
                            for top in o.top_actions.iter().take(10) {
                                println!(
                                    "  - [{}] {}.{} ({}) -> {} 次 (avg {:.0} μs)",
                                    top.domain, top.category, top.action, top.outcome, top.hit_count, top.avg_elapsed_us
                                );
                            }
                        }
                    }
                }
                AuditCommands::Trace { trace_id, json } => {
                    let rows = storage.query_audit_events(&AuditQueryFilter {
                        trace_id: Some(trace_id.clone()),
                        limit: 100,
                        ..Default::default()
                    })?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&rows)?);
                    } else {
                        println!("🔗 Trace `{}` 因果链路 ({} 条):", trace_id, rows.len());
                        for r in &rows {
                            println!(
                                "  [{}] {} | {}.{} | outcome={} | delta={}",
                                r.created_at, r.domain, r.category, r.action, r.outcome, r.before_after_delta
                            );
                        }
                    }
                }
                AuditCommands::Evolve {
                    auto_feedback,
                    min_hits,
                    json,
                } => {
                    let fb_path = agent_steward::feedback::FeedbackEngine::default_feedback_path();
                    let rep = AuditEvolutionAnalyzer::run_evolution_cycle(
                        &storage,
                        Some(&fb_path),
                        auto_feedback,
                        min_hits,
                    )?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&rep)?);
                    } else {
                        println!(
                            "🧬 审计驱动自进化完成: 评估 {} 条 | 租约降级 {:?} | 租约康复 {:?} | 摩擦热点 {} | 新增反馈 {:?} | 目录规则候选 {}",
                            rep.evaluated_events,
                            rep.downgraded_targets,
                            rep.rehabilitated_targets,
                            rep.friction_hotspots.len(),
                            rep.feedback_items_created,
                            rep.placement_candidates.len()
                        );
                    }
                }
                AuditCommands::Export { output_path, json } => {
                    let eng = DuckDbAnalyticsEngine::from_synced_audit_events(storage.provider().clone())?;
                    let exp = eng.export_to_parquet("audit_events", &output_path)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&exp)?);
                    } else {
                        println!(
                            "📦 已导出 audit_events ({} 行, {} 字节) 至 {}",
                            exp.row_count, exp.file_size_bytes, exp.output_path
                        );
                    }
                }
            }
        }

        Commands::Body { action } => {
            use agent_steward::body::{
                AutonomyEnvelope, BodyManifestEngine, ContinuityInput, ContinuityManager,
                HomeostasisVitalsEngine,
            };
            let t0 = std::time::Instant::now();
            match action {
                BodyCommands::Manifest { json } => {
                    let m = BodyManifestEngine::build(&storage);
                    let out = serde_json::to_string_pretty(&m)?;
                    agent_steward::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "steward_body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else {
                        println!("🧬 Agent 本体身份证: {} ({}, {} / {})", m.body_id, m.hostname, m.os, m.arch);
                        println!("📦 活跃接力胶囊: {} | 六大器官状态:", m.active_checkpoints);
                        for o in &m.organs {
                            println!("  - {:12} [{}] {}", o.name, o.status, o.label);
                        }
                    }
                }
                BodyCommands::Vitals { json } => {
                    let v = HomeostasisVitalsEngine::assess(&storage);
                    let out = serde_json::to_string_pretty(&v)?;
                    if json {
                        println!("{}", out);
                    } else {
                        println!(
                            "🩺 五维本体生命体征 [{}]: 综合稳态分 {:.1}/100 ({})",
                            v.body_id, v.overall_score, v.status
                        );
                        for d in &v.dimensions {
                            println!(
                                "  - {:10} ({:>2}%): {:>5.1} [{}] {}",
                                d.label, d.weight_pct, d.score, d.status, d.detail
                            );
                        }
                        if !v.recommended_l1_actions.is_empty() {
                            println!("💡 建议 L1 自愈动作: {}", v.recommended_l1_actions.join("；"));
                        }
                    }
                }
                BodyCommands::Envelope { action: chk_act, target, dry_run, json } => {
                    if let Some(act) = chk_act {
                        let dec = AutonomyEnvelope::evaluate(&act, target.as_deref(), dry_run);
                        if json {
                            println!("{}", serde_json::to_string_pretty(&dec)?);
                        } else {
                            println!(
                                "🛡️ [{}] allowed={} confirm={} — {}",
                                dec.level_code, dec.allowed, dec.requires_confirmation, dec.reason
                            );
                        }
                    } else {
                        let mat = AutonomyEnvelope::policy_matrix();
                        if json {
                            println!("{}", serde_json::to_string_pretty(&mat)?);
                        } else {
                            for rule in &mat {
                                println!("• {} ({}) — {}", rule.level, rule.label, rule.authorization);
                            }
                        }
                    }
                }
                BodyCommands::Handoff {
                    goal,
                    from_brain,
                    session_id,
                    files,
                    facts,
                    blockers,
                    next_step,
                    json,
                } => {
                    let split_csv = |opt: Option<String>| -> Vec<String> {
                        opt.unwrap_or_default()
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect()
                    };
                    let cp = ContinuityManager::save_capsule(
                        &storage,
                        ContinuityInput {
                            from_brain,
                            session_id,
                            goal_summary: goal,
                            active_files: split_csv(files),
                            active_locks: Vec::new(),
                            verified_facts: split_csv(facts),
                            open_blockers: split_csv(blockers),
                            next_step_hint: next_step,
                            token_budget: Some(300),
                        },
                    )?;
                    let out = serde_json::to_string_pretty(&cp)?;
                    agent_steward::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "steward_body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else {
                        println!(
                            "🧬 已保存跨大脑接力胶囊 `{}` (来自 `{}` · SHA256={}...)",
                            cp.checkpoint_id,
                            cp.from_brain,
                            cp.evidence_sha256.chars().take(12).collect::<String>()
                        );
                    }
                }
                BodyCommands::Resume { by_brain, checkpoint_id, json } => {
                    let res = ContinuityManager::resume_latest(&storage, &by_brain, checkpoint_id.as_deref())?;
                    let out = serde_json::to_string_pretty(&res)?;
                    agent_steward::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "steward_body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else if let Some(cp) = res {
                        println!("{}", ContinuityManager::render_brief_snippet(&cp, 300));
                    } else {
                        println!("ℹ️ 当前暂无待接力的活跃跨大脑胶囊。");
                    }
                }
                BodyCommands::Heal { json } => {
                    let rep = HomeostasisVitalsEngine::run_l1_autonomic_heal(&storage)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&rep)?);
                    } else {
                        println!(
                            "🩺 L1 稳态反射自愈完成: {:.1} → {:.1} (Δ={:+.2}) | 清理过期锁={} 剪枝幽灵={}",
                            rep.before_score,
                            rep.after_score,
                            rep.delta_score,
                            rep.expired_locks_cleaned,
                            rep.pruned_ghost_nodes
                        );
                    }
                }
                BodyCommands::Peers { json } => {
                    let cfg = Config::load_or_default(None).unwrap_or_default();
                    let client = agent_steward::mesh::MeshClient::new(&cfg.mesh, cfg.web.auth_token.clone());
                    let peers = client.query_peer_body_manifests();
                    if json {
                        println!("{}", serde_json::to_string_pretty(&peers)?);
                    } else if peers.is_empty() {
                        println!("ℹ️ 未配置 [mesh].peers 多本体对等节点。");
                    } else {
                        for p in &peers {
                            println!("🌐 {} -> ok={} ({}ms)", p.peer, p.ok, p.latency_ms);
                        }
                    }
                }
            }
        }
        }
        Ok(())
    })();

    if !skip_cli_audit {
        let outcome = if exec_res.is_ok() { "success" } else { "failed" };
        agent_steward::audit::AuditLogger::record_best_effort(
            &audit_storage,
            agent_steward::audit::AuditEventInput::in_agent("cli", "cli_command", cli_cmd_name)
                .with_target("command", cli_cmd_name)
                .with_outcome(outcome)
                .with_elapsed_us(cli_t0.elapsed().as_micros() as i64)
                .with_delta(serde_json::json!({
                    "ok": exec_res.is_ok(),
                    "error": exec_res.as_ref().err().map(|e| e.to_string()),
                })),
        );
    }

    exec_res
}

fn cli_command_label(cmd: &Commands) -> &'static str {
    match cmd {
        Commands::Status => "status",
        Commands::Tui { .. } => "tui",
        Commands::Brief { .. } => "brief",
        Commands::Place { .. } => "place",
        Commands::Asset { .. } => "asset",
        Commands::Env { .. } => "env",
        Commands::Fact { .. } => "fact",
        Commands::Memory { .. } => "memory",
        Commands::Links { .. } => "links",
        Commands::Changes { .. } => "changes",
        Commands::Close { .. } => "close",
        Commands::Gc { .. } => "gc",
        Commands::Mcp => "mcp",
        Commands::Daemon => "daemon",
        Commands::Preflight { .. } => "preflight",
        Commands::Web { .. } => "web",
        Commands::Hooks { .. } => "hooks",
        Commands::Storage { .. } => "storage",
        Commands::Coordination { .. } => "coordination",
        Commands::Analytics { .. } => "analytics",
        Commands::McpCatalog { .. } => "mcp-catalog",
        Commands::Skill { .. } => "skill",
        Commands::Service { .. } => "service",
        Commands::Mesh { .. } => "mesh",
        Commands::Platform { .. } => "platform",
        Commands::Demo { .. } => "demo",
        Commands::Bench { .. } => "bench",
        Commands::Feedback { .. } => "feedback",
        Commands::Audit { .. } => "audit",
        Commands::Body { .. } => "body",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_1to1_mcp_subcommands_parse() {
        assert!(Cli::try_parse_from(["steward", "asset", "register", "--path", "/root/new/tmp/a.py", "--type", "script"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "hooks", "pre", "--tool-name", "bind_port", "--args", "{\"port\":8080}"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "hooks", "post", "--tool-name", "write_file", "--args", "{\"path\":\"/tmp/a\"}"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "coordination", "acquire", "-t", "port", "-i", "9090"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "analytics", "heatmap", "--days", "7"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "analytics", "trend", "--metric", "assets", "--days", "14"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "analytics", "export", "--table", "assets", "--output-path", "/root/new/tmp/a.parquet"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "close", "sess-1", "--dry-run"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "mcp-catalog", "scan"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "skill", "route", "PostgreSQL调优", "--budget-tokens", "500"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "service", "bootstrap"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "coordination", "wait", "-t", "port", "-i", "8080"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "coordination", "deadlock"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "mesh", "port", "8080"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "mesh", "acquire", "--peer", "http://10.0.0.2:8756", "-t", "port", "-i", "9000"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "platform", "ports"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "demo", "run", "scene-1"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "feedback", "list"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "feedback", "sync"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "feedback", "reconcile"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "feedback", "probes", "--auto-heal"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "brief"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "brief", "--query", "postgres", "--budget-tokens", "1200"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "brief", "--legacy"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "audit", "query", "--domain", "OUT_OF_AGENT"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "audit", "summary", "--window", "7d"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "audit", "evolve", "--auto-feedback"]).is_ok());
    }
}
