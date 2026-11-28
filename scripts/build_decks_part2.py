#!/usr/bin/env python3
"""Generate Lectures 04~06 (30-min deep-dive HTML decks) and presentation/index.html."""

from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from deck_engine import (
    C_CLAY, C_CLAY_L, C_GOLD, C_GOLD_L, C_GREEN, C_GREEN_L, C_INK, C_LINE,
    C_PAPER, C_PAPER2, C_TEAL, C_TEAL_2, C_TEAL_3, C_TEAL_D, C_TEAL_L, C_TEAL_M, C_WHITE,
    icon_badge, render_deck, svg_bars, svg_closing, svg_code_and_flow, svg_columns,
    svg_compare_rows, svg_cover, svg_matrix_2x2, svg_metrics_table, svg_pipeline,
    vis_benchmark_pareto, vis_competitor_arena, vis_evolution_flywheel,
    vis_freshness_purifier, vis_handoff_capsule, vis_mesh_network,
    vis_microkernel_hub, vis_quality_gates, vis_security_dual_audit,
    vis_storage_tri_engine,
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
OUT = ROOT / "presentation"


def build_talk_04() -> None:
    slides = [
        {
            "cover": True,
            "eyebrow": "封面 · 第 04 讲",
            "title": "海马体、信息新鲜度治理与跨大脑 1ms 接力",
            "svg": svg_cover(
                kicker=f"SERIES 04 / 06 · AGENT BODY · v{APP_VER} · 30 MIN",
                title="海马体、新鲜度治理与跨大脑接力",
                en_sub="Memory SPI, Freshness Reconciliation & Cross-Brain Continuity",
                subtitle1="为什么 Agent 越用越「胡说八道」？因为记忆会发霉",
                subtitle2="可插拔记忆门面 · 七大陈旧漏斗写穿失效 · <=300 tok 换脑接力胶囊",
                spec_line="第 27 表 body_checkpoints · DQ5 99.9% · W12 -91.2% · W17 -92.7%",
                strata=[
                    ("Memory Provider SPI · 开放门面", "内置 SQLite FTS5+RRF · 外挂 Mem0 / Letta / PowerContext", "#EBF1EE", C_INK),
                    ("混合检索 · BM25 × Ollama 向量 RRF", "精确符号 + 语义近义双路融合 · 离线优雅降级", "#CFE0DC", C_INK),
                    ("入库净化 · 噪音拦截与凭据脱敏", "MemoryIntakeFilter 拒收水话 · SecretRedactor 零明文", C_TEAL_D, C_PAPER),
                    ("七大陈旧漏斗治理 · STALE-01~07", "文件改删写穿失效 · 零僵尸 FTS · 幽灵端口剪枝", C_TEAL_2, C_PAPER),
                    ("现实对账器 · FreshnessReconciler", "定期抽检资产/图谱/记忆三端一致性 · DQ5 99.9%", C_TEAL_3, C_PAPER),
                    ("跨大脑接力 · body_checkpoints (#27)", "脱敏+防注入净化 <=300 tok 胶囊 · 换模型 1ms 无缝续跑", "#A4C4C6", C_INK),
                ],
                right_tag="第 4 讲 · 30 分钟单点说透 · 地层剖面规范",
            ),
            "script": (
                "欢迎来到第 4 讲。今天我们专攻 Agent 本体的第三大器官——海马体。\n"
                "市面上做 Agent Memory 的项目一抓一大把，但几乎所有项目都在炫耀「我能存多少条、召回多快」，却很少有人直面两个致命痛点：第一，代码改了、文件删了、端口停了，旧记忆和旧向量发霉了怎么办？第二，上午用 Claude Code，下午换 Antigravity，怎么让新大脑 1 毫秒无缝接力而不泄密？今天 30 分钟我们彻底讲透。"
            ),
        },
        {
            "eyebrow": "01 · 不造轮子，只做门面",
            "title": "Memory Provider SPI：内置零依赖引擎，开放接入整个生态",
            "lead": "记忆赛道已经够拥挤了。<b>Agent Body 用统一 SPI 托底，默认开箱即用，同时兼容 Mem0 / Letta / PowerContext</b>。",
            "svg": svg_columns(
                [
                    {
                        "tag": "BUILTIN · 开箱零外部依赖",
                        "title": "SQLite FTS5 + Ollama RRF",
                        "accent": C_TEAL_D,
                        "bg": C_TEAL_L,
                        "lines": [
                            "单文件 SQLite 零运维直接启动",
                            "FTS5 精确命中错误码与函数名",
                            "Ollama 向量召回语义近义经验",
                            ("RRF 倒数排名融合，离线自动降级", True),
                        ],
                        "foot_lbl": "源码触点",
                        "foot_val": "src/memory/hybrid.rs",
                    },
                    {
                        "tag": "ADAPTERS · 可插拔生态",
                        "title": "Mem0 / Letta / PowerContext",
                        "accent": C_TEAL_2,
                        "lines": [
                            "统一实现 MemoryProvider Trait",
                            "外部服务不可达时自动回退内置库",
                            "绝不因第三方记忆宕机阻断干活",
                            ("W9 混合检索质量 36.3 → 100.0", True),
                        ],
                        "foot_lbl": "源码触点",
                        "foot_val": "src/memory/adapters/",
                    },
                    {
                        "tag": "INTAKE GUARD · 入库守门",
                        "title": "防噪音 + 强制脱敏 + 冲突审计",
                        "accent": C_GREEN,
                        "lines": [
                            "拦截「好的/收到」等无价值水话",
                            "入库前 SecretRedactor 抹除密钥",
                            "Lifecycle 检测认知矛盾与衰减归档",
                            ("COMP-C1 对标：0% 明文泄露", True),
                        ],
                        "foot_lbl": "源码触点",
                        "foot_val": "src/memory/{intake,lifecycle}.rs",
                    },
                ],
                ("记忆只是海马体的基础层；真正决定记忆有没有毒的，是接下来要讲的「信息新鲜度治理」。", "src/memory/spi.rs", "teal"),
            ),
            "next": "为什么很多团队上了向量记忆库后，用满一个月反而觉得 Agent 越来越蠢？",
            "script": (
                "我们对待记忆层的态度非常务实：不跟 Mem0、Letta 拼融资讲故事，而是用 `MemoryProvider` SPI 把它们全接进来，同时提供一个零依赖的 SQLite FTS5 + Ollama RRF 内置引擎兜底。\n"
                "更关键的是三道守门：水话不进库、密钥脱敏后进库、相互矛盾的旧结论由生命周期管理器审计归档。"
            ),
        },
        {
            "eyebrow": "02 · 七大陈旧漏斗",
            "title": "记忆发霉解剖：三级写穿净水引擎根治 STALE-01~07",
            "lead": "文件一改标 STALE · 文件一删级联冲刷 · <b>现网 589 项鲜活度 100%</b>",
            "svg": vis_freshness_purifier(animated=True),
            "anim_caps": [
                "Step 1/3：左侧七大发霉源——改档旧向量、删档 FTS 僵尸、断链与重复记忆",
                "Step 2/3：中央三级净水引擎——content_hash 探针 → ChangeHandler 冲刷 → 现实对账",
                "Step 3/3：右侧实测仪表——589 项鲜活度 100%、向量覆盖 99.5%、0 僵尸召回",
            ],
            "next": "我们看看这些写穿失效与现实对账，在代码里是怎么严丝合缝闭环的。",
            "script": (
                "这是我们在自己宿主机上真刀真枪挖出来的七个陈旧漏斗。\n"
                "最隐蔽的是 STALE-01 和 STALE-03：一个文件明明删了，FTS5 虚拟表没跟着清，Agent 搜出来兴冲冲去读直接 FileNotFound；或者文件重写了一遍，旧向量还躺在库里，语义检索照样按旧内容召回。Phase 38 我们把这七个漏斗逐一焊死。"
            ),
        },
        {
            "eyebrow": "03 · 写穿失效源码",
            "title": "ChangeHandler 级联清理 + FreshnessReconciler 现实对账",
            "lead": "两条防线双保险：<b>实时变更写穿作废 + 周期全表与物理现实对账</b>，现网 DQ5 达 99.9%。",
            "svg": svg_code_and_flow(
                "src/watcher/handler.rs & src/feedback/probes.rs",
                "src/watcher/handler.rs · probes.rs",
                [
                    "// 1. 文件修改 -> 写穿失效 (Write-Through Invalidation)",
                    "storage.mark_asset_embedding_stale(path)?;  // embedding = '[]'",
                    "storage.mark_asset_needs_re_enrich(path)?;  // 待 L2 重标注",
                    "",
                    "// 2. 文件删除 -> 跨四表原子级联清理",
                    "storage.soft_delete_asset_and_purge_fts(path)?;",
                    "storage.delete_fact_node_and_links(path)?;",
                    "storage.archive_auto_sync_memories(path)?;",
                    "",
                    "// 3. 后台周期对账 (FreshnessReconciler::reconcile)",
                    "//    核验磁盘存在性、端口存活、重复摘要、幽灵节点",
                ],
                "W12 & DQ5 实证闭环",
                [
                    ("零僵尸召回", "删除文件后即刻从 FTS5 与属性图抹除，彻底杜绝幻觉路径", "test_change_handler_deleted_cascades"),
                    ("周期自动平账", "每 1800s 巡检自动清理失活资产与幽灵端口节点，并写入 OUT_OF_AGENT 审计", "DQ5 = 99.9% (726/727 健康)"),
                    ("W12 基准验证", "改删写穿失效与一致性自愈节省 91.2% Token，质量从 27.5 跃升至 100.0", "Bench v3.9 W12 PASS"),
                ],
                animated=True,
            ),
            "anim_caps": [
                "① 变更写穿与删除四表级联清理源码",
                "② 实时防线：零僵尸路径进入检索结果",
                "③ 周期防线：FreshnessReconciler 自动对账",
                "④ W12 与 DQ5 实测 99.9% 健康度",
            ],
            "next": "单会话的长期记忆干净了，现在进入本讲最高潮：跨模型、跨客户端的「换脑 1ms 接力」。",
            "script": (
                "我们用数据库缓存一致性的思路来治记忆发霉：写穿失效（Write-Through Invalidation）加后台对账（Reconciler）。\n"
                "文件一改，旧向量和旧摘要当场作废；文件一删，资产、FTS、属性图、自动记忆四张表联动清理。哪怕守护进程曾经重启漏了事件，每 1800 秒的 `FreshnessReconciler` 也会跟物理磁盘重新对一次账，现网 DQ5 稳定在 99.9%。"
            ),
        },
        {
            "eyebrow": "04 · 跨大脑接力痛点",
            "title": "换脑瞬间：50,000 tok 聊天垃圾山 vs 150 tok 发光胶囊",
            "lead": "W17 实测：<b>Token -92.7% · 10ms 着陆续跑 · 零密钥泄露</b>",
            "svg": vis_handoff_capsule(animated=True),
            "anim_caps": [
                "Step 1/2：传统方式搬运 5 万 Token 聊天噪音；本体压缩为 150 tok 四芯脱敏胶囊",
                "Step 2/2：上午 Claude 封存 → 底座保险箱 → 下午 Antigravity 10ms 续跑 → 凌晨 Cron",
            ],
            "next": "让我们直接看第 27 张表 `body_checkpoints` 与 `ContinuityManager` 的硬核实现。",
            "script": (
                "很多人试过跨工具接力：把 Claude Code 的对话历史导出成 Markdown，粘贴到 Cursor 里。结果呢？五万字贴进去，新模型光读噪音就晕了，还顺手把报错日志里的 API Key 喂给了另一家模型。\n"
                "真正工程级的跨大脑接力，必须是一枚极度克制、结构化、脱敏且防投毒的「接力胶囊」，硬上限 300 Token。"
            ),
        },
        {
            "eyebrow": "05 · 第 27 张表源码",
            "title": "body_checkpoints：SQLite / PostgreSQL / DuckDB 三引擎统一落盘",
            "lead": "不是临时 JSON 文件，而是<b>一等公民存储表（#27 表 + 4 个 SPI 方法 + Migration #11）</b>，支持多机与 OLAP 追溯。",
            "svg": svg_code_and_flow(
                "src/storage/schema.sql & src/body/continuity.rs",
                "src/body/continuity.rs · spi.rs",
                [
                    "CREATE TABLE IF NOT EXISTS body_checkpoints (",
                    "  id TEXT PRIMARY KEY,           -- \"cap-xxxx\"",
                    "  body_id TEXT NOT NULL,         -- 绑定宿主机身份指纹",
                    "  source_brain TEXT NOT NULL,    -- \"antigravity\" / \"claude-code\"",
                    "  session_id TEXT NOT NULL,",
                    "  goal TEXT NOT NULL,            -- 核心未竟目标",
                    "  summary_md TEXT NOT NULL,      -- 净化后 <=300 tok 摘要",
                    "  key_files_json TEXT NOT NULL,  -- 关键文件坐标清单",
                    "  open_risks_json TEXT NOT NULL, -- 待解风险与前置约束",
                    "  status TEXT NOT NULL,          -- active | resumed | archived",
                    "  resumed_by_brain TEXT,         -- 接盘大脑身份留痕",
                    "  created_at TEXT NOT NULL",
                    ");",
                ],
                "胶囊落盘四重硬门禁",
                [
                    ("1. SecretRedactor 强脱敏", "自动擦除 sk-/ghp_/Bearer/数据库连接串，确保胶囊 0 明文凭据", "DQ8 脱敏合规 = true"),
                    ("2. PromptInjection 净化", "扫描并中和 \"ignore previous instructions\" 等越狱注入短语，防跨脑投毒", "neutralize_injection_phrases()"),
                    ("3. 硬预算裁剪 <=300 tok", "落盘严格限额 300 tok，注入 Brief 时自动压缩至 <=150 tok", "防上下文膨胀"),
                    ("4. IN_AGENT 全链路审计", "保存与接盘动作双向写入 audit_events，可追溯哪两个大脑完成交接", "src/audit/mod.rs"),
                ],
                animated=True,
            ),
            "anim_caps": [
                "① 第 27 表 body_checkpoints 三引擎统一 Schema",
                "② 门禁一：落盘前强制 SecretRedactor 脱敏",
                "③ 门禁二：防跨大脑提示词投毒净化",
                "④ 门禁三：300 tok 落盘 / 150 tok 注入双预算",
                "⑤ 门禁四：双大脑交接全写入审计账本",
            ],
            "next": "这枚胶囊是怎么在「结束旧会话」和「开启新会话」两个节点全自动流转的？",
            "script": (
                "看这张表结构：`body_checkpoints` 是我们系统里的第 27 张表，SQLite、PostgreSQL、DuckDB 三个存储后端一次性对齐，外加 4 个 Storage SPI 方法。\n"
                "最值得一提的是第二道门禁——防跨大脑提示词投毒。如果 Brain A 刚才读了一个被恶意投毒的网页，不小心把注入指令带进了总结里，胶囊保存时会先过 `PromptInjectionScanner` 并把恶意指令短语中和，绝不让它传染给 Brain B。"
            ),
        },
        {
            "eyebrow": "06 · 全自动接力流",
            "title": "SessionManager 封存 → BriefAssembler 1ms 自动唤醒",
            "lead": "不需要人类手动复制粘贴。<b>旧大脑收尾封存胶囊，新大脑一开工简报自动递上接力棒</b>。",
            "svg": svg_pipeline(
                [
                    {"tag": "BRAIN A 收尾", "title": "session_close / handoff", "accent": C_TEAL_D, "lines": ["传入未竟目标与文件", "ContinuityManager 净化", "写入 body_checkpoints", "status = active"]},
                    {"tag": "HOST BODY", "title": "宿主机静默守护", "accent": C_TEAL_2, "lines": ["Brain A 彻底关闭退出", "胶囊安全留在本地 DB", "无云端同步隐私风险"]},
                    {"tag": "BRAIN B 开工", "title": "BriefAssembler 组装", "accent": C_GREEN, "bg": C_GREEN_L, "lines": ["budget >= 800 自动检测", "跳过 bench:/demo: 隔离", "注入 <=150 tok 接力段", "携带 active_capsule_id"]},
                    {"tag": "HANDOFF DONE", "title": "1ms 确认接盘", "accent": C_GOLD, "lines": ["body handoff resume", "记录 resumed_by_brain", "计入 PR-13 效能账本", "零重扫直奔下一行代码"]},
                ],
                [
                    {
                        "title": "实测隔离纪律：绝不污染正常会话",
                        "accent": C_TEAL,
                        "bg": C_TEAL_L,
                        "lines": [
                            "当 session_id 以 bench: 或 demo: 开头时，严格在独立隔离域运行，不污染真实宿主机胶囊",
                            "当调用方显式指定极小预算（budget < 800 tok）时，自动让位给核心物理事实索引",
                        ],
                    },
                    {
                        "title": "Web 路演剧场第 7 幕：现场可交互演示",
                        "accent": C_GREEN,
                        "bg": C_GREEN_L,
                        "lines": [
                            "scene-7（cross_brain_handoff）在 Storage::open_in_memory() 沙箱内真引擎实跑",
                            "直观对比「裸奔换脑重扫 2,400 tok」vs「本体接力胶囊 175 tok + 敏感密钥脱敏」",
                        ],
                    },
                ],
                animated=True,
                mid="ah407",
            ),
            "anim_caps": [
                "① Brain A 收尾封存脱敏接力胶囊",
                "② 胶囊静默留在宿主机本地库，隐私零出境",
                "③ Brain B 开工简报自动注入 <=150 tok 接力摘要",
                "④ Brain B 确认接盘，计入 PR-13 效能账本",
                "⑤ 沙盒隔离纪律与第 7 幕真引擎演示",
            ],
            "next": "让我们用实机 W17 和 W9/W12 的 Benchmark 数据，检验海马体整套闭环的真实成色。",
            "script": (
                "整个接力流程对人几乎是无感的。\n"
                "Brain A 收尾时调一次 `body_session_close` 或 `body_state(action=handoff_save)`，把目标、关键文件和风险封进胶囊；半小时后你打开另一个客户端，SessionStart 钩子自动跑 `BriefAssembler`，只要预算大于等于 800 tok 且有活跃胶囊，立刻把不超过 150 tok 的接力摘要拼进开工简报里。"
            ),
        },
        {
            "eyebrow": "07 · 实证数据",
            "title": "海马体与跨大脑接力实证成绩单（Bench v3.9）",
            "lead": "从混合检索（W9）、写穿新鲜度（W12）到跨大脑接力（W17），<b>四项指标全部满血 PASS</b>。",
            "svg": svg_metrics_table(
                [
                    ("-92.71%", "W17 跨大脑接力 Token 节省", "2,400 → 175 tok"),
                    ("54.9x", "W17 有效信息密度倍数", "4/4 核心事实全命中"),
                    ("99.9%", "DQ5 新鲜度对账健康度", "726/727 项健康"),
                    ("0%", "COMP-C1 明文凭据泄露率", "5/5 敏感串全脱敏"),
                ],
                ["评测项", "海马体核心验证场景", "Token (基线 → 赋能)", "质量 / 达标率", "状态"],
                [
                    ["W9", "混合语义检索 Recall@5 准确率", "258 → 32 (-87.6%)", "36.3 → 100.0 (3/3)", "PASS"],
                    ["W12", "资产改删写穿失效与一致性自愈", "240 → 21 (-91.2%)", "27.5 → 100.0 (4/4)", "PASS"],
                    ["W17", "跨大脑连续性接力与本体器官自省", "2,400 → 175 (-92.7%)", "37.0 → 100.0 (4/4)", "PASS"],
                    ["DQ5", "全链路新鲜度与现实对账门禁", "失活 0 · 漂移 0 · 冗余 0", "99.9% (>=98.0%)", "PASS"],
                    ["COMP-C1", "对标 Mem0/JSON 无脱敏裸记忆基线", "97.8% Token 节省", "24.1 → 100.0 (+75.9)", "PASS"],
                ],
                [18, 130, 560, 920, 1290],
                footnote="数据来源：Bench v3.9 run-6a0a4995-7302-4bc1-86b6-554a784a6156",
            ),
            "next": "如果把海量历史记忆与胶囊拉到 PostgreSQL 或 DuckDB，存储迁移会不会丢数据？",
            "script": (
                "看这五条硬指标：W9 混合检索省 87.6%、W12 新鲜度写穿省 91.2%、W17 换脑接力省 92.7%，上下文质量全部拉满 100.0 分；COMP-C1 对标裸文本记忆基线，对方 5 条敏感凭据全部明文落盘，我们 0 泄露且省了 97.8% Token。"
            ),
        },
        {
            "eyebrow": "08 · 三引擎迁移",
            "title": f"SQLite 单机起步，经 {SPI_CNT} SPI 一键热迁移 PostgreSQL / DuckDB",
            "lead": f"{TABLE_CNT} 张三端同步表 · <b>SyncPipeline 零停机热迁移</b>",
            "svg": vis_storage_tri_engine(animated=True),
            "anim_caps": [
                f"Step 1/4：Layer 1 接入层——{TOOL_CNT} MCP / {CLI_CNT} CLI / 7 Hooks / Web 同源接入",
                f"Step 2/4：Layer 2 六大器官引擎——{MOD_CNT} 个 Rust 顶层模块高内聚",
                f"Step 3/4：Layer 3 StorageProvider SPI——{SPI_CNT} 方法统一驾驭 {TABLE_CNT} 张业务表",
                "Step 4/4：Layer 4 三圆柱存储引擎——SQLite 单机 / PG 并发集群 / DuckDB 列存 OLAP",
            ],
            "next": "我们在终端里实操一遍「Antigravity 存胶囊 → Claude Code 接盘」全流程。",
            "script": (
                "很多个人工具一上团队规模就得重写存储。我们从第一天起就把 88 个存储操作抽象进了 `StorageProvider` Trait。个人用 SQLite 零配置起飞，团队多机直接一条迁移命令切到 PostgreSQL，做长期行为分析走 DuckDB OLAP 导 Parquet。"
            ),
        },
        {
            "eyebrow": "09 · 实操命令清单",
            "title": "海马体与跨大脑接力 5 条实战命令",
            "lead": "30 秒亲手体验 <b>记忆混合检索、陈旧对账、接力胶囊封存与跨脑唤醒</b>。",
            "svg": svg_code_and_flow(
                "海马体与接力胶囊 CLI 实战",
                f"agent-body v{APP_VER} CLI",
                [
                    "# 1. 写入长期记忆（自动脱敏+去噪）与 RRF 混合检索",
                    "agent-body memory add \"PG 19 ASAN 构建需开启 -O1\" --tags pg,debug",
                    "agent-body memory search \"postgres sanitizer 编译参数\"",
                    "",
                    "# 2. 主动执行记忆矛盾审计与全链路新鲜度对账",
                    "agent-body memory audit",
                    "agent-body feedback reconcile",
                    "",
                    "# 3. Brain A 封存跨大脑接力胶囊（自动脱敏+限额 <=300 tok）",
                    "agent-body body handoff save --brain antigravity \\",
                    "  --goal \"完成 Phase 46 演讲体系\" --files \"presentation/index.html\"",
                    "",
                    "# 4. Brain B 查看并接盘最近活跃胶囊",
                    "agent-body body handoff list",
                    "agent-body body handoff resume --brain claude-code",
                ],
                "等价 MCP 闭环调用",
                [
                    ("body_memory_add / search / audit", "跨会话经验沉淀、RRF 混合检索与冲突归档", "W9 质量 100.0"),
                    ("body_session_close(handoff_note=...)", "长任务收尾自动归档会话并生成接力胶囊", "src/session/mod.rs"),
                    ("body_state(action=handoff_*)", "显式保存、列举或接盘接力胶囊", "PR-13 自动记账"),
                    ("body_brief(query=...)", "新大脑开工自动拼入 <=150 tok 接力摘要", "W17 -92.7% tok"),
                ],
            ),
            "next": "海马体与接力讲清了，但还有一个工程边界必须提前说明：什么东西「不该」塞进记忆和胶囊？",
            "script": (
                "你可以自己试一个极爽的实验：在终端用 `--brain antigravity` 存一枚故意带了 `sk-test1234567890abcdef` 假密钥的胶囊，再用 `handoff list` 看落盘内容——密钥已经被自动替换成脱敏占位符，再调 `brief` 就能看到它稳稳躺在开工简报里。"
            ),
        },
        {
            "eyebrow": "10 · 工程红线",
            "title": "记忆与接力的三条反直觉红线",
            "lead": "记越多不代表越聪明。<b>克制写入、激进作废、严格隔离</b>，才是海马体长青的秘诀。",
            "svg": svg_columns(
                [
                    {
                        "tag": "RULE 01 · 克制写入",
                        "title": "物理事实绝不抄进记忆表",
                        "accent": C_TEAL,
                        "lines": [
                            "端口、进程、目录树走事实图谱实时查",
                            "记忆只存「为什么这么做」和「踩过的坑」",
                            ("反模式：把易变状态写死成记忆导致发霉", True),
                        ],
                    },
                    {
                        "tag": "RULE 02 · 激进作废",
                        "title": "文件一改，旧向量立刻判死刑",
                        "accent": C_CLAY,
                        "lines": [
                            "宁可暂时回退 FTS5 精确匹配",
                            "也绝不用过期向量召回错误上下文",
                            ("底线：精准度永远高于虚假覆盖率", True),
                        ],
                    },
                    {
                        "tag": "RULE 03 · 严格隔离",
                        "title": "沙盒演示与生产胶囊永不混库",
                        "accent": C_GREEN,
                        "lines": [
                            "demo:* / bench:* 严格走独立内存库或前缀隔离",
                            "换脑接力胶囊落盘必过脱敏+防注入双闸",
                            ("底线：生产海马体零测试污染", True),
                        ],
                    },
                ],
                ("把物理事实交给感官、把经验决策交给记忆、把未竟现场交给胶囊——三者绝不串岗。", "三位一体分工纪律", "teal"),
            ),
            "next": "现在身体有了感官、小脑和海马体，接下来必须面对最硬核的一关：安全、审计、自进化与跨机协同。",
            "script": (
                "最后强调这三条红线：第一，别把端口和文件列表写进记忆表，那是感官图谱的活；第二，宁可暂时没有向量，也绝不留过期向量误导大脑；第三，测试和演示数据绝不污染生产海马体。"
            ),
        },
        {
            "eyebrow": "11 · 对比总结",
            "title": "传统 Agent Memory vs 本体海马体全景对比",
            "lead": "一页看清：为什么单纯加一个向量数据库，永远解不了真实研发场景的连续性问题。",
            "svg": svg_compare_rows(
                ("核心维度", "传统 Agent Memory 项目", "Agent Body 本体海马体"),
                [
                    ("数据保鲜机制", "只管追加写入，文件改删后旧记忆长期发霉", "inotify 写穿失效 + 四表级联清理 + 定期现实对账"),
                    ("跨客户端接力", "绑定单一客户端云端会话，换工具即失忆", "宿主机本地 body_checkpoints <=300 tok 跨脑接力"),
                    ("安全与抗投毒", "明文落盘常见，跨会话提示词污染无防护", "SecretRedactor 脱敏 + PromptInjectionScanner 中和"),
                    ("检索韧性", "强依赖云端 Embedding API，断网即瘫痪", "本地 SQLite FTS5 + Ollama RRF，离线自动降级"),
                ],
                ("海马体只有扎根在宿主机物理现实上，才不会变成一座充满陈年谣言的图书馆。", "Bench v3.9 W9/W12/W17 全绿"),
            ),
            "next": "收束第 4 讲，进入下一讲关于安全、双域审计与探针自进化的硬核拆解。",
            "script": (
                "把这张对比表留给大家做选型参考。记忆系统如果没有物理感知做现实对账，用得越久噪音越大；扎根在宿主机本体之上，它才真正具备了跨月、跨模型持续复利的能力。"
            ),
        },
        {
            "eyebrow": "12 · 收束",
            "title": "忘掉发霉的旧账，带走 150 Token 的精华接力棒",
            "lead": "好的海马体不仅在于「记得住」，更在于<b>「忘得及时、传得干净、换脑不断片」</b>。",
            "svg": svg_closing(
                kicker="KEY TAKEAWAYS · 第 04 讲收束",
                headline="海马体闭环：可插拔记忆 × 写穿新鲜度 × 跨大脑 1ms 接力",
                pillars=[
                    ("开放不锁定", "Memory SPI 内置 FTS5+RRF，兼容 Mem0/Letta/PowerContext", "W9 质量 100.0"),
                    ("对账零僵尸", "7 大陈旧漏斗写穿失效 + FreshnessReconciler 自动平账", "DQ5 99.9% · W12 -91.2%"),
                    ("换脑不失忆", "body_checkpoints 脱敏净化 <=300 tok，Brief 自动唤醒", "W17 -92.7% · 54.9x 密度"),
                ],
                open_question="开放思考：当守护进程在半夜主动替你清理了幽灵节点、回收了锁、降级了租约，你怎么证明它干的每一件事都「有据可查、安全可控」？",
                next_talk="下一讲预告 → 第 05 讲：纵深安全、双域审计与探针驱动自进化（05-security-audit-evolution-mesh.html）",
            ),
            "script": (
                "第 4 讲收束。今天我们解决了「记忆发霉」和「换脑断片」两大顽疾。\n"
                "留一个直击生产落地的问题：当你的 Agent 本体 24 小时常驻宿主机，白天服务大脑、半夜自己巡检自愈，你怎么审计它、信任它，并且让它越跑越聪明？\n"
                "第 5 讲，我们揭秘纵深安全、双域审计与探针自进化引擎！"
            ),
        },
    ]
    render_deck(
        doc_title="第 04 讲 · 海马体、信息新鲜度治理与跨大脑 1ms 接力 — Agent Body",
        brand_label="SERIES 04 / 06 · 海马体接力",
        foot_label=f"AGENT BODY · 第 04 讲：海马体、信息新鲜度治理与跨大脑 1ms 接力 (v{APP_VER})",
        slides_data=slides,
        out_path=OUT / "04-hippocampus-freshness-continuity.html",
    )


def build_talk_05() -> None:
    slides = [
        {
            "cover": True,
            "eyebrow": "封面 · 第 05 讲",
            "title": "纵深安全、双域审计与探针驱动自进化",
            "svg": svg_cover(
                kicker=f"SERIES 05 / 06 · AGENT BODY · v{APP_VER} · 30 MIN",
                title="纵深安全、双域审计与自进化",
                en_sub="Defense-in-Depth, Dual-Domain Audit & MeshEnvelope v1.0",
                subtitle1="给 Agent 一具常驻宿主的身体，信任必须建立在密码学与审计之上",
                subtitle2="全链路脱敏抗投毒 · IN/OUT 双域审计 · P1~P5 探针自进化 · 零信任 Mesh",
                spec_line="DQ4 100% · DQ7 100% · W11/W14/W16 全 PASS · 0 明文泄露",
                strata=[
                    ("纵深安全防线 · W11 实证", "SecretRedactor · PromptInjectionScanner · 0600/0700 权限自检", "#EBF1EE", C_INK),
                    ("双域统一审计 · audit_events", "IN_AGENT (脑内交互) + OUT_OF_AGENT (后台巡检/自愈/Watcher)", "#CFE0DC", C_INK),
                    ("防洪与防篡改 · SHA-256 存证", "Watcher 60s 折叠去洪 · 4KB 截断 · before/after 增量快照", C_TEAL_D, C_PAPER),
                    ("内生对比探针 · P1~P5 闭环", "开机/门禁/检索/漂移/图谱 5 切面量化差值自动写反馈总账", C_TEAL_2, C_PAPER),
                    ("审计驱动自进化 · AuditEvolution", "AOPCC 租约滑窗升降级 · Placement 聚类 · L1 稳态自愈", C_TEAL_3, C_PAPER),
                    ("多体联邦 · MeshEnvelope v1.0", "HMAC-SHA256 签名 · ±300s 防重放 · <=10MB 沙盒投递", "#A4C4C6", C_INK),
                ],
                right_tag="第 5 讲 · 30 分钟单点说透 · 地层剖面规范",
            ),
            "script": (
                "欢迎来到第 5 讲。今天我们拆解 Agent 本体最硬核、也最决定生产可用性的两大器官——「稳态与免疫系统」和「多体联邦躯干」。\n"
                "30 分钟里，我们把四个硬问题一次讲透：密钥和提示词注入怎么防？Agent 之内与之外的行为怎么全量审计不漏一笔？系统怎么靠探针和审计数据自己越变越强？多台宿主机之间怎么零信任互联？"
            ),
        },
        {
            "eyebrow": "01 · 纵深安全与双域监控",
            "title": "四道安全铁闸 × IN / OUT 双域审计 60s 折叠漏斗",
            "lead": "DQ4 凭据零落盘 100% · <b>W11 抗投毒 100 分 · DQ7 双域留痕 100%</b>",
            "svg": vis_security_dual_audit(animated=True),
            "anim_caps": [
                "Step 1/2：左侧四道纵深安全铁闸——全链路脱敏、抗提示词投毒、防命令逃逸、0600 红线",
                "Step 2/2：右侧双域监控汇聚 60s 折叠漏斗 (10→1) 并盖 SHA-256 钢印写入审计账本",
            ],
            "next": "防线搭好了，但如果系统在后台默默干了活却没留账，出了问题一样无法追溯。",
            "script": (
                "把一个常驻工具装在主力开发机上，工程师第一个问的永远是安全。\n"
                "我们在 Phase 32 和 Phase 43~44 连续做了三轮安全硬化：采集和落库前过 `SecretRedactor`；读外部内容和跨脑胶囊过 `PromptInjectionScanner`；Web 所有写接口（包括新增的 `/api/v1/body/*`）没带有效 Token 一律返回 401；本地数据库文件权限自动收敛到 0600。"
            ),
        },
        {
            "eyebrow": "02 · 双域审计动机",
            "title": "只审 Agent 不审守护进程，等于只装了半个监控探头",
            "lead": "Phase 43 现网审计曾揭示两大盲区：<b>94.7% 变更日志被 Watcher 刷屏，而 Agent 之外的自愈与清理 0 记录</b>。",
            "svg": svg_compare_rows(
                ("行为域分类", "过去存在的真实盲区", "Phase 43 双域统一审计闭环"),
                [
                    ("IN_AGENT 域（脑内）", "只记含 path 的写调用，19 个无 ROI 工具与 Pre-Hook 静默", "Brief/Hook/MCP/CLI/Web/Mesh 全通道统一入 audit_events"),
                    ("OUT_OF_AGENT · 主动巡检", "BackgroundScheduler 巡检、过期锁 GC、L1 自愈 0 审计", "PROACTIVE_DAEMON 归类记录每次自动修复与受影响数"),
                    ("OUT_OF_AGENT · 被动监听", "inotify 刷出 10,159 条未折叠记录（占 94.7%）", "REACTIVE_WATCHER 60s 同路径折叠防洪，保留净变更计数"),
                    ("OUT_OF_AGENT · 治理维护", "GC 删除 tmp、断链 --apply 改写、规则学习无凭据", "GOVERNANCE_MAINTENANCE 记录 before/after 增量与 SHA-256"),
                ],
                ("无论谁动了宿主机——是大模型、人类工程师还是守护进程自己——全部有据可查。", "ISSUE #26 · src/audit/mod.rs"),
            ),
            "next": "让我们看 `audit_events` 的防篡改设计与 60 秒折叠防洪机制。",
            "script": (
                "这是用户提得特别犀利的一个需求：Agent Body 接入 Agent 后的行为要审计，本体在 Agent 之外主动或被动维护宿主机的行为，同样要有据可查！\n"
                "我们一查现网老表，果然发现大问题：`changes` 表里 94.7% 是未折叠的文件修改洪泛，而后台删临时文件、回收锁、改断链居然一条审计都没有。Phase 43 我们建了统一的 `IN_AGENT + OUT_OF_AGENT` 双域审计表，彻底补齐另一半探头。"
            ),
        },
        {
            "eyebrow": "03 · 审计引擎源码",
            "title": "AuditLogger：脱敏 + 4KB 截断 + 60s 折叠 + SHA-256 存证",
            "lead": "每一条审计记录在落库前都经过<b>四步确定性清洗与哈希封签</b>，现网 DQ7 抽检 500/500 = 100.0%。",
            "svg": svg_code_and_flow(
                "src/audit/mod.rs",
                "src/audit/mod.rs · docs/design_doc/16",
                [
                    "pub enum AuditDomain { InAgent, OutOfAgent }",
                    "pub struct AuditEvent {",
                    "  pub trace_id: String,            // 因果链路关联 ID",
                    "  pub domain: AuditDomain,         // IN_AGENT | OUT_OF_AGENT",
                    "  pub actor: String,               // brain / cli / daemon / watcher",
                    "  pub action: String,              // tool_call / l1_heal / gc_clean",
                    "  pub before_after_delta: String,  // 脱敏+4KB截断后变更摘要",
                    "  pub evidence_sha256: String,     // SHA-256 完整性防篡改指纹",
                    "}",
                    "// Watcher 60s 折叠：同路径高频修改合并为 folded_count++",
                    "// 同步支持 DuckDB OLAP 多维聚合与 Parquet/CSV 导出",
                ],
                "双域审计四大工程特性",
                [
                    ("100% 脱敏防泄露", "写审计前强制经 SecretRedactor 清洗，审计库本身绝不变成凭据泄露源", "DQ7 = 100.0% (500/500)"),
                    ("60s 折叠防磁盘膨胀", "编译/格式化触发百次写入自动折叠为 1 条，保真相又不爆库", "test_audit_logger_watcher_60s_fold"),
                    ("trace_id 因果钻取", "从开工 Brief → Pre-Hook → 工具执行 → Post 诊断串成完整因果链", "Web 2.0 #view-audit 钻取"),
                    ("W16 实证收益", "双域审计与租约自进化闭环节省 83.5% Token，质量从 36.0 升至 100.0", "Bench v3.9 W16 PASS"),
                ],
                animated=True,
            ),
            "anim_caps": [
                "① AuditEvent 数据模型与 60s 折叠防洪逻辑",
                "② 特性一：脱敏写前清洗，DQ7 500/500 零泄露",
                "③ 特性二：60s 折叠消灭 94.7% 监听洪泛",
                "④ 特性三：trace_id 串联全链路因果溯源",
                "⑤ 特性四：W16 实测质量满分与 DuckDB 导出",
            ],
            "next": "审计记录不是存进冷库吃灰的——它直接喂给自进化引擎，让系统越跑越聪明。",
            "script": (
                "很多人把审计当成合规负担，写完就扔。在 Agent Body 中，审计数据是系统自进化的核心燃料。\n"
                "每一条 `AuditEvent` 都带 `trace_id`、`before_after_delta` 和 `evidence_sha256`，经过脱敏、4KB 截断和 60 秒折叠，既干干净净又轻量，直接能同步给 DuckDB 做多维趋势分析。"
            ),
        },
        {
            "eyebrow": "04 · 内生对比探针",
            "title": "P1~P5 五切面对比探针飞轮：让系统自己发现并治愈退化",
            "lead": "数据驱动调优闭环 · <b>上线首轮即自动捕获并修复现网 3 条过期向量</b>",
            "svg": vis_evolution_flywheel(animated=True),
            "anim_caps": [
                "Step 1/3：中心自进化中枢持续比对物理真值与历史快照，输出量化 Delta",
                "Step 2/3：外环 P1~P5 五大探针覆盖事实漂移、召回退化、租约碰壁、规则摩擦与遥测",
                "Step 3/3：右侧四大自动动作——升降级 AOPCC 租约、聚类新规约、去重反馈、量化存证",
            ],
            "next": "单台宿主机的免疫与自进化闭环完成了；如果我有 3 台服务器，它们之间怎么安全协同？",
            "script": (
                "Phase 39 上线 P1~P5 五切面探针的第一天，P3 探针就在我们自己的生产库里真实揪出了 3 个 `stale_vec` 过期向量缺口，并且自动触发了修复。\n"
                "探针抓异常、审计记流水、AuditEvolution 调租约和聚类规则、五维稳态低于 70 自动跑 L1 自愈——整套免疫系统形成了一个无人值守的正向飞轮。"
            ),
        },
        {
            "eyebrow": "05 · 多体联邦协议",
            "title": "MeshEnvelope v1.0：多宿主机零信任签名信封 × 沙盒收件箱",
            "lead": "拒绝重型 Raft · <b>HMAC-SHA256 + ±300s Nonce 防重放 · 10MB 沙盒</b>",
            "svg": vis_mesh_network(animated=True),
            "anim_caps": [
                "Step 1/2：开发机 / GPU 算力机 / 跳板机各自独立自治，零分布式共识泥潭",
                "Step 2/2：跨机通信统一封装 HMAC-SHA256 + Nonce 防重放信封，文件只落 0600 沙盒收件箱",
            ],
            "next": "看实机实测数据：安全、审计、自进化与 Mesh 四条线跑出来的成绩单。",
            "script": (
                "多机协同最怕做成臃肿的分布式共识集群。我们用极其克制的 `MeshEnvelope v1.0` 解决了 90% 的真实跨机诉求：HMAC-SHA256 规范签名、正负 300 秒时间窗加 NonceCache 防重放、按 Peer 分配最小权限 Scope。\n"
                "所有跨机传过来的文件只允许进 `~/.agent-body/mesh_inbox/` 隔离沙盒，限额 10MB 且验 SHA-256；Phase 44 还新增了 `/api/v1/mesh/body_manifest`，让一台主机的大脑能瞬间看清集群里其他宿主机的本体名片和稳态分。"
            ),
        },
        {
            "eyebrow": "06 · 实证数据",
            "title": "安全、审计与 Mesh 实证成绩单（Bench v3.9）",
            "lead": "从安全脱敏（W11/DQ4）、Mesh 零信任（W14）到双域审计自进化（W16/DQ7/DQ8），<b>全项 100% 达标</b>。",
            "svg": svg_metrics_table(
                [
                    ("100.0%", "DQ4 库表凭据脱敏卫生度", "144/144 零明文残留"),
                    ("100.0%", "DQ7 双域审计完整性门禁", "500/500 SHA-256 完整"),
                    ("100.0%", "DQ8 稳态与 L3 禁区合规", "99.9/100 · L3 硬拦截"),
                    ("17,391x", "W14 Mesh 有效信息密度", "Token -60.3% · 质量 100"),
                ],
                ["评测项", "安全 / 审计 / 联邦场景", "实测指标", "质量 / SLA", "状态"],
                [
                    ["W11", "全链路脱敏、防命令注入与抗投毒", "98 → 29 tok (-70.4%)", "21.5 → 100.0 (4/4)", "PASS"],
                    ["W14", "MeshEnvelope v1.0 签名防重放与沙盒", "58 → 23 tok (-60.3%)", "12.5 → 100.0 (4/4)", "PASS"],
                    ["W16", "双域审计脱敏防洪与租约自进化闭环", "14,500 → 2,400 (-83.5%)", "36.0 → 100.0 (4/4)", "PASS"],
                    ["DQ4/DQ7", "凭据卫生度 + 审计 SHA-256 完整性", "144/144 + 500/500", "100.0% / 100.0%", "PASS"],
                    ["DQ8", "本体五维稳态与自治包络合规", "稳态 99.9 · 5维合法", "100.0% (>=99.0%)", "PASS"],
                ],
                [18, 130, 560, 920, 1290],
                footnote="数据来源：Bench v3.9 run-6a0a4995-7302-4bc1-86b6-554a784a6156",
            ),
            "next": "我们在终端里跑一遍审计查询、探针自检与多体名片探测命令。",
            "script": (
                "看这三张 100.0% 的安全与审计成绩单：DQ4 凭据脱敏 100%、DQ7 双域审计完整性 100%、DQ8 稳态与 L3 禁区合规 100%，W11/W14/W16 全部满分通过。"
            ),
        },
        {
            "eyebrow": "07 · 实操命令清单",
            "title": "安全、审计、探针与 Mesh 6 条高频运维命令",
            "lead": "黑白屏 1:1 对应，Web 驾驶舱「双域审计」与「本体中枢」工作台同步可视化。",
            "svg": svg_code_and_flow(
                "安全、审计与 Mesh 实战命令",
                f"agent-body v{APP_VER} CLI",
                [
                    "# 1. 宿主机依赖与文件权限安全姿态预检",
                    "agent-body preflight",
                    "",
                    "# 2. 按双域查询审计流水、聚合统计与自进化调优",
                    "agent-body audit query --domain out_of_agent --limit 20",
                    "agent-body audit summary --days 7",
                    "agent-body audit evolve --apply",
                    "",
                    "# 3. 一键运行 P1~P5 五点对比探针",
                    "agent-body feedback probe",
                    "",
                    "# 4. 查询本地五维稳态 & 跨宿主机 Mesh 本体名片",
                    "agent-body body vitals",
                    "agent-body body peers",
                ],
                "Web 2.0 驾驶舱同步入口",
                [
                    ("#view-audit 双域审计台", "IN/OUT 双域过滤、trace_id 因果钻取、DuckDB OLAP 与一键导出", "body_audit_query"),
                    ("#view-body 本体中枢台", "五维稳态雷达、L0~L3 边界测试器与跨机名片列表", "body_state"),
                    ("#view-benchmark 探针与反馈", "一键跑 P1~P5 探针、查看 user_feedback.md 与记忆同步", "body_feedback"),
                    ("#view-governance Mesh 沙盒", "跨机图谱查询、脱敏对话与 mesh_inbox 收件箱审计", "body_mesh_query"),
                ],
            ),
            "next": "安全与自进化同样需要诚实的边界说明：哪些风险是 Agent Body 能兜的，哪些需要宿主机 OS 配合？",
            "script": (
                "这六条命令把安全预检、双域审计、探针和跨机名片串成一条完整运维线。每次怀疑「后台刚才动了啥」，敲一条 `audit query --domain out_of_agent`，清清楚楚。"
            ),
        },
        {
            "eyebrow": "08 · 安全边界",
            "title": "纵深防御的诚实边界：应用层免疫 ≠ 内核级沙箱",
            "lead": "清楚界定安全责任边界，<b>才能把 Agent Body 与 OS 容器/权限隔离正确组合</b>。",
            "svg": svg_matrix_2x2(
                [
                    {
                        "tag": "AGENT BODY GUARANTEES",
                        "title": "Agent Body 强保证的安全项",
                        "accent": C_GREEN,
                        "bg": C_GREEN_L,
                        "lines": [
                            "全链路凭据脱敏不落库、不进 Brief、不进胶囊",
                            "Pre-Hook 拦截已知高危路径、端口碰撞与锁冲突",
                            "Web 写接口鉴权 + Mesh HMAC 防重放 + 沙盒目录",
                        ],
                    },
                    {
                        "tag": "OS / SANDBOX LAYER",
                        "title": "需宿主机 OS 配合的纵深层",
                        "accent": C_CLAY,
                        "bg": C_CLAY_L,
                        "lines": [
                            "若 Agent 拥有裸 root shell 且绕过 Hook，仍需 OS 权限约束",
                            "不可信第三方代码执行建议叠加容器 / cgroups / seccomp",
                            "Mesh 共享密钥需按主机独立轮换并限制内网可达",
                        ],
                    },
                    {
                        "tag": "L1 SELF-HEAL SCOPE",
                        "title": "自愈能力的硬天花板",
                        "accent": C_TEAL,
                        "bg": C_TEAL_L,
                        "lines": [
                            "仅自动执行幂等元数据清理（过期锁/幽灵节点/向量）",
                            "绝不自动删除用户业务代码或强制 kill 未知生产进程",
                        ],
                    },
                    {
                        "tag": "AUDIT RETENTION",
                        "title": "审计账本可观测与归档",
                        "accent": C_GOLD,
                        "bg": C_GOLD_L,
                        "lines": [
                            "支持按天 prune 清理冷审计，热数据导出 Parquet 长期留存",
                            "SHA-256 存证支持离线复核完整性",
                        ],
                    },
                ],
                "最佳实践：OS 容器/最小权限管住底层执行边界，Agent Body 管住认知、反思与审计免疫层。",
            ),
            "next": "安全与免疫体系闭环后，收束第 5 讲，准备迎接压轴第 6 讲——三轨实证 Benchmark v3.9。",
            "script": (
                "做安全架构最忌讳夸大其词。Agent Body 是认知与反射层的纵深免疫，它不能替代 Linux 用户隔离和容器沙箱；两者叠在一起，才构成生产级 AI 宿主机的完整防御体系。"
            ),
        },
        {
            "eyebrow": "09 · 演进回顾",
            "title": "从 Phase 32 到 Phase 44：安全与自进化的四阶跃迁",
            "lead": "每一阶演进都不是拍脑袋加功能，而是<b>从现网真实痛点反推出来的硬需求</b>。",
            "svg": svg_pipeline(
                [
                    {"tag": "PHASE 32", "title": "纵深安全硬化", "accent": C_TEAL_D, "lines": ["SecretRedactor 扩展", "抗投毒与 Web 401 鉴权", "0700/0600 权限收敛"]},
                    {"tag": "PHASE 38~39", "title": "新鲜度与 P1~P5 探针", "accent": C_TEAL_2, "lines": ["7 大陈旧漏斗清零", "5 切面微秒级对比探针", "自动驱动反馈总账"]},
                    {"tag": "PHASE 41 & 43", "title": "Mesh v1.0 + 双域审计", "accent": C_GREEN, "lines": ["HMAC 防重放 + 沙盒投递", "IN/OUT 双域 60s 折叠", "审计驱动租约升降级"]},
                    {"tag": "PHASE 44", "title": "五维稳态与 L0~L3", "accent": C_GOLD, "lines": ["HomeostasisVitals 雷达", "L1 自主自愈反射", "多体本体名片发现"]},
                ],
                [
                    {
                        "title": "演进心法：每一条规约都有代码测试与 Bench 门禁兜底",
                        "accent": C_TEAL,
                        "bg": C_TEAL_L,
                        "lines": [
                            "没有任何一项安全或自进化能力只停留在文档里——全部绑定 W11/W14/W16/DQ4/DQ7/DQ8 自动回归",
                            "生产代码执行 make lint-panic 零 unwrap/expect 硬门禁",
                        ],
                    },
                ],
                mid="ah510",
            ),
            "next": "看一眼全链路安全免疫如何接入第 6 讲的实证评测体系。",
            "script": (
                "回头看 Phase 32 到 Phase 44 的演进路径，每一步都非常扎实：先补安全底线，再治数据发霉、埋探针，再建跨机零信任与双域审计，最后收敛成五维稳态和 L0~L3 包络。"
            ),
        },
        {
            "eyebrow": "10 · 常见安全问答",
            "title": "企业架构师最常问的三个安全与合规问题",
            "lead": "直面真实生产顾虑，给出可验证的工程答案。",
            "svg": svg_columns(
                [
                    {
                        "tag": "Q1 · 数据出境",
                        "title": "代码和记忆会上传云端吗？",
                        "accent": C_GREEN,
                        "lines": [
                            "答：100% 本地运行，零遥测、零云端上报",
                            "L2 标注与 Embedding 全走本地 Ollama",
                            ("断网环境仍 100% 正常工作", True),
                        ],
                    },
                    {
                        "tag": "Q2 · 误删风险",
                        "title": "后台自愈会删我代码吗？",
                        "accent": C_TEAL,
                        "lines": [
                            "答：L1 自愈仅操作元数据与过期锁",
                            "GC 清理 tmp 默认 dry_run=true 预览",
                            ("L3 路径与生产目录硬拦截", True),
                        ],
                    },
                    {
                        "tag": "Q3 · 审计合规",
                        "title": "审计日志能导出对接 SIEM 吗？",
                        "accent": C_GOLD,
                        "lines": [
                            "答：支持 DuckDB 一键导出 Parquet/CSV",
                            "每条含 trace_id、domain 与 SHA-256",
                            ("满足企业内审与事后复盘要求", True),
                        ],
                    },
                ],
                (f"所有安全承诺均可直接翻阅开源代码与 {TEST_CNT} 项全工作区测试逐行核验。", "Apache-2.0 全透明开源", "teal"),
            ),
            "next": "收束第 5 讲，准备进入整个系列的压轴终章——第 6 讲。",
            "script": (
                "这三个问题是我在和企业架构师交流时被问得最多的：数据不出境、自愈不动业务代码、审计支持标准 Parquet 导出对接现有安全平台。全部开源可审计，没有黑盒。"
            ),
        },
        {
            "eyebrow": "11 · 免疫飞轮总结",
            "title": "免疫飞轮：防得住、查得清、治得好、连得稳",
            "lead": "四大能力首尾相扣，让 Agent 本体具备 7×24 小时无人值守的工程韧性。",
            "svg": svg_bars(
                "安全、审计与稳态核心门禁实测达标全景（Bench v3.9）",
                [
                    ("DQ4 库表凭据脱敏卫生度（目标 >= 100%）", 100.0, "100.0%", "PASS · 0 泄露", C_GREEN),
                    ("DQ7 双域行为审计 SHA-256 完整性（目标 >= 99%）", 100.0, "100.0%", "PASS · 500/500", C_GREEN),
                    ("DQ8 本体五维稳态与 L3 禁区合规（目标 >= 99%）", 100.0, "100.0%", "PASS · 99.9分", C_TEAL_D),
                    ("W11 纵深安全上下文质量（基线 21.5）", 100.0, "100.0", "PASS · +78.5分", C_TEAL_2),
                    ("W14 Mesh v1.0 零信任上下文质量（基线 12.5）", 100.0, "100.0", "PASS · +87.5分", C_TEAL_3),
                    ("W16 双域审计与自进化上下文质量（基线 36.0）", 100.0, "100.0", "PASS · +64.0分", C_TEAL_3),
                ],
                "六大安全与免疫硬指标全部 100% 满格通过，拒绝「安全靠口头承诺」。",
            ),
            "next": "进入第 5 讲收束，引出压轴第 6 讲——如何用三轨实证 Benchmark 与工程纪律守住每一次版本迭代。",
            "script": (
                "整页六条全部 100% 满格。把安全、审计和自进化全部指标化、自动化，才配叫生产级底座。"
            ),
        },
        {
            "eyebrow": "12 · 收束",
            "title": "信任不是靠口号，是靠脱敏、哈希与可复现审计",
            "lead": "当 Agent 本体同时拥有<b>免疫防线、双域账本、自进化探针与零信任联邦</b>，你才敢真正放手让它 24 小时守护宿主机。",
            "svg": svg_closing(
                kicker="KEY TAKEAWAYS · 第 05 讲收束",
                headline="纵深免疫与自进化：让宿主机本体既固若金汤，又越跑越聪明",
                pillars=[
                    ("纵深四闸", "脱敏 + 抗投毒 + 防注入 + 0600/401 鉴权", "DQ4 100% · W11 PASS"),
                    ("双域闭环", "IN/OUT 审计 60s 折叠 + P1~P5 探针 + 租约自进化", "DQ7 100% · W16 PASS"),
                    ("零信任联邦", "MeshEnvelope v1.0 HMAC 防重放 + 沙盒投递 + 名片", "W14 PASS · 17,391x 密度"),
                ],
                open_question="开放思考：我们在前五讲报出了一堆「省 99% Token」「质量 99.8 分」的数字——你怎么确信这些数字不是挑好听的编出来的？",
                next_talk="压轴终章 → 第 06 讲：三轨实证 Benchmark v3.9 与可复现工程体系（06-empirical-benchmark-engineering.html）",
            ),
            "script": (
                "第 5 讲到这里结束。最后我替最挑剔的听众问自己一个尖锐问题：前五讲你晒了这么多漂亮数字，怎么证明不是挑软柿子捏、不是把上下文截断换来的假省钱？\n"
                "下一讲也是本系列压轴终章，我们把三轨实证 Benchmark v3.9 和整套防漂移工程体系彻底摊在阳光下！"
            ),
        },
    ]
    render_deck(
        doc_title="第 05 讲 · 纵深安全、双域审计与探针驱动自进化 — Agent Body",
        brand_label="SERIES 05 / 06 · 安全与进化",
        foot_label=f"AGENT BODY · 第 05 讲：纵深安全、双域审计与探针驱动自进化 (v{APP_VER})",
        slides_data=slides,
        out_path=OUT / "05-security-audit-evolution-mesh.html",
    )


def build_talk_06() -> None:
    slides = [
        {
            "cover": True,
            "eyebrow": "封面 · 第 06 讲",
            "title": "三轨实证 Benchmark v3.9 与可复现工程体系",
            "svg": svg_cover(
                kicker=f"SERIES 06 / 06 · AGENT BODY · v{APP_VER} · 30 MIN",
                title="三轨实证 Benchmark 与工程体系",
                en_sub="Pareto Token-Quality Gate, Live Competitor Bench & Zero-Drift Ops",
                subtitle1="拒绝自嗨口号：既要省 Token，更要上下文质量不降反升",
                subtitle2=f"帕累托双门禁 Q · {WL_CNT} 负载 + {MICRO_CNT} 微基准 + {DQ_CNT} DQ + {COMP_CNT} 竞品 · 微内核:{KERNEL_CNT} 周边:{CONTRIB_CNT}",
                spec_line=f"Spec v{BENCH_VER} · SHA-256 存证 · {TEST_CNT} 测试 0 失败 · 0 Panic · 0 Drift",
                strata=[
                    ("帕累托双门禁 · 拒绝截断作弊", "Q = 40% 事实 + 30% 信噪 + 20% 行动 + 10% 安全", "#EBF1EE", C_INK),
                    ("轨道 A · 19 负载 + 6 微基准 + 10 DQ", "W1~W19 A/B 真跑 · M1~M6 微秒 SLA · DQ1~DQ10 宿主与遥测门禁", "#CFE0DC", C_INK),
                    ("轨道 B · PR-01~14 生产效能账本", "每一次真实 MCP/CLI/Web/Hook 调用自动归因记账", C_TEAL_D, C_PAPER),
                    ("轨道 C · COMP-C1~C4 热门竞品同跑", "对标 Mem0 / OpenViking / AgentFS / CLI 组合同机实测", C_TEAL_2, C_PAPER),
                    ("零漂移工程纪律 · Zero-Rescan", "EVOLUTION_GUIDE · evolution_manifest.py · lint-panic · 43/43 Parity", C_TEAL_3, C_PAPER),
                    ("一键复现 · 2 分钟本地独立验真", "make check / test / evolution-check / bench run --suite full", "#A4C4C6", C_INK),
                ],
                right_tag="第 6 讲 · 30 分钟压轴终章 · 地层剖面规范",
            ),
            "script": (
                "欢迎来到系列演讲的压轴第 6 讲。前五讲我们把 Agent 本体的理念、感官、小脑、海马体和免疫系统全拆了一遍。\n"
                "今天 30 分钟，我们聊最体现工程师骨气的一件事：怎么用一套防作弊、可复现、带 SHA-256 存证的三轨实证 Benchmark，和一套零漂移工程纪律，证明这个项目不是 PPT 玩具，而且你随时可以在自己机器上 2 分钟复现出完全一致的结果。"
            ),
        },
        {
            "eyebrow": "01 · 帕累托双门禁",
            "title": "帕累托二维象限：省 99.23% Token 的同时，质量跃升 98.0",
            "lead": "严禁靠截断信息骗压缩率 · <b>落入右下角红区直接门禁熔断</b>",
            "svg": vis_benchmark_pareto(animated=True),
            "anim_caps": [
                "Step 1/3：二维帕累托象限——裸奔大脑落左下 35.6 分，本体跃迁右上黄金区 98.0 分",
                "Step 2/3：三大实证圆环——Token 省 99.23%、质量飙升 +64.2 分、39/39 门禁全绿",
                "Step 3/3：内核微秒级 SLA——无锁探针 7μs、放置 33μs、3 跳图 139μs、FTS5 259μs",
            ],
            "next": "基于这套防作弊标尺，我们搭建了「A/B 对照 + 生产账本 + 竞品同跑」三轨实证体系。",
            "script": (
                "AI 领域最泛滥的营销话术就是「省了 95% Token」。我每次看到都想问一句：你是不是把关键信息全截断了？\n"
                "Phase 36 我们把 Benchmark 彻底重构为帕累托双门禁：每个场景写死客观真值断言，用四维公式算基线和赋能的上下文质量分 Q。既要省 Token，质量还得不降反升，少一条必含事实直接红灯。这就把所有偷工减料的路全堵死了。"
            ),
        },
        {
            "eyebrow": "02 · 三轨实证全景",
            "title": "轨道 A × 轨道 B × 轨道 C：实验室、现网、竞品三维交叉验真",
            "lead": "单靠实验室跑分不够客观；<b>把实验室 A/B、日常生产流水和开源竞品拉到同一坐标系</b>。",
            "svg": svg_columns(
                [
                    {
                        "tag": "TRACK A · 标准可控 A/B",
                        "title": "19 负载 + 6 微基准 + 10 DQ",
                        "accent": C_TEAL_D,
                        "bg": C_TEAL_L,
                        "lines": [
                            "W1~W19 覆盖 6 大器官与 AWR 诊断",
                            "M1~M6 锁定微秒级内核与无锁探针 SLA",
                            "DQ1~DQ10 直查宿主机库与内生遥测健康",
                            ("一键全量回归，帕累托质量零降级", True),
                        ],
                        "foot_lbl": "实测结果",
                        "foot_val": "35 / 35 全部 PASS",
                    },
                    {
                        "tag": "TRACK B · 生产效能账本",
                        "title": "PR-01 ~ PR-14 自动记账",
                        "accent": C_GREEN,
                        "bg": C_GREEN_L,
                        "lines": [
                            "日常每次 MCP/CLI/Web/Hook 自动记账",
                            "严格区分「真实省下的」与「演示沙盒」",
                            "按 24h / 7d / 30d / all 聚合 ROI",
                            ("含 PR-13 接力归因与 PR-14 AWR 账本", True),
                        ],
                        "foot_lbl": "源码触点",
                        "foot_val": "src/benchmark/{provenance,ledger}.rs",
                    },
                    {
                        "tag": "TRACK C · 竞品横向对标",
                        "title": "COMP-C1 ~ C4 同机真跑",
                        "accent": C_CLAY,
                        "bg": C_CLAY_L,
                        "lines": [
                            "C1 对标 Mem0/JSON 裸记忆基线",
                            "C2 对标 OpenViking 摘要 + 全量技能",
                            "C3 对标 AgentFS SQLite VFS 导入",
                            "C4 对标 rg + psutil + ss CLI 组合",
                        ],
                        "foot_lbl": "诚实标注",
                        "foot_val": "LIVE_BENCHMARKED 4/4 PASS",
                    },
                ],
                ("实验室防退化、生产账本看真 ROI、竞品对标知己知彼——三轨缺一不可。", "Benchmark Spec v3.9", "teal"),
            ),
            "next": "让我们逐一展开轨道 A 的 19 大负载与 6 大微基准真实成绩。",
            "script": (
                "为什么设计三轨？因为实验室跑分容易脱离实际，所以加了轨道 B 生产效能账本，日常每一次真实调用都按 PR-01 到 PR-14 自动记账；光跟裸奔自己比又容易井底之蛙，所以加了轨道 C 同机实跑主流开源竞品。"
            ),
        },
        {
            "eyebrow": "03 · 轨道 A (一)",
            "title": "W1 ~ W19 全景成绩单：19/19 全部帕累托双赢",
            "lead": "总消耗 <b>650,073 → 5,065 tok（-99.22%）</b>，D6 上下文质量 <b>35.8 → 99.8/100（+64.0）</b>，信息密度 <b>92.1x</b>。",
            "svg": svg_metrics_table(
                [
                    ("-99.22%", "总 Prompt Token 节省", "节省 645,008 tok"),
                    ("+64.0 分", "上下文质量净胜 ΔQ", "35.8 → 99.8 / 100"),
                    ("57 次", "事前拦截风险总数", "端口/锁/注入/L3"),
                    ("531 个", "阻断垃圾噪音文件", "Shallow Tier 硬阻断"),
                ],
                ["代表负载", "核心覆盖器官", "Token (基线 → 赋能)", "质量 (基线 → 赋能)", "密度 / 状态"],
                [
                    ["W1 / W2 / W3", "物理感官：工具 · 端口 · 分级扫盘", "27,804 → 413 (-98.5%)", "51.2 → 100.0", "160.9x · PASS"],
                    ["W4 / W5 / W6", "规约与协作：放置 · 爆炸半径 · 死锁", "1,670 → 325 (-80.5%)", "25.1 → 100.0", "27.7x 速 · PASS"],
                    ["W7 / W8 / W9", "路由与海马体：百级技能 · Brief · RRF", "602,799 → 1,608 (-99.7%)", "39.2 → 98.9", "838.0x · PASS"],
                    ["W11~W15", "安全 · 新鲜度 · 加权图 · Mesh · AOPCC", "844 → 125 (-85.2%)", "26.6 → 100.0", "17391x · PASS"],
                    ["W16 / W17", "双域审计自进化 · 跨大脑 1ms 接力", "16,900 → 2,575 (-84.8%)", "36.5 → 100.0", "54.9x · PASS"],
                ],
                [18, 180, 560, 880, 1180],
                footnote="实测批次：run-6a0a4995-7302-4bc1-86b6-554a784a6156 · 全部 17 项均 100% 命中真值事实断言",
            ),
            "next": "大场景赢了，底层 Rust 内核的微秒级延迟和宿主机数据质量门禁守得住吗？",
            "script": (
                "把 19 个负载分组铺开看非常震撼：不管在感官、小脑、海马体、安全还是跨脑接力上，没有哪怕一个负载是靠牺牲质量换省 Token 的。平均质量从 35.8 飙升到 99.8，几乎全部满分。"
            ),
        },
        {
            "eyebrow": "04 · 轨道 A (二)",
            "title": "M1 ~ M6 微秒内核 SLA 与 DQ1 ~ DQ10 现网宿主机门禁",
            "lead": "全部 6 项微基准远低于 P99 上限；<b>全部 10 项现网真实数据库质量门禁 100% 绿灯</b>。",
            "svg": svg_columns(
                [
                    {
                        "tag": "M1 ~ M6 · 内核微基准",
                        "title": "P99 全面优于 SLA 上限",
                        "accent": C_TEAL_D,
                        "lines": [
                            "M1 3 跳图谱遍历：P99 430μs (<=1500)",
                            "M2 FTS5 资产检索：P99 914μs (<=1500)",
                            "M3 放置规则决策：P99 103μs (<=400)",
                            "M4 DuckDB 热力图：P99 15.7ms (<=60ms)",
                            ("M5 Parquet 导出：12.7ms (<=150ms)", True),
                        ],
                        "foot_lbl": "微基准结论",
                        "foot_val": "6 / 6 PASS · 零性能退化",
                    },
                    {
                        "tag": "DQ1 ~ DQ4 · 感官与安全卫生",
                        "title": "真实生产库抽检全绿",
                        "accent": C_GREEN,
                        "lines": [
                            "DQ1 资产描述元数据：100.0% (1000/1000)",
                            "DQ2 Ollama 向量覆盖：99.5% (199/200)",
                            "DQ3 依赖拓扑物理有效：100.0% (2000/2000)",
                            "DQ4 库表凭据零明文：100.0% (144/144)",
                            ("实机 ~/.agent-body/body.db 直测", True),
                        ],
                        "foot_lbl": "上半区结论",
                        "foot_val": "4 / 4 PASS",
                    },
                    {
                        "tag": "DQ5 ~ DQ8 · 新鲜度、审计与稳态",
                        "title": "全链路一致性与免疫全绿",
                        "accent": C_TEAL_2,
                        "lines": [
                            "DQ5 新鲜度现实对账：99.9% (726/727)",
                            "DQ6 图谱分层与连通：100.0% (215/215)",
                            "DQ7 双域审计 SHA-256：100.0% (500/500)",
                            "DQ8 五维稳态 & L3 禁区：100.0% (99.9分)",
                            ("真正经得起现网拷问的数据质量", True),
                        ],
                        "foot_lbl": "下半区结论",
                        "foot_val": "4 / 4 PASS",
                    },
                ],
                ("DQ1~DQ10 跑的不是假造的 Mock 数据，而是我主力开发机上 21,230 个真实资产与 441 个事实节点。", "真实生产宿主机直测", "green"),
            ),
            "next": "再看轨道 C：把市面上热门开源竞品拉到同一台机器上实测，结果一目了然。",
            "script": (
                "这里我特别想强调 DQ2：今天下午刚跑完 Phase 45 时，因为我们新写了一批文件还没来得及向量化，DQ2 掉到了 82.5%，门禁毫不留情地亮了红灯；补跑完本地 Ollama 向量化，DQ2 立刻回到 99.5% 全绿。真正的门禁就是敢在自己身上亮红灯。"
            ),
        },
        {
            "eyebrow": "05 · 轨道 C 竞品实测",
            "title": "COMP-C1 ~ C4：四大赛道同机真跑 VS 视觉擂台",
            "lead": "严格区分 LIVE_BENCHMARKED 与 ARCH_VERIFIED · <b>红绿柱一目了然</b>",
            "svg": vis_competitor_arena(animated=True),
            "anim_caps": [
                "Step 1/4：C1 长期记忆——0 明文泄露 (-97.8% tok)，SPI 开放兼容 Mem0/Letta",
                "Step 2/4：C2 上下文路由——OpenViking 144,730 tok vs 本体零副本 931 tok",
                "Step 3/4：C3 工作区治理——AgentFS 290 全吞 vs 本体 10 净入拦黑名单",
                "Step 4/4：C4 宿主探针——多子进程 89ms 0 跳 vs 本体 20ms 单进程 + 3 跳图",
            ],
            "next": "跑完一次全绿很容易，难的是迭代到 46 个 Phase、27 个模块后，文档、代码、测试依然零漂移——我们怎么做到的？",
            "script": (
                "我们在竞品对标里守一条学术底线：跑了真代码的才标 `LIVE_BENCHMARKED`，看源码对比的标 `ARCH_VERIFIED`，绝不混着吹。四条赛道同机跑完，Agent Body 在脱敏安全、零副本省 Token、白黑灰防噪音和零子进程拓扑上的优势一目了然。"
            ),
        },
        {
            "eyebrow": "06 · 零漂移工程体系",
            "title": "7 道确定性机器门禁流水线：让大仓演进 75 期永不腐烂",
            "lead": f"evolution-check · arch-check · 0 unwrap · <b>{TEST_CNT} 全测 · Playwright 9/9</b>",
            "svg": vis_quality_gates(test_cnt=TEST_CNT, mod_cnt=MOD_CNT, tool_cnt=TOOL_CNT, animated=True),
            "anim_caps": [
                f"Step 1/2：顶部 7 道全自动门禁锁死零漂移、零内核污染、零 Panic 与 {TEST_CNT} 项全绿",
                "Step 2/2：底部交付账单——6.7MB 零 C++ 探针、100% 位置无关与 2 轮零重扫定位",
            ],
            "next": "连你现在看到的这 9 份 HTML 演讲稿，也纳入了自动化视觉门禁！",
            "script": (
                f"很多人好奇：一个人带 AI 迭代 75 个 Phase、写数万行 Rust、微内核加 {CONTRIB_CNT} 个插件和几十份文档，为什么没有烂尾成屎山？\n"
                f"秘诀就是把「靠自觉」全部替换成「确定性门禁」：`make evolution-check` 扫源码并做 A~G 交叉核对，`make arch-check` 防微内核跨层污染，`make lint-panic` 扫生产代码禁 `unwrap()`，`cargo test --workspace` 跑全 {TEST_CNT} 项测试。"
            ),
        },
        {
            "eyebrow": "07 · 视觉自动化门禁",
            "title": "Playwright 无头渲染验真：连 PPT 都要跑 0 溢出、0 重叠测试",
            "lead": "规约要求对 HTML/SVG 视觉产出精益求精。<b>scripts/verify_presentations.py 逐页实测 9 份 HTML</b>。",
            "svg": svg_code_and_flow(
                "scripts/verify_presentations.py",
                "scripts/verify_presentations.py (Playwright)",
                [
                    "# 对 presentation/*.html 全部 9 份文件逐页启动真实 Chromium：",
                    "1. 捕获 console.error / pageerror -> 必须为 0",
                    "2. 逐页检查 .slide scrollWidth <= clientWidth + 2 (0 溢出)",
                    "3. 展开全部动画 step，检查每个 SVG <text> 严格落在 viewBox 内",
                    "4. 两两计算所有可见 SVG <text> 的 getBoundingClientRect() 交集",
                    "   -> 若交集 > 4x4px 立即报 exact overlap 坐标与文本！",
                    "5. 实测键盘 G(目录) / N(单页讲稿) / T(全文讲稿) / Esc 交互",
                ],
                "确定性交付闭环",
                [
                    ("同一套地层剖面引擎", "scripts/deck_engine.py 统一生成，保证 6 讲与旧双 Deck 100% 视觉同源", "0 外部依赖 · 0 Emoji"),
                    ("零肉眼盲猜", "不是「我觉得应该没重叠」，而是真实浏览器像素级 BoundingBox 断言", "Chromium Headless 实测"),
                    ("随时重新生成", "后续 Phase 升级新指标，改数据重跑脚本 + Playwright 一分钟验完", "可复用资产沉淀"),
                ],
            ),
            "next": "为了践行「极度执着于可复现」，下一页给你一份 2 分钟一键验真清单。",
            "script": (
                "我们连演讲 PPT 都不靠肉眼盲猜。你现在看到的这套 6 讲专题加总览页，全部用 `scripts/verify_presentations.py` 启动真实 Chromium 逐页遍历了一遍：检查有没有 JS 报错、有没有文字冲出 SVG 画布、有没有两行文字框打架重叠、快捷键能不能正常呼出目录和讲稿。"
            ),
        },
        {
            "eyebrow": "08 · 2 分钟复现指南",
            "title": "可复现性宣言：拿到仓库，2 分钟独立验真全部结论",
            "lead": "不靠我口述，<b>任何人克隆仓库后按以下 5 条命令，即可独立复现全部测试与 Benchmark 数字</b>。",
            "svg": svg_code_and_flow(
                "从零独立复现全套门禁与基准",
                "Makefile · README.md",
                [
                    "# 1. 校验六重质量门禁与微内核防腐边界",
                    "make quality-gate",
                    "",
                    f"# 2. 全工作区 {TEST_CNT} 项单元与集成测试",
                    "RUST_TEST_THREADS=1 make test",
                    "",
                    "# 3. 按需构建零 C++ 精简探针并执行离线自检",
                    "cargo build --release -p agent-body-probe && ./target/release/agent-body-probe self-check",
                    "",
                    "# 4. 独立运行 Benchmark v3.9 全量三轨评测（~6 秒）",
                    "./target/release/agent-body bench run --suite full --save false",
                    "",
                    "# 5. 启动 Web 2.0 驾驶舱与 /metrics 端点（默认 :8765）",
                    "./target/release/agent-body web",
                ],
                "复现环境极简要求",
                [
                    ("旗舰与精简版任选", "旗舰版含 DuckDB/WebUI；probe(6.7M)/dba(6.4M)/minimal(3.9M) 零 C++", "SQLite 静态内嵌"),
                    ("增强可选", "本机安装 Ollama (nomic-embed-text + qwen3.5:0.8b) 解锁 L2/L3", "离线自动优雅降级"),
                    ("零污染跑批", "--save false 在隔离内存/临时沙箱执行 W1~W19 与 COMP-C1~C4", "不污染宿主机文件"),
                    ("验证本套 PPT", "make verify-decks (scripts/verify_presentations.py)", "9 份 HTML 逐页验真"),
                ],
            ),
            "next": "看一眼整个仓库目前的物理工程规模与健康指标快照。",
            "script": (
                f"对可复现的执着，是我们区分「玩具 Demo」和「基础设施」的分水岭。这五条命令复制粘贴跑完，你会亲眼看到 0 drift、0 panic、全工作区 {TEST_CNT} 项测试零失败、6.7MB 零 C++ 探针自检通过和一份带 SHA-256 指纹的 v{BENCH_VER} 实测报告。"
            ),
        },
        {
            "eyebrow": "09 · 工程体检全景",
            "title": f"v{APP_VER} 完整工程快照：{MOD_CNT} 模块 · 微内核/{CONTRIB_CNT} 扩展 · {TEST_CNT} 测试",
            "lead": "小体量单二进制家族，同时具备数据库级存储抽象与微内核模块化扩展密度。",
            "svg": svg_metrics_table(
                [
                    (f"{MOD_CNT} 模块", "旗舰内核模块数", f"{TOOL_CNT} MCP · {SPI_CNT} SPI · {TABLE_CNT} 表"),
                    (f"{KERNEL_CNT} / {CONTRIB_CNT} / {DIST_CNT}+1", "微内核·扩展·发行版", "probe 6.7M · 0 C++"),
                    (f"{TOOL_CNT} / {TOOL_CNT}", "黑白屏 1:1:1 同步率", "MCP / CLI / Web + /metrics"),
                    (f"{TEST_CNT} / 0", "全工作区测试全绿", "0 failed · 0 unwrap"),
                ],
                ["架构切面", "精确规模与实测状态", "对应硬门禁", "状态"],
                [
                    ["存储与基准", f"{TABLE_CNT} 表 · {SPI_CNT} SPI · Bench v{BENCH_VER} (W1~19 / M1~6 / DQ1~10)", "duckdb + spi + bench 测试", "PASS"],
                    ["协议与可观测", f"{TOOL_CNT} 原生 MCP · {CLI_CNT} CLI · Web 9 工作台 · /metrics", "test_web_parity + body metrics", "PASS"],
                    ["微内核/扩展", f"{KERNEL_CNT} kernel · {CONTRIB_CNT} contrib 五件套 · {DIST_CNT} 精简版 3.9~6.7M", "make arch-check + self-check", "PASS"],
                    ["文档与演说", "EVOLUTION_GUIDE + 手册 + 3 SKILL + 9 份 HTML 演讲", "evolution-check A~G + Playwright", "PASS"],
                ],
                [18, 220, 820, 1290],
                footnote=f"基线版本：agent-body v{APP_VER} (Phase 1 ~ Phase 75) · Apache-2.0",
            ),
            "next": "回顾从「#27 本体升维 → #52 微内核 → #54 全一等公民大一统」的零返工演进脉络。",
            "script": (
                f"这是 Phase 75 收官时的完整工程体检表：{MOD_CNT} 个旗舰模块、{TOOL_CNT} 个原生工具、{SPI_CNT} 个存储 SPI、加上 {KERNEL_CNT} 个纯机制微内核 crate、{CONTRIB_CNT} 个标准五件套 `contrib` 扩展、{DIST_CNT} 款零 C++ 精简发行版，以及全工作区 {TEST_CNT} 项测试零失败。"
            ),
        },
        {
            "eyebrow": "10 · 微内核与周边分离",
            "title": f"PostgreSQL 式微内核：{KERNEL_CNT} 核 × {CONTRIB_CNT} 磁吸扩展 → 4 大发行版",
            "lead": "Phase 70~75 · <b>先收敛再拆微内核，最后以交叉铁门禁全栈大一统</b>",
            "svg": vis_microkernel_hub(animated=True),
            "anim_caps": [
                f"Step 1/3：中心六边形纯机制微内核（{KERNEL_CNT} Crates · 8 Catalog · 6 Routine · 7 Hook）",
                f"Step 2/3：{CONTRIB_CNT} 个标准五件套 Contrib 扩展磁吸挂载，0 内核业务污染",
                "Step 3/3：四座积木塔按需装配——服务器与政务云直接上 6.7MB 零 C++ 探针",
            ],
            "next": "最后，谈谈我们对「AI 时代基础软件」的终局判断。",
            "script": (
                f"看这条从 Phase 44 走到 Phase 75 的演进脉络：先做本体升维与 `<1000L` 模块拆分，再在第 70~73 期完成 PostgreSQL 式微内核与周边分离，随后在第 74~75 期用 ISSUE #53/#54 补齐 `obs_telemetry` 可观测扩展、Prometheus `/metrics` 与 A~G 全一等公民交叉验证铁门禁，全程零返工。"
            ),
        },
        {
            "eyebrow": "11 · 开源共建倡议",
            "title": "开源社区如何参与共建 Agent 本体底座",
            "lead": "Apache-2.0 协议开放全部源码、设计文档、Benchmark 套件与演讲体系，欢迎全球工程师同行。",
            "svg": svg_columns(
                [
                    {
                        "tag": "FOR USERS · 5 分钟上手",
                        "title": "给你的主力机装上身体",
                        "accent": C_TEAL_D,
                        "lines": [
                            "cargo build --release 或一键脚本",
                            "接入 Claude Code / Antigravity / Cursor",
                            "体验开工简报、端口防撞与换脑接力",
                        ],
                    },
                    {
                        "tag": "FOR CONTRIBUTORS · 贡献代码",
                        "title": "按 EVOLUTION_GUIDE 零重扫开发",
                        "accent": C_GREEN,
                        "lines": [
                            "扩展 macOS/Windows 原生探针细节",
                            "接入更多 Memory SPI 生态适配器",
                            "提交新的真实宿主 Benchmark 负载",
                        ],
                    },
                    {
                        "tag": "FOR SPEAKERS · 布道传播",
                        "title": "直接复用本套 6 讲离线 HTML",
                        "accent": C_CLAY,
                        "lines": [
                            "零外部依赖，离线单文件直接投屏",
                            "内置 N/T 口播讲稿与分步动画",
                            "适合技术大会、团队内训与视频号",
                        ],
                    },
                ],
                ("仓库地址：https://github.com/digoal/agent-body · 欢迎 Star、Issue 与 PR。", "Apache-2.0 Open Source", "teal"),
            ),
            "next": "来到整个 6 讲系列的最终收束页。",
            "script": (
                "无论你是想在自己电脑上立刻提效的开发者，想贡献代码的 Rust/AI 工程师，还是想在团队内部布道脑体分离架构的技术负责人，这个仓库里的代码、文档、Benchmark 和这套 6 讲离线 PPT，全部 Apache-2.0 开源免费用。"
            ),
        },
        {
            "eyebrow": "12 · 系列终章",
            "title": "把确定性留给本体，把想象力还给大脑",
            "lead": "6 讲 × 30 分钟至此圆满收官。愿每一台开发机上的 AI 大脑，都能拥有一具<b>清醒、敏锐、克制、可信</b>的物理身体。",
            "svg": svg_closing(
                kicker="SERIES FINALE · 第 06 讲收束（全系列完）",
                headline="Agent Body：用数据库级工程纪律，锻造 AI 时代的宿主机具身底座",
                pillars=[
                    ("帕累托实证", "Token -99.22% × 质量 35.8→99.8 × 密度 92.1x", "W1~W19 · M1~M6 · DQ1~DQ10 全面对齐"),
                    ("零漂移纪律", f"evolution-check + arch-check + {TEST_CNT} 测试 + Playwright", "可独立 2 分钟完整复现"),
                    ("长期主义", "脑体分离、递弱代偿、三层兼容、Apache-2.0 开源", "github.com/digoal/agent-body"),
                ],
                open_question="终局之问：如果五年后每台个人电脑与服务器都默认运行着一层「Agent 本体底座」，它最不可或缺的三条标准协议应该是什么？",
                next_talk="返回系列导航总览 → presentation/index.html · 或体验路演总览 presentation.html",
            ),
            "script": (
                "六讲整整三个小时的体系化分享到这里圆满收官。\n"
                "我们从脑体分离的哲学出发，一路拆解了物理感官图谱、AOPCC 小脑反射、海马体新鲜度与跨大脑接力、纵深安全与双域自进化，最后用三轨实证 Benchmark 和零漂移工程体系完成闭环。\n"
                "把确定性留给本体，把想象力还给大脑。谢谢每一位听完这六讲的朋友，期待在 GitHub Issue 和 PR 里与大家继续切磋！"
            ),
        },
    ]
    render_deck(
        doc_title="第 06 讲 · 三轨实证 Benchmark v3.9 与可复现工程体系 — Agent Body",
        brand_label="SERIES 06 / 06 · 实证工程",
        foot_label=f"AGENT BODY · 第 06 讲：三轨实证 Benchmark v3.9 与可复现工程体系 (v{APP_VER})",
        slides_data=slides,
        out_path=OUT / "06-empirical-benchmark-engineering.html",
    )


def build_index_hub() -> None:
    """Build visual-first presentation/index.html gallery hub with bespoke architecture banner & icons."""
    cards = [
        ("01", "脑体分离与 Agent 本体哲学", "01-brain-body-separation.html", "30 min · 14 页", "brain", C_TEAL_D,
         ["比较优势天平", "六大器官解剖", "递弱代偿红线"]),
        ("02", "宿主机物理感官图谱与零污染扫盘", "02-host-reality-graph.html", "30 min · 14 页", "radar", C_TEAL,
         ["0.2ms /proc 探针", "敲门不进屋", "CodeGraph 显微镜"]),
        ("03", "小脑反射与自适应并发控制 AOPCC", "03-cerebellum-and-aopcc.html", "30 min · 13 页", "bolt", C_CLAY,
         ["ETC 乐观立交", "<2ms 碰壁秒愈", "WFG 死锁破环"]),
        ("04", "海马体保鲜与跨大脑 10ms 接力", "04-hippocampus-freshness-continuity.html", "30 min · 13 页", "capsule", C_GREEN,
         ["三级写穿净水", "150t 发光胶囊", "三引擎热迁移"]),
        ("05", "纵深安全、双域审计与多体联邦", "05-security-audit-evolution-mesh.html", "30 min · 13 页", "shield", C_TEAL_2,
         ["四道安全铁闸", "60s 折叠漏斗", "P1~P5 飞轮 / Mesh"]),
        ("06", f"三轨实证 Benchmark v{BENCH_VER} 与微内核工程", "06-empirical-benchmark-engineering.html", "30 min · 13 页", "trophy", C_GOLD,
         ["帕累托 -99.2%", "四赛道 VS 擂台", f"{TEST_CNT} 测 · 微内核"]),
    ]

    def _mini_icon_svg(ic_name: str, col: str) -> str:
        return (
            f'<svg width="46" height="46" viewBox="0 0 60 60" xmlns="http://www.w3.org/2000/svg">'
            f'{icon_badge(ic_name, 30, 30, 26, col)}'
            f"</svg>"
        )

    cards_html = "".join(
        f"""<a class="deck-card" href="{href}" style="--acc:{col}">
          <div class="dc-head">
            {_mini_icon_svg(ic, col)}
            <div class="dc-top-col"><span class="dc-no">SERIES {no} / 06</span><span class="dc-meta">{meta}</span></div>
          </div>
          <h3>{title}</h3>
          <div class="chips">{"".join(f'<span class="chip">{t}</span>' for t in tags)}</div>
          <span class="dc-go">进入图解演讲 →</span>
        </a>"""
        for no, title, href, meta, ic, col, tags in cards
    )

    # Top Visual Architecture Map SVG Banner (concise, iconic, clickable visual orientation)
    banner_svg = f"""
    <svg class="hero-map" viewBox="0 0 1240 218" width="100%" xmlns="http://www.w3.org/2000/svg">
      <rect x="2" y="2" width="1236" height="214" rx="18" fill="#FFFFFF" stroke="#DCD7CC" stroke-width="1.4"/>
      <text x="28" y="34" font-family="var(--f-m)" font-size="11.5" fill="#1F5E73" font-weight="700" letter-spacing="2">VISUAL ARCHITECTURE MAP · 脑体分离一图速览</text>
      <text x="1212" y="34" font-family="var(--f-m)" font-size="11.5" fill="#7D8B88" text-anchor="end">27 模块 · 43 MCP · 88 SPI · 3 核 / 10 扩展 / 4 发行版 · {TEST_CNT} 测</text>
      <!-- Left: Hot-Swappable Brains -->
      <rect x="28" y="54" width="236" height="142" rx="14" fill="#FAF8F4" stroke="#1F5E73" stroke-width="1.6"/>
      {icon_badge("brain", 74, 102, 22, C_TEAL)}
      <text x="108" y="96" font-family="var(--f-r)" font-size="18" fill="#162A33" font-weight="700">瞬态大脑</text>
      <text x="108" y="118" font-family="var(--f-m)" font-size="11.5" fill="#1F5E73" font-weight="600">热插拔算力</text>
      <text x="146" y="160" font-family="var(--f-m)" font-size="11.5" fill="#445A63" text-anchor="middle">Claude · Antigravity · Cursor</text>
      <text x="146" y="180" font-family="var(--f-m)" font-size="11" fill="#7D8B88" text-anchor="middle">Codex · Cron Worker</text>
      <!-- Connector -->
      <line x1="264" y1="125" x2="318" y2="125" stroke="#1F5E73" stroke-width="3" stroke-dasharray="6 4"/>
      <!-- Middle: 6 Persistent Body Organs -->
      <rect x="322" y="54" width="584" height="142" rx="14" fill="#E6F2F4" fill-opacity="0.55" stroke="#1F5E73" stroke-width="1.8"/>
      <text x="614" y="78" font-family="var(--f-m)" font-size="11" fill="#154454" font-weight="700" text-anchor="middle" letter-spacing="1.6">AGENT BODY · 永驻宿主机的六大生理器官</text>
      <g transform="translate(338,90)">
        <rect x="0" y="0" width="84" height="90" rx="10" fill="#FFFFFF" stroke="#154454" stroke-width="1.3"/>
        {icon_badge("eye", 42, 34, 18, C_TEAL_D)}
        <text x="42" y="74" font-family="var(--f-s)" font-size="12" fill="#162A33" font-weight="700" text-anchor="middle">感官图谱</text>
      </g>
      <g transform="translate(432,90)">
        <rect x="0" y="0" width="84" height="90" rx="10" fill="#FFFFFF" stroke="#C85A32" stroke-width="1.3"/>
        {icon_badge("bolt", 42, 34, 18, C_CLAY)}
        <text x="42" y="74" font-family="var(--f-s)" font-size="12" fill="#162A33" font-weight="700" text-anchor="middle">小脑反射</text>
      </g>
      <g transform="translate(526,90)">
        <rect x="0" y="0" width="84" height="90" rx="10" fill="#FFFFFF" stroke="#257A60" stroke-width="1.3"/>
        {icon_badge("capsule", 42, 34, 18, C_GREEN)}
        <text x="42" y="74" font-family="var(--f-s)" font-size="12" fill="#162A33" font-weight="700" text-anchor="middle">海马接力</text>
      </g>
      <g transform="translate(620,90)">
        <rect x="0" y="0" width="84" height="90" rx="10" fill="#FFFFFF" stroke="#B5852A" stroke-width="1.3"/>
        {icon_badge("spine", 42, 34, 18, C_GOLD)}
        <text x="42" y="74" font-family="var(--f-s)" font-size="12" fill="#162A33" font-weight="700" text-anchor="middle">规约脊柱</text>
      </g>
      <g transform="translate(714,90)">
        <rect x="0" y="0" width="84" height="90" rx="10" fill="#FFFFFF" stroke="#2D7187" stroke-width="1.3"/>
        {icon_badge("shield", 42, 34, 18, C_TEAL_2)}
        <text x="42" y="74" font-family="var(--f-s)" font-size="12" fill="#162A33" font-weight="700" text-anchor="middle">免疫 AWR</text>
      </g>
      <g transform="translate(808,90)">
        <rect x="0" y="0" width="84" height="90" rx="10" fill="#FFFFFF" stroke="#1F5E73" stroke-width="1.3"/>
        {icon_badge("mesh", 42, 34, 18, C_TEAL)}
        <text x="42" y="74" font-family="var(--f-s)" font-size="12" fill="#162A33" font-weight="700" text-anchor="middle">多体联邦</text>
      </g>
      <!-- Connector -->
      <line x1="906" y1="125" x2="960" y2="125" stroke="#257A60" stroke-width="3" stroke-dasharray="6 4"/>
      <!-- Right: Benchmark Outcome -->
      <rect x="964" y="54" width="248" height="142" rx="14" fill="#154454"/>
      <text x="1088" y="84" font-family="var(--f-m)" font-size="11" fill="#A6CFD5" font-weight="700" text-anchor="middle" letter-spacing="1.6">BENCHMARK v{BENCH_VER} 实测</text>
      <text x="1028" y="138" font-family="var(--f-m)" font-size="28" fill="#FAF8F4" font-weight="700" text-anchor="middle">-99.2%</text>
      <text x="1028" y="164" font-family="var(--f-s)" font-size="12" fill="#B8DBE0" text-anchor="middle">Token 节省</text>
      <line x1="1096" y1="100" x2="1096" y2="174" stroke="#326C7F" stroke-width="1.4"/>
      <text x="1154" y="138" font-family="var(--f-m)" font-size="28" fill="#7DE0B8" font-weight="700" text-anchor="middle">98.0</text>
      <text x="1154" y="164" font-family="var(--f-s)" font-size="12" fill="#B8DBE0" text-anchor="middle">上下文质量</text>
    </svg>
    """

    html_doc = f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Agent Body · 视觉图解演讲与路演总览</title>
<style>
:root{{
  --paper:#FAF8F4; --paper2:#F2EFE9; --line:#DCD7CC; --ink:#162A33; --ink2:#445A63;
  --mute:#7D8B88; --teal:#1F5E73; --teal-l:#E6F2F4; --clay:#C85A32; --clay-l:#FCEEE8; --green:#257A60;
  --f-r:"Songti SC","STSong","Noto Serif CJK SC",Georgia,serif;
  --f-s:"PingFang SC","Hiragino Sans GB","Noto Sans CJK SC","Microsoft YaHei",sans-serif;
  --f-m:"SF Mono",Menlo,Consolas,"Courier New",monospace;
}}
*{{box-sizing:border-box;margin:0;padding:0}}
body{{background:#E7E4DC;color:var(--ink);font-family:var(--f-s);padding:32px 24px 60px;-webkit-font-smoothing:antialiased}}
.wrap{{max-width:1360px;margin:0 auto;background:var(--paper);
  background-image:radial-gradient(#DDD8CC .9px,transparent .9px);background-size:28px 28px;
  border:1px solid var(--line);border-radius:22px;padding:44px 54px;box-shadow:0 16px 48px rgba(22,42,51,.08)}}
.top-bar{{display:flex;justify-content:space-between;align-items:flex-end;gap:24px;flex-wrap:wrap}}
.eyebrow{{display:inline-block;font-family:var(--f-m);font-size:11.5px;letter-spacing:2px;
  color:var(--teal);background:var(--teal-l);padding:4px 12px;border-radius:12px;font-weight:700}}
h1{{font-family:var(--f-r);font-size:40px;font-weight:700;margin-top:10px;letter-spacing:.6px}}
.keys-pill{{font-family:var(--f-m);font-size:12px;color:var(--ink2);background:#FFFFFF;
  border:1px solid var(--line);border-radius:12px;padding:10px 16px;display:flex;gap:14px;flex-wrap:wrap}}
.keys-pill b{{color:var(--teal)}}
.hero-map{{display:block;margin:24px 0 30px}}
.sec-lbl{{font-family:var(--f-m);font-size:11.5px;letter-spacing:2.2px;color:var(--teal);font-weight:700;margin-bottom:14px}}
.hero-grid{{display:grid;grid-template-columns:1fr 1fr;gap:22px;margin-bottom:34px}}
.hero-card{{display:flex;flex-direction:column;text-decoration:none;background:#FFFFFF;
  border:1.6px solid var(--line);border-top:6px solid var(--teal);border-radius:18px;
  padding:24px 28px;color:var(--ink);transition:transform .18s,border-color .18s,box-shadow .18s}}
.hero-card.clay{{border-top-color:var(--clay)}}
.hero-card:hover{{transform:translateY(-3px);border-color:var(--teal);box-shadow:0 12px 28px rgba(22,42,51,.1)}}
.hero-card.clay:hover{{border-color:var(--clay)}}
.hero-card h2{{font-family:var(--f-r);font-size:23px;margin:10px 0 12px}}
.dc-head{{display:flex;align-items:center;gap:14px}}
.dc-top-col{{display:flex;flex-direction:column;gap:3px;font-family:var(--f-m);font-size:11.5px;color:var(--mute)}}
.dc-meta{{color:var(--teal);font-weight:700}}
.chips{{display:flex;flex-wrap:wrap;gap:8px;margin-top:4px;flex:1;align-content:flex-start}}
.chip{{font-family:var(--f-m);font-size:12px;color:var(--ink2);background:var(--paper2);
  border:1px solid var(--line);border-radius:10px;padding:4px 10px;font-weight:600}}
.series-grid{{display:grid;grid-template-columns:repeat(3,1fr);gap:20px}}
.deck-card{{display:flex;flex-direction:column;text-decoration:none;background:#FFFFFF;
  border:1.5px solid var(--line);border-top:5px solid var(--acc,var(--teal));border-radius:16px;
  padding:22px 24px;color:var(--ink);transition:transform .18s,border-color .18s,box-shadow .18s}}
.deck-card:hover{{transform:translateY(-3px);border-color:var(--acc,var(--teal));box-shadow:0 10px 24px rgba(22,42,51,.09)}}
.deck-card h3{{font-family:var(--f-r);font-size:19.5px;margin:14px 0 12px;line-height:1.35}}
.dc-go{{font-family:var(--f-m);font-size:12.5px;color:var(--clay);margin-top:16px;font-weight:700}}
.foot{{margin-top:34px;padding-top:16px;border-top:1px solid var(--line);display:flex;
  justify-content:space-between;font-family:var(--f-m);font-size:11.5px;color:var(--mute);flex-wrap:wrap;gap:12px}}
@media(max-width:980px){{.hero-grid,.series-grid{{grid-template-columns:1fr}}.wrap{{padding:28px 22px}}}}
</style>
</head>
<body>
<div class="wrap">
  <div class="top-bar">
    <div>
      <span class="eyebrow">AGENT BODY · VISUAL PRESENTATION SUITE · v{APP_VER}</span>
      <h1>Agent 本体图解演讲与路演总览</h1>
    </div>
    <div class="keys-pill">
      <span><b>G</b> 目录</span><span><b>N</b> 本页讲稿</span><span><b>T</b> 全文联动</span>
      <span><b>P</b> 自动</span><span><b>A/S/R</b> 步进动画</span><span><b>F</b> 全屏</span>
    </div>
  </div>
  {banner_svg}
  <div class="sec-lbl">FLAGSHIP DECKS · 两大旗舰图解全景版</div>
  <div class="hero-grid">
    <a class="hero-card" href="presentation.html">
      <div class="dc-head">
        {_mini_icon_svg("rocket", C_TEAL)}
        <div class="dc-top-col"><span>ROADSHOW · 对外路演版</span><span class="dc-meta">17 页 · 15~20 分钟 · 全定制视觉图解</span></div>
      </div>
      <h2>Agent 本体对外路演版（presentation.html）</h2>
      <div class="chips">
        <span class="chip">比较优势天平</span><span class="chip">蒙眼翻车现场</span><span class="chip">脑体解剖图</span>
        <span class="chip">ETC 乐观立交</span><span class="chip">150t 发光胶囊</span><span class="chip">帕累托象限</span><span class="chip">微内核 4 发行版</span>
      </div>
      <span class="dc-go">打开路演版 →</span>
    </a>
    <a class="hero-card clay" href="agent-body-deck.html">
      <div class="dc-head">
        {_mini_icon_svg("gear", C_CLAY)}
        <div class="dc-top-col"><span>TECHNICAL EXCHANGE · 深度交流版</span><span class="dc-meta">25 页 · 45~60 分钟 · 内核架构图谱</span></div>
      </div>
      <h2>Agent 本体深度技术交流版（agent-body-deck.html）</h2>
      <div class="chips">
        <span class="chip">27 模块 / 88 SPI</span><span class="chip">三层有向图</span><span class="chip">目录锁 + WFG</span>
        <span class="chip">写穿净水器</span><span class="chip">五维雷达</span><span class="chip">600% CPU AWR 曲线</span><span class="chip">10 Contrib 扩展</span>
      </div>
      <span class="dc-go">打开深度交流版 →</span>
    </a>
  </div>
  <div class="sec-lbl">DEEP-DIVE SERIES · 6 讲 × 30 分钟单点说透图解专题</div>
  <div class="series-grid">
    {cards_html}
  </div>
  <div class="foot">
    <span>digoal 德哥 · 2026-11 · 浅色纸基工程蓝图 · 零外部依赖 · Playwright 9/9 零溢出零重叠</span>
    <span>github.com/digoal/agent-body · Apache-2.0</span>
  </div>
</div>
</body>
</html>
"""
    (OUT / "index.html").write_text(html_doc, encoding="utf-8")


if __name__ == "__main__":
    build_talk_04()
    build_talk_05()
    build_talk_06()
    build_index_hub()
    print("✅ Built lectures 04~06 and index.html in presentation/")
