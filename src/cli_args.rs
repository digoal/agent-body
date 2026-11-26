use std::path::PathBuf;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "agent-body", version = agent_body::VERSION, about = "Agent Body — 脑体分离 Agent 本体（宿主机具身底座 · Rust 版）")]
pub(crate) struct Cli {
    #[arg(short, long, help = "自定义配置文件路径")]
    pub(crate) config: Option<PathBuf>,

    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
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
    Mcp {
        #[arg(long, help = "MCP 工具暴露档位: core (10工具省78% Token) | dev | ops | full")]
        profile: Option<String>,
    },

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

        #[arg(long, default_value_t = true, action = clap::ArgAction::Set, num_args = 0..=1, default_missing_value = "true", help = "是否清理受管临时目录下属于本会话的临时文件")]
        clean_temp: bool,

        #[arg(long, default_value_t = false, help = "仅预览待清理临时文件与归档摘要，不物理删除")]
        dry_run: bool,
    },

    #[command(about = "宿主机孤儿进程与临时垃圾资源安全扫描与回收")]
    Gc {
        #[arg(long, help = "待清理的临时目录路径 (缺省使用 [workspace].temp_dir，强制安全白名单)")]
        tmp_dir: Option<PathBuf>,

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
pub(crate) enum BodyCommands {
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
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set, num_args = 0..=1, default_missing_value = "true")]
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
    #[command(about = "抓取本体内生等待事件与 /proc/self 内核性能快照（可选设为黄金性能基准）")]
    PerfSnapshot {
        #[arg(long, default_value = "cli-snap")]
        label: String,
        #[arg(long, default_value_t = false)]
        set_baseline: bool,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(about = "生成本体 AWR 快照/黄金基线差分性能报告与 ADDM 瓶颈诊断处方")]
    PerfAwr {
        #[arg(long)]
        snap_a: Option<String>,
        #[arg(long)]
        snap_b: Option<String>,
        #[arg(long, help = "查看已保存的历史 AWR 报告 ID (如 awr-...)")]
        report_id: Option<String>,
        #[arg(long, default_value_t = true, num_args = 0..=1, default_missing_value = "true", action = clap::ArgAction::Set)]
        vs_baseline: bool,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(about = "列出已保存的 AWR 性能快照库元数据与黄金基线标识")]
    PerfSnapshots {
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(about = "列出已保存的历史 AWR 差分诊断报告清单")]
    PerfReports {
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(about = "删除指定 AWR 性能快照或历史诊断报告（同步清理索引）")]
    PerfDelete {
        #[arg(long, default_value = "auto", help = "删除目标类型: auto | snapshot | report")]
        kind: String,
        #[arg(long, help = "快照 ID (snap-...) 或 AWR 报告 ID (awr-...)")]
        id: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(about = "在 L1_AUTONOMIC 包络内执行一键性能与内存自优化 (PRAGMA optimize + WAL + malloc_trim)")]
    PerfTune {
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(about = "查看或切换三态运行模式 (body_only / bypass / shadow_ab) 与 MCP 工具档位 (core / dev / ops / full)")]
    RuntimeMode {
        #[arg(long, help = "设置并持久化运行模式: body_only | bypass | shadow_ab")]
        set: Option<String>,
        #[arg(long, help = "设置 shadow_ab 主响应侧: live | body")]
        shadow_primary: Option<String>,
        #[arg(long, help = "设置 MCP Schema 暴露档位: core | dev | ops | full")]
        mcp_profile: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(about = "查看真实世界 Shadow A/B 对照收益与回退诊断报告 (ISSUE #43)")]
    AbReport {
        #[arg(long, default_value = "7d")]
        window: String,
        #[arg(long, default_value_t = 200)]
        limit: usize,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(about = "立即执行一次原生实时搜索 vs agent-body 索引 A/B 对照并入库")]
    AbCompare {
        #[arg(help = "检索查询词")]
        query: String,
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum AuditCommands {
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
        #[arg(long, help = "导出路径 (缺省导出到 [workspace].temp_dir/audit_events_export.csv)")]
        output_path: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum FeedbackCommands {
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
pub(crate) enum BenchCommands {
    #[command(about = "列出 18 大标准 A/B 负载与 PR-01~14 推导规则")]
    Workloads,
    #[command(about = "运行宿主机真机 A/B 对照基准 (full/macro/micro/dq/competitors/W1~W18)")]
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
pub(crate) enum DemoCommands {
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

        #[arg(long, help = "目标端数据库文件路径 (默认 ~/.agent-body/steward.duckdb)")]
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
    },

    #[command(about = "全文与语义搜索资产（支持多模态、领域与 KV 过滤及 FTS+向量混合 RRF 检索）")]
    Search {
        #[arg(help = "搜索关键词（传空字符串时可纯按过滤器检索）")]
        query: String,

        #[arg(short, long, default_value_t = 10)]
        limit: usize,

        #[arg(long, default_value = "standard", help = "披露粒度: micro | standard | full")]
        detail: String,

        #[arg(long, help = "按多模态类型过滤: image | audio | video | pdf | office | dataset | code | document | config")]
        modality: Option<String>,

        #[arg(long, help = "按业务领域过滤 (如 web_dashboard, storage_engine)")]
        domain: Option<String>,

        #[arg(long, help = "要求 desc_kv 中存在指定键 (如 table_names, columns, dimensions)")]
        kv_key: Option<String>,

        #[arg(long, help = "要求 desc_kv（或 --kv-key 对应值）匹配子串")]
        kv_value: Option<String>,

        #[arg(long, help = "禁用 Ollama 向量 RRF 混合检索，仅使用确定性 FTS5 与结构化过滤")]
        no_hybrid: bool,
    },

    #[command(about = "手动向大管家登记一个文件或目录资产")]
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

        #[arg(long, default_value_t = true, action = clap::ArgAction::Set, num_args = 0..=1, default_missing_value = "true", help = "是否启用 Ollama 向量混合 RRF 检索")]
        hybrid: bool,
    },

    #[command(about = "全面体检长期记忆健康度 (检测认知冲突矛盾与衰减淘汰候选)")]
    Audit {
        #[arg(long, help = "若存在冲突，指定归档淘汰的旧记忆 ID")]
        archive_id: Option<String>,
    },
}
