#!/usr/bin/env python3
"""Build presentation/agent-body-deck.html (24-slide Deep Technical Exchange Deck).

Decks-as-Code generator following the '地层剖面 · 工程蓝图' visual system.
Grounded 100% in Agent Body v1.0.0 GA source code, live Benchmark v3.8,
and live AWR telemetry.
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from deck_engine import (
    C_CLAY,
    C_CLAY_L,
    C_GOLD,
    C_GOLD_L,
    C_GREEN,
    C_GREEN_L,
    C_INK,
    C_INK2,
    C_LINE,
    C_MUTE,
    C_PAPER,
    C_PAPER2,
    C_TEAL,
    C_TEAL_2,
    C_TEAL_3,
    C_TEAL_D,
    C_TEAL_L,
    C_WHITE,
    render_deck,
    svg_bars,
    svg_closing,
    svg_code_and_flow,
    svg_columns,
    svg_compare_rows,
    svg_cover,
    svg_matrix_2x2,
    svg_metrics_table,
    svg_pipeline,
)

ROOT = Path(__file__).resolve().parent.parent
PRES_DIR = ROOT / "presentation"


def build_tech_deck() -> None:
    slides = [
        # 01 封面
        {
            "cover": True,
            "eyebrow": "封面 · 深度技术架构全景",
            "title": "Agent Body · 深度技术架构与内核工程全景",
            "svg": svg_cover(
                kicker="AGENT BODY · DEEP TECHNICAL EXCHANGE · v1.0.0",
                title="Agent 本体技术全景",
                en_sub="27 Modules · 43 MCP · 88 SPI · 27 Tables · Bench v3.8 · 18 Wait Events",
                subtitle1="从「比较优势」到「脑体分离」：面向数据库与 AI Infra 工程师的深度拆解",
                subtitle2="物理感官 × 分层图谱 × AOPCC 并发 × 写穿保鲜 × 纵深安全 × 双域审计 × 内生 AWR",
                spec_line="Rust 1.99 单二进制 · SQLite / PG / DuckDB 三引擎 · 320+ 测试 · 五重质量门禁",
                strata=[
                    ("接入与协议层 · Interface", "43 原生 MCP · 30 CLI · Session/Pre/Post Hooks · Web 9 工作台", "#EBF1EE", C_INK),
                    ("宿主机感官与图谱 · Fact/Links", "/proc · Socket · L1/L2/L3 有向加权图 · CodeGraph 联邦", "#CFE0DC", C_INK),
                    ("小脑并发与锁 · AOPCC/Locks", "MVCC 乐观租约 · 三态分级 · <2ms 自愈 · WFG 死锁检测", "#A4C4C6", C_INK),
                    ("海马体与接力 · Memory/Body", "FTS5+向量 RRF · 7 大新鲜度漏斗治理 · <=300 tok 接力胶囊", "#74A5AD", C_PAPER),
                    ("安全审计与自进化 · Audit/Probe", "SecretRedactor · 抗投毒 · IN/OUT 双域审计 · P1~P5 探针", "#47808D", C_PAPER),
                    ("内生性能工程 · Wait/AWR/ADDM", "6 类 18 事件 (<100ns) · /proc/self · 32 快照黄金基线 · L1 调优", "#2E6373", C_PAPER),
                    ("存储 SPI 与联邦 · Storage/Mesh", "88 SPI 三后端热迁移 · DuckDB OLAP · MeshEnvelope v1.0", "#1E4B5A", C_PAPER),
                ],
                right_tag="地层剖面 · 工程蓝图 · 深度技术交流版 (24 页)",
            ),
            "script": (
                "各位技术同行好，这份 24 页的 Deck 是 Agent Body 的完整架构深度交流版，大约 45 到 60 分钟。\n"
                "与面向路演的 16 页版本不同，今天我们不讲空泛的概念，而是直接下钻到 27 个 Rust 模块、88 个存储 SPI、27 张数据表、AOPCC 期望代价数学边界、MeshEnvelope 签名防重放协议，以及借鉴 PostgreSQL/Oracle 内核落地的等待事件与 AWR 性能工程。\n"
                "所有架构指标和实测数据全部直接取自当前代码库和现网宿主机，欢迎随时打断就任何设计取舍展开硬核讨论。"
            ),
        },
        # 02 第一性原理：从比较优势到脑体分离
        {
            "eyebrow": "01 · FIRST PRINCIPLES · BRAIN-BODY SEPARATION",
            "title": "第一性原理：为什么把持久化状态窄化为「记忆」注定撞墙",
            "lead": "当前 AI Infra 过度聚焦「对话历史向量化」，却忽略了宿主机上<b>实时物理态、规则约束、并发互斥与跨大脑状态交接</b>。",
            "svg": svg_compare_rows(
                headers=("系统工程维度", "纯记忆工具（传统范式）", "Agent Body（脑体分离范式）"),
                rows=[
                    ("事实真值来源", "依赖对话摘要与静态文档，极易过期失真", "直采 /proc / Socket / Systemd / 磁盘真值，零幻觉"),
                    ("执行回路控制", "无执行前门禁，写崩端口与上游服务才报错", "AOPCC 三态并发：悲观拦截高危 + 乐观直通免对话税"),
                    ("数据新鲜度", "文件改删后旧向量长期残留形成僵尸召回", "content_hash 写穿失效 + 删除级联净化 + 现实对账"),
                    ("多客户端切换", "记忆与会话绑定在单一 IDE/CLI 厂商壳内", "Brain-Agnostic：<=300 tok 脱敏胶囊 10ms 跨大脑接力"),
                    ("可观测与自演进", "黑盒运行，无性能归因与客观 ROI 存证", "双域审计 + P1~P5 探针 + 内生 AWR + Bench v3.8 存证"),
                ],
                takeaway=(
                    "核心判据：LLM 是无状态算力大脑，宿主机需要一层永驻的、确定性的具身操作系统抽象。",
                    "参见 docs/idea_doc/04-from-steward-to-agent-body.md",
                ),
            ),
            "next": "确立了脑体分离之后，整套系统的分层拓扑和存储底座是如何组织的？",
            "script": (
                "做系统架构最怕切错问题边界。过去一年开源社区涌现了大量 Agent Memory 项目，但它们大多把「持久化状态」窄化成了「聊天记录的语义检索」。\n"
                "在真实宿主机干活时，Agent 遇到的五大硬问题——物理事实真值、执行前并发防撞、文件改删后的向量腐烂、多 IDE 切换失忆、以及运维可观测性——没有一个是靠给聊天记录建向量索引能解决的。\n"
                "因此我们把抽象层下沉一级：LLM 是瞬态大脑，Agent Body 是永驻宿主机的具身底座。下面看全景分层架构。"
            ),
        },
        # 03 系统全景分层架构与 88 方法 Storage SPI
        {
            "eyebrow": "02 · SYSTEM ARCHITECTURE · 27 MODULES & 88-METHOD SPI",
            "title": "系统全景：四层拓扑与 88 方法三后端 Storage SPI",
            "lead": "严格分层解耦：接入层零业务逻辑，引擎层按六大器官内聚，存储层通过 <b>StorageProvider Trait (88 方法)</b> 无损热切换三引擎。",
            "svg": svg_pipeline(
                steps=[
                    {
                        "tag": "LAYER 1 · 接入层",
                        "title": "43/43 黑白屏同源",
                        "accent": C_TEAL_D,
                        "lines": [
                            "MCP stdio (43 body_*)",
                            "CLI Clap (30 顶层命令)",
                            "Hooks + Web HTTP/SSE",
                        ],
                    },
                    {
                        "tag": "LAYER 2 · 本体中枢",
                        "title": "六大器官引擎",
                        "accent": C_TEAL,
                        "lines": [
                            "fact / links / brief",
                            "coordination / placement",
                            "body / audit / feedback",
                        ],
                    },
                    {
                        "tag": "LAYER 3 · 统一门面",
                        "title": "StorageProvider SPI",
                        "accent": C_GOLD,
                        "lines": [
                            "88 个强类型语义方法",
                            "27 张规范化业务表",
                            "SyncPipeline 增量同步",
                        ],
                    },
                    {
                        "tag": "LAYER 4 · 三引擎后端",
                        "title": "SQLite / PG / DuckDB",
                        "accent": C_GREEN,
                        "lines": [
                            "SQLite WAL+FTS5 默认零配",
                            "PG pgvector + Advisory Lock",
                            "DuckDB 列存 OLAP + Parquet",
                        ],
                    },
                ],
                bottom_panels=[
                    {
                        "title": "为什么同时保留 SQLite / PostgreSQL / DuckDB？",
                        "accent": C_TEAL,
                        "lines": [
                            "SQLite (bundled FTS5)：单二进制开箱即用，个人开发机 0 运维成本，读写亚毫秒",
                            "PostgreSQL：多 Agent 高并发集群场景支持 pgvector、tsvector 与原生 Advisory Lock",
                            ("DuckDB：受控 threads=2 / memory_limit=256MB，专职审计/热力图 OLAP 与 Parquet 导出", True),
                        ],
                    },
                    {
                        "title": "关键工程纪律（src/lib.rs 27 个顶层模块）",
                        "accent": C_CLAY,
                        "lines": [
                            "src/api/ 严禁堆砌业务逻辑，仅做协议编解码、鉴权路由与审计埋点",
                            "Schema 变更强制 schema.sql / pg_schema.sql / duckdb_schema.sql 三端同步",
                            ("make evolution-check 实时校验 27模块 / 43MCP / 88SPI / 27表 0 drift", True),
                        ],
                    },
                ],
            ),
            "next": "看清了骨架，我们从第一器官「宿主机感官」开始，逐层解剖核心实现。",
            "script": (
                "这是 Agent Body 的四层全景骨架。最外层是接入层，43 个原生 MCP 工具、30 个顶层 CLI 命令、Hook 桥接脚本和内嵌 Web 服务全部复用同一套内核引擎。\n"
                "存储层采用 Facade 加 88 方法 StorageProvider SPI 设计，底层支持三套引擎无缝热迁移：默认是内嵌 SQLite WAL + FTS5，单二进制零配置即跑；多节点重度并发可切 PostgreSQL 利用原生 Advisory Lock 和 pgvector；分析报表和审计导出则由受限内存的 DuckDB 承担，并通过 SyncPipeline 单事务增量同步。"
            ),
        },
        # 04 感官层① · 零幻觉物理事实采集
        {
            "eyebrow": "03 · SENSORY I · GROUND-TRUTH HOST PROBES",
            "title": "感官层①：零幻觉物理事实采集与系统底噪过滤",
            "lead": "所有事实必须可指回物理内核出处（<code>/proc</code>、<code>/etc/systemd</code>、<code>$PATH</code>），并通过启发式规则滤除 95% 内核底噪。",
            "svg": svg_columns(
                [
                    {
                        "tag": "FACT 01 · 工具能力",
                        "title": "CapabilityEngine",
                        "accent": C_TEAL_D,
                        "lines": [
                            "扫描 $PATH 工具版本与路径",
                            "附带推荐调用参数与替代方案",
                            "如 pdftotext -layout 免 pip 折腾",
                            ("W1 实测：加速 565x / -99.4% tok", True),
                        ],
                        "foot_lbl": "源码位置",
                        "foot_val": "src/fact/capability.rs",
                    },
                    {
                        "tag": "FACT 02 · 进程与端口",
                        "title": "ProcessNetTopology",
                        "accent": C_TEAL,
                        "lines": [
                            "直读 /proc 映射 PID↔Port↔Cwd",
                            "过滤 kworker/systemd 内核底噪",
                            "折叠超长 node -e / python -c",
                            ("cmdline 强制经 SecretRedactor", True),
                        ],
                        "foot_lbl": "源码位置",
                        "foot_val": "src/fact/process_net.rs",
                    },
                    {
                        "tag": "FACT 03 · 自动化与资料",
                        "title": "Automation & Catalog",
                        "accent": C_GREEN,
                        "lines": [
                            "解析 systemd unit ExecStart/依赖",
                            "纳管 crontab 后台定时偷跑任务",
                            "编目 Git 仓库与 .codegraph 状态",
                            ("现网纳管：21,403 资产 / 81 进程", True),
                        ],
                        "foot_lbl": "源码位置",
                        "foot_val": "src/fact/{automation,assets_catalog}.rs",
                    },
                ],
                bottom_banner=(
                    "单一只读入口 body_fact_query / body_port_check：以索引优先（Index-First）替代每次暴力 fork 外部子进程。",
                    "GROUND REALITY · ZERO SIMULATION",
                    "teal",
                ),
            ),
            "next": "物理事实采上来之后，面对宿主机动辄数十万文件，如何避免把数据库扫爆并提取高价值语义？",
            "script": (
                "第一器官的底层是物理事实采集器。这里有两个非常实战的细节。\n"
                "第一是底噪过滤与脱敏：Linux 机器上随时跑着上百个 kworker、systemd 基础进程，还有带几千字符内联脚本的 node -e 或 python -c 进程，如果不加折叠和脱敏直接喂给大模型，既废 Token 又可能把命令行里的密码漏进上下文。我们在 /proc 采集层就完成了底噪剪枝、内联脚本折叠和凭据脱敏。\n"
                "第二是工具能力语义化：不止告诉大脑「有 pdftotext」，还告诉它「推荐用 -layout 参数，比装 pypdf 快十倍」，这正是 W1 能省 99.4% Token 的秘密。"
            ),
        },
        # 05 感官层② · 分级扫描与三层元数据双轨标注
        {
            "eyebrow": "04 · SENSORY II · TIERED SCANNING & DUAL-TRACK METADATA",
            "title": "感官层②：敲门不进屋与本地 Ollama 三层元数据标注",
            "lead": "白黑灰三级扫描阻断构建垃圾；<b>L1 Rust 微秒语法提取 × L2 本地小模型异步精炼</b>，零云端 Token 消灭空洞元数据。",
            "svg": svg_code_and_flow(
                left_title="三层正交元数据 Key 体系 (src/scanner/metadata.rs)",
                file_ref="L0 溯源 + L1 通用 + L2 特化",
                code_lines=[
                    "// L0 管线溯源（驱动写穿失效与增量重标）",
                    "content_hash, mtime_secs, embedding_status: \"ready\"|\"stale\"",
                    "annotation_status: \"l1_only\"|\"l2_enriched\"|\"needs_re_enrich\"",
                    "",
                    "// L1 通用语义核心（Rust 确定性微秒提取，0 网络依赖）",
                    "purpose, domain, capabilities[], entry_point, usage_hint",
                    "",
                    "// L2 类型特化 Facet（本地 Ollama qwen3.5:0.8b 脱敏异步精炼）",
                    "script:  { runtime, cli_args, side_effects }",
                    "project: { stack, build_cmd, has_codegraph }",
                    "doc:     { audience, key_decisions, freshness_tag }",
                    "",
                    "// 三级渐进披露：Micro (~25t) / Standard (~85t) / Full (~180t)",
                ],
                right_title="GRANULARITY TIERING RULES",
                right_cards=[
                    ("白名单目录（深度递归到底）", "/root/new/shared_proj/、/root/new/work/ 全量构建元数据、FTS5 与向量", "DQ1 元数据覆盖率 100.0% · DQ2 向量覆盖率 99.5%"),
                    ("黑名单目录（敲门不进屋）", "node_modules/、.git/、target/、.cache/、~/.ssh/ 仅记目录节点，严禁下钻", "FileWatcher 同步剪枝，消灭 7,825 个无效 inotify 句柄"),
                    ("受限目录（受控浅层索引）", "/root/new/src/ 克隆源码仓仅索引 README 与入口，微观代码交 CodeGraph", "W3 实测：单次阻断 534 个噪音文件，Token -95.8%"),
                ],
            ),
            "next": "每个实体都有了高质量元数据与向量，下面看它们如何织成一张「有证据分层、有方向权重」的事实图谱。",
            "script": (
                "解决「不扫爆库」靠的是白黑灰三级粒度法则。白名单业务目录深挖到底；黑名单如 node_modules、target、.git 和 .ssh 做到「敲门不进屋」——只登记目录自身存在，绝不递归子文件，连 inotify 监听都一并剪枝，防止文件句柄爆炸。\n"
                "而在元数据侧，我们设计了 L0/L1/L2 三层正交字典：L1 由 Rust 在微秒级做确定性语法提取，保证离线可用；L2 由本机 Ollama 跑 0.8B 小模型在脱敏后异步精炼，零云端 Token 消耗；读取时按预算提供 Micro、Standard、Full 三级渐进披露。"
            ),
        },
        # 06 图谱智能 · L1/L2/L3 证据分层与宏微联邦
        {
            "eyebrow": "05 · GRAPH INTELLIGENCE · L1/L2/L3 & CODEGRAPH FEDERATION",
            "title": "图谱智能：三层证据有向加权图与宏微双层联邦",
            "lead": "严格隔离<b>物理硬依赖 (L1)</b>、<b>结构上卷 (L2)</b> 与<b>隐式语义近邻 (L3)</b>，以跳数衰减公式精确推演 3 跳爆炸半径。",
            "svg": svg_code_and_flow(
                left_title="证据分层图谱与加权衰减推演 (src/fact/graph.rs & src/links/)",
                file_ref="ISSUE #23 · Phase 40",
                code_lines=[
                    "pub enum EvidenceTier {",
                    "  L1Explicit,   // 物理硬依赖: Systemd/Cron/Import  w: 0.85..=1.00",
                    "  L2Structural, // 文件上卷/服务同体桥接/工具链     w: 0.55..=0.84",
                    "  L3Semantic,   // 向量 cos>=0.78 + 标签混合 (Top-3) w: 0.35..=0.65",
                    "}",
                    "",
                    "// 3 跳爆炸半径风险衰减传播 (默认以 L1+L2 为强影响基准)",
                    "// S(path) = prod(w_e * 0.8^(hop - 1))",
                    "",
                    "// 现网验证 (DQ6 = 100.0% / M1 = 148 μs P50):",
                    "//   fact_edges: 776/776 证据分层合法 (100.0%)",
                    "//   project 连通率: 1/215 (0.5%) -> 216/216 (100.0%)",
                ],
                right_title="ANTI-FALSE-POSITIVE & FEDERATION",
                right_cards=[
                    ("Links→FactEdges 自动上卷 (Roll-up)", "将 4,985 条微观文件引用边自动聚合成宏观项目/服务拓扑边", "消除 99.5% 工程节点孤岛，同时保留微观出处"),
                    ("模板与测试夹具免疫（DQ3 = 99.5%）", "LinksEngine 自动豁免 #[cfg(test)]、占位符与 .md 示例代码路径", "根治 76.2% 假阳性断链，W10 Precision 100%"),
                    ("CodeGraph 宏微联邦（规约 #15）", "Agent Body 管宏观跨介质拓扑，符号/AST 调用下钻桥接 CodeGraph", "body_codegraph_bridge 同步核验 .codegraph.db mtime 滞后"),
                ],
            ),
            "next": "感官与图谱就绪后，进入最核心的执行回路优化：AOPCC 乐观-悲观自适应并发控制。",
            "script": (
                "Phase 40 之前我们检查现网库，发现一个尴尬事实：215 个工程节点里有 214 个是孤岛，因为底层五千条文件引用没有上卷到项目层，服务配置和运行进程也没桥接。\n"
                "于是我们落地了 L1/L2/L3 三层证据有向加权图谱：L1 放物理硬依赖，L2 做文件引用上卷和服务同体桥接，L3 用本地向量余弦大于等于 0.78 加标签挖掘跨项目隐式关联，并强制 Top-3 稀疏剪枝防止图稠密化。\n"
                "推演爆炸半径时，按跳数 0.8 指数衰减，默认用 L1+L2 做强影响面判据，绝不让语义相似边误报成物理故障。实测 216 个工程 100% 连通，3 跳查询 P50 仅 148 微秒。"
            ),
        },
        # 07 并发控制 · AOPCC 数学模型与碰壁秒级自愈（带动画）
        {
            "eyebrow": "06 · CONCURRENCY · AOPCC MATH & FAIL-FAST FALLBACK (INTERACTIVE)",
            "title": "并发控制：AOPCC 期望代价边界与 <2ms 碰壁自愈",
            "lead": "引入数据库并发控制第一性原理：当 <b>P(valid) · C_turn > (1 - P(valid)) · C_fail</b> 时，乐观直通严格优于全悲观前置查询。",
            "svg": svg_pipeline(
                steps=[
                    {
                        "tag": "MODEL · 期望代价",
                        "title": "整轮对话税 C_turn",
                        "accent": C_CLAY,
                        "lines": [
                            "悲观成本 = C_turn (2~4s+10k tok)",
                            "乐观成本 = (1-p)*(C_fail+C_heal)",
                            "p>=0.90 且可逆 → 乐观完胜",
                        ],
                    },
                    {
                        "tag": "LEASE · 租约签发",
                        "title": "Brief 签发 MVCC 租约",
                        "accent": C_TEAL_D,
                        "lines": [
                            "<=120 tok 紧凑直通契约",
                            "覆盖高置信 CLI / :5432 / tmp",
                            "绑定 epoch_hash 纪元指纹",
                        ],
                    },
                    {
                        "tag": "CLASSIFY · 三态分流",
                        "title": "客户端 vs 服务端",
                        "accent": C_TEAL,
                        "lines": [
                            "OPTIMISTIC: psql -p 5432 直连",
                            "CAUTIOUS_CAS: 常规新建文件",
                            "PESSIMISTIC: bind / 高危写 / 锁",
                        ],
                    },
                    {
                        "tag": "HEAL · 碰壁秒愈",
                        "title": "PostToolUse <2ms",
                        "accent": C_GREEN,
                        "lines": [
                            "捕获 ECONNREFUSED / ENOENT",
                            "反查宿主机注入替代端口/路径",
                            "自动降级该租约置信度防复发",
                        ],
                    },
                ],
                bottom_panels=[
                    {
                        "title": "核心实现触点（src/brief/ + src/hooks/）",
                        "accent": C_TEAL,
                        "lines": [
                            "OptimisticLeaseEngine：基于历史审计滑窗动态计算每类租约置信度 p_hat",
                            "ConcurrencyPolicyClassifier：精确区分 client connect 与 server bind 语义",
                            ("OptimisticFailureDiagnoser：捕获 4 大类典型错误并在 <2ms 返回结构化处方", True),
                        ],
                    },
                    {
                        "title": "W15 & PR-11 实证结果（Benchmark v3.8）",
                        "accent": C_GREEN,
                        "lines": [
                            "Token 消耗：174 → 28 tok (-83.9%) · 上下文质量：52.0 → 100.0 (+48.0)",
                            "加速比 3.22x，同时在生产账本按 PR-11 精确核定省去的整轮对话数 rounds_saved",
                            ("自进化闭环：若某服务近 24h 碰壁率飙升，自动从 OPTIMISTIC 降级为悲观校验", True),
                        ],
                    },
                ],
                animated=True,
                mid="ahtech_aopcc",
            ),
            "anim_caps": [
                "Step 1/5：建模整轮对话税 C_turn 与失败回滚代价 C_fail，划定乐观直通的严格数学边界",
                "Step 2/5：SessionStart 签发 <=120 tok MVCC 乐观租约（附带 epoch_hash 与置信度 p）",
                "Step 3/5：三态动作分类器精确区分客户端直连（乐观）与服务端 bind/高危写（悲观）",
                "Step 4/5：PostToolUse 捕获碰壁退出，2ms 内反查宿主机注入替代处方并降级失效租约",
                "Step 5/5：W15 实测省 Token 83.9%、质量 100 分，结合审计滑窗实现租约自动升降级",
            ],
            "next": "单 Agent 回路用 AOPCC 加速，那多个 Agent 同时操作同一台宿主机时如何防止互相踩踏与死锁？",
            "script": (
                "这是整套系统里极具数据库味道的一个设计：AOPCC 乐观-悲观自适应并发控制。\n"
                "我们来算一笔精确的期望代价账：哪怕 body_port_check 只花 0.2 毫秒，只要让大模型额外发起一轮工具调用，就会产生一次整轮对话税 C_turn——包含 5k 到 30k 的上下文重发和 2 到 4 秒的推理延迟。对于像 psql 连本机 5432 这种可逆、失败代价极低、成功率大于 95% 的操作，全悲观前置查询在数学上是严格亏本的。\n"
                "因此我们在 SessionStart 签发 120 Token 以内的 MVCC 乐观租约，分类器把客户端连接和临时沙盒直接放行；一旦碰壁，Post-Hook 在 2 毫秒内捕获错误码，反查宿主机注入真实替代方案，同时自动降低该租约的置信度。反之，对端口 bind、高爆炸半径改写和共享写，死守悲观前置拦截。"
            ),
        },
        # 08 多 Agent 协作锁与死锁治理
        {
            "eyebrow": "07 · COORDINATION · HIERARCHICAL LOCKS & WFG DEADLOCK",
            "title": "多 Agent 协作锁：层级互斥传播与 WFG 死锁环路检测",
            "lead": "面向多智能体并发作业，提供 <b>5 类资源锁、父子目录递归互斥、Wait-For Graph 死锁检测与 UDS/SSE 唤醒队列</b>。",
            "svg": svg_code_and_flow(
                left_title="多 Agent 协作锁与死锁环路检测 (src/coordination/)",
                file_ref="Phase 11 & Phase 17",
                code_lines=[
                    "pub enum ResourceKind { Port, Directory, Service, File, Custom }",
                    "pub enum LockMode     { Shared, Exclusive }",
                    "",
                    "// 1. 目录层级递归互斥传播:",
                    "//    持有 /work/proj 排他锁 -> 自动阻塞 /work/proj/src/a.rs",
                    "//    持有 /work/proj/src/a.rs -> 自动阻塞 /work/proj 独占锁",
                    "",
                    "// 2. Wait-For Graph (WFG) 环路检测:",
                    "//    Agent-A 持 R1 等 R2, Agent-B 持 R2 等 R1",
                    "//    DFS 瞬时发现环路 [A -> B -> A], 拒绝成环并给出破局建议",
                    "",
                    "// 3. 释放唤醒: delete_lock -> lock_wait_queue -> SSE/UDS 推送",
                    "// 4. PG 后端: 联动 pg_try_advisory_lock 跨连接事务级保证",
                ],
                right_title="LOCK LIFECYCLE & BENCHMARK",
                right_cards=[
                    ("Pre-Hook 强联动（悲观门禁）", "任何 Agent 准备 bind 端口或写受锁目录时，body_hook_pre 毫秒阻断冲突", "避免两路 Agent 同时改坏同一份源码或抢占同一端口"),
                    ("TTL 自动续租与守护 GC", "支持租期 TTL 续约，BackgroundScheduler 定时回收超时孤儿锁并写审计", "彻底杜绝 Agent 异常退出导致的永久死锁"),
                    ("W6 并发死锁破局实测", "Token 400 → 125 (-68.8%) · 质量 33.3 → 100.0 · 加速 46.49x", "原生工具：body_lock_{acquire,release,list,wait,deadlock_check}"),
                ],
            ),
            "next": "解决了并发冲突，接下来看第四器官「规约脊柱」如何守住目录秩序和百级 SKILL 的上下文预算。",
            "script": (
                "当一台宿主机上同时跑着两三个编码 Agent 或后台 Cron Worker 时，最常见的问题就是互相覆盖文件或抢占测试端口。\n"
                "我们的 CoordinationEngine 支持端口、目录、服务、文件、自定义五类资源以及共享/排他两种锁模式，核心亮点有三点：一是父子目录递归互斥，锁了父目录自动拦住子路径；二是内置 Wait-For Graph 有向图环路检测，瞬间发现 A 等 B、B 等 A 的死锁环路并拒绝成环；三是提供可订阅等待队列，持锁方释放时通过 UDS/SSE 主动唤醒，避免轮询空转。W6 基准实测死锁破局加速 46.49 倍。"
            ),
        },
        # 09 规约脊柱 · 放置自演化与 Context 防爆路由
        {
            "eyebrow": "08 · GOVERNANCE SPINE · PLACEMENT LEARNER & CONTEXT ROUTER",
            "title": "规约脊柱：放置规则自演化与百级 SKILL Context 防爆",
            "lead": "<b>41 μs 放置建议</b>消灭文件乱丢，<b>搬移纠偏自学习</b>沉淀隐性偏好，<b>预算路由器</b>将 130+ SKILL 压降至 358 Token。",
            "svg": svg_columns(
                [
                    {
                        "tag": "PLACEMENT · 41 μs",
                        "title": "8 大目录放置引擎 2.0",
                        "accent": C_TEAL_D,
                        "lines": [
                            "script/project/skill/src/note/tmp",
                            "支持项目级配置覆盖与意图打分",
                            "强制小写蛇形与命名冲突预检",
                            ("M3 微基准 P50 仅 41 μs", True),
                        ],
                        "foot_lbl": "源码位置",
                        "foot_val": "src/placement/engine_v2.rs",
                    },
                    {
                        "tag": "SELF-LEARNING · 纠偏",
                        "title": "PlacementLearner 2.0",
                        "accent": C_GREEN,
                        "lines": [
                            "捕获主人跨目录手动搬移行为",
                            "结合历史 audit_events 聚类错放",
                            "自动归纳并提议新增放置规则",
                            ("规则落盘受 L2 确认与审计留痕", True),
                        ],
                        "foot_lbl": "源码位置",
                        "foot_val": "src/placement/learner.rs",
                    },
                    {
                        "tag": "CONTEXT ROUTER · 防爆",
                        "title": "MCP / SKILL / 服务通讯录",
                        "accent": C_CLAY,
                        "lines": [
                            "统一编目 130+ SKILL 与外部服务",
                            "按任务意图做 Top-K 预算裁剪",
                            "严禁静态全量塞爆 LLM 注意力",
                            ("W7 实测：609,758 → 358 tok (-99.9%)", True),
                        ],
                        "foot_lbl": "源码位置",
                        "foot_val": "src/ecosystem/",
                    },
                ],
                bottom_banner=(
                    "规约引擎把主人的「口头习惯」（CodeGraph 优先、北京时区、零编造、目录规范）变成可执行、可检验的确定性契约。",
                    "W4 质量 16.5→100.0 · W7 密度 838.0x",
                    "teal",
                ),
            ),
            "next": "下面进入海马体器官：先看可插拔记忆 SPI 与 RRF 混合检索，再看我们如何根治 7 大陈旧漏斗。",
            "script": (
                "规约脊柱解决两个工程顽疾。第一是目录失控：PlacementEngineV2 在 41 微秒内给出目标路径和命名校验，而且当主人手动把一个乱放的文件移回正确目录时，PlacementLearner 会捕捉到这次纠偏并自动归纳成新规则。\n"
                "第二是上下文爆炸：现在很多宿主机装着上百个 SKILL 和几十个 MCP Server，如果全量注入 System Prompt，光是工具描述就能吃掉几十万 Token。我们的 ContextBudgetRouter 按当前任务意图只召回 Top-K 最相关技能和外部服务通讯录，W7 实测直接把 60.9 万 Token 压到了 358 Token，信息密度暴涨 838 倍。"
            ),
        },
        # 10 海马体① · 可插拔记忆 SPI 与 RRF 混合检索
        {
            "eyebrow": "09 · HIPPOCAMPUS I · PLUGGABLE MEMORY SPI & RRF",
            "title": "海马体①：可插拔记忆 SPI、RRF 混合检索与生命周期治理",
            "lead": "严守「不重复造轮子」：内置零依赖 <b>SQLite FTS5 + Ollama 向量 RRF</b> 引擎，同时一行配置兼容 Mem0 / Letta / PowerContext。",
            "svg": svg_pipeline(
                steps=[
                    {
                        "tag": "INTAKE · 准入门禁",
                        "title": "脱敏 + 重要性评分",
                        "accent": C_CLAY,
                        "lines": [
                            "MemoryIntakeFilter 阻断噪音",
                            "SecretRedactor 强制脱敏",
                            "同路径 auto_sync 幂等归档",
                        ],
                    },
                    {
                        "tag": "SPI · 可插拔抽象",
                        "title": "MemoryProvider Trait",
                        "accent": C_TEAL_D,
                        "lines": [
                            "Builtin Local (默认零依赖)",
                            "Mem0 / Letta / PowerContext",
                            "外部不可用时自动优雅降级",
                        ],
                    },
                    {
                        "tag": "SEARCH · 混合召回",
                        "title": "FTS5 + 向量 RRF",
                        "accent": C_TEAL,
                        "lines": [
                            "倒数排名融合 (k=60)",
                            "解决「说法不同捞不出来」",
                            "M2 FTS5 P50 335 μs · W9 100分",
                        ],
                    },
                    {
                        "tag": "LIFECYCLE · 衰减仲裁",
                        "title": "半衰期与矛盾仲裁",
                        "accent": C_GREEN,
                        "lines": [
                            "按访问频次与时间半衰期衰减",
                            "检测对立语义结论触发仲裁",
                            "BriefAssembler 40% 记忆预算",
                        ],
                    },
                ],
                bottom_panels=[
                    {
                        "title": "RRF 混合检索为什么比纯向量更适合工程场景？",
                        "accent": C_TEAL,
                        "lines": [
                            "工程排障常含精确错误码/端口/文件名（FTS5 BM25 完胜纯向量）",
                            "同时存在语义改写诉求（本地 Ollama Embedding 余弦相似度补足召回）",
                            ("RRF(d) = 1/(60 + rank_fts(d)) + 1/(60 + rank_vec(d))", True),
                        ],
                    },
                    {
                        "title": "现网数据覆盖与生态兼容性",
                        "accent": C_GREEN,
                        "lines": [
                            "DQ2 本地 Ollama Embedding 覆盖率 99.5%，支持 scripts/backfill_embeddings.py 增量补齐",
                            "通过 config.toml 一行切换外部记忆后端，兼顾开箱即用与大生态集成",
                            ("src/memory/{spi,builtin_provider,lifecycle}.rs + adapters/", True),
                        ],
                    },
                ],
            ),
            "next": "存得进、搜得到只是及格线；真正的硬骨头是：磁盘文件每天都在变，如何保证知识库永不腐烂？",
            "script": (
                "记忆层我们坚持两条原则：一是开放生态不造轮子，通过 MemoryProvider SPI 同时支持内置引擎和 Mem0、Letta、PowerContext，且外部服务挂了自动降级回本地 SQLite；二是不靠纯向量单腿走路，工程场景既有精确报错码又有自然语言描述，FTS5 加本地向量的 RRF 倒数排名融合才能双杀，W9 语义召回拿到满分 100。\n"
                "同时入库前强制过重要性门禁、凭据脱敏和同路径自动记忆幂等归档，避免单文件改十次把开工简报刷屏。"
            ),
        },
        # 11 海马体② · 全生命周期信息新鲜度与写穿失效
        {
            "eyebrow": "10 · HIPPOCAMPUS II · INFORMATION FRESHNESS & RECONCILIATION",
            "title": "海马体②：根治七大陈旧漏斗，写穿失效与现实对账",
            "lead": "文件改了向量还在用旧的、文件删了 FTS 还在召回僵尸、端口关了图谱节点还在亮灯——Phase 38 用<b>写穿失效 + 级联净化</b>彻底根治。",
            "svg": svg_compare_rows(
                headers=("陈旧漏斗 (STALE-01~07)", "治理前隐患", "Phase 38 写穿与对账根治机制"),
                rows=[
                    ("内容变更旧向量残留", "文件改动后 embedding 仍为旧内容向量", "content_hash/mtime 变化立即重置 '[]' 并标 stale"),
                    ("L2 元数据语义过期", "旧 Ollama 摘要与新代码逻辑背离", "写穿标记 annotation_status = needs_re_enrich"),
                    ("删除文件僵尸召回", "文件删除后 FTS 虚拟表仍返回已删记录", "三后端 FTS 强制 status='active' + 级联删 fact/links"),
                    ("同路径记忆霸屏", "单文件反复保存产生数十条重复 auto_sync 记忆", "AutoSyncEngine 新写归档旧版，严格幂等去重"),
                    ("幽灵端口与 CodeGraph 滞后", "已关闭服务端口与过期索引长期误导大脑", "FreshnessReconciler 巡检剪除幽灵节点并核验 mtime"),
                ],
                takeaway=(
                    "现网 DQ5 实测：抽检 589 项（资产/Fact节点/自动记忆）健康 589 项，达成 100.0%（失活/漂移/幽灵/冗余全 0）。",
                    "W12 实测：Token -91.2% · 质量 27.5→100.0",
                ),
            ),
            "next": "单会话新鲜度达到 100% 后，看跨大脑会话中断时，如何用 <=300 tok 胶囊实现「换脑不失忆」。",
            "script": (
                "任何一个跑过两周以上的本地知识库都会腐烂。我们在 Phase 38 系统排查出七大陈旧漏斗：文件改了向量不更新、L2 标注过期、删除后 FTS 僵尸召回、自动记忆重复霸屏、关闭端口成幽灵节点、回填脚本没复位 ready 状态、CodeGraph 索引滞后。\n"
                "现在这套写穿失效与现实对账机制已经全链路贯通：Watcher 一捕获改动立刻重置旧向量和标注状态，文件一删跨表级联净化，后台 FreshnessReconciler 定期拿库内快照和磁盘、/proc 真实端口对账。现网 DQ5 抽检 589 项新鲜度达到 100.0%，失活、漂移、幽灵、冗余全部清零。"
            ),
        },
        # 12 跨大脑连续性 · ContinuityCapsule
        {
            "eyebrow": "11 · CONTINUITY · CROSS-BRAIN HANDOFF CAPSULE",
            "title": "跨大脑连续性：BodyManifest 与 <=300 tok 接力胶囊",
            "lead": "大脑 A 因额度耗尽、上下文满载或切工具中断时，本体封装<b>脱敏抗投毒的极简接力胶囊</b>，大脑 B 在 10ms 内无缝接棒。",
            "svg": svg_code_and_flow(
                left_title="本体身份证与连续性胶囊契约 (src/body/ & body_checkpoints)",
                file_ref="ISSUE #27 · Phase 44",
                code_lines=[
                    "pub struct BodyManifest {",
                    "  body_id: String,          // 确定性宿主机指纹 body-54f9ef...",
                    "  organs:  [OrganHealth; 6],// 感官/小脑/海马体/脊柱/代谢/联邦",
                    "  vitals_score: f64,        // 五维稳态综合分 (现网 93.9/100)",
                    "}",
                    "",
                    "pub struct ContinuityCapsule { // 严格 Token 预算 <= 300 tok",
                    "  from_brain: String,       // 如 antigravity / claude-code",
                    "  goal: String,             // 核心任务目标 (脱敏+抗投毒清洗)",
                    "  touched_files: Vec<Path>, // 已改动文件与关键检查点",
                    "  next_actions: Vec<String>,// 确定性下一步动作清单",
                    "  active_leases: Vec<Lease>,// 可继承的高置信乐观租约",
                    "}",
                ],
                right_title="CROSS-BRAIN HANDOFF BENCHMARK",
                right_cards=[
                    ("大脑无关性（Brain-Agnostic）", "无论接手的是 Cursor、Claude Code、Codex 还是 Cron Worker，协议完全一致", "body_state(action=\"manifest\" | \"checkpoint\" | \"handoff\")"),
                    ("安全零泄露底线", "胶囊生成前强制经 SecretRedactor 脱敏与 PromptInjectionScanner 清洗", "严禁把上一个大脑的临时密钥或投毒指令带入新会话"),
                    ("W17 & PR-13 实测收益", "Token 2,400 → 175 (-92.7%) · 质量 37.0 → 100.0 (+63.0) · 加速 5.34x", "单次跨大脑交接实测至少省去 >=2 轮摸索对话"),
                ],
            ),
            "next": "拥有了连续性记忆与主动自愈能力后，本体到底「什么能自己干、什么必须请示、什么绝对不能碰」？",
            "script": (
                "脑体分离落地到代码里，最直观的实体就是第 27 个模块 src/body/ 和第 27 张表 body_checkpoints。\n"
                "任何新大脑一着陆，10 毫秒内拿到 BodyManifest 确认自己附身在哪台宿主机、六大器官是否健康；如果上一个大脑刚中断，直接读出一份严格控制在 300 Token 以内、经过脱敏和抗投毒清洗的 ContinuityCapsule，目标、已改文件、下一步动作清清楚楚。W17 实测单次换脑交接省 92.7% Token，至少省掉两轮对话。"
            ),
        },
        # 13 自治边界 · 四级权限包络与五维稳态
        {
            "eyebrow": "12 · AUTONOMY · L0~L3 ENVELOPE & HOMEOSTASIS VITALS",
            "title": "自治边界：四级权限包络 (L0~L3) 与五维稳态生命体征",
            "lead": "用<b>四级自治包络</b>划死不可逾越的痛觉红线，用<b>五维稳态评分 (0~100)</b> 像体检报告一样实时量化本体健康度。",
            "svg": svg_matrix_2x2(
                [
                    {
                        "tag": "L0_OBSERVE · 纯只读感知",
                        "title": "永不阻塞 · 零副作用",
                        "accent": C_TEAL,
                        "bg": C_WHITE,
                        "lines": [
                            "Brief 组装、物理事实查询、图谱爆炸半径",
                            "等待事件采样、/proc/self 探针、AWR 报告",
                            "规则：默认放行，毫秒级返回",
                        ],
                    },
                    {
                        "tag": "L1_AUTONOMIC · 可逆稳态反射",
                        "title": "自主自愈 · 100% 留痕",
                        "accent": C_GREEN,
                        "bg": C_WHITE,
                        "lines": [
                            "Watcher 60s 折叠、写穿失效、幽灵端口剪枝",
                            "过期锁 GC、重复记忆归档、租约置信度降级",
                            "ADDM 一键 PRAGMA shrink_memory + malloc_trim",
                        ],
                    },
                    {
                        "tag": "L2_SUPERVISED · 受控宿主写操作",
                        "title": "默认 dry-run / 强鉴权",
                        "accent": C_GOLD,
                        "bg": C_WHITE,
                        "lines": [
                            "HostGC 物理删文件、LinkRepair --apply 改源码",
                            "Mesh 收件箱 accept、放置规则自学习落盘",
                            "规则：需大脑显式确认或 Web auth_token 鉴权",
                        ],
                    },
                    {
                        "tag": "L3_FORBIDDEN · 绝对痛觉禁区",
                        "title": "硬编码 100% 拦截",
                        "accent": C_CLAY,
                        "bg": C_CLAY_L,
                        "lines": [
                            "/etc/shadow、/etc/sudoers、~/.ssh/ 私钥目录",
                            "明文凭据落盘与跨机 Mesh 直写宿主业务目录",
                            "DQ8 实测：L3 拦截率 100% · 现网稳态分 93.9/100",
                        ],
                    },
                ],
                footer_banner="五维生命体征（HomeostasisVitals）：感官清晰度 · 反射灵敏度 · 海马体纯净度 · 代谢健康度 · 免疫可溯度（加权 0~100）。",
            ),
            "next": "权限包络划定了行为边界，下面看针对凭据泄露、命令注入与间接提示词投毒的纵深安全防线。",
            "script": (
                "很多人担心「Agent 自治」会不会失控乱删文件。我们从第一天起就把所有本体行为硬划分成四级权限包络。\n"
                "L0 纯只读永不阻塞；L1 是可逆的内部稳态反射，比如写穿失效、清理过期锁、释放 SQLite 内存页，可以自主完成但必须 100% 写审计；L2 涉及真实磁盘写操作，默认全部 dry-run，必须显式确认或带 Bearer Token 鉴权；L3 是绝对痛觉红线，碰 /etc/shadow、~/.ssh/ 或明文密钥落盘直接 100% 强拦。配合五维生命体征，现网 DQ8 稳态分 93.9。"
            ),
        },
        # 14 纵深安全 · 全链路脱敏、防注入与抗投毒
        {
            "eyebrow": "13 · DEFENSE-IN-DEPTH · REDACTION & ANTI-POISONING",
            "title": "纵深安全：全链路凭据零落盘、防注入与抗提示词投毒",
            "lead": "针对代码审计暴露的 5 类高危缺口（<code>SEC-GAP-01~05</code>），构建覆盖采集、存储、Hook、Web、Mesh 的五道硬防线。",
            "svg": svg_columns(
                [
                    {
                        "tag": "DEFENSE 01 · 凭据零落盘",
                        "title": "SecretRedactor 全覆盖",
                        "accent": C_CLAY,
                        "lines": [
                            "拦截 .env* / .ssh / *.pem / *.key",
                            "正则擦除 URI 口令与高危 Token",
                            "覆盖扫描/cmdline/记忆/审计/Mesh",
                            ("DQ4 现网抽检：101/101 零残留", True),
                        ],
                        "foot_lbl": "源码位置",
                        "foot_val": "src/scanner/metadata.rs",
                    },
                    {
                        "tag": "DEFENSE 02 · 执行防注入",
                        "title": "沙盒转义与写鉴权",
                        "accent": C_TEAL_D,
                        "lines": [
                            "HookDispatcher 禁裸拼 sh -c",
                            "DuckDB 导出校验目录白名单+转义",
                            "Web 绑定 127.0.0.1 + auth_token",
                            ("~/.agent-body 0700 / db 0600", True),
                        ],
                        "foot_lbl": "校验入口",
                        "foot_val": "src/hooks/ & src/web/server.rs",
                    },
                    {
                        "tag": "DEFENSE 03 · 抗上下文投毒",
                        "title": "PromptInjectionScanner",
                        "accent": C_GREEN,
                        "lines": [
                            "识别外部仓/文档/Mesh 隐藏指令",
                            "命中 Ignore previous 等立即清洗",
                            "阻断恶意仓库借 Brief 劫持大脑",
                            ("W11 实测：质量 21.5→100.0 (+78.5)", True),
                        ],
                        "foot_lbl": "安全规格",
                        "foot_val": "docs/design_doc/11-security-spec.md",
                    },
                ],
                bottom_banner=(
                    "安全不是事后补丁：所有安全防线均纳入 W11 / W14 / DQ4 自动化回归基准，任何退化立即阻断发版。",
                    "SECURITY AS BENCHMARKED SLA",
                    "clay",
                ),
            ),
            "next": "单机安全铁桶一块之后，跨主机多本体协同如何做到零信任认证与沙盒隔离？",
            "script": (
                "宿主机底座天然接触源码、进程命令行和配置文件，安全必须是硬骨头。\n"
                "我们围绕 11-security-spec 落地了五道纵深防线：第一，全链路 SecretRedactor，进程命令行、记忆、审计、本地模型输入和跨机对话一律先脱敏再入库；第二，Hook 命令插值强制 POSIX 安全转义，DuckDB 导出限定白名单目录防 SQL/路径穿越；第三，Web 默认仅监听回环且全量写端点强制 Bearer Token，数据目录权限收敛至 0700/0600；第四，引入 PromptInjectionScanner，防止恶意开源仓库在 README 里埋间接提示词注入劫持大脑。"
            ),
        },
        # 15 多体联邦 · MeshEnvelope v1.0 线协议
        {
            "eyebrow": "14 · FEDERATED MESH · MESHENVELOPE v1.0 WIRE PROTOCOL",
            "title": "多体联邦：MeshEnvelope v1.0 零信任签名与沙盒协同",
            "lead": "拒绝重型 Raft/Rsync/聊天软件：以轻量 <b>HMAC-SHA256 签名 + 防重放 + Peer Scopes</b> 支撑跨机图谱、对话与文件投递。",
            "svg": svg_code_and_flow(
                left_title="MeshEnvelope v1.0 规范签名与防重放 (src/mesh/)",
                file_ref="ISSUE #24 · Phase 41",
                code_lines=[
                    "// HTTP Headers (全面切为 X-Body-* 协议头)",
                    "X-Body-Node-Id:   body-node-east-01",
                    "X-Body-Timestamp: 1795348200123        // +-300s 时间窗校验",
                    "X-Body-Nonce:     9f4e2a...            // NonceCache 防重放",
                    "X-Body-Scopes:    read:facts,exchange:dialogue,transfer:artifact",
                    "X-Body-Signature: hmac_sha256(secret, canonical_string + body_sha256)",
                    "",
                    "// 三大受控协同原语:",
                    "POST /api/v1/mesh/graph    -> 跨机图谱联邦只读探查",
                    "POST /api/v1/mesh/dialogue -> 结构化意图 (ASK/HANDOFF/SHARE/REPLY)",
                    "POST /api/v1/mesh/transfer -> <=10MB + SHA256 -> ~/.agent-body/mesh_inbox/",
                ],
                right_title="ZERO-TRUST & SANDBOX GUARANTEES",
                right_cards=[
                    ("Peer Scopes 最小权限授权", "按节点细粒度授予 read:facts / write:locks / exchange:dialogue / transfer:artifact", "越权请求在签名验证阶段直接 403 拒止"),
                    ("双向脱敏抗投毒对话", "跨机对话仅限 4 种结构化意图，出站入站双检 SecretRedactor + InjectionScanner", "防止一台被污染的节点横向感染整个集群大脑"),
                    ("沙盒收件箱零直写覆盖", "文件传输硬限 <=10MB、阻断敏感后缀、0600 落盘 mesh_inbox/ 待本机 accept", "W14 实测：篡改/重放 100% 拦截，质量 12.5→100.0"),
                ],
            ),
            "next": "单机和多机的安全边界都守住了，接下来看「可观测性与自进化」核心：双域行为审计与 DuckDB OLAP。",
            "script": (
                "多台开发机或测试机需要协同，比如 A 机的 Agent 想问 B 机某个服务跑在哪个端口，或者递交一份 2MB 的诊断报告。\n"
                "我们恪守递弱代偿，绝不造 Raft 共识也不造大文件同步工具，而是设计了干净的 MeshEnvelope v1.0 线协议：每个请求带时间戳、Nonce、Scope 声明和 HMAC-SHA256 规范签名，服务端严检正负 300 秒时间窗和 NonceCache 防重放。跨机传文件硬限 10MB，严格落进 0600 权限的 mesh_inbox 沙盒，必须本机显式 accept 才能移出，杜绝跨机直写覆盖业务目录。"
            ),
        },
        # 16 双域行为审计 · IN_AGENT + OUT_OF_AGENT
        {
            "eyebrow": "15 · DUAL-DOMAIN AUDIT · IN_AGENT + OUT_OF_AGENT",
            "title": "双域全链路行为审计：拒绝「黑盒自治」，60s 折叠防洪",
            "lead": "打破「只记写文件、不管后台自治」的盲区：<b>接入域 + 域外自治域 100% 留痕</b>，以 DuckDB 列存引擎驱动 OLAP 溯源。",
            "svg": svg_columns(
                [
                    {
                        "tag": "DOMAIN A · IN_AGENT",
                        "title": "Agent 接入全通道审计",
                        "accent": C_TEAL_D,
                        "lines": [
                            "Brief 租约签发与 Pre/Post 裁决",
                            "全部 43 个 MCP / CLI / Web 写操作",
                            "Mesh 跨机请求与协作锁变更",
                            ("写入耗时 <80 μs，零阻塞降级", True),
                        ],
                        "foot_lbl": "因果字段",
                        "foot_val": "trace_id / session_id / mode",
                    },
                    {
                        "tag": "DOMAIN B · OUT_OF_AGENT",
                        "title": "本体自治与被动维护审计",
                        "accent": C_CLAY,
                        "lines": [
                            "PROACTIVE_DAEMON 30m 巡检",
                            "REACTIVE_WATCHER 60s 折叠防洪",
                            "HostGC / LinkRepair / Reconciler",
                            ("彻底消灭后台静默黑盒变更", True),
                        ],
                        "foot_lbl": "防洪成效",
                        "foot_val": "10 次高频保存折叠为 1 条",
                    },
                    {
                        "tag": "PROVENANCE · 存证与 OLAP",
                        "title": "脱敏 Delta + DuckDB 分析",
                        "accent": C_GREEN,
                        "lines": [
                            "before_after_delta (<=4KB 脱敏)",
                            "每条附 evidence_sha256 完整性指纹",
                            "单事务增量同步 DuckDB 导出 Parquet",
                            ("DQ7 抽检 500/500 完整零泄露", True),
                        ],
                        "foot_lbl": "原生工具",
                        "foot_val": "body_audit_query / analytics_*",
                    },
                ],
                bottom_banner=(
                    "历史痛点根治：曾实测 changes 表 94.7% 被编辑器保存洪泛淹没；分层防洪后底层变更流与高价值审计账本彻底解耦。",
                    "W16 实测：质量 36.0→100.0 · DQ7 = 100.0%",
                    "teal",
                ),
            ),
            "next": "全量双域审计账本攒下来，绝不只是为了事后查案——它直接喂给 P1~P5 探针，驱动产品闭环自进化。",
            "script": (
                "做 Phase 43 之前我们翻现网库发现两个大坑：一是 changes 表一万多条记录里 94.7% 都是编辑器保存产生的高频 I/O 噪音；二是后台巡检、GC、断链修复、过期锁回收这些「Agent 之外的本体行为」竟然是 0 审计的。\n"
                "于是我们把底层 I/O 变更流和高价值行为审计彻底分离：IN_AGENT 覆盖全部 43 工具与 Hook，OUT_OF_AGENT 100% 覆盖后台巡检、GC、修复和 60 秒折叠防洪后的 Watcher。每条记录带脱敏前后差值和 SHA-256 指纹，单条写入不到 80 微秒，现网 DQ7 抽检 500 条完整率 100%。"
            ),
        },
        # 17 闭环自进化 · P1~P5 五点对比探针飞轮
        {
            "eyebrow": "16 · SELF-EVOLUTION · P1~P5 PROBES & FEEDBACK FLYWHEEL",
            "title": "闭环自进化：P1~P5 五点对比探针与数据驱动调优飞轮",
            "lead": "严禁拍脑袋优化：在执行回路 5 个关键切面部署微秒级对比探针，用 <b>Before / After / Delta 数字差值</b>自动驱动纠偏。",
            "svg": svg_pipeline(
                steps=[
                    {
                        "tag": "P1 / P2 · 事前切面",
                        "title": "Brief & Pre-Hook",
                        "accent": C_TEAL_D,
                        "lines": [
                            "核验简报耗时与租约有效率",
                            "统计悲观拦截命中与误报率",
                            "捕获高频碰撞端口与目录",
                        ],
                    },
                    {
                        "tag": "P3 / P4 · 事中事后",
                        "title": "检索效用 & 漂移",
                        "accent": C_TEAL,
                        "lines": [
                            "检测 stale_vec / 僵尸召回",
                            "统计乐观直通碰壁率 p_hat",
                            "验证 <2ms 处方自愈成功率",
                        ],
                    },
                    {
                        "tag": "P5 · 周期回归",
                        "title": "图谱与基准比对",
                        "accent": C_GOLD,
                        "lines": [
                            "对比历史 Benchmark 批次退化",
                            "审计图谱孤岛率与断链回弹",
                            "生成结构化偏差 Delta 清单",
                        ],
                    },
                    {
                        "tag": "FLYWHEEL · 闭环执行",
                        "title": "自愈 + 反馈沉淀",
                        "accent": C_GREEN,
                        "lines": [
                            "自动升降级 AOPCC 租约权限",
                            "聚类高频错放喂 PlacementLearner",
                            "幂等去重写 user_feedback.md",
                        ],
                    },
                ],
                bottom_panels=[
                    {
                        "title": "真实自愈案例（Phase 39 首轮上线实录）",
                        "accent": C_GREEN,
                        "lines": [
                            "P3 探针上线首轮即捕获现网 3 条 stale_vec 向量未及时重算的真实缺口",
                            "自动触发 FreshnessReconciler 修复并幂等记录至 user_feedback.md，探针回归 5/5 全绿",
                            ("agent-body feedback probes  |  agent-body feedback reconcile", True),
                        ],
                    },
                    {
                        "title": "审计驱动的 AOPCC 租约动态升降级（PR-12）",
                        "accent": C_TEAL,
                        "lines": [
                            "AuditEvolutionAnalyzer 消费近 24h audit_events 滑动窗口统计真实碰壁率",
                            "连续稳定高命中 → 升格签发乐观租约；频发失败 → 自动降级回悲观前置校验",
                            ("src/audit/evolution.rs + src/feedback/probes.rs", True),
                        ],
                    },
                ],
            ),
            "next": "自进化管住了业务效果和新鲜度，那本体自身如果出现 CPU 飙升或内存增长怎么查？下面看 Phase 50 的数据库级内生性能工程。",
            "script": (
                "把双域审计和五点探针连起来，才真正构成自进化飞轮。我们在开工 Brief、事前门禁、检索效用、事后漂移和周期回归五个切面埋了微秒级探针，不断拿物理真值、SLA 基线和上一版 Benchmark 做对比。\n"
                "上线第一轮，P3 探针就真实抓到现网 3 条 stale 向量缺口并自动愈合。同时，进化分析器每 24 小时扫描审计滑窗，某个本机服务如果碰壁率升高，自动把它的乐观租约降级；如果长期稳定，自动升格放行。所有改进都有 Before/After 数字佐证。"
            ),
        },
        # 18 内生性能工程① · 等待事件与 /proc/self 内核探针（带动画）
        {
            "eyebrow": "17 · BUILT-IN PERF I · WAIT EVENTS & /PROC/SELF (INTERACTIVE)",
            "title": "内生性能工程①：6 类 18 等待事件 (<100ns) 与 /proc/self 探针",
            "lead": "借鉴 PostgreSQL <code>pg_stat_activity</code> / Oracle Wait Events：无锁原子直方图 + 零特权内核探针，<b>单二进制零 eBPF 依赖</b>。",
            "svg": svg_code_and_flow(
                left_title="无锁等待事件直方图与 RAII 埋点 (src/body/perf_awr/)",
                file_ref="ISSUE #32 · Phase 50",
                code_lines=[
                    "// 6 大类 18 种子事件: Lock / IO / Net / CPU / IPC / Idle",
                    "pub struct WaitEventSlot {",
                    "  calls: AtomicU64, total_us: AtomicU64, max_us: AtomicU64,",
                    "  // 8 档对数分位桶: <=50us, 200us, 1ms, 5ms, 20ms, 100ms, 500ms, >500ms",
                    "  buckets: [AtomicU64; 8],",
                    "}",
                    "// RAII WaitEventGuard: 零堆分配, 单次埋点实测 <100 ns",
                    "let _g = WaitEventGuard::start(WaitEvent::LockSqliteMutex);",
                    "",
                    "// 零特权 /proc/self 内核遥测 (对标轻量 eBPF):",
                    "//   /proc/self/status -> VmRSS, VmHWM, voluntary/nonvoluntary_ctxt_switches",
                    "//   /proc/self/io     -> read_bytes, write_bytes | /proc/self/fd -> open_fds",
                ],
                right_title="BODY TIME MODEL & ZERO OVERHEAD",
                right_cards=[
                    ("Body Time 百分比分解", "将总耗时正交分解为 CPU / Lock / IO / Net / IPC，一眼定位真瓶颈", "杜绝「把 SQLite 锁争用误判成 Ollama 慢」的瞎猜排障"),
                    ("操作级统计（对标 pg_stat_statements）", "OperationStatsRegistry 聚合高频工具与后台子阶段 calls / P50 / P95 / P99", "内置于 SQLite lock_conn、LinksEngine 与 FreshnessReconciler"),
                    ("零特权可移植设计", "不引入重型 libbpf/OTel 依赖，普通容器与非 root 宿主机直接生效", "单二进制交付体积与冷启动 SLA 零退化"),
                ],
                animated=True,
            ),
            "anim_caps": [
                "Step 1/4：无锁 AtomicU64 8 档对数分位桶 + RAII WaitEventGuard 实现 <100ns 零分配埋点",
                "Step 2/4：Body Time 模型将系统耗时正交分解为 CPU / Lock / IO / Net / IPC 五大维度",
                "Step 3/4：OperationStatsRegistry 对标 pg_stat_statements 追踪各操作 P50/P95/P99",
                "Step 4/4：零特权直读 /proc/self 获取内存、句柄与上下文切换，零外部 eBPF/OTel 依赖",
            ],
            "next": "有了等待事件和内核遥测，下面看 AWR 快照黄金基线差分、ADDM 自动诊断，以及 600% CPU 事故的实战复盘。",
            "script": (
                "Phase 50 是很多数据库工程师眼前一亮的特性。我们没有去接臃肿的 OpenTelemetry 或需要 root 权限的 libbpf，而是直接把数据库最精华的性能模型原生写进了 Rust 内核。\n"
                "左边代码展示了核心数据结构：6 大类 18 种等待事件，每个槽位是纯 AtomicU64 加 8 档对数分位桶，用 RAII Guard 包住锁和 I/O 切面，实测单次埋点不到 100 纳秒、零堆分配。\n"
                "配合零特权直读 /proc/self 拿到的 VmRSS、FD 句柄和上下文主被动切换次数，我们能瞬间把系统开销分解成 CPU、Lock、IO、Net、IPC 的 Body Time 比例，像看 PostgreSQL wait_event 一样清澈透明。"
            ),
        },
        # 19 内生性能工程② · AWR 黄金基线差分、ADDM 与 600% CPU 根治实战
        {
            "eyebrow": "18 · BUILT-IN PERF II · AWR BASELINE, ADDM & 600% CPU POSTMORTEM",
            "title": "内生性能工程②：AWR 黄金基线、ADDM 自调优与 600% CPU 复盘",
            "lead": "持久化 32 份快照与 <b>Golden Baseline</b> 差分；5 大 ADDM 规则在 L1 包络内一键触发内存回收与 WAL 整理。",
            "svg": svg_columns(
                [
                    {
                        "tag": "AWR · 黄金基线差分",
                        "title": "32 快照滚动对比",
                        "accent": C_TEAL_D,
                        "lines": [
                            "复用 system_meta 表，0 新表膨胀",
                            "支持设定 Golden Baseline 基准态",
                            "输出任意两态资源与等待事件差分",
                            ("现网 CLI VmRSS 实测：18.2 MB", True),
                        ],
                        "foot_lbl": "CLI / MCP 入口",
                        "foot_val": "body_state(perf_snapshot/perf_awr)",
                    },
                    {
                        "tag": "ADDM · 5 大诊断规则",
                        "title": "L1 一键自调优",
                        "accent": C_GREEN,
                        "lines": [
                            "检测锁争用 / WAL 膨胀 / RSS / FD",
                            "自动输出证据链与调优处方",
                            "perf_tune 触发 PRAGMA optimize",
                            ("+ shrink_memory + malloc_trim(0)", True),
                        ],
                        "foot_lbl": "安全边界",
                        "foot_val": "严守 L1_AUTONOMIC 可逆包络",
                    },
                    {
                        "tag": "POSTMORTEM · 真实根治",
                        "title": "600% CPU / 1.3G 事故",
                        "accent": C_CLAY,
                        "lines": [
                            "根因：DuckDB 8 线程 + 3.5万单事务",
                            "并发打开治理页触发 174s 全量拷贝",
                            "三层根治：2 线程/256MB+单事务+15s缓存",
                            ("叠加流式分页，守护 RSS < 70 MB", True),
                        ],
                        "foot_lbl": "对应议题",
                        "foot_val": "ISSUE #30 & #32 已彻底根治",
                    },
                ],
                bottom_banner=(
                    "性能工程闭环：把「曾经踩过的 600% CPU / 7,825 inotify 句柄大坑」固化为内生 AWR 黄金基线与 ADDM 自动化防线。",
                    "agent-body body perf-awr | perf-tune",
                    "teal",
                ),
            ),
            "next": "看完了六大器官与内核性能工程，下面我们用 Benchmark v3.8 完整数据审视整套系统的实证表现。",
            "script": (
                "这里分享一个非常真实的生产排障复盘：在 Phase 48 前后，我们观测到守护进程 CPU 一度飙到 600%、内存冲到 1.3G。一开始大家怀疑是本地 Ollama，实测排查发现 Ollama 跑在独立 cgroup，真正元凶是 Web 治理页并发触发 DuckDB 同步时开了 8 线程无内存上限，还对三万五千行跑了三万五千次独立自动提交事务。\n"
                "我们用三层根治直接把 DuckDB 收敛到 2 线程、256MB 上限、单事务批量提交加 15 秒单飞缓存，并把这次经验沉淀成了 Phase 50 的 AWR 黄金基线和 ADDM 自调优：现在任何内存或锁争用苗头，一条 body perf-awr 立马现形，一条 perf-tune 直接在 L1 权限内回收 SQLite 缓存和 glibc 堆页。"
            ),
        },
        # 20 实证评测① · Benchmark v3.8 帕累托双门禁
        {
            "eyebrow": "19 · BENCHMARK v3.8 · TOKEN-QUALITY PARETO DUAL-GATE",
            "title": "实证评测①：Benchmark v3.8 帕累托双门禁与全量真机明细",
            "lead": "确定性四维质量函数 <b>Q = 40% R_fact + 30% P_signal + 20% A_action + 10% S_safety</b>，杜绝靠截断信息骗取高压缩率。",
            "svg": svg_metrics_table(
                kpis=[
                    ("-99.23%", "D1 Token 总节省", "662,502 → 5,114 tok"),
                    ("+64.2 分", "D6 质量净提升", "35.6 → 98.0 / 100"),
                    ("94.5x", "有效信息密度倍数", "Facts / 1k Tokens"),
                    ("7μs~26.9ms", "6 大微基准全绿", "M6 7μs · M3 33μs · M1 139μs"),
                ],
                headers=["评测维度 / ID", "考核核心目标", "基线 → 本体实测", "质量分 / 达标率", "SLA"],
                rows=[
                    ["W1~W8 (基础)", "工具/端口/扫描/放置/图/锁/技能/简报", "642,245 → 2,297 tok (-99.6%)", "平均 36.2 → 99.6", "PASS"],
                    ["W9~W12 (质量)", "RRF召回 / 抗误报 / 安全脱敏 / 新鲜度", "767 → 107 tok (-86.0%)", "平均 35.3 → 100.0", "PASS"],
                    ["W13~W18 (高阶)", "分层图 / Mesh / AOPCC / 审计 / 接力 / AWR", "19,490 → 2,710 tok (-86.1%)", "平均 31.7 → 100.0", "PASS"],
                    ["M1~M6 (微基准)", "3跳图 / FTS5 / 放置 / 热力 / Parquet / Wait", "139μs / 259μs / 33μs / 4.8ms / 26.9ms / 7μs", "100% 远低于 SLA", "PASS"],
                    ["DQ1~DQ9 (现网)", "元数据/拓扑/脱敏/新鲜/图/审计/稳态/遥测", "DQ1/4/6/7/8/9 100%·DQ5 98.5%", "18/18 WaitEvent 全覆盖", "PASS"],
                    ["PR-01~14 (账本)", "生产运行态全渠道效能实时存证", "<100 μs 记账 + SHA-256 指纹", "含 BodyPerfAwrAnalyzed", "PASS"],
                ],
                col_xs=[16, 220, 620, 1020, 1320],
                footnote="实机 Benchmark v3.8 (v1.0.0 GA) · 总耗时 ~3.7s · 阻断噪音 538 个 · 事前拦截风险 61 次",
            ),
            "next": "看完自对照和生产存证账本，再看轨道 C：四大赛道热门开源竞品的同机实测对比与定位差异。",
            "script": (
                "我们在 Benchmark v3.1 到 v3.7 迭代中最核心的一条修正，就是把「帕累托双门禁」写成了铁律：每个负载必须同时考核事实召回、信噪比、可行动性和安全性，必须保证本体质量分严格大于等于基线且必含事实 100% 命中。\n"
                "从真机明细表可以看到：18 个业务负载无一例外全部实现「省 Token + 质量大幅跃升」，总体 Token 降了 99.23%，质量分从 35.6 升到 98.0；6 大内核微基准全部达标 SLA；9 项现网数据健康与遥测门禁全维体检。"
            ),
        },
        # 21 实证评测② · 四赛道竞品同机对标与生态边界
        {
            "eyebrow": "20 · COMPETITORS · 4-TRACK LIVE BENCHMARK & ECOSYSTEM",
            "title": "实证评测②：四赛道竞品同机对标与架构验证边界",
            "lead": "严格区分 <b>LIVE_BENCHMARKED（同机同数据集实跑）</b> 与 <b>ARCH_VERIFIED（源码能力核实）</b>，不拉踩、不捏造。",
            "svg": svg_compare_rows(
                headers=("赛道与对标对象", "竞品强项与典型局限", "Agent Body 同机实测差异与协同策略"),
                rows=[
                    ("C1 长期记忆 · Mem0/Letta/PC", "强在对话提取与云端记忆图谱；弱在无宿主机事实与脱敏", "0/5 明文泄露 (-97.8% tok)；通过 Memory SPI 兼容拥抱"),
                    ("C2 上下文路由 · OpenViking", "强在虚拟文件系统分层摘要；弱在副本搬迁与静态技能膨胀", "0 副本搬迁，931 vs 144,730 tok (-99.4%，密度 233x)"),
                    ("C3 文件治理 · AgentFS", "强在 SQLite VFS 沙盒隔离；弱在全量导入无黑白名单防爆", "10 净入库 vs 290 全量 (-96.5% tok，质量 +73.6 分)"),
                    ("C4 宿主探针 · rg+psutil+ss", "强在单点命令行灵活；弱在多子进程开销与无跨服务 3 跳图", "0 子进程 20ms vs 89ms，且多出 2 个跨服务上游影响点"),
                    ("微观代码 · CodeGraph", "强在 AST/符号级调用图谱；不覆盖进程/端口/Systemd", "宏微双层联邦：绝不替代 CodeGraph，直接桥接下钻"),
                ],
                takeaway=(
                    "生态定位：Agent Body 填补「宿主机物理具身 + 并发稳态」空白，与记忆框架和 CodeGraph 形成互补底座。",
                    "详见 docs/design_doc/12-competitor-benchmark-spec.md",
                ),
            ),
            "next": "再看这整套系统在工程化、可维护性和防漂移方面，是如何靠 AI Agent 持续演进到 Phase 50 而不腐烂的。",
            "script": (
                "技术人看竞品对比最反感踩一捧一。我们在设计 12-competitor-benchmark-spec 时定下两条红线：能同机同数据集跑的标 LIVE_BENCHMARKED，没跑的只做源码级 ARCH_VERIFIED，绝不编跑分。\n"
                "从四个赛道能非常清楚地看到生态分工：Mem0、Letta 专注对话记忆，我们通过 SPI 兼容它；CodeGraph 专注代码符号 AST，我们通过 Bridge 联邦它；而 Agent Body 死磕的「物理宿主机实时感官 + 并发防撞 + 新鲜度对账 + 双域审计 + 内生 AWR」，正是整个开源生态里最稀缺的那块底座。"
            ),
        },
        # 22 黑白屏同源、零重扫导航与五重质量门禁
        {
            "eyebrow": "21 · ENGINEERING EXCELLENCE · ZERO-RESCAN & 5-FOLD QUALITY GATE",
            "title": "工程化实践：43/43 黑白屏同源、零重扫与五重确定性门禁",
            "lead": "大型项目演进到 50 个 Phase 最怕「越改越乱」——我们用 <b>EVOLUTION_GUIDE + CodeGraph + 五重硬门禁</b>把纪律写进机器。",
            "svg": svg_columns(
                [
                    {
                        "tag": "ZERO-RESCAN · 零重扫",
                        "title": "2 轮精准锁定变更触点",
                        "accent": C_TEAL_D,
                        "lines": [
                            "Tier-0 只读 EVOLUTION_GUIDE + 活跃图",
                            "8 大高频场景 O(1) 变更触点矩阵",
                            "自建 .codegraph/ 符号穿透源码",
                            ("15 个根目录核心文件自动防漏登", True),
                        ],
                        "foot_lbl": "检查命令",
                        "foot_val": "make evolution-check (<100ms)",
                    },
                    {
                        "tag": "QUALITY GATE · 五重门禁",
                        "title": "把质量纪律固化为代码",
                        "accent": C_GREEN,
                        "lines": [
                            "lint-panic: 0 处生产 unwrap/expect",
                            "lint-hygiene: 0 处硬编码路径/控吞错",
                            "lint-strict: -D warnings + clippy",
                            ("cargo test: 320+ 自动化测试全绿", True),
                        ],
                        "foot_lbl": "一键联检",
                        "foot_val": "make quality-gate && make test",
                    },
                    {
                        "tag": "PARITY & DECKS · 同源",
                        "title": "43/43 同步与演说代码化",
                        "accent": C_CLAY,
                        "lines": [
                            "Web /api/v1/parity/matrix 强守 100%",
                            "7 幕真引擎路演沙盒 + 昼夜双主题",
                            "演说全由 Python 引擎确定性生成",
                            ("make verify-decks: Playwright 9/9", True),
                        ],
                        "foot_lbl": "对应规约",
                        "foot_val": "README 规约 #13 / #16 / #17 / #20",
                    },
                ],
                bottom_banner=(
                    "AI 时代做复杂工程的秘诀：能脚本化和门禁化的绝不靠口头叮嘱，每次提交 5 秒自动化验真，保证架构永不漂移。",
                    "28 DESIGN PRINCIPLES · MACHINE-ENFORCED",
                    "teal",
                ),
            ),
            "next": "倒数第二页，看位置无关交付、跨平台抽象与递弱代偿的最终边界红线。",
            "script": (
                "很多同行好奇：你们用 AI Agent 连续演进了 50 个 Phase、近四万行 Rust 代码，怎么没把代码改烂、没把文档改漂移？\n"
                "答案就四个字：机器门禁。我们把所有血泪教训全写成了自动化脚本：新增模块/工具/表/根文件忘了更文档，evolution-check 直接红灯；生产路径手滑写了 unwrap 或 CARGO_MANIFEST_DIR，lint-panic 和 lint-hygiene 直接拒止；连这 9 份演说 HTML 都改成了代码生成加 Playwright 像素级防重叠检查。纪律进机器，演进才可持续。"
            ),
        },
        # 23 位置无关交付、跨平台抽象与边界克制
        {
            "eyebrow": "22 · PORTABILITY & BOUNDARIES · RELOCATABLE & ANTI-CREEP",
            "title": "位置无关交付、跨平台探针与递弱代偿红线",
            "lead": "Phase 49 彻底清除编译期路径依赖实现 <b>100% 位置无关单二进制</b>；同时重申 6 条「坚决不做」架构边界。",
            "svg": svg_matrix_2x2(
                [
                    {
                        "tag": "RELOCATABLE · 位置无关 (规约 #28)",
                        "title": "搬到任意目录 100% 全绿",
                        "accent": C_GREEN,
                        "bg": C_WHITE,
                        "lines": [
                            "清除生产/基准中全部 CARGO_MANIFEST_DIR",
                            "仓库改名或容器内运行 bench full 100% 通过",
                            "模块化治理：单文件 <=1500 行，错误可观测",
                        ],
                    },
                    {
                        "tag": "CROSS-PLATFORM · 三端适配",
                        "title": "PlatformProbe 抽象与四层贯通",
                        "accent": C_TEAL,
                        "bg": C_WHITE,
                        "lines": [
                            "Linux (/proc, inotify, systemd) 为首选深化内核",
                            "macOS / Windows 纯函数解析跨平台编译容错",
                            "preflight 启动预检 + INSTALL.md + Docker",
                        ],
                    },
                    {
                        "tag": "BOUNDARY 01 · 不越界做大脑",
                        "title": "绝不变成重型 Agent 框架",
                        "accent": C_CLAY,
                        "bg": C_CLAY_L,
                        "lines": [
                            "不内置 LLM ReAct 规划循环与业务推理",
                            "不替代 LangGraph / AutoGen / CrewAI 编排",
                            "43 工具严控膨胀：新能力按 action 内聚扩展",
                        ],
                    },
                    {
                        "tag": "BOUNDARY 02 · 不重复造轮子",
                        "title": "尊重既有基础设施分工",
                        "accent": C_GOLD,
                        "bg": C_GOLD_L,
                        "lines": [
                            "不替代 Docker/gVisor 沙箱与 Vault 密钥管理",
                            "不造重型 Raft 共识、通用 OLTP 与大文件同步",
                            "微观代码交给 CodeGraph，记忆生态留给 SPI",
                        ],
                    },
                ],
                footer_banner="三层命名兼容承诺：品牌升级为「Agent 本体」，工程标识 agent-body / body_* / ~/.agent-body/ 100% 稳定可预期。",
            ),
            "next": "最后一页，总结演进终局，并附上 6 讲 × 30 分钟专题系列与开源协作入口。",
            "script": (
                "Phase 49 我们经历了一次特别真实的教训：把仓库从旧名字改成 agent-body 后，W10 基准突然报红，一查是基准代码里残留了一个编译期宏 CARGO_MANIFEST_DIR 指向了旧路径。\n"
                "我们借机把整套代码库做了一次彻底的卫生治理：100% 位置无关化、把超大文件按领域拆干净、把静默吞错压到预算线以下。同时始终守住下面两格的递弱代偿红线——只做最基础、最硬核的具身底座，绝不膨胀成臃肿的大框架。"
            ),
        },
        # 24 终局与专题导航
        {
            "eyebrow": "23 · EPILOGUE · OS EMBODIED SUBSTRATE & DEEP-DIVE SERIES",
            "title": "终局：从宿主机单二进制走向 OS 原生具身底座",
            "lead": "今天已跑通生产闭环；如果想把某个子系统再拆透 30 分钟，欢迎直接进入配套的 <b>6 讲体系化专题演说</b>。",
            "svg": svg_closing(
                kicker="AGENT BODY · ARCHITECTURE COMPLETE · v1.0.0 GA",
                headline="瞬态大脑千变万化，永驻本体稳如磐石",
                pillars=[
                    ("专题 01~02 · 认知与感官", "脑体分离第一性原理 · L1~L3 分层图谱与宏微联邦", "01-brain-body · 02-host-reality-graph"),
                    ("专题 03~04 · 反射与记忆", "AOPCC 并发控制与锁 · 新鲜度治理与换脑不失忆", "03-cerebellum-aopcc · 04-hippocampus"),
                    ("专题 05~06 · 免疫与实证", "纵深安全/双域审计/Mesh · Bench v3.8 与工程门禁", "05-security-audit-mesh · 06-benchmark"),
                ],
                open_question="欢迎围绕「OS 原生具身原语」「数据库性能工程迁移至 AI Infra」「多本体联邦协议」展开深度 Q&A",
                next_talk="返回总览导航：presentation/index.html · 路演精简版：presentation/presentation.html",
            ),
            "script": (
                "以上就是 Agent Body v1.0.0 GA 的全部 24 页深度技术架构。\n"
                "回过头看，这个项目其实只回答了一个本质问题：当大模型本身越来越强、切换越来越频繁时，宿主机侧需要沉淀一层怎样的确定性基础设施，才能让每个瞬态大脑一着陆就耳聪目明、不踩坑、不失忆、省 Token 且可审计？\n"
                "我们用 Rust 单二进制、27 个模块、43 个原生工具、88 个存储 SPI、Benchmark v3.8 和内生 AWR 给出了第一份可复现的工程答卷。如果大家对其中某一个子系统——比如 AOPCC、分层图谱、或者 AWR 实现——想再深挖半小时，可以直接点开配套的 6 讲专题系列，或者直接拉代码跑一遍 make quality-gate 和 bench full。下面进入 Q&A，欢迎拍砖！"
            ),
        },
    ]

    render_deck(
        doc_title="Agent Body · Agent 本体深度技术交流版 (v1.0.0 GA)",
        brand_label="AGENT BODY · 深度技术交流版",
        foot_label="AGENT BODY · DEEP TECHNICAL ARCHITECTURE · v1.0.0 GA",
        slides_data=slides,
        out_path=PRES_DIR / "agent-body-deck.html",
    )
    print(f"✅ Generated {PRES_DIR / 'agent-body-deck.html'} ({len(slides)} slides)")


if __name__ == "__main__":
    build_tech_deck()
