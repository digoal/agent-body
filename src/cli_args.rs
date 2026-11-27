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

    #[command(about = "启动基于浏览器的轻量级Agent 本体拓扑图与协作锁监控大屏")]
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
    #[command(about = "列出 19 大标准 A/B 负载与 PR-01~14 推导规则")]
    Workloads,
    #[command(about = "运行宿主机真机 A/B 对照基准 (full/macro/micro/dq/competitors/W1~W19)")]
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


pub(crate) use crate::cli_subcommands::*;
