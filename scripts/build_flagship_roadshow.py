#!/usr/bin/env python3
"""Build presentation/presentation.html (16-slide Executive Roadshow Deck).

Decks-as-Code generator following the '地层剖面 · 工程蓝图' visual system.
Grounded 100% in Agent Body v1.1.0 source code, live Benchmark v3.9,
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

from evolution_manifest import extract_manifest

_MANIFEST = extract_manifest()
_COUNTS = _MANIFEST["counts"]
APP_VER = _MANIFEST["app_version"]
BENCH_VER = _MANIFEST["bench_spec_version"]
MOD_CNT = _COUNTS["modules"]
TOOL_CNT = _COUNTS["mcp_tools"]
SPI_CNT = _COUNTS["spi_methods"]
TABLE_CNT = _COUNTS["tables"]
WL_CNT = _COUNTS["workloads"]
MICRO_CNT = _COUNTS["micro"]
DQ_CNT = _COUNTS["dq"]
COMP_CNT = _COUNTS["competitors"]
PR_CNT = _COUNTS["pr_rules"]

ROOT = Path(__file__).resolve().parent.parent
PRES_DIR = ROOT / "presentation"


def build_roadshow_deck() -> None:
    slides = [
        # 01 封面
        {
            "cover": True,
            "eyebrow": "封面 · 路演总览",
            "title": "Agent Body · 脑体分离的宿主机具身底座",
            "svg": svg_cover(
                kicker=f"AGENT BODY · EXECUTIVE ROADSHOW · v{APP_VER}",
                title="Agent 本体",
                en_sub="Brain-Body Separation · Embodied Host Substrate",
                subtitle1="大语言模型是可热切换的瞬态大脑，本系统是永驻宿主机的物理身体",
                subtitle2="感官图谱 × 小脑反射 × 海马体接力 × 规约脊柱 × 代谢稳态 × 多体联邦",
                spec_line="Rust 单二进制 · 27 模块 · 43 原生 MCP · 88 SPI · Bench v3.9 (-99.23% Token / 质量 +64.2)",
                strata=[
                    ("瞬态大脑层 · LLM Clients", "Antigravity / Claude / Cursor / Codex / Cron", "#EBF1EE", C_INK),
                    ("感官图谱层 · Host Reality", "/proc · Socket · L1/L2/L3 有向加权图谱 · CodeGraph", "#CFE0DC", C_INK),
                    ("小脑反射层 · AOPCC & Locks", "悲观前置门禁 · 乐观直通租约 · <2ms 碰壁自愈 · WFG 锁", "#A4C4C6", C_INK),
                    ("海马体接力层 · Continuity", "FTS5+向量 RRF · 写穿失效保鲜 · <=300 tok 跨大脑胶囊", "#74A5AD", C_PAPER),
                    ("规约脊柱层 · Governance", "8 大放置规则自演化 · 黑白灰分级扫描 · Context 防爆路由", "#47808D", C_PAPER),
                    ("安全审计与稳态 · AWR", "全链路脱敏抗投毒 · 双域审计 · Wait Events / AWR / ADDM", "#2E6373", C_PAPER),
                    ("多体联邦层 · Mesh v1.0", "HMAC-SHA256 防重放 · Peer Scopes · 沙盒收件箱", "#1E4B5A", C_PAPER),
                ],
                right_tag="地层剖面 · 工程蓝图 · 对外路演版 (16 页)",
            ),
            "script": (
                "各位好，今天用十五到二十分钟，向大家完整汇报一个从真实工程痛点里长出来的开源项目——Agent Body，中文我们叫它「Agent 本体」。\n"
                "先看封面右边这七层地层剖面。最上面浅色的是瞬态大脑层，也就是我们每天切换使用的各种大模型和客户端；下面六层越往下越深、越稳固，从感官图谱、小脑反射、海马体接力、规约脊柱、安全审计与 AWR 稳态，一直到底层的跨机联邦，共同构成 Agent 永驻物理宿主机的「身体」。\n"
                "这页底部的硬数字全部来自当前宿主机真机实测：Rust 单二进制交付，27 个顶层模块、43 个原生 MCP 工具、88 个存储 SPI 方法，在最新的 Benchmark v3.9 套件里，实现了 Prompt Token 节省 99.23%，同时上下文质量从 35.6 升至 98.0 (+64.2)。下面我们从头讲清，为什么这个东西非做不可。"
            ),
        },
        # 02 起源 · 比较优势
        {
            "eyebrow": "01 · ORIGIN · COMPARATIVE ADVANTAGE",
            "title": "比较优势：为什么顶尖高手都把环境琐事交给固定底座",
            "lead": "经济学里的<b>比较优势</b>指出：让稀缺的推理算力专攻核心创造，把宿主机盘点、防撞与记账交给永驻本地的确定性底座。",
            "svg": svg_columns(
                [
                    {
                        "tag": "REALITY 01",
                        "title": "硅谷园区物业",
                        "accent": C_TEAL_D,
                        "lines": [
                            "创业者在负一楼沉浸写代码一周",
                            "饮食起居与琐事全部由物业兜底",
                            ("核心收益：心智带宽 100% 留给主业", True),
                        ],
                        "foot_lbl": "角色分工",
                        "foot_val": "主业创造 vs 环境托管",
                    },
                    {
                        "tag": "REALITY 02",
                        "title": "组织参谋与执行底座",
                        "accent": C_TEAL,
                        "lines": [
                            "决策者专攻战略拍板与关键谈判",
                            "专业幕僚负责日程、人脉、档案与避坑",
                            ("核心收益：零重复查证，决策极速", True),
                        ],
                        "foot_lbl": "角色分工",
                        "foot_val": "高价值判断 vs 事实记忆",
                    },
                    {
                        "tag": "REALITY 03",
                        "title": "AI Agent 与本体",
                        "accent": C_CLAY,
                        "lines": [
                            "LLM 大脑推理极贵且跨会话失忆",
                            "Agent Body 永驻本机零 Token 守候",
                            ("核心收益：省 99% 探索税，质量反升", True),
                        ],
                        "foot_lbl": "范式定位",
                        "foot_val": "瞬态大脑 vs 永驻本体",
                    },
                ],
                bottom_banner=(
                    "将感官盘点与稳态防撞内化为物理底座——合在一起，就是服务于瞬态大脑的 Agent 本体。",
                    "ECONOMICS → SYSTEMS",
                    "teal",
                ),
            ),
            "next": "当大脑没有这具身体时，每天在真实开发机上会发生什么？看一个真实案例。",
            "script": (
                "这个项目的起心动念，来自经济学里最朴素的一个词：比较优势。\n"
                "我曾经看过硅谷 101 采访一位创业者，他在办公区负一楼连续沉浸写了一周代码，因为园区物业把吃住琐事全包了。决策者配专业幕僚体系也是同理，让稀缺的高阶判断力只干最值钱的活。\n"
                "转头看今天的 AI Agent：我们花着昂贵的 Token 请最聪明的大模型来干活，结果它每天大量时间都在干什么？在反复 ls、grep、查端口、猜路径、重装本机早就有的工具。它缺的不是更高的智商，而是一具永驻在机器上、替它记住环境并拦住低级失误的「身体」。"
            ),
        },
        # 03 痛点 · PostgreSQL 调研惨案与记忆盲区
        {
            "eyebrow": "02 · PAIN POINTS · THE BLIND AGENT",
            "title": "PostgreSQL 调研惨案：50 个记忆项目漏掉的物理现实",
            "lead": "市面上的记忆工具只记住「聊过什么」，却对宿主机上的<b>物理现实、运行时状态、依赖拓扑与跨会话接力</b>视而不见。",
            "svg": svg_compare_rows(
                headers=("高频现场痛点", "裸奔大脑（无 Agent 本体）", "真实代价与翻车后果"),
                rows=[
                    ("重复造轮子", "不知本机已有 PG 源码/pdftotext，从头 clone 编译", "单次浪费 10~30 分钟 + 重复拉爆磁盘"),
                    ("端口盲撞", "不知 :5432 / :8080 已被占用，直接 bind 服务", "进程崩溃、排查多耗 3~5 轮昂贵对话"),
                    ("改崩上游", "改了公共配置或脚本，不知哪个 systemd/cron 依赖它", "后台定时任务静默挂掉，事后难以溯源"),
                    ("工作区污染", "随手在当前目录堆脚本和临时文件，无视主人规范", "目录失控、黑名单 node_modules 扫爆上下文"),
                    ("换脑即失忆", "从 Claude 切到 Cursor 或会话中断，上下文全丢", "新大脑从零重扫仓库，重复燃烧数万 Token"),
                ],
                takeaway=(
                    "记忆工具解决的是「对话回放」，Agent Body 解决的是「物理宿主机具身存在」。",
                    "实证：单次冷启动裸奔扫描可达 60 万+ Token 噪音",
                ),
            ),
            "next": "为了彻底根治这五种翻车，我们把架构从「外挂插件」升维为「脑体分离」。",
            "script": (
                "举个我自己每天都会遇到的例子：我经常让 Agent 帮我调研 PostgreSQL 内核源码、特性和应用场景。但以前每次开新会话，它不知道本地早就浅克隆好了源码、建好了 CodeGraph 地图，也不知道机器上装着什么编译环境，上来就要重新 git clone、重新编译，白白浪费十几分钟和几万 Token。\n"
                "更致命的是另外四件事：不知道端口谁占着直接撞车；不知道改一个脚本会炸掉哪个 systemd 服务；不知道文件该放哪儿随手乱丢；换个客户端或者会话一断，刚才干到哪一步全忘了。\n"
                "市面上几十个记忆项目都在卷「对话摘要」，却唯独漏掉了宿主机物理世界这四件半硬事。这正是 Agent Body 存在的根本理由。"
            ),
        },
        # 04 范式跃迁 · 脑体分离与六大器官
        {
            "eyebrow": "03 · PARADIGM · BRAIN-BODY SEPARATION",
            "title": "范式跃迁：脑体分离与六大本体器官",
            "lead": "LLM 客户端随时可换（<b>Brain-Agnostic</b>），永驻宿主机的 Agent Body 以六大器官承接感知、反射、记忆、规约、稳态与联邦。",
            "svg": svg_columns(
                [
                    {
                        "tag": "ORGAN 01 · 感官",
                        "title": "宿主机事实图谱",
                        "accent": C_TEAL_D,
                        "lines": [
                            "工具/进程/端口/Systemd/Cron",
                            "L1/L2/L3 证据分层有向图",
                            "与 CodeGraph 宏微双层联邦",
                            ("零幻觉物理真值只读入口", True),
                        ],
                        "foot_lbl": "核心模块",
                        "foot_val": "fact / links / scanner",
                    },
                    {
                        "tag": "ORGAN 02 · 小脑",
                        "title": "AOPCC 与协作锁",
                        "accent": C_CLAY,
                        "lines": [
                            "悲观门禁守住高危写操作",
                            "乐观租约免查直通省轮次",
                            "<2ms 碰壁自愈注入替代处方",
                            ("WFG 死锁检测 + 等待唤醒", True),
                        ],
                        "foot_lbl": "核心模块",
                        "foot_val": "hooks / brief / coordination",
                    },
                    {
                        "tag": "ORGAN 03 · 海马体",
                        "title": "新鲜度与连续性",
                        "accent": C_GREEN,
                        "lines": [
                            "FTS5 + Ollama 向量 RRF 混合",
                            "写穿失效 + 删除级联净化",
                            "<=300 tok 跨大脑接力胶囊",
                            ("可插拔 SPI：Mem0/Letta", True),
                        ],
                        "foot_lbl": "核心模块",
                        "foot_val": "memory / body / feedback",
                    },
                    {
                        "tag": "ORGAN 04~06 · 脊柱/代谢/联邦",
                        "title": "规约 · 审计 · AWR · Mesh",
                        "accent": C_GOLD,
                        "lines": [
                            "8 大放置规约 + Context 路由",
                            "双域审计 + P1~P5 探针自进化",
                            "Wait Events + AWR + ADDM",
                            ("MeshEnvelope v1.0 跨机协同", True),
                        ],
                        "foot_lbl": "核心模块",
                        "foot_val": "placement / audit / mesh",
                    },
                ],
                bottom_banner=(
                    "单二进制承载 27 个 Rust 模块、43 个原生 MCP 工具、88 个存储 SPI（SQLite / PostgreSQL / DuckDB 热迁移）。",
                    "v1.1.0 · PRODUCTION READY",
                    "teal",
                ),
            ),
            "next": "下面我们逐个解剖这六大器官里最硬核的工程机制，先看第一器官：宿主机事实感官。",
            "script": (
                "想明白痛点之后，我们确立了整个项目的第一性原理：脑体分离。\n"
                "不论你今天用 Antigravity、Claude Code、Cursor 还是后台 Cron Worker，它们都是可热切换的瞬态大脑皮层；而 Agent Body 是永驻在你这台 Linux 宿主机上的物理身体。\n"
                "这具身体由六大器官协同运转：第一是感官，实时测绘物理事实与依赖图谱；第二是小脑，负责并发控制、防撞反射与多 Agent 锁；第三是海马体，管长期记忆保鲜和跨大脑接力；第四到第六分别是规约脊柱、安全审计与 AWR 内生稳态代谢，以及跨主机联邦。全部打包进一个零外部依赖的 Rust 单二进制里。"
            ),
        },
        # 05 器官 1 · 宿主机事实感官与 L1/L2/L3 图谱
        {
            "eyebrow": "04 · ORGAN 1 · HOST REALITY & TIERED GRAPH",
            "title": "宿主机事实感官：L1/L2/L3 证据分层与宏微联邦",
            "lead": "所有事实均源自 <code>/proc</code>、Socket、Systemd 与磁盘真值；图谱严格区分<b>物理硬依赖、结构上卷与语义近邻</b>，杜绝把相似当因果。",
            "svg": svg_pipeline(
                steps=[
                    {
                        "tag": "L1 · 物理硬依赖",
                        "title": "权重 0.85 ~ 1.00",
                        "accent": C_TEAL_D,
                        "lines": [
                            "Systemd 单元→进程→端口",
                            "Cron 定时任务→脚本路径",
                            "直接文件 import / 引用",
                        ],
                    },
                    {
                        "tag": "L2 · 结构聚合上卷",
                        "title": "权重 0.55 ~ 0.84",
                        "accent": C_TEAL,
                        "lines": [
                            "4,985 条文件边上卷项目级",
                            "同体服务与运行进程桥接",
                            "工具链与工作目录关联",
                        ],
                    },
                    {
                        "tag": "L3 · 隐式语义发现",
                        "title": "权重 0.35 ~ 0.65",
                        "accent": C_GOLD,
                        "lines": [
                            "本地向量余弦 >= 0.78",
                            "每节点 Top-K <= 3 防稠密",
                            "仅作跨项目探索参考",
                        ],
                    },
                    {
                        "tag": "宏微双层联邦",
                        "title": "Body × CodeGraph",
                        "accent": C_GREEN,
                        "lines": [
                            "宏观：项目/服务/端口拓扑",
                            "微观：符号/AST 交 CodeGraph",
                            "3 跳衰减爆炸半径 148 μs",
                        ],
                    },
                ],
                bottom_panels=[
                    {
                        "title": "现网实测连通度跃迁（DQ6 = 100.0%）",
                        "accent": C_TEAL,
                        "lines": [
                            "重构前：214/215 工程节点孤立（孤立率 99.5%），仅 49 条显式边",
                            "重构后：776/776 条属性图边 100% 证据分层合规，216/216 工程全连通",
                            ("爆炸半径衰减公式: S = prod(w_e * 0.8^(hop-1))", True),
                        ],
                    },
                    {
                        "title": "抗假阳性与零幻觉底线（DQ3 = 99.5%）",
                        "accent": C_GREEN,
                        "lines": [
                            "自动免疫 .rs #[cfg(test)] 夹具、模板占位符与 .md 示例代码路径",
                            "本地 Ollama 小模型 (qwen3.5:0.8b) 异步双轨标注，零云端 Token",
                            ("agent-body links impact <path>  |  body_codegraph_bridge", True),
                        ],
                    },
                ],
            ),
            "next": "看清了物理世界，但每次操作前都去问一遍本体，是不是又太慢、太费轮次？看小脑反射的破局。",
            "script": (
                "先看感官器官。很多图谱方案最大的问题是「把语义相似当成物理依赖」，结果爆炸半径一查全是噪音。\n"
                "我们在 Agent Body 里定了铁律：图谱边必须分三层。L1 是物理硬依赖，比如 systemd 拉起进程、进程监听端口，权重 0.85 以上；L2 是结构上卷，把底层近五千条文件引用自动聚合成项目级关系，把服务配置和活进程桥接起来；L3 才是基于本地向量余弦大于等于 0.78 的隐式语义发现，且每个节点严格剪枝 Top-3，绝不伪装成硬依赖。\n"
                "现网实测中，这一招让 216 个工程节点的连通率从 0.5% 直接拉到了 100%，3 跳爆炸半径推演 P50 只要 148 微秒；再往下查函数调用，直接桥接 CodeGraph，做到宏微联邦、各司其职。"
            ),
        },
        # 06 器官 2 · 小脑反射与 AOPCC（带步进动画）
        {
            "eyebrow": "05 · ORGAN 2 · CEREBELLUM & AOPCC (INTERACTIVE)",
            "title": "小脑反射：乐观直通 + 悲观门禁，消灭整轮对话税",
            "lead": "借鉴数据库并发控制：事前全查虽只要 1ms，却迫使 LLM 多走一整轮推理（<b>整轮对话税</b>）；AOPCC 按风险自动分流。",
            "svg": svg_pipeline(
                steps=[
                    {
                        "tag": "STEP 1 · 开工签发",
                        "title": "MVCC 乐观租约",
                        "accent": C_TEAL_D,
                        "lines": [
                            "SessionStart 注入 <=120 tok",
                            "高置信 (p>=0.90) 服务/工具",
                            "附带纪元指纹 epoch_hash",
                        ],
                    },
                    {
                        "tag": "STEP 2 · 动作分级",
                        "title": "三态并发分类器",
                        "accent": C_TEAL,
                        "lines": [
                            "OPTIMISTIC：连 :5432 / tmp",
                            "CAUTIOUS_CAS：常规新建文件",
                            "PESSIMISTIC：bind/高危写/锁",
                        ],
                    },
                    {
                        "tag": "STEP 3 · 直通或拦截",
                        "title": "免查直行 / 1ms 红灯",
                        "accent": C_GREEN,
                        "lines": [
                            "客户端连 PG 直接执行 0 查询",
                            "服务端 bind 冲突 Pre-Hook 拦截",
                            "WFG 死锁环路检测毫秒破局",
                        ],
                    },
                    {
                        "tag": "STEP 4 · 碰壁秒愈",
                        "title": "<2ms 替代处方",
                        "accent": C_CLAY,
                        "lines": [
                            "PostToolUse 捕获失败退出",
                            "反查宿主机注入真实端口/路径",
                            "自动降级失效租约防再踩坑",
                        ],
                    },
                ],
                bottom_panels=[
                    {
                        "title": "为什么不能「事事悲观」？（第一性原理）",
                        "accent": C_CLAY,
                        "lines": [
                            "即便 MCP 查一次只要 1ms，大模型重发 10k 上下文 + 推理也要 2~4 秒",
                            "可逆只读与客户端试探（如 psql -p 5432）直连成功率 >95%，免查即大赚",
                            ("W15 实测：Token 省 83.9% · 质量 52.0→100.0 · 加速 3.22x", True),
                        ],
                    },
                    {
                        "title": "为什么绝不「盲目乐观」？（安全底线）",
                        "accent": C_TEAL,
                        "lines": [
                            "端口 bind、高爆炸半径改写、多 Agent 共享写、敏感系统路径强制悲观门禁",
                            "失败代价 C_fail 极高时，宁可多花 1 轮确认，绝不拿生产环境赌博",
                            ("ConcurrencyPolicyClassifier + OptimisticFailureDiagnoser", True),
                        ],
                    },
                ],
                animated=True,
                mid="ahaopcc",
            ),
            "anim_caps": [
                "Step 1/5：SessionStart 签发 <=120 tok 乐观直通租约（含高置信工具与本机服务）",
                "Step 2/5：三态动作分类器精确区分客户端直连（乐观）与服务端绑定/高危写（悲观）",
                "Step 3/5：高置信只读/客户端直连免查直行，高危写由 Pre-Hook 1ms 悲观拦截",
                "Step 4/5：若乐观直行偶发碰壁，PostToolUse 在 <2ms 内反查宿主机注入替代处方并降级租约",
                "Step 5/5：在零安全妥协前提下彻底消除低风险场景的「整轮对话税」",
            ],
            "next": "小脑解决了单次执行的快与稳，那当会话中断或切换大模型大脑时，怎么保证不失忆？",
            "script": (
                "这一页是我个人最喜欢的架构洞察，灵感直接来自数据库的悲观锁与乐观锁。\n"
                "早期我们认为，每次 Agent 干活前都先查一遍本体最安全。但实测下来发现一个隐形杀手：就算本体 1 毫秒就回复了，大模型为了发起这次工具调用，必须把一两万 Token 的上下文再重发一遍、再推理三秒钟——这就是「整轮对话税」。\n"
                "比如我想连本机 5432 端口的 PostgreSQL 做个验证，直接连就好了，连不上再兜底。于是我们设计了 AOPCC 自适应并发控制：开工时简报顺手签发 120 Token 以内的乐观直通租约；常规直连和临时目录直接放行；一旦碰壁报错，Post-Hook 在 2 毫秒内反查宿主机真实状态，把替代端口和命令直接塞给大脑，并自动降级坏掉的租约。而遇到端口绑定、高危路径或多人共享写，则死守悲观门禁。又快又稳，两头占全。"
            ),
        },
        # 07 器官 3 · 海马体与「换脑不失忆」
        {
            "eyebrow": "06 · ORGAN 3 · HIPPOCAMPUS & CONTINUITY",
            "title": "海马体接力：全生命周期保鲜与「换脑不失忆」",
            "lead": "把旧记忆变成「不会过期的真知识」，并在切换大模型或重开会话时，用 <b><=300 tok 连续性接力胶囊</b>无缝交棒。",
            "svg": svg_columns(
                [
                    {
                        "tag": "FRESHNESS · 写穿失效",
                        "title": "根治七大陈旧漏斗",
                        "accent": C_TEAL_D,
                        "lines": [
                            "文件变更立即重置旧向量 stale",
                            "删除后 FTS 零僵尸并级联清理",
                            "同路径自动记忆幂等归档去重",
                            ("现网 DQ5 抽检 589 项：100.0%", True),
                        ],
                        "foot_lbl": "对账引擎",
                        "foot_val": "FreshnessReconciler",
                    },
                    {
                        "tag": "MEMORY SPI · 混合检索",
                        "title": "开放生态不造轮子",
                        "accent": C_GREEN,
                        "lines": [
                            "内置 SQLite FTS5 + 向量 RRF",
                            "支持时间半衰期与矛盾仲裁",
                            "一行配置桥接 Mem0/Letta/PC",
                            ("M2 FTS5 实测 P50：335 μs", True),
                        ],
                        "foot_lbl": "向量覆盖",
                        "foot_val": "DQ2 本地 Ollama 99.5%",
                    },
                    {
                        "tag": "CONTINUITY · 跨大脑接力",
                        "title": "<=300 tok 接力胶囊",
                        "accent": C_CLAY,
                        "lines": [
                            "大脑 A 结项写入 body_checkpoints",
                            "脱敏 + 抗投毒清洗后极简封装",
                            "大脑 B 开工 10ms 恢复现场进度",
                            ("W17 实测：Token -92.7% / 省 >=2 轮", True),
                        ],
                        "foot_lbl": "原生入口",
                        "foot_val": "body_state(checkpoint/handoff)",
                    },
                ],
                bottom_banner=(
                    "换壳不换魂、换脑不失忆：Antigravity 写到一半的架构重构，切给 Claude Code 或 Cursor 10 毫秒无缝续跑。",
                    "CONTINUITY CAPSULE <= 300 TOK",
                    "green",
                ),
            ),
            "next": "记忆保鲜且可接力，接下来看第四器官：规约脊柱如何管住目录秩序与百级 SKILL 的上下文爆炸。",
            "script": (
                "第三器官是海马体。传统 RAG 和记忆库用久了都会腐烂——文件改了向量还是旧的，文件删了搜索还能搜出僵尸记录。\n"
                "我们在 Phase 38 彻底治好了七大陈旧漏斗：内容哈希一变，写穿清空旧向量并标记重标；文件一删，跨表级联净化；现网抽检 589 项对象新鲜度达到 100%。\n"
                "更进一步，依托脑体分离，我们做出了跨大脑连续性接力胶囊：大脑 A 干到一半把关键目标、已改文件和下一步动作压缩进 300 Token 以内的脱敏胶囊；大脑 B 接手时 10 毫秒着陆，直接从断点继续，不再花两三轮重新问「咱们之前干到哪儿了」。"
            ),
        },
        # 08 器官 4 · 规约脊柱与 Context 防爆
        {
            "eyebrow": "07 · ORGAN 4 · GOVERNANCE SPINE & CONTEXT ROUTER",
            "title": "规约脊柱：8 大放置自演化与百级 SKILL 防爆路由",
            "lead": "用 <b>41 μs 确定性放置决策</b>守住工作区秩序，用<b>三层渐进披露</b>把 130+ SKILL 的 60 万 Token 压缩到 358 Token。",
            "svg": svg_bars(
                title_left="核心治理与防爆能力实测表现（Benchmark v3.9 现网数据）",
                rows=[
                    ("W7 百级 SKILL 与 MCP 防爆路由（609,758 → 358 tok）", 99.9, "-99.9%", "信息密度 838.0x", C_TEAL_D),
                    ("W1 宿主工具能力语义发现（21,079 → 119 tok）", 99.4, "-99.4%", "加速 565.17x", C_TEAL),
                    ("W3 分级扫描黑名单阻断（敲门不进屋，阻断 534 噪音）", 95.8, "-95.8%", "10/10 事实命中", C_GREEN),
                    ("W5 3 跳依赖爆炸半径推演（613 → 23 tok）", 96.2, "-96.2%", "M1 耗时 148 μs", C_TEAL_2),
                    ("W4 8 类资产放置合规门禁（质量 16.5 → 100.0）", 73.1, "-73.1%", "M3 耗时 41 μs", C_CLAY),
                ],
                footer_text="核心机制：白名单深挖到底 · 黑名单止于当层（绝不扫爆 .git/node_modules）· 用户搬移文件自动聚类沉淀新规则（PlacementLearner）",
            ),
            "next": "本体能力越强，安全边界越必须铁板一块。看第五器官：纵深防御、双域审计与闭环自进化。",
            "script": (
                "第四器官是规约脊柱。今天很多团队给 Agent 挂了一百多个 SKILL 和几十个 MCP 工具，一开工上下文就被工具说明塞掉几十万 Token，大模型注意力直接稀释。\n"
                "Agent Body 的 Context 防爆路由器把所有技能、MCP 和外部服务收编建目，按当前任务意图只披露最相关的 Top-K 微型索引。看图上第一条实测数据：W7 场景下，Prompt Token 从 60.9 万骤降到 358，压缩率 99.9%，关键事实 3/3 全部命中。\n"
                "同时，8 大资产放置引擎在 41 微秒内告诉大脑脚本该放哪儿、临时文件该放哪儿；哪怕主人手动把文件挪了位置，规则学习器也能自动归纳出新习惯。"
            ),
        },
        # 09 器官 5 · 纵深安全、双域审计与闭环自进化
        {
            "eyebrow": "08 · ORGAN 5 · SECURITY, DUAL-DOMAIN AUDIT & EVOLUTION",
            "title": "免疫系统：四级自治包络、双域审计与自进化飞轮",
            "lead": "全链路凭据零落盘、抗间接提示词投毒；<b>Agent 内外一切行为 100% 留痕</b>，用真实数字驱动产品闭环自进化。",
            "svg": svg_matrix_2x2(
                [
                    {
                        "tag": "SECURITY · 纵深防线 (DQ4 = 100%)",
                        "title": "全链路脱敏与抗投毒",
                        "accent": C_CLAY,
                        "bg": C_WHITE,
                        "lines": [
                            "SecretRedactor 拦断密钥/令牌/URI 口令落盘",
                            "PromptInjectionScanner 清洗外部语料投毒",
                            "Hook POSIX 转义防注入 + 0600/0700 权限",
                        ],
                    },
                    {
                        "tag": "AUTONOMY · 四级权限包络 (DQ8 = 93.9)",
                        "title": "L0 感知 → L3 绝对痛觉红线",
                        "accent": C_TEAL_D,
                        "bg": C_WHITE,
                        "lines": [
                            "L0 只读永不阻塞 · L1 可逆稳态自愈",
                            "L2 宿主写强制 dry-run / auth_token 鉴权",
                            "L3 绝对禁区：/etc/shadow、~/.ssh/ 100% 拦截",
                        ],
                    },
                    {
                        "tag": "AUDIT · 双域全链路账本 (DQ7 = 100%)",
                        "title": "IN_AGENT + OUT_OF_AGENT 留痕",
                        "accent": C_TEAL,
                        "bg": C_WHITE,
                        "lines": [
                            "接入域：43 工具 / Brief / Hook / Web / Mesh",
                            "域外自治：巡检 / GC / 修复 / Watcher 60s 折叠",
                            "每条附带 delta + evidence_sha256 + DuckDB OLAP",
                        ],
                    },
                    {
                        "tag": "EVOLUTION · 五点对比探针 (P1~P5)",
                        "title": "数据驱动闭环自进化",
                        "accent": C_GREEN,
                        "bg": C_WHITE,
                        "lines": [
                            "P1~P5 切面量化对比物理真值 vs 历史快照",
                            "消费近 24h 审计滑窗动态升降级 AOPCC 租约",
                            "摩擦热点去重写入 user_feedback.md 自动闭环",
                        ],
                    },
                ],
                footer_banner="严禁「黑盒自治」与「拍脑袋优化」：任何后台改动与自进化动作均有 SHA-256 存证与 Before/After 数字佐证。",
            ),
            "next": "当单机本体成熟后，如何安全跨主机协作？本体自身性能瓶颈又如何像数据库一样自诊断？",
            "script": (
                "把一台机器托付给 Agent 本体，大家最关心的一定是安全与可控。\n"
                "我们用四张网构筑了本体的免疫系统：左上角是安全脱敏与抗投毒，现网 DQ4 抽检明文凭据残留率 0%；右上角是 L0 到 L3 四级自治包络，可逆稳态可以自己修，但碰业务源码或物理删除默认 dry-run，碰 /etc/shadow 或私钥目录 100% 强拦。\n"
                "下面两块是双域审计与自进化飞轮：不管是 Agent 调了工具，还是本体后台自己做了巡检、GC、断链修复，100% 带脱敏前后差值和 SHA-256 指纹记入审计账本，并用 P1 到 P5 五个内生探针持续监测漂移，真正做到有据可查、越用越稳。"
            ),
        },
        # 10 器官 6 · 多体联邦与内生 AWR 性能工程（带步进动画）
        {
            "eyebrow": "09 · ORGAN 6 · MESH v1.0 & BUILT-IN AWR (INTERACTIVE)",
            "title": "多体联邦与内生代谢：把数据库 AWR 装进 Agent 本体",
            "lead": "跨机协同采用 <b>MeshEnvelope v1.0</b> 零信任信封；本体自身借鉴 PostgreSQL/Oracle 内核，内置 <b>Wait Events + AWR + ADDM</b>。",
            "svg": svg_pipeline(
                steps=[
                    {
                        "tag": "WAIT EVENTS",
                        "title": "6 类 18 等待事件",
                        "accent": C_TEAL_D,
                        "lines": [
                            "Lock/IO/Net/CPU/IPC/Idle",
                            "无锁 AtomicU64 8 档对数桶",
                            "RAII 埋点开销 < 100 ns",
                        ],
                    },
                    {
                        "tag": "/PROC/SELF",
                        "title": "零特权内核探针",
                        "accent": C_TEAL,
                        "lines": [
                            "直读 VmRSS / VmHWM / FDs",
                            "上下文主动/被动切换计数",
                            "物理读写字节 + SQLite WAL",
                        ],
                    },
                    {
                        "tag": "AWR BASELINE",
                        "title": "快照与黄金基线",
                        "accent": C_GOLD,
                        "lines": [
                            "滚动保留 32 份性能快照",
                            "Golden Baseline 跨版本差分",
                            "Body Time 百分比分解",
                        ],
                    },
                    {
                        "tag": "ADDM + L1 TUNE",
                        "title": "根因诊断与自调优",
                        "accent": C_GREEN,
                        "lines": [
                            "5 大瓶颈规则自动给处方",
                            "PRAGMA shrink + malloc_trim",
                            "曾根治 600% CPU / 1.3G 峰值",
                        ],
                    },
                ],
                bottom_panels=[
                    {
                        "title": "多本体联邦协议（MeshEnvelope v1.0 · 绝不造重型 Raft）",
                        "accent": C_TEAL,
                        "lines": [
                            "HMAC-SHA256 规范签名 + +-300s 时间窗与 NonceCache 防重放 + Peer Scopes",
                            "支持跨机图谱联邦、双向脱敏抗投毒结构化对话与 <=10MB 沙盒收件箱投递",
                            ("W14 实测：签名防重放 100% 拦截 · 上下文质量 12.5→100.0", True),
                        ],
                    },
                    {
                        "title": "零工具膨胀内聚接入（43/43 黑白屏 1:1:1 同步）",
                        "accent": C_GREEN,
                        "lines": [
                            "直接挂载于第 43 原生工具 body_state (perf_snapshot / perf_awr / perf_tune)",
                            "CLI / MCP / Web「本体与生命体征」大屏一键抓快照、定基线、触发 L1 调优",
                            ("agent-body body perf-awr  |  agent-body body perf-tune", True),
                        ],
                    },
                ],
                animated=True,
                mid="ahawr",
            ),
            "anim_caps": [
                "Step 1/5：无锁 WaitEventRegistry 以 <100ns 开销捕获 Lock/IO/Net/CPU/IPC 18 类等待事件",
                "Step 2/5：零特权直读 /proc/self 获取 VmRSS、FD 句柄、上下文切换与 SQLite WAL 尺寸",
                "Step 3/5：AWR 引擎持久化 32 份快照并支持设定 Golden Baseline 进行任意两态差分",
                "Step 4/5：ADDM 5 大规则自动诊断根因，在 L1 包络内一键触发 shrink_memory + malloc_trim",
                "Step 5/5：与 MeshEnvelope v1.0 联邦协议共同内聚在 43 工具体系内，零外部依赖",
            ],
            "next": "架构讲得再好，都要拿真机数字说话。接下来看 Benchmark v3.9 的三轨实证成绩单。",
            "script": (
                "第六器官把数据库内核的经典工程思想搬进了 AI 基础设施。\n"
                "我们在生产排查过一次 DuckDB 并发同步把 CPU 打到 600%、内存冲到 1.3G 的事故。修完之后我们想：既然自己是做 PostgreSQL 出身，为什么不给 Agent 本体装上原生的等待事件、AWR 黄金基线和 ADDM 自动诊断？\n"
                "于是我们在 Phase 50 用不到 100 纳秒的无锁原子计数器，实现了 6 大类 18 种等待事件直方图，配合零特权直读 /proc/self 内核探针，一键抓取快照、对比黄金基线，还能在 L1 权限内直接触发 SQLite 内存收缩与 glibc 堆页归还。\n"
                "跨主机方面，我们拒绝造重型 Raft，用轻量 HMAC-SHA256 防重放的 MeshEnvelope v1.0 搞定跨机图谱查询、脱敏对话和 10MB 沙盒文件投递。"
            ),
        },
        # 11 实证一 · Benchmark v3.9 帕累托双门禁
        {
            "eyebrow": "10 · EMPIRICAL PROOF · BENCHMARK v3.9 PARETO GATE",
            "title": "实证成绩单：省 99.23% Token，上下文质量反升 +64.2 分",
            "lead": "严禁靠丢信息骗压缩率！Benchmark v3.9 强制执行<b>帕累托双门禁</b>：既要省 Token，又要质量分 <i>Q</i> 不降反升且必含事实 100% 命中。",
            "svg": svg_metrics_table(
                kpis=[
                    ("-99.23%", "Prompt Token 节省 (D1)", "662,502 → 5,114 tok"),
                    ("98.0/100", "上下文质量分 (D6)", "基线 35.6 → 本体 98.0 (+64.2)"),
                    ("94.5x", "有效信息密度提升", "Facts / 1k Tokens 实测"),
                    ("39 / 39", "W/M/DQ/COMP 全维度", "W1~W19 + M1~M6 + DQ1~DQ10 + C1~C4"),
                ],
                headers=["核心负载 / 微基准", "场景描述", "Token / 耗时实测", "质量分 (基线→本体)", "状态"],
                rows=[
                    ["W7 · 防爆路由", "百级 SKILL 预算路由与渐进披露", "609,997 → 358 tok (-99.9%)", "41.2 → 100.0 (+58.8)", "PASS"],
                    ["W15 · AOPCC", "乐观直通 + 悲观门禁 + <2ms 自愈", "174 → 28 tok (-83.9%)", "52.0 → 100.0 (+48.0)", "PASS"],
                    ["W16 · 双域审计", "脱敏防洪 10→1 与租约自进化", "14,500 → 2,400 tok (-83.5%)", "36.0 → 100.0 (+64.0)", "PASS"],
                    ["W17~W18", "跨大脑接力与内生 AWR/ADDM 自诊断", "2,400→175 / 2,200→60 tok", "质量 100.0 (+63/+67)", "PASS"],
                    ["M1 / M2 / M3 / M6", "3跳图 / FTS5 / 放置 / 无锁 WaitEvent P50", "139μs / 259μs / 33μs / 7μs", "全低于 SLA 150~1500 μs", "PASS"],
                    ["DQ1 ~ DQ10", "元数据/拓扑/脱敏/新鲜度/图谱/稳态/遥测", "DQ1/4/6/7/8/9 100%·DQ5 98.5%", "18/18 WaitEvent 全埋点", "PASS"],
                ],
                col_xs=[16, 260, 680, 1020, 1320],
                footnote="真机落库存证：实机 Benchmark v3.9 · SHA-256 防篡改存证 · 生产调用按 PR-01~14 实时记账",
            ),
            "next": "光跟裸奔的自己比还不够，跟外部热门开源项目同机同数据比，表现如何？",
            "script": (
                "我们极其厌恶拍脑袋吹牛，所以把实证基准直接做成了核心能力，并且定下死规矩：绝不允许靠删掉关键事实来刷 Token 节省率。\n"
                "看这组真机跑出来的 Benchmark v3.9 成绩单，报告带 SHA-256 防篡改指纹：19 个标准业务负载总 Prompt Token 从 66.25 万降到 5,114，省了 99.23%；与此同时，由事实召回、信噪比、可行动性和安全性合成的四维上下文质量分从 35.6 飙升到 98.0，有效信息密度提升了 94.5 倍。\n"
                "再看微基准：放置决策 33 微秒、3 跳图遍历 139 微秒、FTS5 检索 259 微秒、WaitEvent 无锁探针 7 微秒；9 项宿主健康度门禁覆盖遥测与稳态全维度。"
            ),
        },
        # 12 实证二 · 四大赛道竞品横向对标
        {
            "eyebrow": "11 · COMPETITIVE LANDSCAPE · COMP-C1 ~ C4",
            "title": "四赛道同机横向实测：绝不捏造跑分，直面差异边界",
            "lead": "在长期记忆、上下文数据库、Agent 文件系统与 CLI 探针四大开源赛道同机实跑，严格区分 <b>LIVE_BENCHMARKED</b> 与 <b>ARCH_VERIFIED</b>。",
            "svg": svg_metrics_table(
                kpis=[
                    ("COMP-C1", "长期记忆脱敏赛道", "0/5 泄露 vs 5/5 明文"),
                    ("COMP-C2", "上下文路由赛道", "931 vs 144,730 tok"),
                    ("COMP-C3", "AgentFS 治理赛道", "10 净入库 vs 290 全量"),
                    ("COMP-C4", "宿主探针拓扑赛道", "0 子进程 + 3 跳影响图"),
                ],
                headers=["赛道 ID", "同机对标基线 / 竞品", "竞品实测表现", "Agent Body 实测表现", "质量净增 / Token 省"],
                rows=[
                    ["COMP-C1", "裸文本记忆流水 (Mem0/JSON 无脱敏)", "5/5 明文凭据落盘", "0/5 泄露 · Top-5 FTS 召回", "+75.9 分 / -97.8%"],
                    ["COMP-C2", "OpenViking 摘要树 + 全量技能注入", "60 虚拟文件 / 144,730 tok", "0 副本搬迁 / 931 tok", "+54.5 分 / -99.4%"],
                    ["COMP-C3", "AgentFS (SQLite VFS 全量导入)", "290 文件无过滤全入库", "10 净入库 · 阻断 4 黑名单目录", "+73.6 分 / -96.5%"],
                    ["COMP-C4", "rg + psutil + ss 多子进程组合", "3 子进程 89ms / 0 跳拓扑", "0 子进程 20ms / 2 上游影响点", "+60.2 分 / -47.0%"],
                ],
                col_xs=[16, 150, 560, 860, 1180],
                footnote="透明原则：Mem0 / Letta / PowerContext 为可插拔兼容生态（非零和替代），Agent Body 专注宿主机物理具身护城河",
            ),
            "next": "内核与数据都经得起拷打，那开发者和决策者日常怎么直观地使用与演示它？",
            "script": (
                "好产品不能只在温室里和自己比。我们在 Benchmark 轨道 C 里拉了四个最热门的开源赛道同机同数据集实测。\n"
                "在长期记忆赛道，对比无脱敏裸流水，我们做到 0 明文凭据泄露且 Token 省 97.8%；在上下文路由赛道，对比静态全量技能注入，我们零副本搬迁把 14.4 万 Token 压到 931；在文件系统治理赛道，对比全量导入的 VFS，我们直接拦住黑名单目录，净入库 10 份核心资产；在宿主机探针赛道，对比拼装 rg+psutil+ss 多子进程，我们单进程快 4 倍多，还多出跨服务 3 跳依赖图。\n"
                "同时我们态度非常开放：对 Mem0、Letta、PowerContext，我们不是替代，而是通过 SPI 一行配置兼容拥抱。"
            ),
        },
        # 13 产品形态 · 43/43 黑白屏双向同源与 7 幕路演剧场
        {
            "eyebrow": "12 · PRODUCT EXPERIENCE · 43/43 PARITY & LIVE DEMO",
            "title": "产品形态：43/43 黑白屏双向同源与 7 幕真引擎剧场",
            "lead": "黑屏（MCP / CLI / SKILL / Hooks）供大脑极速调用，白屏（单二进制 Web 指挥舱）供人类实时治理、审计与对外路演。",
            "svg": svg_columns(
                [
                    {
                        "tag": "BLACK SCREEN · 大脑接口",
                        "title": "43 MCP · 30 CLI · Hooks",
                        "accent": C_TEAL_D,
                        "lines": [
                            "43 个原生 body_* MCP 工具",
                            "SessionStart / Pre / Post 自动挂钩",
                            "3 套内置 SKILL 开箱即用",
                            ("新能力内聚子动作，0 工具膨胀", True),
                        ],
                        "foot_lbl": "协议契约",
                        "foot_val": "JSON-RPC 2.0 stdio / UDS",
                    },
                    {
                        "tag": "WHITE SCREEN · 人类指挥舱",
                        "title": "昼夜双主题 · 9 大工作台",
                        "accent": C_TEAL,
                        "lines": [
                            "暖纸蓝图 / 深空夜间一键切换 (T)",
                            "真实拓扑图、协作锁、审计、AWR",
                            "每次点击同步回显等价 CLI/MCP",
                            ("Parity 矩阵自动化守卫 100% 对齐", True),
                        ],
                        "foot_lbl": "访问方式",
                        "foot_val": "agent-body web --port 8756",
                    },
                    {
                        "tag": "ROADSHOW · 7 大黄金剧目",
                        "title": "真引擎沙盒 · 一键无痕复位",
                        "accent": C_CLAY,
                        "lines": [
                            "左右分屏：盲人翻车 vs 本体化解",
                            "含第 7 幕「换脑不失忆」实战演示",
                            "全部跑真实引擎，严禁假数据",
                            ("demo:* 沙盒隔离，演完一键复原", True),
                        ],
                        "foot_lbl": "同源入口",
                        "foot_val": "body_demo_scenario / Web UI",
                    },
                ],
                bottom_banner=(
                    "杜绝「白屏好看没功能、黑屏能用看不见」的割裂：所有 API 经 /api/v1/parity/matrix 强校验 43/43 100% 同步。",
                    "1:1:1 BLACK/WHITE PARITY",
                    "teal",
                ),
            ),
            "next": "这么完整的全栈系统，部署和运行成本到底重不重？看工程交付账单。",
            "script": (
                "在产品体验上，我们定了一条死规矩：黑白屏 100% 双向同源。\n"
                "给 AI 大脑用的是黑屏：43 个原生 MCP 工具、30 个顶层 CLI 命令，还有自动挂载的 SessionStart、Pre-Hook、Post-Hook；给人类主理人用的是单二进制内嵌的 Web 指挥舱，支持暖纸蓝图和深空夜间一键切换，9 大工作台覆盖拓扑、事实雷达、锁、生态、规约、路演、Benchmark、审计与本体中枢，你在网页上点的每一个按钮，旁边都会同步回显等价的 CLI 和 MCP 请求。\n"
                "而且 Web 里还内置了 7 幕路演演示剧场——全部是真实引擎在隔离沙盒里真跑，演完一键无痕复位。"
            ),
        },
        # 14 工程交付账单与五重质量门禁
        {
            "eyebrow": "13 · ENGINEERING BILL · ZERO-PANIC & QUALITY GATE",
            "title": "工程账单：单二进制、位置无关与五重确定性门禁",
            "lead": "用系统级数据库工程标准打造 AI 底座：<b>零外部运行时依赖、零生产 Panic、位置无关单二进制</b>，每次提交过五重硬门禁。",
            "svg": svg_code_and_flow(
                left_title="五重确定性质量门禁 (Makefile · 5 秒一键联检)",
                file_ref="Makefile & scripts/",
                code_lines=[
                    "# 一键全检：零漂移 + 零 Panic + 卫生 + 零警告 + 全测 + 演说验真",
                    "$ make quality-gate && make test && make verify-decks",
                    "",
                    "1. evolution-check : 27模块 / 43MCP / 88SPI / 0断链 / 0 drift",
                    "2. lint-panic      : 生产代码 0 处 .unwrap() / .expect()",
                    "3. lint-hygiene    : 0 个>1500L文件 · let _= 43/50 · 0硬编码",
                    "4. lint-strict     : cargo rustc -D warnings + clippy 零警告",
                    "5. cargo test      : 347+ 单元/集成/Web/并发/E2E 全绿 PASS",
                    "6. verify-decks    : Playwright 无头验真 9/9 HTML 0 溢出/重叠",
                ],
                right_title="PRODUCTION RUNTIME BUDGET",
                right_cards=[
                    ("极致轻量化与低开销", "单次 CLI/MCP 常驻 <15MB；全功能守护 <90MB、空闲 CPU <0.5%", "SQLite WAL + PRAGMA shrink_memory + malloc_trim(0)"),
                    ("100% 位置无关与跨平台", "搬移二进制至任意目录或 Docker 容器直接运行，PlatformProbe 抽象三端", "Linux 原生深化 + macOS / Windows 容错适配"),
                    ("零重扫演进体系", "EVOLUTION_GUIDE.md + 自建 .codegraph/，新 Agent 2 轮精准锁触点", "27 模块 / 27 表 / 3 后端存储 (SQLite / PG / DuckDB)"),
                ],
            ),
            "next": "做工程最怕什么都想做结果沦为平庸。接下来亮出我们的架构克制清单。",
            "script": (
                "很多 AI 基础设施演示很炫，一上生产就内存泄漏、依赖地狱。Agent Body 是按数据库内核的工程纪律打磨的。\n"
                "左边是我们每次提交必过的五重确定性门禁：文档与代码计数零漂移、生产代码零 unwrap/expect 防 Panic、零编译期硬编码路径保证二进制搬到哪儿都能跑、严格零警告编译、320 多项自动化测试全绿，连今天这两份演说 HTML 都要过 Playwright 无头浏览器验真。\n"
                "运行账单同样克制：单次 CLI 常驻不到 15 兆，全功能常驻守护不到 90 兆，空闲 CPU 低于 0.5%，单二进制零外部运行时依赖。"
            ),
        },
        # 15 架构克制 · 递弱代偿边界
        {
            "eyebrow": "14 · ARCHITECTURAL RESTRAINT · ANTI-FRAMEWORK CREEP",
            "title": "递弱代偿：明确「做什么」与「坚决不做什么」",
            "lead": "越基础的能力越被深度依赖。升维为 Agent 本体，绝不意味着膨胀成臃肿的全栈 Agent Runtime。",
            "svg": svg_matrix_2x2(
                [
                    {
                        "tag": "DO · 核心聚焦 01",
                        "title": "宿主机具身感官与防撞反射",
                        "accent": C_GREEN,
                        "bg": C_GREEN_L,
                        "lines": [
                            "物理事实图谱（工具/进程/端口/服务/依赖）",
                            "AOPCC 并发控制、Pre-Hook 拦截与多 Agent 锁",
                            "8 大放置规范自演化与 Context 防爆路由",
                        ],
                    },
                    {
                        "tag": "DO · 核心聚焦 02",
                        "title": "跨大脑连续性与稳态代谢",
                        "accent": C_TEAL,
                        "bg": C_TEAL_L,
                        "lines": [
                            "写穿保鲜海马体与 <=300 tok 跨大脑接力胶囊",
                            "双域全链路脱敏审计与 P1~P5 闭环自进化",
                            "内生 Wait Events / AWR / ADDM 与轻量 Mesh",
                        ],
                    },
                    {
                        "tag": "DO NOT · 坚决不做 01",
                        "title": "不做大脑与编排框架",
                        "accent": C_CLAY,
                        "bg": C_CLAY_L,
                        "lines": [
                            "不内置主战 LLM ReAct 规划循环与业务推理",
                            "不替代 LangGraph / AutoGen / CrewAI 编排器",
                            "不做面向人类日常聊天的个人助理 IM",
                        ],
                    },
                    {
                        "tag": "DO NOT · 坚决不做 02",
                        "title": "不重复造重型基础设施",
                        "accent": C_GOLD,
                        "bg": C_GOLD_L,
                        "lines": [
                            "不替代 Docker / gVisor 容器沙箱与 Vault 密钥库",
                            "不造通用 OLTP 数据库与重型 Raft/Paxos 共识",
                            "不替代 Rsync/S3 大文件同步与 CodeGraph AST",
                        ],
                    },
                ],
                footer_banner="做专、做深、做透物理宿主具身底座：把大脑还给大模型，把微观代码交给 CodeGraph，把记忆生态留给 SPI。",
            ),
            "next": "最后，用一页看清 Agent Body 的终局愿景，以及 3 行命令 5 分钟接入方式。",
            "script": (
                "项目走到 Phase 50，我越发觉得克制比加功能更难、也更重要。王东岳先生讲「递弱代偿」：越基础、越纯粹的底层能力，越不可替代。\n"
                "上面两格是我们死磕到底的护城河：宿主机物理感官、小脑防撞反射、跨大脑连续性、双域审计、内生 AWR 稳态和轻量联邦。\n"
                "下面两格是我们明确写进规约 #2 的「坚决不做」清单：不内置业务 LLM 循环、不替代多 Agent 编排框架、不造容器沙箱和密钥库、不造重型 Raft。正因为边界极度清晰，它才能零冲突地嵌入任何 AI 开发工作流。"
            ),
        },
        # 16 终局与 5 分钟上手
        {
            "eyebrow": "15 · VISION & QUICK START",
            "title": "终局：面向智能体时代的下一代 OS 原生具身底座",
            "lead": "今天它是外挂在 Linux 宿主机上的 Rust 单二进制；明天，它所定义的<b>感官、反射、连续性与稳态原语</b>，将成为操作系统的标配。",
            "svg": svg_closing(
                kicker="NEXT-GEN OS EMBODIED SUBSTRATE FOR AI AGENTS",
                headline="让每个瞬态大脑，一着陆就拥有耳聪目明的物理身体",
                pillars=[
                    ("1. 编译与启动（1 分钟）", "cargo build --release && make restart", "零外部依赖 · 自动初始化 ~/.agent-body/body.db"),
                    ("2. 接入大脑（2 分钟）", "挂载 MCP (agent-body mcp) + 生成 Hooks", "Antigravity / Claude Code / Cursor 即刻生效"),
                    ("3. 真机验证（2 分钟）", "agent-body bench run --suite full", "agent-body web --port 8756 体验 7 幕路演与 AWR"),
                ],
                open_question="深度技术架构请移步「深度交流版 agent-body-deck.html (24 页)」与「6 讲 × 30 分钟专题系列 index.html」",
                next_talk="开源仓库：github.com/digoal/agent-body · 全套手册见 docs/manual/01~10",
            ),
            "script": (
                "最后收个尾。操作系统过去五十年是为人类设计的——提供文件系统、进程树和终端；但在智能体时代，操作系统需要一层原生面向 AI 大脑的具身底座，替大脑管好物理感官、痛觉反射、工作态连续性和代谢稳态。\n"
                "Agent Body 已经用开源代码把这条路跑通了。只需要三步、五分钟：编译启动守护、挂载 MCP 与 Hooks、跑一遍全量 Benchmark 和 Web 指挥舱，你就能亲眼看到它如何替你的 Agent 省下 99% 探索 Token 且让回答质量大幅跃升。\n"
                "如果你想深入看 27 个模块、AOPCC 数学边界和 AWR 内核实现，欢迎打开旁边的 24 页深度技术交流版，或者 6 讲专题系列。感谢聆听，欢迎交流拍砖！"
            ),
        },
    ]

    render_deck(
        doc_title=f"Agent Body · Agent 本体对外路演版 (v{APP_VER})",
        brand_label="AGENT BODY · 对外路演版",
        foot_label=f"AGENT BODY · BRAIN-BODY SEPARATION · ROADSHOW v{APP_VER}",
        slides_data=slides,
        out_path=PRES_DIR / "presentation.html",
    )
    print(f"✅ Generated {PRES_DIR / 'presentation.html'} ({len(slides)} slides)")


if __name__ == "__main__":
    build_roadshow_deck()
