#!/usr/bin/env python3
"""Build presentation/agent-body-deck.html (25-slide Visual-First Deep Technical Deck).

Decks-as-Code generator using bespoke architectural SVG diagrams & visual code maps.
Grounded 100% in Agent Body v1.1.0 source code, live Benchmark v4.1, and AWR telemetry.
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from deck_engine import (
    C_CLAY,
    C_GOLD,
    C_GREEN,
    C_TEAL,
    C_TEAL_2,
    C_TEAL_D,
    render_deck,
    svg_bars,
    svg_closing,
    svg_code_and_flow,
    svg_columns,
    svg_pipeline,
    vis_agent_usage_mcp_hook_skill,
    vis_aopcc_highway,
    vis_awr_telemetry,
    vis_benchmark_pareto,
    vis_blind_agent_scene,
    vis_competitor_arena,
    vis_cover_visual,
    vis_do_dont_wall,
    vis_env_change_to_db_pipeline,
    vis_evolution_flywheel,
    vis_freshness_purifier,
    vis_handoff_capsule,
    vis_host_graph_federation,
    vis_lock_and_wfg,
    vis_mesh_network,
    vis_microkernel_hub,
    vis_quality_gates,
    vis_security_dual_audit,
    vis_storage_tri_engine,
    vis_tiered_scan_doors,
    vis_vitals_radar_autonomy,
)

from evolution_manifest import extract_manifest

_MANIFEST = extract_manifest()
_COUNTS = _MANIFEST["counts"]
APP_VER = _MANIFEST["app_version"]
BENCH_VER = _MANIFEST["bench_spec_version"]
MOD_CNT = _COUNTS["modules"]
TOOL_CNT = _COUNTS["mcp_tools"]
CLI_CNT = _COUNTS["cli_commands"]
SPI_CNT = _COUNTS["spi_methods"]
TABLE_CNT = _COUNTS["tables"]
WL_CNT = _COUNTS["workloads"]
MICRO_CNT = _COUNTS["micro"]
DQ_CNT = _COUNTS["dq"]
COMP_CNT = _COUNTS["competitors"]
PR_CNT = _COUNTS["pr_rules"]
KERNEL_CNT = _COUNTS["kernel_crates"]
CONTRIB_CNT = _COUNTS["contrib_exts"]
DIST_CNT = _COUNTS["distributions"]
TEST_CNT = _COUNTS["verified_tests"]

ROOT = Path(__file__).resolve().parent.parent
PRES_DIR = ROOT / "presentation"


def build_tech_deck() -> None:
    slides = [
        # 01 视觉封面 · 微内核与三引擎架构插画
        {
            "cover": True,
            "eyebrow": "封面 · 深度技术架构全景",
            "title": "Agent Body · 深度技术架构与内核工程全景",
            "svg": vis_cover_visual(
                kicker=f"AGENT BODY · DEEP TECHNICAL EXCHANGE · v{APP_VER}",
                title="Agent 本体技术全景",
                en_sub=f"{MOD_CNT} Modules · {TOOL_CNT} MCP · {SPI_CNT} SPI · {KERNEL_CNT} Kernel / {CONTRIB_CNT} Contrib",
                tagline="面向数据库内核与 AI Infra 架构师的图解工程全景",
                badges=[
                    (f"{SPI_CNT} SPI", f"{TABLE_CNT} 表 · 三存储热切", C_TEAL),
                    (f"{CONTRIB_CNT} 扩展", f"{KERNEL_CNT} 微内核 · 4 发行版", C_GREEN),
                    (f"{TEST_CNT} 测", f"Bench v{BENCH_VER} 全绿", C_CLAY),
                ],
                theme="tech_arch",
                right_tag=f"图解工程蓝图 · 深度技术交流版 (27 页)",
            ),
            "script": (
                "各位技术同行好，这份 27 页的 Deck 是 Agent Body 的完整架构深度交流版，大约 45 到 60 分钟。\n"
                f"今天我们全部用工程原理图、网络拓扑和实测曲线直穿核心：{MOD_CNT} 个 Rust 模块、{SPI_CNT} 个存储 SPI、{TABLE_CNT} 张数据表、AOPCC 期望代价数学边界、内生 Wait Events / AWR / Prometheus `/metrics` 与 W3C Trace 性能工程，以及 Phase 70~75 借鉴 PostgreSQL 插件体系完成的微内核 (`kernel/`) 与周边 (`contrib/`) 大一统实践。"
            ),
        },
        # 02 第一性原理 · 蒙眼大脑为何注定撞墙
        {
            "eyebrow": "01 · FIRST PRINCIPLES · BRAIN-BODY SEPARATION",
            "title": "第一性原理：把状态窄化为「对话记忆」为何注定撞墙",
            "lead": "物理真值 · 执行门禁 · 写穿保鲜 · 换脑接力 · <b>纯聊天向量库全盲</b>",
            "svg": vis_blind_agent_scene(animated=True),
            "anim_caps": [
                "Step 1/4：传统 Agent Memory 只给聊天记录建索引，大脑对宿主机物理世界完全失明",
                "Step 2/4：不知本机已有源码重 clone 爆盘，不知 :5432 被占直接 bind 撞车",
                "Step 3/4：不知上游 Cron/Systemd 依赖改崩服务，扫爆 node_modules 稀释上下文",
                "Step 4/4：跨 IDE 切换即失忆——宿主机需要一层永驻、确定性的具身底座",
            ],
            "next": f"确立脑体分离后，看整套系统的四层分层拓扑与 {SPI_CNT} 方法三引擎 Storage SPI。",
            "script": (
                "做系统架构最怕切错问题边界。过去一年开源社区涌现大量 Agent Memory 项目，但它们把持久化状态窄化成了聊天记录向量检索。\n"
                "图中五处红色事故点没有一个是给聊天记录建向量索引能解决的。因此我们把抽象层下沉一级：LLM 是瞬态大脑，Agent Body 是永驻宿主机的具身底座。"
            ),
        },
        # 03 系统全景 · 四层拓扑与三引擎存储 SPI
        {
            "eyebrow": f"02 · SYSTEM ARCHITECTURE · {MOD_CNT} MODULES & {SPI_CNT} SPI",
            "title": f"系统全景：四层解耦拓扑 × {SPI_CNT} 方法三引擎热迁移",
            "lead": f"接入层零业务逻辑 · <b>SQLite / PostgreSQL / DuckDB 无损热切</b>",
            "svg": vis_storage_tri_engine(animated=True),
            "anim_caps": [
                f"Step 1/4：Layer 1 接入层——{TOOL_CNT} MCP / {CLI_CNT} CLI / 7 Hooks / Web 零业务逻辑同源",
                f"Step 2/4：Layer 2 六大器官引擎——{MOD_CNT} 个 Rust 顶层模块高内聚协同",
                f"Step 3/4：Layer 3 StorageProvider SPI——{SPI_CNT} 强类型方法 + {TABLE_CNT} 张三端同步表",
                "Step 4/4：Layer 4 三引擎圆柱——SQLite 零配单机 / PG 并发集群 / DuckDB 列存 OLAP",
            ],
            "next": "看清整体骨架后，先看一次真实任务中 Agent 如何经 MCP 与硬 Hook 调用本体（含删掉客户端 skills 目录的路由机制）。",
            "script": (
                f"这是 Agent Body 的四层全景图：接入层零业务逻辑，{MOD_CNT} 个模块按六大器官内聚，存储层通过 {SPI_CNT} 方法 SPI 统一门面驾驭 SQLite、PostgreSQL 和 DuckDB 三引擎。\n"
                "个人开发机用内嵌 SQLite WAL+FTS5 零运维；多 Agent 高并发集群热切 PostgreSQL 利用 pgvector 与 Advisory Lock；审计分析由受限 2 线程/256MB 的 DuckDB 负责。"
            ),
        },
        # 04 调用链路 · MCP × 硬 Hook 协同与去客户端 Skills 目录精准路由
        {
            "eyebrow": "03 · RUNTIME INVOCATION · MCP × HOOK × SKILL ROUTE",
            "title": "调用链路：MCP × 硬 Hook 协同与去客户端 Skills 目录精准路由",
            "lead": "client_templates.rs 硬 Hook 拦截 + <b>ContextBudgetRouter 4 因子排序</b>",
            "svg": vis_agent_usage_mcp_hook_skill(animated=True),
            "anim_caps": [
                "Step 1/4：SessionStart(brief) / Active MCP(43工具) / PreToolUse(门禁) / PostToolUse(AutoSync+<2ms处方)",
                "Step 2/4：删客户端 ~/.agents/skills/ 后，SkillCatalogScanner 仅扫 ~/new/skills/ Frontmatter 入库",
                "Step 3/4：body_skill_route 先跑 skills_catalog_fts MATCH，再 rank_skills(+60/+25/+20/+15) + 预算裁剪",
                "Step 4/4：返回 Top-3 摘要与绝对路径，大脑按需 read_file 加载，609k → 358 tok (-99.9%)",
            ],
            "next": "调用闭环走通后，下钻第一器官：零幻觉物理事实探针与底噪过滤。",
            "script": (
                "先看源码级的接入调用链路：`src/hooks/client_templates.rs` 为 Antigravity、Claude、Cursor、Codex、Hermes 一键生成 `hooks.json` 与 `body_bridge.py`。\n"
                "每次任务：开工 `SessionStart` 跑 `agent-body brief` 注入事实、租约与胶囊；任务中大脑通过 stdio JSON-RPC 按需调 `body_skill_route`、`body_fact_query`、`body_codegraph_bridge`、`body_lock_acquire` 等 MCP；动手前 `PreToolUse` 经 `src/api/hooks.rs` 做端口/目录锁/红线校验；完成后 `PostToolUse` 异步拉起 `AutoSyncEngine` 或在 2ms 内回注碰壁处方。\n"
                "当删掉客户端自带 `skills/` 目录后，技能实体托管在 `[workspace].skills_dirs`（如 `~/new/skills/`），由 `src/ecosystem/scanner.rs` 抽 Frontmatter、防提示词投毒脱敏写入 `skills_catalog` + `skills_catalog_fts`；运行任务时 `src/ecosystem/router.rs` 经 FTS5 BM25 粗召回和 `rank_skills`（全名 +60、词命中 +25、触发词 +20、描述 +15）返回 Top-K 路径，大脑按需 `read_file` 加载。"
            ),
        },
        # 05 感官层① · 零幻觉物理事实采集
        {
            "eyebrow": "04 · SENSORY I · GROUND-TRUTH PROBES",
            "title": "感官层①：/proc 0.2ms 直读与 95% 内核底噪过滤",
            "lead": "索引优先替代暴力 fork · <b>cmdline 全链路强制脱敏</b>",
            "svg": svg_columns(
                [
                    {
                        "tag": "FACT 01 · 工具能力",
                        "title": "CapabilityEngine",
                        "icon": "terminal",
                        "metric": "565x",
                        "accent": C_TEAL_D,
                        "lines": [
                            "扫描 $PATH 版本与路径",
                            "附推荐参数免 pip 折腾",
                            ("W1 Token 实测 -99.4%", True),
                        ],
                        "foot_val": "src/fact/capability.rs",
                    },
                    {
                        "tag": "FACT 02 · 进程网络",
                        "title": "ProcessNetTopology",
                        "icon": "radar",
                        "metric": "0.2ms",
                        "accent": C_TEAL,
                        "lines": [
                            "直读 /proc 映 PID↔Port",
                            "过滤 kworker 系统底噪",
                            ("折叠长脚本 + 强制脱敏", True),
                        ],
                        "foot_val": "src/fact/process_net.rs",
                    },
                    {
                        "tag": "FACT 03 · 服务编目",
                        "title": "Automation & Catalog",
                        "icon": "server",
                        "metric": "21,403",
                        "accent": C_GREEN,
                        "lines": [
                            "解析 systemd ExecStart",
                            "纳管 crontab 后台任务",
                            ("编目 Git 仓与 CodeGraph", True),
                        ],
                        "foot_val": "src/fact/automation.rs",
                    },
                ],
                bottom_banner=(
                    "单一只读入口 body_fact_query / body_port_check：Index-First 取代反复 fork 子进程。",
                    "ZERO SIMULATION",
                    "teal",
                ),
            ),
            "next": "面对数十万磁盘文件，如何做到「敲门不进屋」且零云端 Token 精炼语义？",
            "script": (
                "感官层底层做两件实战硬事：一是在 /proc 采集层滤除 95% kworker 底噪、折叠超长内联脚本并过 SecretRedactor 脱敏；二是把 $PATH 工具语义化，连推荐参数一并告知大脑，直接把 W1 加速 565 倍。"
            ),
        },
        # 06 感官层② · 白黑灰敲门不进屋 + 三层元数据管道
        {
            "eyebrow": "05 · SENSORY II · TIERED SCAN & METADATA",
            "title": "感官层②：白/黑/灰「敲门不进屋」× 零云端三层标注",
            "lead": "消灭 7,825 无效 inotify 句柄 · <b>W3 阻断 534 噪音 (-95.8%)</b>",
            "svg": vis_tiered_scan_doors(animated=True),
            "anim_caps": [
                "Step 1/4：宿主机磁盘 21,403 纳管资产经白/黑/灰三道门精准分流，Watcher 同步剪枝",
                "Step 2/4：白名单敞开绿门深挖到底，DQ1 元数据 100% · DQ2 本地向量 99.5%",
                "Step 3/4：黑名单 node_modules/.git/target 紧锁红门「敲门不进屋」，挡下 7,825 句柄",
                "Step 4/4：灰名单源码仓转交 CodeGraph；右侧 L0/L1/L2 本地 Ollama 0.8b 零云端精炼",
            ],
            "next": "当宿主机文件增删改、进程启停、端口切换时，这些变化如何实时落入数据库？",
            "script": (
                "看这三扇门：绿色白名单业务目录深挖到底；红色黑名单只记目录节点绝不下钻，连 inotify 一并剪枝；琥珀灰名单开源源码只读入口，微观代码转交 CodeGraph。\n"
                "右边是零云端 Token 的三层正交标注管道：L0 管写穿哈希，L1 由 Rust 微秒静态提取，L2 由本地 Ollama 0.8b 异步脱敏精炼。"
            ),
        },
        # 07 实时入库机制 · 四通道感知 × FTS5 触发器 × 级联写穿对账
        {
            "eyebrow": "06 · REALTIME DB SYNC · 4 CHANNELS & TRIGGERS",
            "title": "实时入库机制：四通道感知 × FTS5 触发器 × 级联写穿对账",
            "lead": "FileWatcher + AutoSync + EnvCollector + Scheduler · <b>schema_reconcile Trigger 原子同步</b>",
            "svg": vis_env_change_to_db_pipeline(animated=True),
            "anim_caps": [
                "Step 1/4：四通道采集——FileWatcher(500ms防抖) / PostToolUse(AutoSync) / 60s /proc / 1800s 巡检",
                "Step 2/4：净化与写穿——改写标 embedding_status='stale'；删除清 links/fact_nodes/归档 memories",
                f"Step 3/4：{SPI_CNT} SPI 落盘——SQLite AFTER INS/UPD/DEL 触发器自动同步 6 列 assets_fts 与 trigram",
                "Step 4/4：EventBus → [[hooks]]/[[pipelines]] + Web SSE 推送，下次 brief/changes_since 即刻可见",
            ],
            "next": "节点元数据与实时入库机制齐备后，看如何织成 L1/L2/L3 有向加权图谱并桥接 CodeGraph。",
            "script": (
                "环境变化落库靠四条独立互补的源码通道：`src/watcher/mod.rs` 的 `FileWatcher` 基于 inotify/FSEvents 在启动时剪枝黑名单、500ms 去重防抖后交 `ChangeHandler`；`src/hooks/auto_sync.rs` 由 `PostToolUse` 或 Git `post-commit` 异步触发秒级回写；`src/env/mod.rs` 每 60 秒读 `/proc` 与监听端口后原子替换 `active_processes`；`src/daemon/scheduler.rs` 每 1800 秒在独立 OS 线程跑 `FreshnessReconciler::reconcile` 兜底对账。\n"
                "在写穿机制上：文件修改经 `merge_with_existing_asset` 发现哈希变化立即把旧向量标 `stale`，高并发写走 `AuditLogger` 60 秒滑动窗折叠；文件删除则置 `assets.status='missing'`，配合 `schema_reconcile.rs` 定义的 SQLite `AFTER INSERT/UPDATE/DELETE` 触发器实时同步 `assets_fts`、`assets_trigram_fts` 和 `skills_catalog_fts`，同时清除 `links` 断链、`fact_nodes` 幽灵节点与旧摘要记忆（PG 端写入墓碑），最后经 `EventBus` 推向 Web SSE。"
            ),
        },
        # 08 图谱智能 · L1/L2/L3 有向图与宏微联邦
        {
            "eyebrow": "07 · GRAPH INTELLIGENCE · L1/L2/L3 & FEDERATION",
            "title": "图谱智能：三层证据有向加权图 × CodeGraph 显微镜",
            "lead": "4,985 文件边自动上卷 · <b>孤岛率 99.5% → 0% · M1 148 μs</b>",
            "svg": vis_host_graph_federation(animated=True),
            "anim_caps": [
                "Step 1/4：L1 实线硬依赖 (0.85~1.00)：Systemd → 进程 → 端口，零误报基石",
                "Step 2/4：L2 虚线结构上卷 (0.55~0.84)：4,985 文件引用汇聚成宏观工程拓扑",
                "Step 3/4：L3 语义点线 (cos>=0.78, Top-3)：防稠密化，默认不计入强爆炸半径",
                "Step 4/4：右侧 CodeGraph 显微镜接管微观 AST 函数图，DQ6=100% / DQ3=99.5%",
            ],
            "next": "感官与图谱就绪，进入执行回路最核心机制：AOPCC 自适应并发控制。",
            "script": (
                "Phase 40 之前 215 个工程节点有 214 个孤立；重构引入 L1 物理硬依赖、L2 文件引用上卷与同体桥接、L3 稀疏语义近邻三层图谱后，216 个工程 100% 连通。\n"
                "爆炸半径按 0.8 跳数指数衰减，3 跳推演 148 微秒，微观 AST 下钻无缝桥接 CodeGraph。"
            ),
        },
        # 07 并发控制 · AOPCC 高速 ETC 立交与碰壁自愈
        {
            "eyebrow": "08 · CONCURRENCY · AOPCC MATH & SELF-HEAL",
            "title": "并发控制：AOPCC 期望代价立交 × <2ms 碰壁秒愈环",
            "lead": "p·C_turn > (1-p)·C_fail → <b>ETC 乐观直飞完胜全悲观</b>",
            "svg": vis_aopcc_highway(animated=True),
            "anim_caps": [
                "Step 1/4：量化整轮对话税 C_turn (2~4s + 10k tok)，划定乐观直通严格数学边界",
                "Step 2/4：SessionStart 签发 <=120 tok MVCC 租约，三态分类器精准分流",
                "Step 3/4：客户端直连走 ETC 绿道 0 额外轮，bind / 高危写死守红灯悲观铁闸",
                "Step 4/4：偶发碰壁 2ms 内 PostToolUse 反查宿主注入替代处方并降级租约",
            ],
            "next": "单 Agent 回路用 AOPCC 加速，多 Agent 并发如何防踩踏与破死锁？",
            "script": (
                "即便本地查端口只要 0.2ms，大模型多走一轮对话就要付 2~4 秒和上万 Token。对于可逆且成功率 >95% 的客户端连接，全悲观在数学上必亏。\n"
                "AOPCC 用 MVCC 乐观租约放行 ETC 绿道，用 1ms Pre-Hook 守死红灯高危写，用 <2ms Post-Hook 兜底偶发碰壁，W15 实测省 Token 83.9%、加速 3.22x。"
            ),
        },
        # 08 多 Agent 协作锁 · 目录树传播 + WFG 死锁破环
        {
            "eyebrow": "09 · COORDINATION · HIERARCHICAL LOCKS & WFG",
            "title": "多 Agent 协作锁：目录树层级传播 × WFG 死锁闪电破环",
            "lead": "5 类资源 · 共享/排他模式 · <b>W6 破局加速 46.49x · 零轮询唤醒</b>",
            "svg": vis_lock_and_wfg(animated=True),
            "anim_caps": [
                "Step 1/3：左侧目录树锁——锁住 src/ 排他锁自动向下阻断 lib.rs/main.rs，docs/ 共享读并行",
                "Step 2/3：右侧 Wait-For Graph——瞬时检测 A→R2→B→R1→A 红色死锁环并闪电破环",
                "Step 3/3：Wait-Queue 推送唤醒 + TTL 自动回收僵尸锁，PG 后端联动 Advisory Lock",
            ],
            "next": "多 Agent 互斥稳了，再看第四器官：规约脊柱如何防目录失控与 SKILL 爆炸。",
            "script": (
                "多智能体同机并发最怕覆盖源码或互等死锁。左边树图展示父子目录递归互斥传播，右边环图展示 WFG 死锁环路检测：一旦成环瞬间拒绝并给出破局建议，释放时由 Wait-Queue 推送唤醒，W6 加速 46.49 倍。"
            ),
        },
        # 09 规约脊柱 · 防爆压缩与 41μs 放置柱状图
        {
            "eyebrow": "10 · GOVERNANCE SPINE · PLACEMENT & ROUTER",
            "title": "规约脊柱：百级 SKILL 609k → 358t 防爆 × 41 μs 放置自演化",
            "lead": "PlacementLearner 2.0 纠偏聚类 · <b>W7 信息密度跃升 838x</b>",
            "svg": svg_bars(
                title_left=f"规约与预算路由核心指标（Benchmark v{BENCH_VER} 真机实测）",
                rows=[
                    ("W7 百级 SKILL / MCP 预算路由 (609k→358t)", 99.9, "-99.9%", "密度 838.0x", C_TEAL_D),
                    ("W1 宿主工具能力语义路由 (21k→119t)", 99.4, "-99.4%", "加速 565.2x", C_TEAL),
                    ("W3 分级黑名单防爆阻断 (534 噪音)", 95.8, "-95.8%", "10/10 事实", C_GREEN),
                    ("W4 8 类资产放置建议 (质量 16.5→100)", 73.1, "-73.1%", "M3 41 μs", C_CLAY),
                ],
                footer_text="PlacementEngineV2 (41μs) + PlacementLearner 捕获主人搬移纠偏聚类新规约 + ContextBudgetRouter 防爆",
            ),
            "next": "进入海马体器官：先看可插拔 Memory SPI 与 RRF 混合检索。",
            "script": (
                "规约脊柱用两招管住秩序与上下文：一是 41 微秒放置引擎配纠偏自学习器，主人手动挪文件它自动归纳新习惯；二是 Context 防爆路由器按意图 Top-K 披露，把 130+ SKILL 的 60.9 万 Token 压到 358 Token。"
            ),
        },
        # 10 海马体① · 可插拔记忆 SPI 与 RRF 流水线
        {
            "eyebrow": "11 · HIPPOCAMPUS I · MEMORY SPI & RRF",
            "title": "海马体①：可插拔 Memory SPI × FTS5 + 向量 RRF 融合",
            "lead": "内置零依赖 335 μs · <b>一行配置热插 Mem0 / Letta / PowerContext</b>",
            "svg": svg_pipeline(
                steps=[
                    {"tag": "INTAKE", "title": "脱敏准入门禁", "icon": "shield", "accent": C_CLAY, "lines": ["过滤噪音 · 强制脱敏", "同路径幂等归档"]},
                    {"tag": "SPI", "title": "可插拔门面", "icon": "plug", "accent": C_TEAL_D, "lines": ["Builtin 零依赖默认", "热插 Mem0 / Letta"]},
                    {"tag": "SEARCH", "title": "RRF 混合召回", "icon": "microscope", "accent": C_TEAL, "lines": ["FTS5 BM25 + 向量", "M2 P50 仅 335 μs"]},
                    {"tag": "LIFECYCLE", "title": "半衰与仲裁", "icon": "clock", "accent": C_GREEN, "lines": ["时间衰减 + 矛盾仲裁", "40% 简报记忆预算"]},
                ],
                bottom_panels=[
                    {
                        "title": "RRF 倒数排名融合 (工程场景双杀)",
                        "icon": "graph",
                        "accent": C_TEAL,
                        "lines": [
                            "精确错误码/端口靠 FTS5，语义改写靠本地向量",
                            ("RRF(d) = 1/(60+rank_fts) + 1/(60+rank_vec)", True),
                        ],
                    },
                    {
                        "title": "本地向量覆盖与零锁定生态",
                        "icon": "database",
                        "accent": C_GREEN,
                        "lines": [
                            "DQ2 Ollama 向量覆盖 99.5% · 外部挂掉自动降级",
                            ("src/memory/{spi,builtin_provider,lifecycle}.rs", True),
                        ],
                    },
                ],
                mid="tech_mem",
            ),
            "next": "存得进、搜得到只是起点，真正的硬骨头是文件改删后如何防止记忆发霉。",
            "script": (
                "记忆层坚持开放 SPI 与混合召回：工程排障既有精确报错码又有自然语言改写，FTS5 加本地向量的 RRF 倒数排名融合在 W9 拿下满分 100；同时外部记忆服务不可用时无缝回落内置 SQLite。"
            ),
        },
        # 11 海马体② · 七大陈旧漏斗与写穿净水器
        {
            "eyebrow": "12 · HIPPOCAMPUS II · FRESHNESS PURIFIER",
            "title": "海马体②：三级写穿净水引擎，彻底根治七大陈旧漏斗",
            "lead": "DQ5 抽检 589/589 = 100.0% · <b>W12 Token -91.2% · 质量 27.5 → 100</b>",
            "svg": vis_freshness_purifier(animated=True),
            "anim_caps": [
                "Step 1/3：左侧七大发霉源——文件改后旧向量、删档僵尸、断链与重复记忆堆积",
                "Step 2/3：中央三级净水滤芯——content_hash 标 STALE → ChangeHandler 级联冲刷 → 现实对账",
                "Step 3/3：右侧出水口实测——589 项现网鲜活度 100.0%、向量覆盖 99.5%、0 僵尸召回",
            ],
            "next": "单机保鲜 100% 后，看换脑中断时如何用 <=300 tok 发光胶囊 10ms 接棒。",
            "script": (
                "看这台三滤芯净水器：左边的改档旧向量、删档 FTS 僵尸、重复记忆一进来，先被哈希探针打标 STALE，再经 ChangeHandler 跨表级联冲洗，最后由 FreshnessReconciler 周期对账磁盘真值。现网 589 项全部 100% 鲜活。"
            ),
        },
        # 12 跨大脑连续性 · 发光胶囊接力图
        {
            "eyebrow": "13 · CONTINUITY · CROSS-BRAIN CAPSULE",
            "title": "跨大脑连续性：BodyManifest × <=300 tok 脱敏接力胶囊",
            "lead": "Brain-Agnostic · <b>W17 Token -92.7% · 10ms 无缝唤醒续跑</b>",
            "svg": vis_handoff_capsule(animated=True),
            "anim_caps": [
                "Step 1/2：50,000 tok 历史垃圾山 vs 150 tok 四芯发光胶囊（目标/变更/红线/脱敏锁）",
                "Step 2/2：大脑 A 封存进 body_checkpoints 三引擎表 → 大脑 B 10ms 着陆继承租约续跑",
            ],
            "next": "跨大脑状态连续打通后，看如何用四级硬法律包络与五维体征雷达管住自治边界。",
            "script": (
                "跨大脑切换的痛点在于聊天记录太脏太重。我们把工作态抽炼成 300 Token 以内的脱敏抗投毒胶囊，落盘三引擎统一的 body_checkpoints 表。换到任意新客户端，10 毫秒还原现场目标、已改文件和活跃租约。"
            ),
        },
        # 13 自治边界 · 五维生命体征雷达 + L0~L3 同心圆环
        {
            "eyebrow": "14 · AUTONOMY & HOMEOSTASIS VITALS",
            "title": "自治边界：五维生命体征几何雷达 × L0~L3 同心权限包络",
            "lead": "现网 DQ8 稳态分 93.9 · <b>L1 自主自愈 · L3 物理禁区 100% 拦截</b>",
            "svg": vis_vitals_radar_autonomy(animated=True),
            "anim_caps": [
                "Step 1/2：左侧五边形生命体征雷达——新鲜度/连通度/向量/遥测/脱敏实时综合 98.7",
                "Step 2/2：右侧四重同心权限包络——L0 绿心只读 → L1 自愈 → L2 审批 → L3 红圈禁区",
            ],
            "next": "权限边界刻进类型系统后，看覆盖采集/存储/Hook/Web/Mesh 的纵深安全防线。",
            "script": (
                "左边五维雷达图实时给身体打分，一旦跌破阈值触发可逆自愈；右边四重同心圆环把「身体能做什么」刻成 Rust 类型硬法律：L0 永不阻塞，L1 自主自愈且留痕，L2 默认 dry-run 需授权，L3 碰 /etc/shadow 或 ~/.ssh 100% 物理拦截。"
            ),
        },
        # 14 纵深安全与双域审计 · 四道盾牌 + 折叠漏斗
        {
            "eyebrow": "15 · DEFENSE-IN-DEPTH & DUAL AUDIT",
            "title": "纵深安全：四道铁闸零泄露 × 抗间接提示词投毒",
            "lead": "DQ4 101/101 零明文残留 · <b>W11 抗投毒质量 21.5 → 100 (+78.5)</b>",
            "svg": vis_security_dual_audit(animated=True),
            "anim_caps": [
                "Step 1/2：左侧四道纵深安全铁闸——全链路脱敏、抗提示词投毒、POSIX 防注入、0600 落盘",
                "Step 2/2：右侧双域监控同步留痕，所有安全防线纳入 W11 / W14 / DQ4 自动化回归门禁",
            ],
            "next": "单机安全铁桶一块后，看多台宿主机如何通过零信任 MeshEnvelope v1.0 协同。",
            "script": (
                "安全绝不是事后补丁。我们针对审计缺口构建了脱敏、抗投毒、防命令逃逸和最小权限四道铁闸，全部纳入 W11、W14 和 DQ4 基准回归，任何退化直接阻断发版。"
            ),
        },
        # 15 多体联邦 · MeshEnvelope v1.0 零信任拓扑图
        {
            "eyebrow": "16 · FEDERATED MESH · MESHENVELOPE v1.0",
            "title": "多体联邦：HMAC-SHA256 防重放信封 × 10MB 沙盒收件箱",
            "lead": "拒绝重型 Raft / Rsync · <b>W14 篡改重放 100% 拦截 · Q 12.5 → 100</b>",
            "svg": vis_mesh_network(animated=True),
            "anim_caps": [
                "Step 1/2：开发机 / GPU 算力机 / 跳板机各自独立自治，零分布式共识运维负担",
                "Step 2/2：跨机通信统一封装 HMAC-SHA256 + ±300s Nonce 防重放信封，文件只落沙盒收件箱",
            ],
            "next": "单机与多机安全收口后，看双域全链路审计如何消灭后台黑盒与保存洪泛。",
            "script": (
                "多主机协同我们拒绝造 Raft 共识。每台宿主机自治独立，跨机只发带 HMAC-SHA256 签名、正负 300 秒时间窗、Nonce 防重放和 Peer Scopes 白名单的轻量信封；跨机传文件硬限 10MB 且强制落 0600 沙盒收件箱，杜绝越权直写。"
            ),
        },
        # 16 双域全链路审计 · 图标三支柱
        {
            "eyebrow": "17 · DUAL-DOMAIN AUDIT · IN + OUT",
            "title": "双域审计：接入域 + 域外自治 100% 留痕与 60s 折叠防洪",
            "lead": "根治 94.7% 编辑器保存洪泛 · <b>DQ7 500/500 零泄露 · <80 μs</b>",
            "svg": svg_columns(
                [
                    {
                        "tag": "DOMAIN A · IN_AGENT",
                        "title": "Agent 接入全通道",
                        "icon": "brain",
                        "metric": "<80μs",
                        "accent": C_TEAL_D,
                        "lines": [
                            "Brief 租约与 Pre/Post 裁决",
                            f"{TOOL_CNT} MCP / CLI / Web / Mesh",
                            ("零阻塞异步降级记账", True),
                        ],
                        "foot_val": "trace_id / session_id",
                    },
                    {
                        "tag": "DOMAIN B · OUT_OF_AGENT",
                        "title": "后台守护自治审计",
                        "icon": "filter",
                        "metric": "10→1",
                        "accent": C_CLAY,
                        "lines": [
                            "30m 巡检 · GC · 断链修复",
                            "Watcher 60s 折叠防洪",
                            ("彻底消灭后台静默黑盒", True),
                        ],
                        "foot_val": "REACTIVE_WATCHER",
                    },
                    {
                        "tag": "PROVENANCE · OLAP",
                        "title": "SHA-256 存证 + DuckDB",
                        "icon": "audit",
                        "metric": "100%",
                        "accent": C_GREEN,
                        "lines": [
                            "<=4KB 脱敏前后 Delta",
                            "单事务增量同步 DuckDB",
                            ("DQ7 500/500 完整零漏", True),
                        ],
                        "foot_val": "body_audit_query",
                    },
                ],
                bottom_banner=(
                    "底层 I/O 变更流与高价值行为审计彻底解耦，W16 实测质量 36.0 → 100.0。",
                    "DQ7 = 100.0%",
                    "teal",
                ),
            ),
            "next": "审计账本不仅用来事后查案，更直接驱动 P1~P5 探针自进化飞轮。",
            "script": (
                "以前 changes 表 94.7% 被编辑器保存洪泛淹没，后台巡检和 GC 又是 0 审计。Phase 43 拆清双域并加上 60 秒折叠防洪后，单条落盘不到 80 微秒，附带 SHA-256 指纹直通 DuckDB 列存分析。"
            ),
        },
        # 17 闭环自进化 · P1~P5 环形飞轮图
        {
            "eyebrow": "18 · SELF-EVOLUTION · P1~P5 FLYWHEEL",
            "title": "闭环自进化：P1~P5 五点对比探针驱动的数据调优飞轮",
            "lead": "严禁拍脑袋调参 · <b>实时消费 24h 审计滑窗动态升降级 AOPCC 租约</b>",
            "svg": vis_evolution_flywheel(animated=True),
            "anim_caps": [
                "Step 1/3：中心自进化引擎持续吞吐物理真值与历史快照，输出 Before/After Delta",
                "Step 2/3：外环 P1~P5 五点探针覆盖事实漂移、召回退化、租约碰壁、规则摩擦与遥测",
                "Step 3/3：右侧四大自动动作——升降级租约、聚类新规约、去重反馈、量化存证",
            ],
            "next": "业务效果有了自进化飞轮，那本体自身内核性能如何观测？看数据库级 Wait Events。",
            "script": (
                "把双域审计和外环 P1 到 P5 五个探针连起来，才真正构成自进化飞轮。上线首轮 P3 探针就自动捕获并修复了现网 3 条 stale 向量；同时每 24 小时按审计滑窗动态升降级 AOPCC 租约，让系统越用越稳。"
            ),
        },
        # 18 内生性能工程① · 无锁 Wait Events 代码-机制联动图
        {
            "eyebrow": "19 · BUILT-IN PERF I · WAIT EVENTS (<100ns)",
            "title": "内生性能工程①：无锁 6 类 18 等待事件 × /proc/self 探针",
            "lead": "零堆分配 RAII 埋点 (&lt;100ns) · <b>单二进制零 eBPF / OTel 依赖</b>",
            "svg": svg_code_and_flow(
                left_title="无锁等待事件直方图 (src/body/perf_awr/)",
                file_ref="ISSUE #32 · Phase 50",
                code_lines=[
                    "// 6 大类 18 种子事件: Lock / IO / Net / CPU / IPC / Idle",
                    "pub struct WaitEventSlot {",
                    "  calls: AtomicU64, total_us: AtomicU64, max_us: AtomicU64,",
                    "  buckets: [AtomicU64; 8], // 50us .. >500ms 对数分位桶",
                    "}",
                    "// RAII 埋点: 零堆分配, 单次开销 <100 ns",
                    "let _g = WaitEventGuard::enter(WaitEventId::LockSqliteMutex);",
                    "// 零特权 /proc/self 探针: VmRSS / VmHWM / FDs / ctxt_switches",
                ],
                right_title="BODY TIME MODEL & ZERO OVERHEAD",
                right_cards=[
                    ("Body Time 正交分解", "CPU / Lock / IO / Net / IPC 一眼定位真瓶颈", "杜绝把 SQLite 锁争用误判成 Ollama 慢"),
                    ("操作级 P50 / P95 / P99", "对标 pg_stat_statements 聚合高频工具延迟", "18/18 WaitEventId 全覆盖 (hygiene 强检)"),
                    ("零特权单二进制移植", "无需 root / libbpf，非特权容器直接开箱生效", "M6 无锁探针实测 P50 仅 7 μs"),
                ],
                animated=True,
            ),
            "anim_caps": [
                "Step 1/4：左侧无锁 AtomicU64 8 档对数分位桶 + RAII Guard 实现 <100ns 零分配埋点",
                "Step 2/4：Body Time 百分比模型将耗时正交分解，告别盲猜排障",
                "Step 3/4：操作级分位统计直连 SQLite / LinksEngine / Reconciler",
                "Step 4/4：零特权直读 /proc/self 获取 VmRSS 与 FD，兼容任意容器环境",
            ],
            "next": "看 AWR 黄金基线、ADDM 自调优以及 600% CPU 生产事故的真实压降曲线。",
            "script": (
                "左边短短 8 行代码就是核心机密：纯 AtomicU64 加 8 档对数桶，RAII 埋点不到 100 纳秒、零堆分配，配合零特权 /proc/self 探针，把数据库内核的等待事件模型原生搬进了单二进制本体。"
            ),
        },
        # 19 内生性能工程② · AWR 仪表盘与 600% CPU 根治曲线
        {
            "eyebrow": "20 · BUILT-IN PERF II · AWR & 600% CPU POSTMORTEM",
            "title": "内生性能工程②：AWR 黄金基线 × 600% CPU 事故压降曲线",
            "lead": "32 份滚动快照 · <b>ADDM 5 规则自动定位 · L1 一键内存回收</b>",
            "svg": vis_awr_telemetry(animated=True),
            "anim_caps": [
                "Step 1/2：左侧 6 类 18 等待事件占比条 + /proc/self + Prometheus /metrics 遥测底座",
                "Step 2/2：右侧真实事故复盘——DuckDB 8 线程+3.5万单事务导致 600% CPU，三层根治压回 <2%",
            ],
            "next": f"六大器官与内核性能工程全景看完，进入 Benchmark v{BENCH_VER} 实证评测。",
            "script": (
                "看右边这条真实复盘曲线：Phase 48 曾抓到 Web 页触发 DuckDB 8 线程和 3.5 万次独立事务把 CPU 打爆到 600%、内存 1.3G。三层收敛后常驻降到 2% 以下，并把整套体检固化成 AWR 黄金基线与 ADDM 一键 L1 自调优。"
            ),
        },
        # 20 实证评测① · Benchmark v4.1 帕累托二维象限
        {
            "eyebrow": f"21 · BENCHMARK v{BENCH_VER} · PARETO DUAL-GATE",
            "title": f"实证评测①：Benchmark v{BENCH_VER} 帕累托跃迁与微秒 SLA",
            "lead": f"Q = 40%事实 + 30%信噪 + 20%行动 + 10%安全 · <b>W1~W{WL_CNT} 全赢</b>",
            "svg": vis_benchmark_pareto(animated=True),
            "anim_caps": [
                "Step 1/3：二维帕累托坐标——从裸奔 35.6 分跃迁至本体 98.0 分，拒绝伪省 Token 失真区",
                f"Step 2/3：三大实证圆环——Token 省 99.23%、质量 +64.2 分、W1~W{WL_CNT} + DQ1~{DQ_CNT} PASS",
                "Step 3/3：内核微秒级 SLA——M6 7μs / M3 33μs / M1 139μs / M2 259μs 全远低红线",
            ],
            "next": "看完自对照与生产存证，再看四大赛道开源竞品的同机真跑擂台。",
            "script": (
                f"我们用确定的四维质量函数守住帕累托双门禁：{WL_CNT} 个业务负载全部实现省 Token 与质量双赢，总体 Token 省 99.23%，信息密度提 94.5 倍，{MICRO_CNT} 大内核微基准全部微秒级通过，PG 18.6 M3 百万事件流 P99 4.42 ms。"
            ),
        },
        # 21 实证评测② · 四大赛道同机 VS 擂台
        {
            "eyebrow": "22 · COMPETITORS · 4-TRACK LIVE ARENA",
            "title": "实证评测②：四赛道同机对标擂台与开放生态边界",
            "lead": "严格区分 LIVE_BENCHMARKED 与 ARCH_VERIFIED · <b>不捏造、不拉踩</b>",
            "svg": vis_competitor_arena(animated=True),
            "anim_caps": [
                "Step 1/4：C1 长期记忆——0 明文泄露 (-97.8% tok)，与 Mem0/Letta SPI 兼容拥抱",
                "Step 2/4：C2 上下文路由——零副本搬迁 931 vs 144,730 tok (-99.4%)",
                "Step 3/4：C3 文件治理——黑白灰分级阻断 4 类黑名单目录 (-96.5% tok)",
                "Step 4/4：C4 宿主探针——0 子进程 20ms 快 4.4x，多出跨服务 3 跳上游影响图",
            ],
            "next": "进入 Phase 70~75 最新架构跃迁：PostgreSQL 式微内核与四大发行版。",
            "script": (
                "四条赛道同机实跑的结果用红绿比例柱直观呈现：我们在物理具身、零泄露脱敏和防爆路由上拉开数量级优势，同时对记忆框架通过 SPI 兼容拥抱，对微观 AST 通过 CodeGraph 联邦协作。"
            ),
        },
        # 22 微内核与周边分离架构 · 星型插槽与四发行版积木塔
        {
            "eyebrow": "23 · MICROKERNEL & CONTRIB · PHASE 70~75",
            "title": f"微内核与周边分离：{KERNEL_CNT} 核 × {CONTRIB_CNT} 扩展 × 4 大发行版",
            "lead": "kernel/ 0 业务词污染 · <b>contrib/ 五件套隔离 · Probe 6.7MB 零 C++</b>",
            "svg": vis_microkernel_hub(animated=True),
            "anim_caps": [
                f"Step 1/3：中心六边形微内核（{KERNEL_CNT} Crates · 8 Catalog · 6 Routine · 7 Hook · ScopedSql）",
                f"Step 2/3：{CONTRIB_CNT} 个标准五件套周边扩展通过双轨沙盒热插拔挂载",
                "Step 3/3：右侧四座发行版积木塔——旗舰全量兼容 + Probe 6.7M / DBA 6.4M / Minimal 3.9M",
            ],
            "next": "内核边界拆清后，看这套历经 75 个 Phase 的大仓如何用机器门禁保持零腐烂。",
            "script": (
                f"Phase 70 到 75 我们把纯机制收进 {KERNEL_CNT} 个微内核 crate，把 OS/中间件/国产库/mTLS/SIEM/CMDB/可观测遥测全拆成 {CONTRIB_CNT} 个五件套扩展。\n"
                "同一套底座既保住旗舰版 100% 兼容，又直接编译出 6.7MB 零 libc++、零 DuckDB 的政务云探针。"
            ),
        },
        # 23 工程化实践 · 7 道确定性机器门禁
        {
            "eyebrow": "24 · ENGINEERING EXCELLENCE · QUALITY GATES",
            "title": "工程化实践：零重扫演进 × 7 道确定性质量门禁流水线",
            "lead": f"A~G 交叉防漂移 · <b>{TEST_CNT} 测试全绿 · Playwright 9/9 像素门禁</b>",
            "svg": vis_quality_gates(test_cnt=TEST_CNT, mod_cnt=MOD_CNT, tool_cnt=TOOL_CNT, animated=True),
            "anim_caps": [
                f"Step 1/2：7 道自动化门禁锁死零漂移、零内核污染、零 Panic、零>1000L 文件与 {TEST_CNT} 全测",
                "Step 2/2：6.7MB 精简探针 + 100% 位置无关交付 + 2 轮精准锁触点的零重扫体系",
            ],
            "next": "倒数第二页，重申跨平台位置无关交付与递弱代偿最终红线。",
            "script": (
                "AI 连续演进 75 个 Phase 还不腐烂的秘诀，就是把所有工程纪律写进机器门禁。从文档数字漂移、微内核边界、零 unwrap 到连 PPT 都要过 Playwright 防重叠测试，全部自动化守卫。"
            ),
        },
        # 24 边界与克制 · 递弱代偿红线墙
        {
            "eyebrow": "25 · PORTABILITY & ANTI-CREEP BOUNDARY",
            "title": "递弱代偿红线墙：只做最纯粹的具身微内核底座",
            "lead": "100% 位置无关交付 · <b>三层命名零破坏承诺 · 绝不沦为重框架</b>",
            "svg": vis_do_dont_wall(animated=True),
            "anim_caps": [
                "Step 1/3：左侧深耕护城河——物理感官、反射门禁、保鲜记忆、双域 AWR 与轻量联邦",
                "Step 2/3：中央递弱代偿红线——保持极简克制，微内核 0 业务污染",
                "Step 3/3：右侧绝不越界——不抢大脑推理、不替代编排器、不重造沙箱与共识",
            ],
            "next": "最后一页，总结终局愿景与 6 讲 × 30 分钟专题系列入口。",
            "script": (
                f"全工作区已实现 100% 位置无关交付，搬到任意目录或容器 {TEST_CNT} 项测试全绿。我们同时把「绝不做什么」刻成边界红线，把创造力留给大脑，把确定性留给身体。"
            ),
        },
        # 25 终局与专题系列入口
        {
            "eyebrow": "26 · EPILOGUE · OS EMBODIED SUBSTRATE",
            "title": "终局：从宿主机单二进制走向模块化 OS 原生具身底座",
            "lead": "今天开箱即用 · <b>明天成为智能体时代操作系统的底层原语</b>",
            "svg": svg_closing(
                kicker="FROM HOST SUBSTRATE TO AGENT-NATIVE OS PRIMITIVE",
                headline="大脑会月月迭代，但耳聪目明、稳健可溯的身体永远属于你",
                pillars=[
                    ("6 讲 × 30 分钟专题", "presentation/index.html", "哲学 / 感官 / 小脑 / 海马体 / 安全 / 实证"),
                    ("一键编译与质量门禁", "make quality-gate && make test", f"旗舰 + probe 6.7M · {TEST_CNT} 测试全绿"),
                    ("真机基准与 Web 指挥舱", "agent-body bench run --suite full", "agent-body web --port 8765"),
                ],
                open_question="欢迎深入 6 讲体系专题（index.html）或直接克隆仓库 2 分钟独立复现全部指标！",
                next_talk="返回总览导航：presentation/index.html · 对外路演版：presentation/presentation.html",
            ),
            "script": (
                "非常感谢各位耐心听完这 27 页全图解技术深度交流！如果对某个器官的源码实现感兴趣，可以点击底部导航回到 index.html，进入 6 讲 × 30 分钟专题逐一拆透。欢迎拍砖与共建！"
            ),
        },
    ]

    render_deck(
        doc_title=f"Agent Body · Agent 本体深度技术交流版 (v{APP_VER})",
        brand_label="AGENT BODY · 深度技术交流版",
        foot_label=f"AGENT BODY · DEEP TECHNICAL ARCHITECTURE · v{APP_VER}",
        slides_data=slides,
        out_path=PRES_DIR / "agent-body-deck.html",
    )
    print(f"✅ Generated {PRES_DIR / 'agent-body-deck.html'} ({len(slides)} slides)")


if __name__ == "__main__":
    build_tech_deck()
