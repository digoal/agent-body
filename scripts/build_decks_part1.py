#!/usr/bin/env python3
"""Generate Lectures 01~03 (30-min deep-dive HTML decks) into presentation/."""

from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from deck_engine import (
    C_CLAY, C_CLAY_L, C_GOLD, C_GOLD_L, C_GREEN, C_GREEN_L, C_INK, C_LINE,
    C_PAPER, C_PAPER2, C_TEAL, C_TEAL_2, C_TEAL_3, C_TEAL_D, C_TEAL_L, C_TEAL_M, C_WHITE,
    render_deck, svg_bars, svg_closing, svg_code_and_flow, svg_columns,
    svg_compare_rows, svg_cover, svg_matrix_2x2, svg_metrics_table, svg_pipeline,
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
OUT = ROOT / "presentation"


def build_talk_01() -> None:
    slides = [
        {
            "cover": True,
            "eyebrow": "封面 · 第 01 讲",
            "title": "脑体分离与 Agent 本体哲学",
            "svg": svg_cover(
                kicker=f"SERIES 01 / 06 · AGENT BODY · v{APP_VER} · 30 MIN",
                title="脑体分离与 Agent 本体哲学",
                en_sub="Brain-Body Separation & Agent Embodiment Substrate",
                subtitle1="为什么大模型需要一具永驻宿主机的物理身体",
                subtitle2="从比较优势出发 · 划清本体定位边界 · 严守递弱代偿演进",
                spec_line="27 模块 · 43 原生工具 · 88 SPI · 6 大生理器官 · Apache-2.0",
                strata=[
                    ("可插拔大脑 (Transient Brain)", "Claude Code · Antigravity · Codex · Cursor · Cron", "#EBF1EE", C_INK),
                    ("统一神经接口 (43/43 Parity)", "MCP stdio · CLI · Web 2.0 · Pre/Post Hooks", "#CFE0DC", C_INK),
                    ("感觉神经 · 宿主机事实图谱", "Host Reality Graph · /proc · $PATH · systemd · CodeGraph", C_TEAL_D, C_PAPER),
                    ("小脑反射 · 规约脊柱 · 自治包络", "AOPCC 乐观直通 · Pre-Hook 红线 · L0~L3 权限矩阵", C_TEAL_2, C_PAPER),
                    ("海马体记忆 · 跨大脑接力胶囊", "Memory SPI · 新鲜度写穿 · body_checkpoints <=300 tok", C_TEAL_3, C_PAPER),
                    ("稳态免疫 · 多体联邦躯干", "五维生命体征 · 双域审计 · MeshEnvelope v1.0", "#A4C4C6", C_INK),
                ],
                right_tag="第 1 讲 · 30 分钟单点说透 · 地层剖面规范",
            ),
            "script": (
                "大家好，我是德哥。这是 Agent Body 体系化 30 分钟系列演讲的第 1 讲。\n"
                "今天整整 30 分钟，我们只打穿一个最底层、也最容易被误解的哲学与架构问题：什么是「脑体分离」，为什么我们把项目明确定义为「Agent 本体（Agent Embodiment Substrate）」，而不是外挂调度框架或个人工具。\n"
                "在写一行代码之前，先把定位想透，后面 27 个模块、43 个工具的边界才不会长歪。"
            ),
        },
        {
            "eyebrow": "01 · 定位边界",
            "title": "为什么必须定义为「Agent 本体」而非外挂辅助工具",
            "lead": "名称决定用户心智模型。<b>将定位聚焦为「Agent 本体（Agent Body）」，彻底划清与多 Agent 编排器及个人助理的边界</b>。",
            "svg": svg_compare_rows(
                ("常见行业标签", "未接触用户的第一直觉误读", "Agent Body 真实产品初衷与架构定位"),
                [
                    ("多 Agent 编排器", "管理、派发、驱动多个 Agent 干活的 Orchestrator", "绝不抢主战规划权，只替 Agent 看清物理世界"),
                    ("人类个人助理", "面向人类用户的日程/邮件/聊天个人助手", "服务对象是 Agent 而非人类，零云端 Token 税"),
                    ("记忆工具 (Memory)", "类似 Mem0 / Zep 的聊天摘要向量库", "记忆只占 1/6，其余五项都是宿主机物理现实与反射"),
                    ("Agent 本体 (Body)", "大模型在宿主机上的永驻具身底座（脑体分离）", "统筹感官、记忆与反射器官，定位无歧义且上限彻底打开"),
                ],
                ("外挂标签让人以为你在造第 51 个框架，新定位一句话讲清你是 Agent 的身体。", "来源：docs/idea_doc/04-agent-body-philosophy.md"),
            ),
            "next": "既然定位边界这么关键，那我们回到第一性原理：大脑和身体到底该怎么分工？",
            "script": (
                "做开源项目久了你会发现，命名起偏半寸，沟通成本翻十倍。\n"
                "如果把这类底座比作外置调度器，别人本能反应是「你这是不是又一个 CrewAI、AutoGen 多 Agent 编排框架？」；如果比作个人助理，又容易以为是面向人类老板的日历工具。\n"
                "这两个误解都会把路走窄。因为我们真正做的，是给所有瞬态大模型配一具常驻在这台 Linux/macOS/Windows 机器上的「身体」。感官盘点、防撞护栏与上下文压缩只是这具本体的内在生理器官。"
            ),
        },
        {
            "eyebrow": "01 · 第一性原理",
            "title": "比较优势：主战大脑的时间太贵，不该耗在物理杂务",
            "lead": "大卫·李嘉图比较优势在 AI 时代的直接推论：<b>云端大模型擅长高杠杆推理，本地本体擅长亚毫秒物理确定性</b>。",
            "svg": svg_columns(
                [
                    {
                        "tag": "CLOUD BRAIN · 瞬态大脑",
                        "title": "云端大模型 (LLM)",
                        "accent": C_CLAY,
                        "lines": [
                            "擅长：跨文件架构推演、代码生成",
                            "擅长：复杂故障根因推理与创造",
                            ("短板：每次会话冷启动完全失忆", True),
                            ("短板：单轮对话税 5k~30k tok + 2~4s", True),
                            "短板：看不见 /proc、端口、cron",
                        ],
                        "foot_lbl": "稀缺资源",
                        "foot_val": "上下文窗口 × 推理注意力",
                    },
                    {
                        "tag": "HOST BODY · 永驻本体",
                        "title": "Agent Body",
                        "accent": C_TEAL,
                        "lines": [
                            "擅长：/proc 0.2ms 探测端口与进程",
                            "擅长：inotify 实时感知文件改删",
                            ("优势：Rust 单二进制，0 云端 Token", True),
                            ("优势：跨会话、跨模型永驻宿主机", True),
                            "优势：确定性脱敏、防撞、加锁",
                        ],
                        "foot_lbl": "核心职责",
                        "foot_val": "感官 · 反射 · 海马体 · 稳态",
                    },
                    {
                        "tag": "PARETO OPTIMUM · 协同终局",
                        "title": "脑体分离帕累托最优",
                        "accent": C_GREEN,
                        "lines": [
                            "脑只管思考，体负责感知与记忆",
                            "换模型、换客户端，身体状态不丢",
                            ("实测：Prompt Token -99.22%", True),
                            ("实测：上下文质量 35.8 → 99.8", True),
                            "实测：有效信息密度提升 92.1 倍",
                        ],
                        "foot_lbl": "Bench v3.9 实证",
                        "foot_val": "run-6a0a4995 · SLA PASS",
                    },
                ],
                ("脑体分离不是哲学口号，是算力成本与物理延迟逼出来的唯一工程最优解。", "Bench v3.9 全绿实测", "teal"),
                animated=True,
            ),
            "anim_caps": [
                "① 云端大脑的比较优势与物理盲区",
                "② 永驻宿主机的本地本体优势",
                "③ 脑体协同达成 Token 与质量双赢",
                "④ 工程结论：成本与延迟决定分工",
            ],
            "next": "如果把大模型比作大脑，那这具常驻宿主机的身体，具体由哪六大器官组成？",
            "script": (
                "这一页是整套架构的经济账。\n"
                "硅谷创业者住在园区里，物业管水电餐食，创业者 100% 心流写代码；企业决策者配专职幕僚团队，自己只做核心拍板。背后都是大卫·李嘉图的比较优势：不是决策者不会订机票，而是他的机会成本太高。\n"
                "大模型也一样。你让一个几百亿参数的云端推理模型，花五轮工具调用去 find、ps、ss、cat 检查本机有没有装 pdftotext、5432 端口谁占着，既慢又贵还容易产生幻觉。\n"
                "把确定性的物理感知下沉到本地 Rust 本体，实测 Bench v3.9 上 Prompt Token 省了 99.22%，上下文质量反倒从 35.8 飙到 99.8。"
            ),
        },
        {
            "eyebrow": "02 · 生理隐喻",
            "title": "脑体分离解剖图：一个大脑可以热插拔，身体永驻宿主",
            "lead": "今天用 Claude Code，明天换 Antigravity，半夜跑 Cron 脚本——<b>大脑随时切换，但身体的感官、规约、伤疤与接力胶囊都在原地</b>。",
            "svg": svg_pipeline(
                [
                    {"tag": "BRAIN A", "title": "Claude Code", "accent": C_TEAL_D, "lines": ["上午做架构重构", "写入决策与接力胶囊", "会话结束退出内存"]},
                    {"tag": "HOST BODY", "title": "永驻本体底座", "accent": C_GREEN, "bg": C_GREEN_L, "lines": ["BodyManifest 身份", "body_checkpoints 表", "事实图谱 + 审计账本"]},
                    {"tag": "BRAIN B", "title": "Antigravity", "accent": C_TEAL_2, "lines": ["下午 1ms 拉取简报", "自动继承未竟目标", "零重扫直入联调"]},
                    {"tag": "BRAIN C", "title": "凌晨 Cron", "accent": C_GOLD, "lines": ["无头自动化巡检", "共用同一套锁与规约", "异常写入双域审计"]},
                ],
                [
                    {
                        "title": "没有本体：每次换工具都是「重新投胎」",
                        "accent": C_CLAY,
                        "bg": C_CLAY_L,
                        "lines": [
                            "Claude Code 踩过的端口坑，换到 Cursor 再踩一遍",
                            "Antigravity 梳理好的项目结构，新会话重扫 20 分钟",
                            "多个客户端并发改同一配置，彼此看不见直接覆盖",
                        ],
                    },
                    {
                        "title": "有了本体：大脑是无状态算力，身体是有状态资产",
                        "accent": C_TEAL,
                        "bg": C_TEAL_L,
                        "lines": [
                            "确定性 body_id 绑定宿主机（SHA-256 指纹）",
                            "43 个 MCP 工具 + CLI + Web 1:1 暴露同一具身体",
                            ("agent-body body manifest / vitals / handoff", True),
                        ],
                    },
                ],
                animated=True,
                mid="ah104",
            ),
            "anim_caps": [
                "① 上午 Brain A 干活并留下接力状态",
                "② 永驻宿主机本体持久化一切物理与认知资产",
                "③ 下午 Brain B 换脑接入 1ms 零重扫接盘",
                "④ 夜间无头 Cron 共享同一套锁与规约",
                "⑤ 对比总结：大脑无状态化，身体资产化",
            ],
            "next": "这具身体内部不是一锅大杂烩，而是严格分工的六大生理器官。",
            "script": (
                "看这张流转图，你就能瞬间理解「脑体分离」的威力。\n"
                "现在的 AI 研发常态是什么？一个人同时开 Claude Code、Antigravity、Cursor、Codex，还有后台十几个 Cron 任务。如果状态绑死在某一家客户端的云端会话里，你每次切换客户端，本质上都在让新 Agent 重新投胎。\n"
                "当我们把身体抽离成宿主机上的常驻底座后，所有客户端瞬间退化为「可热插拔的算力大脑」。上午用 Claude Code 写完一半，离开时落一枚 <=300 tok 的接力胶囊；下午打开 Antigravity，开工简报自动把目标、关键文件与未解风险递到嘴边。"
            ),
        },
        {
            "eyebrow": "02 · 六大器官",
            "title": "Agent 本体的六大生理器官全景",
            "lead": "27 个 Rust 模块并非随意堆砌，而是严丝合缝地映射到<b>感觉、小脑、海马体、脊柱、免疫、躯干</b>六大器官。",
            "svg": svg_columns(
                [
                    {
                        "tag": "ORGAN 01 · 感觉神经",
                        "title": "物理感官图谱",
                        "accent": C_TEAL_D,
                        "lines": ["只读采集物理真值", "/proc 端口 · systemd", "白黑灰扫盘 + CodeGraph", ("模块: fact/scanner/links", True)],
                    },
                    {
                        "tag": "ORGAN 02 · 小脑反射",
                        "title": "AOPCC 与痛觉红线",
                        "accent": C_TEAL_2,
                        "lines": ["开工签发乐观直通租约", "Pre-Hook 毫秒拦截冲突", "Post-Hook 碰壁自愈处方", ("模块: brief/hooks/coord", True)],
                    },
                    {
                        "tag": "ORGAN 03 · 海马体",
                        "title": "记忆与跨脑接力",
                        "accent": C_TEAL_3,
                        "lines": ["Memory SPI 可插拔门面", "文件改删写穿新鲜度", "body_checkpoints 胶囊", ("模块: memory/session/body", True)],
                    },
                    {
                        "tag": "ORGAN 04 · 规约脊柱",
                        "title": "放置与自治包络",
                        "accent": C_GREEN,
                        "lines": ["8 大资产放置规则引擎", "纠偏聚类自演化学习", "L0~L3 四级自治权限矩阵", ("模块: placement/body", True)],
                    },
                ],
                ("外加 ORGAN 05 稳态与免疫（五维体检 + 双域审计 + L1 自愈）与 ORGAN 06 多体联邦（MeshEnvelope v1.0 跨机协同）。", "src/body/manifest.rs", "teal"),
            ),
            "next": "能力这么多，最危险的诱惑就是膨胀成大而全的 Agent 框架——我们怎么克制自己？",
            "script": (
                "如果把 27 个模块平铺出来讲，听众五分钟就晕了。但用六大生理器官一归类，骨架立刻清晰：\n"
                "第一是感觉神经，对应宿主机事实图谱，回答「机器上此刻有什么」；第二是小脑反射，对应 AOPCC、Hook 门禁和协作锁，在毫秒内挡住撞端口、覆写和死锁；第三是海马体，负责长期记忆、信息新鲜度写穿和跨大脑接力胶囊；第四是规约脊柱，管目录放置规范和 L0 到 L3 的自治包络。\n"
                "再加上第五器官稳态与免疫（双域审计、五维体检与 L1 自愈），第六器官多体联邦（跨宿主机零信任协作）。每一个器官都能在 src/body/manifest.rs 里一键自省它的就绪态。"
            ),
        },
        {
            "eyebrow": "03 · 架构克制",
            "title": "递弱代偿原则：坚决画清「做什么」与「绝不做什么」",
            "lead": "升维成 Agent 本体，绝不意味着吞并大脑。<b>越是底座，越要保持只做确定性基础设施的克制</b>。",
            "svg": svg_matrix_2x2(
                [
                    {
                        "tag": "DO · 确定性物理感知",
                        "title": "我们坚决做深",
                        "accent": C_TEAL,
                        "bg": C_TEAL_L,
                        "lines": [
                            "零幻觉只读事实图谱（/proc, $PATH, systemd, cron）",
                            "毫秒级防撞红线（锁、端口、敏感路径、脱敏）",
                            "跨会话记忆门面 + 新鲜度写穿 + 接力胶囊",
                        ],
                    },
                    {
                        "tag": "DO NOT · 抢占主战大脑",
                        "title": "我们坚决不做",
                        "accent": C_CLAY,
                        "bg": C_CLAY_L,
                        "lines": [
                            "绝不内置多 Agent 任务拆解与 ReAct 主规划循环",
                            "绝不充当 LLM 推理网关或重造 RAG 聊天机器人",
                            "绝不替用户做 L2/L3 破坏性写决策（只预警不越权）",
                        ],
                    },
                    {
                        "tag": "DO · 可观测与自进化",
                        "title": "我们坚决闭环",
                        "accent": C_GREEN,
                        "bg": C_GREEN_L,
                        "lines": [
                            "IN_AGENT + OUT_OF_AGENT 双域审计可追溯",
                            "P1~P5 内生对比探针 + 五维稳态 L1 自主自愈",
                            "三轨实证 Benchmark v3.9 帕累托门禁守卫",
                        ],
                    },
                    {
                        "tag": "DO NOT · 重造重型轮子",
                        "title": "我们坚决借力",
                        "accent": C_GOLD,
                        "bg": C_GOLD_L,
                        "lines": [
                            "不重造符号级 AST 分析（联邦桥接 CodeGraph）",
                            "不重造分布式共识与 IM（Mesh 严守轻量零信任信封）",
                            "不垄断记忆引擎（SPI 开放接入 Mem0/Letta/PowerContext）",
                        ],
                    },
                ],
                "架构红线：底座只提供「物理现实 + 安全反射 + 连续状态」，把 100% 创造力留给大脑。",
                animated=True,
            ),
            "anim_caps": [
                "① 做深：确定性物理感知与安全反射",
                "② 不做：绝不抢占大模型主战规划循环",
                "③ 闭环：双域审计、探针自进化与实证基准",
                "④ 借力：不重造 AST、分布式共识与记忆轮子",
            ],
            "next": "定位升维后，老用户已经配好的已有 MCP 工具、CLI 命令和 ~/.agent-body 目录怎么办？",
            "script": (
                "很多人一听到「Agent 本体」，第一担心就是：你是不是要越做越重，最后变成又一个大而全的 Agent 平台？\n"
                "我们在 README 规约第 1 条和第 27 条写死了反膨胀铁律：绝不内置 ReAct 主规划循环，绝不调度多 Agent 干活，绝不重造 CodeGraph 或 Raft。\n"
                "就像人的小脑和脊髓绝不会去替大脑写诗，但只要手碰到开水，脊髓 1 毫秒内就把手缩回来——底座越克制、越确定，上层能挂载的大脑生态就越广阔。"
            ),
        },
        {
            "eyebrow": "03 · 平滑演进",
            "title": "三层命名零破坏契约：品牌升维，协议零断层",
            "lead": "架构演进最怕「为了改名把线上全部搞崩」。我们用<b>三层分层契约</b>兼顾心智升级与 100% 向后兼容。",
            "svg": svg_columns(
                [
                    {
                        "tag": "TIER 1 · 品牌与产品叙事",
                        "title": "全面升维为 Agent 本体",
                        "accent": C_TEAL_D,
                        "lines": [
                            "README / 演讲 PPT / Web 顶栏",
                            "主标题：脑体分离 · Agent 本体",
                            "副叙事：宿主机具身底座",
                            ("受众：新用户、架构师、开源社区", True),
                        ],
                        "foot_lbl": "门禁检查",
                        "foot_val": "evolution_manifest.py",
                    },
                    {
                        "tag": "TIER 2 · 概念与接口注释",
                        "title": "器官映射 + 兼容释义",
                        "accent": C_TEAL_2,
                        "lines": [
                            "Cargo.toml / CLI --help",
                            "MCP Tool Description / SKILL",
                            "明确统一：全部能力收拢为本体器官",
                            ("受众：接入的大模型与开发者", True),
                        ],
                        "foot_lbl": "三处同步",
                        "foot_val": "skills/ -> ~/.gemini & /root/new",
                    },
                    {
                        "tag": "TIER 3 · 二进制与协议标识",
                        "title": "100% 稳定零破坏红线",
                        "accent": C_GREEN,
                        "lines": [
                            "二进制：agent-body 不变",
                            "MCP 前缀：body_* 43 工具不变",
                            "数据目录：~/.agent-body/ 不变",
                            ("兼容：老配置零迁移直接平滑升级", True),
                        ],
                        "foot_lbl": "硬约束",
                        "foot_val": "零 Breaking Change",
                    },
                ],
                ("向上重塑认知，向下零迁移成本——这是成熟基础软件演进的标配纪律。", "docs/design_doc/17 §二", "green"),
            ),
            "next": "理念讲透了，现在我们直接钻进代码，看 Phase 44 新建的第 27 模块 src/body/ 到底长什么样。",
            "script": (
                "工程上最忌讳拍脑袋打破兼容。我们通过分层演进策略把所有旧工程形态平滑统一为 `agent-body` 与 `body_*` 工具族，让老用户的 `hooks.json`、MCP 配置、后台服务平滑过渡不出错。\n"
                "所以我们立了三层契约：第一层品牌与文档全面升维；第二层 CLI 帮助、MCP 描述与三套 SKILL 同步切换心智；第三层二进制名、MCP 工具前缀、`~/.agent-body/` 目录纹丝不动，直接原生提供 `43` 个全齐备工具（含第 43 工具 `body_state`）。"
            ),
        },
        {
            "eyebrow": "04 · 源码剖析 (一)",
            "title": "第 27 模块 src/body/：五大本体原语的极简实现",
            "lead": "没有空泛抽象，<b>4 个聚焦源文件</b>直接落地身份名片、接力胶囊、五维稳态与四级自治包络。",
            "svg": svg_code_and_flow(
                "src/body/mod.rs & manifest.rs",
                "src/body/manifest.rs:14-68",
                [
                    "// src/body/mod.rs — 第 27 模块统一导出",
                    "pub mod manifest;    // BodyManifest + compute_body_id()",
                    "pub mod continuity;  // ContinuityCapsule <= 300 tok",
                    "pub mod vitals;      // HomeostasisVitals 五维 + L1 自愈",
                    "pub mod envelope;    // AutonomyEnvelope L0~L3 边界矩阵",
                    "",
                    "pub fn compute_body_id() -> &'static str {",
                    "  BODY_ID_CACHE.get_or_init(|| {",
                    "    // SHA-256(hostname | os | arch | machine-id)",
                    "    // 前 16 位十六进制 -> \"body-54f9ef0a9a203852\"",
                    "  })",
                    "}",
                    "// 实机输出: body-54f9ef0a9a203852 (digoal-home)",
                ],
                "五大本体原语职责划分",
                [
                    ("1. BodyManifest（身体身份证）", "OnceLock 缓存确定性 body_id，实时汇报 6 大器官就绪态与 43/43 能力摘要", "agent-body body manifest"),
                    ("2. ContinuityCapsule（接力胶囊）", "落盘至第 27 表 body_checkpoints，经脱敏+防注入净化，严控 <=300 tok", "agent-body body handoff save/resume"),
                    ("3. HomeostasisVitals（五维稳态）", "感官/反射/海马体/代谢/免疫实时打分（实机 99.9/100），低分触发 L1 自愈", "agent-body body vitals"),
                    ("4. AutonomyEnvelope + Mesh 名片", "显式判定 L0 只读 / L1 自愈 / L2 需确认 / L3 禁区，支持跨机交换本体名片", "agent-body body envelope / peers"),
                ],
                animated=True,
            ),
            "anim_caps": [
                "① src/body/ 4 个核心子模块与确定性 body_id",
                "② 原语 1：BodyManifest 确定性身份与器官自省",
                "③ 原语 2：ContinuityCapsule 脱敏接力胶囊",
                "④ 原语 3：HomeostasisVitals 五维体检与自愈",
                "⑤ 原语 4/5：AutonomyEnvelope 与多体联邦名片",
            ],
            "next": "这五大原语里，最能体现「安全与自治平衡」的是哪一个？我们看四级自治包络。",
            "script": (
                "打开 `src/body/` 目录，只有四个干净的文件：`manifest.rs`、`continuity.rs`、`vitals.rs`、`envelope.rs`。\n"
                "先看 `compute_body_id()`：它读取宿主机 hostname、OS、arch 和 machine-id 做 SHA-256 哈希，用 Rust 标准库 `OnceLock` 缓存，在我的主机上稳定输出 `body-54f9ef0a9a203852`。\n"
                "为什么必须有确定性 `body_id`？因为当你有多台开发机、GPU 服务器通过 Mesh 互联时，每个大脑必须清楚自己此刻附着的是哪一具身体、有哪些器官已就绪。"
            ),
        },
        {
            "eyebrow": "04 · 源码剖析 (二)",
            "title": "AutonomyEnvelope：把「身体能做什么」刻成四级硬法律",
            "lead": "没有权限边界的自治就是裸奔。<b>L0~L3 四级包络</b>用确定性规则在 1 毫秒内裁决每一次宿主机动作。",
            "svg": svg_matrix_2x2(
                [
                    {
                        "tag": "L0_OBSERVE · 只读观测",
                        "title": "零副作用 · 随时放行",
                        "accent": C_TEAL,
                        "bg": C_TEAL_L,
                        "lines": [
                            "场景：查端口、搜资产、读图谱、查稳态体检",
                            "判定：allowed = true, requires_confirm = false",
                            "典型调用：body_fact_query / body manifest",
                        ],
                    },
                    {
                        "tag": "L1_AUTONOMIC · 自主反射自愈",
                        "title": "幂等可逆 · 后台自动执行",
                        "accent": C_GREEN,
                        "bg": C_GREEN_L,
                        "lines": [
                            "场景：回收过期锁、剪除幽灵端口节点、向量补全",
                            "判定：allowed = true，全程写入 OUT_OF_AGENT 审计",
                            "触发：BackgroundScheduler 发现稳态分 < 70.0",
                        ],
                    },
                    {
                        "tag": "L2_GUARDED · 显式确认门禁",
                        "title": "有状态变更 · 需确认或持锁",
                        "accent": C_GOLD,
                        "bg": C_GOLD_L,
                        "lines": [
                            "场景：改写共享配置、执行 GC 清理、断链自动修复",
                            "判定：allowed = true, requires_confirm = true",
                            "防线：默认 dry_run=true，写接口强制 Bearer Auth",
                        ],
                    },
                    {
                        "tag": "L3_FORBIDDEN · 绝对生理禁区",
                        "title": "硬拦截 · 无论什么大脑都拒之门外",
                        "accent": C_CLAY,
                        "bg": C_CLAY_L,
                        "lines": [
                            "场景：/etc/shadow、~/.ssh/id_rsa、rm -rf /、明文凭据外泄",
                            "判定：allowed = false, requires_confirm = false",
                            "DQ8 实测：L3 拦截 100% 命中，零逃逸",
                        ],
                    },
                ],
                "实机演练：agent-body body envelope --action write_file --target /etc/shadow  →  [L3_FORBIDDEN] allowed=false",
            ),
            "next": "光有包络还不够，身体每天 24 小时活着，怎么知道自己「健不健康」？",
            "script": (
                "很多人担心：你让一个守护进程在机器上自己跑自愈，万一它把我的文件删了怎么办？\n"
                "这就是 `AutonomyEnvelope` 的意义。我们把所有动作切成 L0 到 L3 四级：L0 只读随便看；L1 自主反射只允许幂等可逆的清理，比如回收过期协作锁、清理已关闭的幽灵端口节点，而且一笔一笔写进审计表；L2 涉及有状态修改，默认 dry-run 或需要显式确认与鉴权；L3 是绝对生理禁区，比如改 `/etc/shadow` 或读私钥，直接硬拒绝，任何提示词注入都绕不过去。"
            ),
        },
        {
            "eyebrow": "04 · 源码剖析 (三)",
            "title": "HomeostasisVitals：五维生命体征雷达与 L1 自主自愈",
            "lead": "把抽象的「系统健康」拆成可量化的五维百分制评分，<b>低于 70 分自动唤醒小脑反射自愈</b>。",
            "svg": svg_bars(
                "实机宿主 digoal-home 五维本体生命体征实测（run-6a0a4995 / DQ8 实证）",
                [
                    ("01 感官维度 (Sensory) · 资产元数据 & 图谱覆盖", 100.0, "100.0", "健康 · 零漂移", C_TEAL_D),
                    ("02 反射维度 (Reflex) · 24h AOPCC 租约有效率", 99.7, "99.7", "健康 · 毫秒反射", C_TEAL_2),
                    ("03 海马体维度 (Hippocampus) · 记忆冲突与新鲜度", 100.0, "100.0", "健康 · 零僵尸", C_TEAL_3),
                    ("04 代谢维度 (Metabolism) · 磁盘水位与过期锁清理", 100.0, "100.0", "健康 · 0 过期锁", C_GREEN),
                    ("05 免疫维度 (Immunity) · 脱敏审计与 L3 禁区守卫", 100.0, "100.0", "健康 · 0 明文泄露", C_GREEN),
                ],
                "实机综合稳态分：99.9 / 100 (healthy) · 后台每 1800s 自动巡检，低分自动触发 run_l1_autonomic_heal()",
            ),
            "next": "一个好的底座，必须做到「大脑用 MCP、人用 CLI、看大盘用 Web，三端能力分毫不差」。",
            "script": (
                "看这五条实机雷达条，不是画的假图，是我在 `digoal-home` 主机上跑 `agent-body body vitals` 的真实输出：综合分 99.9 / 100。\n"
                "感官维度看资产与图谱新鲜度；反射维度看过去 24 小时乐观直通租约命中率；海马体维度看记忆冲突与过期状态；代谢维度看磁盘水位和过期锁堆积；免疫维度看审计 SHA-256 完整性和明文凭据零泄露。\n"
                "只要任何一维拉低总分跌破 70，`BackgroundScheduler` 就会自动执行 `L1` 反射自愈，把过期锁清掉、把幽灵节点剪掉，全程留痕。"
            ),
        },
        {
            "eyebrow": "05 · 三端同构",
            "title": "43/43 黑白屏 1:1:1 同构：第 43 原生工具全景落地",
            "lead": "不搞「Web 有按钮、Agent 调不到」的割裂。<b>body_state 在 MCP、CLI、Web 共享同一内核引擎</b>。",
            "svg": svg_columns(
                [
                    {
                        "tag": "MCP STDIO · 给大脑调用",
                        "title": "body_state (#43)",
                        "accent": C_TEAL_D,
                        "lines": [
                            "action = manifest / vitals",
                            "action = envelope / peers",
                            "action = handoff_save",
                            "action = handoff_resume",
                            ("自动计入 PR-13 效能账本", True),
                        ],
                        "foot_lbl": "源码触点",
                        "foot_val": "src/api/mcp_tools.rs",
                    },
                    {
                        "tag": "CLI · 给工程师与脚本",
                        "title": "agent-body body",
                        "accent": C_TEAL_2,
                        "lines": [
                            "body manifest / vitals",
                            "body envelope --action --target",
                            "body handoff save / list / resume",
                            "body peers (跨机本体名片)",
                            ("零依赖终端即敲即得", True),
                        ],
                        "foot_lbl": "源码触点",
                        "foot_val": "src/main.rs (Commands::Body)",
                    },
                    {
                        "tag": "WEB 2.0 · 可视化驾驶舱",
                        "title": "#view-body 本体中枢",
                        "accent": C_GREEN,
                        "lines": [
                            "8 条 /api/v1/body/* REST 接口",
                            "写接口强制 Bearer Auth (401)",
                            "五维稳态仪表盘 + 胶囊管理台",
                            "路演剧场第 7 幕：换脑不失忆",
                            ("Parity 矩阵实测 43/43 = 100%", True),
                        ],
                        "foot_lbl": "源码触点",
                        "foot_val": "src/web/parity_api.rs",
                    },
                ],
                ("任何新能力必须三端同步交付，由 test_web_parity_matrix_100_percent 单元测试硬门禁守卫。", "43/43 Parity PASS", "teal"),
            ),
            "next": "光说架构好没用，我们拉出实机 Benchmark v3.9 账单，看「脑体分离」到底带来了什么量化收益。",
            "script": (
                "我们项目有一条铁律叫「黑白屏 1:1:1 双向同步」：黑屏是给 Agent 调的 MCP 和给工程师敲的 CLI，白屏是 Web 2.0 驾驶舱。\n"
                "Phase 44 新增第 43 个原生工具 `body_state` 时，三个通道同时落地，底层全调同一套 `src/body/` Rust 内核。连测试里都写死了 `test_web_parity_matrix_100_percent`，少对齐一个工具编译测试直接红。"
            ),
        },
        {
            "eyebrow": "05 · 实证成绩单",
            "title": "Benchmark v3.9 全绿基线：W18 内生 AWR 诊断与 DQ9 遥测门禁",
            "lead": "所有数据均来自本机实跑持久化存证 <b>run-6a0a4995-7302-4bc1-86b6-554a784a6156</b>，绝无手工估算。",
            "svg": svg_metrics_table(
                [
                    ("-99.22%", "17 负载总体 Token 节省", "650,073 → 5,065 tok"),
                    ("99.8/100", "D6 上下文质量（+64.0）", "基线 35.8 → 赋能 99.8"),
                    ("92.1x", "有效信息密度提升", "Facts / 1k Tokens"),
                    ("10 / 10", "DQ1~DQ10 宿主与遥测门禁", "全维度核验"),
                ],
                ["评测项", "场景说明", "基线 → 赋能", "质量/达标", "状态"],
                [
                    ["W17", "跨大脑连续性接力与本体器官自省", "2400 → 175 tok (-92.7%)", "37.0 → 100.0 (4/4)", "PASS"],
                    ["DQ8", "本体五维稳态健康度与自治包络合规", "稳态 99.9/100 · L3 硬拦截", "100.0% (>=99.0%)", "PASS"],
                    ["DQ2", "本地 Ollama Embedding 向量覆盖率", "199 / 200 抽样已生成向量", "99.5% (>=99.0%)", "PASS"],
                    ["scene-7", "Web 路演剧场第 7 幕：换脑不失忆", "独立内存沙箱真引擎实跑", "零宿主污染 · 一键复位", "PASS"],
                    ["cargo test", "全仓单元 + 集成测试（0 失败）", "347+ 单元与集成", "347+ passed / 0 failed", "PASS"],
                ],
                [18, 150, 560, 960, 1290],
                footnote="Benchmark v3.9 实机 SHA-256 存证 · 覆盖 W1~W19 / M1~M6 / DQ1~DQ10 / PR-01~14",
            ),
            "next": "从第 1 讲出发，接下来 5 讲我们将把这具身体的每一个器官彻底拆开讲透。",
            "script": (
                "看这份刚跑出来的 `v3.9` 全绿基线：`W1` 到 `W18` 19 个负载全过，`M1` 到 `M6` 6 个微基准全过，`DQ1` 到 `DQ9` 全维度覆盖，`COMP-C1` 到 `C4` 4 个热门竞品对标全过。\n"
                "单看新增的 `W17`：换脑接力从裸奔重扫的 2400 tok 压到 175 tok，省了 92.7%，上下文质量从 37.0 直接拉满到 100.0，4 条关键接力事实全部命中。"
            ),
        },
        {
            "eyebrow": "06 · 体系导航",
            "title": "6 讲 × 30 分钟：把 Agent 本体每一寸骨架拆透",
            "lead": "每一讲只聚焦<b>一个核心技术切面</b>，配有真实源码坐标、动画推演与实测数据，独立成篇又首尾咬合。",
            "svg": svg_columns(
                [
                    {
                        "tag": "TALK 01 · 本讲",
                        "title": "脑体分离哲学",
                        "accent": C_TEAL_D,
                        "bg": C_TEAL_L,
                        "lines": ["本体定位边界", "六大生理器官", "递弱代偿防膨胀", "src/body/ 源码"],
                    },
                    {
                        "tag": "TALK 02 · 感觉神经",
                        "title": "物理感官图谱",
                        "accent": C_TEAL_2,
                        "lines": ["/proc 0.2ms 探测", "白黑灰零污染扫盘", "L1/L2/L3 加权图谱", "CodeGraph 联邦"],
                    },
                    {
                        "tag": "TALK 03 · 小脑反射",
                        "title": "AOPCC 并发控制",
                        "accent": C_TEAL_3,
                        "lines": ["整轮对话税剖析", "乐观租约 vs 悲观锁", "Post-Hook 碰壁自愈", "WFG 死锁破局"],
                    },
                    {
                        "tag": "TALK 04~06 · 进阶三讲",
                        "title": "海马体 · 安全 · 实证",
                        "accent": C_GREEN,
                        "lines": ["04 新鲜度与跨脑接力", "05 双域审计与 Mesh", "06 Bench v3.9 工程", "完整闭环可复现"],
                    },
                ],
                ("建议按 01 → 06 顺序阅读，或在 presentation/index.html 按需直达任一专题。", "6 × 30 min 系列课程", "teal"),
            ),
            "next": "最后，用三个锚点和一个留给在座架构师的开放问题，收束今天的第 1 讲。",
            "script": (
                "这套系列一共六讲，每讲 30 分钟，绝不贪多求全、走马观花。\n"
                "今天第 1 讲把脑体分离的世界观立住了；第 2 讲专攻宿主机物理感官图谱；第 3 讲专攻数据库启发的 AOPCC 并发控制；第 4 讲拆解信息新鲜度与跨大脑接力；第 5 讲讲透纵深安全、双域审计与 Mesh；第 6 讲压轴展示三轨实证 Benchmark 与可复现工程体系。"
            ),
        },
        {
            "eyebrow": "06 · 收束",
            "title": "大脑会月月换代，身体必须留在你自己机器上",
            "lead": "模型能力正在快速商品化；真正决定 AI 长期复利的，是<b>留在本地宿主机上的物理认知与工程秩序</b>。",
            "svg": svg_closing(
                kicker="KEY TAKEAWAYS · 第 01 讲收束",
                headline="Agent Body：给每一个瞬态大脑，配一具永驻宿主的可靠身体",
                pillars=[
                    ("定位清澈", "不是多 Agent 编排器，也不是个人助理", "Agent Embodiment Substrate"),
                    ("边界克制", "严守递弱代偿：只做确定性感知、反射与连续态", "L0~L3 自治包络 · 0 越权"),
                    ("实证说话", "三层兼容零断层，43/43 同构，347+ 测试全绿", "Bench v3.9 · -99.22% Token"),
                ],
                open_question="开放思考：当你的团队同时使用 4 种以上 AI 编程客户端，谁来为它们共享的宿主机状态负责？",
                next_talk="下一讲预告 → 第 02 讲：宿主机物理感官图谱与零污染扫盘（02-host-reality-graph.html）",
            ),
            "script": (
                "最后留一句话送给大家：云端大模型每三个月就换一代，今天最强是 A，下个月可能是 B。你的工程资产、机器状态、踩坑经验绝不能绑死在某一家云端聊天框里。\n"
                "大脑可以随时换代，但身体必须稳稳留在你自己的宿主机上。\n"
                "留一个开放问题供大家茶歇讨论：当你的团队同时在用四五种 AI 客户端操作同一套开发环境，究竟该由谁来守护它们共享的物理现实？\n"
                "谢谢大家，我们第 2 讲见！"
            ),
        },
    ]
    render_deck(
        doc_title="第 01 讲 · 脑体分离与 Agent 本体哲学 — Agent Body",
        brand_label="SERIES 01 / 06 · 脑体分离",
        foot_label=f"AGENT BODY · 第 01 讲：脑体分离与 Agent 本体哲学 (v{APP_VER})",
        slides_data=slides,
        out_path=OUT / "01-brain-body-separation.html",
    )


def build_talk_02() -> None:
    slides = [
        {
            "cover": True,
            "eyebrow": "封面 · 第 02 讲",
            "title": "宿主机物理感官图谱与零污染扫盘",
            "svg": svg_cover(
                kicker=f"SERIES 02 / 06 · AGENT BODY · v{APP_VER} · 30 MIN",
                title="宿主机物理感官图谱与零污染扫盘",
                en_sub="Host Reality Graph, Zero-Noise Scanning & CodeGraph Federation",
                subtitle1="零幻觉不是靠提示词祈祷，而是只读物理真值",
                subtitle2="/proc 0.2ms 探测 · 白黑灰分级扫盘 · L1/L2/L3 加权图谱 · 宏微联邦",
                spec_line="W1 -99.4% · W3 阻断 531 噪音文件 · DQ3 100% · DQ6 100%",
                strata=[
                    ("感官入口 · body_fact_query", "capability / port / automation / impact / repo / doc", "#EBF1EE", C_INK),
                    ("物理实时探针 · 零子进程开销", "Linux /proc/net/tcp · PlatformProbe (macOS/Windows)", "#CFE0DC", C_INK),
                    ("分级颗粒度扫盘 · 敲门不进屋", "Whitelist 深扫 · Blacklist/Shallow 阻断 · 断点续传状态机", C_TEAL_D, C_PAPER),
                    ("三层语义元数据管道", "L1 确定性启发 · L2 Ollama 标注 · L3 Embedding 向量", C_TEAL_2, C_PAPER),
                    ("三层有向加权属性图", "L1_EXPLICIT (0.85~1) · L2_STRUCTURAL · L3_SEMANTIC", C_TEAL_3, C_PAPER),
                    ("宏微代码联邦 · CodeGraph Bridge", "Agent Body 宏观跨仓 + CodeGraph 微观 AST 符号穿透", "#A4C4C6", C_INK),
                ],
                right_tag="第 2 讲 · 30 分钟单点说透 · 地层剖面规范",
            ),
            "script": (
                "欢迎来到第 2 讲。上一讲我们确立了脑体分离的世界观，这一讲我们拆解 Agent 本体的第一大器官——「感觉神经」，也就是宿主机物理感官图谱（Host Reality Graph）。\n"
                "30 分钟里，我们只把一件事讲透：怎么让 Agent 在 1 毫秒内看清这台机器上装了什么工具、谁占着端口、哪些 cron 在跑、代码仓怎么依赖，同时绝对不把 node_modules 和 target/ 扫进库里把上下文撑爆。"
            ),
        },
        {
            "eyebrow": "01 · 盲人摸象",
            "title": "没有感官图谱，Agent 在宿主机上每天怎么翻车",
            "lead": "传统方案靠 Agent 临时敲 `find`、`ps`、`ss`、`rg`——<b>慢、耗 Token、漏看跨服务隐藏依赖</b>。",
            "svg": svg_compare_rows(
                ("高频工程动作", "裸奔 Agent 的盲摸路径", "物理感官图谱 1 毫秒直达"),
                [
                    ("提取 PDF / 转码视频", "没看见本机已有 pdftotext / ffmpeg，pip 装库报错", "fact_query(capability) 给出路径 + 最佳实践参数"),
                    ("启动本地 Web 服务", "盲绑 8080 端口，撞死现网看板或报错重试 3 轮", "/proc/net/tcp 0.2ms 返回占用 PID/Cwd + 空闲推荐端口"),
                    ("修改公共脚本/配置", "看不见有 systemd 单元与 crontab 强引用该文件", "automation + links 3 跳反查上游服务爆炸半径"),
                    ("调研开源项目源码", "没看见 /root/new/src/ 已有 110 仓与 .codegraph", "asset_search 0.4ms 命中现成源码树与代码地图"),
                ],
                ("W1 实测：工具能力发现从 21,079 tok 压至 131 tok（-99.4%），加速 358.31x。", "Bench v3.9 W1 实证"),
            ),
            "next": "为什么我们说「零幻觉」是架构属性，而不是提示词技巧？",
            "script": (
                "很多人以为减少幻觉靠在 System Prompt 里写「请不要编造」。没用的，机器里此刻跑着什么进程，大模型训练权重里根本不可能知道。\n"
                "我们在 Bench v3.9 的 W1 场景里实测：让 Agent 找本机 PDF/视频/JSON 处理工具，裸奔全扫要花 21,079 tok；调一次 `body_fact_query` 只花 131 tok，省了 99.4%，速度快了 358 倍，而且 5 条工具事实 100% 命中。"
            ),
        },
        {
            "eyebrow": "02 · 物理实时探针",
            "title": "0.2ms /proc 直读与内核底噪过滤：绝不把 kworker 塞给大脑",
            "lead": "在 Linux 上直接解析 `/proc/net/tcp` 与 `/proc/<pid>`，<b>零外部子进程派生</b>，并自动折叠多行内联脚本。",
            "svg": svg_code_and_flow(
                "src/fact/process_net.rs & src/env/mod.rs",
                "src/env/mod.rs · src/platform/",
                [
                    "// 1. 零子进程：直接读取 /proc/net/tcp & /proc/net/tcp6",
                    "//    inode -> /proc/<pid>/fd/* 精准映射监听端口与进程",
                    "// 2. 底噪过滤：剔除 kworker/migration/systemd-udevd/codegraph-watchdog",
                    "// 3. 命令规范化：折叠多行 node -e / python3 -c 防撑爆 Brief",
                    "// 4. 写前脱敏：SecretRedactor 剥离 URI 密码与 --token 参数",
                    "",
                    "pub trait PlatformProbe: Send + Sync {",
                    "  fn scan_listening_ports(&self) -> Vec<ListeningPort>;",
                    "  fn scan_cron_and_services(&self) -> Vec<AutomationUnit>;",
                    "}",
                    "// LinuxProcProbe | MacosProbe(lsof/launchd) | WindowsProbe(netstat/schtasks)",
                ],
                "物理探针四道工程防线",
                [
                    ("亚毫秒零派生", "直接读内核 procfs，端口查重 0.2ms 完成，彻底摆脱 ss/netstat 子进程开销", "COMP-C4: 0 子进程 vs 3 子进程"),
                    ("强力底噪过滤", "过滤内核线程与 watchdog 噪音，只给大脑看有业务语义的活跃服务与端口", "env::tests::test_is_noise_process"),
                    ("落库前强制脱敏", "进程命令行里的 postgres://user:pass@ 与 --api-key 在进 SQLite 前就地擦除", "DQ4 凭据卫生度 100.0%"),
                    ("三端平台抽象", "通过 PlatformProbe Trait 统一 Linux / macOS / Windows 采集语义", "src/platform/{linux,macos,windows}.rs"),
                ],
                animated=True,
            ),
            "anim_caps": [
                "① /proc 直读与 PlatformProbe 三端抽象代码",
                "② 防线一：零子进程亚毫秒读取",
                "③ 防线二：内核底噪与超长内联脚本折叠",
                "④ 防线三：写库前强制凭据脱敏",
                "⑤ 防线四：Linux / macOS / Windows 统一探针",
            ],
            "next": "看清了进程和端口，那磁盘上动辄几十万个文件怎么扫才不会把数据库和上下文彻底污染？",
            "script": (
                "写过宿主机监控的工程师都知道，直接把 `ps aux` 扔给模型是一场灾难：满屏 `kworker`、`systemd-udevd`，甚至有人在命令行里写了几百行的 `node -e` 内联脚本，或者带了数据库明文密码。\n"
                "我们在采集层就做了四件事：第一，Linux 直接读 `/proc` 零子进程；第二，过滤系统底噪并折叠超长内联脚本；第三，进库前先过 `SecretRedactor`，把 URI 密码和 token 彻底抹掉；第四，抽象出 `PlatformProbe` Trait，macOS 和 Windows 也能无缝接上。"
            ),
        },
        {
            "eyebrow": "03 · 零污染扫盘",
            "title": "白 / 黑 / 灰分级颗粒度扫盘：敲门，但不进屋",
            "lead": "遇到 `target/`、`node_modules/`、`.git/`、`.codegraph/`，<b>只建档目录本身一条标量记录，硬阻断递归</b>。",
            "svg": svg_columns(
                [
                    {
                        "tag": "WHITELIST · 深层精索引",
                        "title": "白名单工作区",
                        "accent": C_GREEN,
                        "bg": C_GREEN_L,
                        "lines": [
                            "覆盖：src/、docs/、scripts/、skills/",
                            "行为：递归扫描全部源码与文档",
                            "提取：L1 启发元数据 + 依赖边",
                            ("入库：全量 FTS5 + 向量检索就绪", True),
                        ],
                        "foot_lbl": "颗粒度",
                        "foot_val": "File-Level Deep Index",
                    },
                    {
                        "tag": "BLACKLIST · 浅层硬阻断",
                        "title": "黑名单噪音目录",
                        "accent": C_CLAY,
                        "bg": C_CLAY_L,
                        "lines": [
                            "覆盖：target/、node_modules/、.git/",
                            "覆盖：.venv/、__pycache__/、dist/",
                            "行为：Shallow Tier 仅记目录存在",
                            ("收益：W3 实测阻断 531 个噪音文件", True),
                        ],
                        "foot_lbl": "颗粒度",
                        "foot_val": "Directory-Only Stub",
                    },
                    {
                        "tag": "RESTRICTED · 机密绝缘层",
                        "title": "敏感文件硬隔离",
                        "accent": C_GOLD,
                        "bg": C_GOLD_L,
                        "lines": [
                            "覆盖：.env、*.pem、*.key、id_rsa",
                            "行为：绝不读取正文、绝不建 FTS",
                            "绝不送本地 Ollama 做语义摘要",
                            ("收益：从源头杜绝私钥进上下文", True),
                        ],
                        "foot_lbl": "安全等级",
                        "foot_val": "Zero-Content Read",
                    },
                ],
                ("大仓库扫盘采用 ScanStateMachine 断点续传（每次后台推 200 条），防 I/O 尖峰且随时可恢复。", "src/scanner/state_machine.rs", "teal"),
            ),
            "next": "文件进了白名单之后，怎么在不烧一分钱云端 Token 的前提下，读懂每个文件是干嘛的？",
            "script": (
                "这是我最得意的工程设计之一，叫做「敲门，但不进屋」。\n"
                "如果一个 Rust 项目里有 `target/`，一个前端项目里有 `node_modules/`，普通扫描器一头扎进去，几万个构建产物瞬间把 SQLite 和 FTS5 灌成垃圾场，搜什么都出噪音。\n"
                "我们的分级扫描器看到黑名单目录，只在门口记一笔「这里有个 `target/` 目录」，立刻掉头绝不递归。面对 `.env` 和私钥文件更是直接绝缘，连文件内容都不打开。实测 `COMP-C3` 对标 AgentFS：对方导入 290 个文件，我们净入库 10 个核心文件，Token 省了 96.5%。"
            ),
        },
        {
            "eyebrow": "04 · 三层语义元数据",
            "title": "L1 确定性启发 + L2 本地 Ollama + L3 向量化管道",
            "lead": "把 90% 的结构化提取交给亚毫秒规则，把 10% 的深度意图精炼交给本地 0.8B 小模型，<b>全程零云端费用</b>。",
            "svg": svg_pipeline(
                [
                    {"tag": "STAGE L1 · 0.1ms", "title": "确定性启发提取", "accent": C_TEAL_D, "lines": ["解析 Cargo/package.json", "提取 H1/docstring/fn", "识别领域与入口点", "离线 100% 兜底"]},
                    {"tag": "STAGE L2 · 本地", "title": "Ollama 小模型精炼", "accent": C_TEAL_2, "lines": ["qwen3.5:0.8b 后台批跑", "防 Meta 泄露 Grounding", "不覆盖已有高质量标注", "Daemon 闲时增量推"]},
                    {"tag": "STAGE L3 · 向量", "title": "Embedding 向量化", "accent": C_GREEN, "lines": ["nomic-embed-text 768d", "注入 norm/preview 摘要", "支撑语义召回与 L3 边", "现网覆盖率 99.5%"]},
                    {"tag": "DETAIL LEVEL", "title": "渐进式披露输出", "accent": C_GOLD, "lines": ["micro: 仅路径+一行摘要", "standard: 核心元数据", "full: 完整结构化档案", "按预算精确控 Token"]},
                ],
                [
                    {
                        "title": "为什么绝不依赖云端 LLM 做索引？",
                        "accent": C_TEAL,
                        "bg": C_TEAL_L,
                        "lines": [
                            "隐私：本地私有代码与文档永远不出宿主机网卡",
                            "成本：21,230 个宿主机资产全量索引，云端账单 = $0",
                            "韧性：即使 Ollama 离线，L1 确定性元数据仍保障 DQ1 = 100.0%",
                        ],
                    },
                    {
                        "title": "实机覆盖水位（2026-11-22 实测）",
                        "accent": C_GREEN,
                        "bg": C_GREEN_L,
                        "lines": [
                            "DQ1 资产描述元数据覆盖率：100.0%（1000/1000 抽检）",
                            "DQ2 本地 Ollama Embedding 覆盖率：99.5%（199/200 抽检）",
                            ("agent-body asset scan <dir> --enrich --enrich-limit 30", True),
                        ],
                    },
                ],
                animated=True,
                mid="ah205",
            ),
            "anim_caps": [
                "① L1 确定性启发：毫秒级兜底覆盖 100%",
                "② L2 本地 Ollama 小模型：闲时异步精炼意图",
                "③ L3 本地 Embedding：768 维向量入库",
                "④ 渐进披露：micro / standard / full 按需省 Token",
                "⑤ 隐私、零成本与实机 DQ1/DQ2 实证",
            ],
            "next": "单个文件看懂了，但真实宿主机里，文件、进程、服务和项目之间是网状互联的——图谱怎么建？",
            "script": (
                "看这条三段式元数据流水线：文件一进白名单，L1 确定性启发在 0.1 毫秒内就把 Cargo.toml、Markdown 标题、Rust 模块注释抽出来，保证哪怕你没装 Ollama，元数据覆盖率也是 100%。\n"
                "如果本机跑了 Ollama，守护进程就会在后台悄悄用 0.8B 小模型精炼用途描述，再用 `nomic-embed-text` 生成 768 维向量。今天我们实测宿主机 21,230 个资产，DQ1 覆盖率 100%，DQ2 向量覆盖率 99.5%，花掉的云端 API 费用是零。"
            ),
        },
        {
            "eyebrow": "05 · 图谱破孤岛",
            "title": "从 99.5% 孤立到 100% 连通：三层有向加权属性图",
            "lead": "Phase 40 现网实测曾暴露 214/215 项目节点孤立。我们用 <b>L1_EXPLICIT / L2_STRUCTURAL / L3_SEMANTIC</b> 彻底打通。",
            "svg": svg_columns(
                [
                    {
                        "tag": "L1_EXPLICIT · 0.85~1.00",
                        "title": "物理硬依赖边",
                        "accent": C_TEAL_D,
                        "lines": [
                            "进程监听端口 (LISTENS_ON)",
                            "systemd/cron 执行脚本",
                            "源码/文档精确文件引用",
                            ("可信度：物理实证 100%", True),
                        ],
                        "foot_lbl": "用途",
                        "foot_val": "端口防撞 · 爆炸半径核心",
                    },
                    {
                        "tag": "L2_STRUCTURAL · 0.55~0.84",
                        "title": "结构上卷与服务桥接",
                        "accent": C_TEAL_2,
                        "lines": [
                            "5,116 条文件引用上卷至项目",
                            "systemd 单元 ↔ 活跃进程桥接",
                            "工具能力与工程目录依赖挖掘",
                            ("突破：消除项目/服务同体分裂", True),
                        ],
                        "foot_lbl": "源码触点",
                        "foot_val": "src/fact/semantic_discovery.rs",
                    },
                    {
                        "tag": "L3_SEMANTIC · 0.35~0.65",
                        "title": "隐式语义关系发现",
                        "accent": C_GREEN,
                        "lines": [
                            "Ollama 向量余弦 >= 0.78",
                            "融合标签/域名 Jaccard 打分",
                            "严格 Top-K=3 剪枝防边爆炸",
                            ("收益：发现看似断连的跨仓关联", True),
                        ],
                        "foot_lbl": "现网 DQ6",
                        "foot_val": "工程连通 215/215 (100%)",
                    },
                ],
                ("W13 实测：三层加权图谱与隐式语义发现节省 84.8% Token，质量从 19.5 跃升至 100.0。", "DQ6: fact_edges=778/778 合法", "green"),
            ),
            "next": "图谱里如果混进了测试代码和 Markdown 示例里的假路径，爆炸半径就会误报——怎么免疫？",
            "script": (
                "Phase 40 之前我们查过一次自己的生产库，吓了一跳：`fact_nodes` 有 355 个节点，但 215 个项目节点里有 214 个是孤岛，99.5% 没连起来！\n"
                "原因很简单：文件之间的 `links` 边有五千多条，但没有上卷到项目级；systemd 单元和 `/proc` 里的运行进程各建各的节点，没桥接；两个在业务上强相关的项目没有硬导入，也就断连了。\n"
                "上线三层有向加权图谱后，L1 管物理硬依赖、L2 管结构上卷与服务桥接、L3 用向量余弦加 Top-K 剪枝挖隐式关联，今天现网 DQ6 工程节点连通率直接做到 215/215 = 100%。"
            ),
        },
        {
            "eyebrow": "06 · 抗假阳性硬化",
            "title": "LinksEngine 免疫测试夹具与模板占位符：DQ3 = 100.0%",
            "lead": "正则扫路径最容易把文档示例和单元测试误当真实依赖。<b>上下文感知过滤器</b>让断链审计零误报。",
            "svg": svg_code_and_flow(
                "src/links/mod.rs & src/links/audit.rs",
                "src/links/mod.rs · src/links/impact.rs",
                [
                    "// 1. 排除 Rust #[cfg(test)] 模块与注释里的测试夹具路径",
                    "// 2. 排除 Markdown 代码块内的示例路径与占位符 (<path>, ${VAR})",
                    "// 3. 过滤 JSON 端口号误匹配（如 \"port\": 8080 里的样例）",
                    "// 4. 每一条有效边记录精确行号与 provenance 出处",
                    "",
                    "// 跳数衰减加权爆炸半径推演 (Weighted Impact Analysis)",
                    "score(hop) = edge.weight * decay^(hop - 1)",
                    "// 默认只沿 L1_EXPLICIT + L2_STRUCTURAL 传播，",
                    "// L3_SEMANTIC 显式标注为「语义提示」不污染硬依赖报警。",
                ],
                "抗假阳性与爆炸半径设计",
                [
                    ("零假断链自污染", "Phase 37 修复前文档示例制造了大量假断链；免疫后现网 2000/2000 边 100% 物理有效", "DQ3 = 100.0% (>=85%)"),
                    ("断链自愈修复", "文件被移动或重命名后，Levenshtein 相似度匹配候选目标，支持 dry-run 与 --apply 修复", "body_links_repair"),
                    ("三跳加权爆炸半径", "修改一个底层脚本前，毫秒级反查 3 跳内受影响的 systemd、cron 与下游项目", "W5: -96.2% tok · 质量 100.0"),
                ],
            ),
            "next": "Agent Body 管住了宿主机宏观文件与服务，那进到一个 10 万行代码的仓库内部，怎么定位函数调用链？",
            "script": (
                "我们在自己狗食阶段踩过一个特别真实的坑：文档里写了个示例路径 `/etc/demo.conf`，结果 `links audit` 天天报警说断链。\n"
                "Phase 37 我们把 `LinksEngine` 彻底硬化：跳过 Rust `#[cfg(test)]` 块、跳过注释、跳过 Markdown 代码示例和 `${VAR}` 模板占位符，同时每一条真边都记下精确行号。现在现网 `DQ3` 抽检全量 2000 条依赖边，物理有效率 100.0%。"
            ),
        },
        {
            "eyebrow": "07 · 宏微双层联邦",
            "title": "Agent Body × CodeGraph：宏观跨仓与微观 AST 符号无缝接力",
            "lead": "严格遵守递弱代偿——<b>不重造 AST 解析器，而是把 Agent Body 宏观图谱与 CodeGraph 微观符号地图联邦打通</b>。",
            "svg": svg_pipeline(
                [
                    {"tag": "MACRO · Agent Body", "title": "跨仓资产定位", "accent": C_TEAL_D, "lines": ["定位 110+ 开源仓库", "识别 .codegraph/ 状态", "校验索引滞后度"]},
                    {"tag": "BRIDGE · 联邦桥", "title": "codegraph_bridge", "accent": C_GREEN, "bg": C_GREEN_L, "lines": ["探测 codegraph 二进制", "拼接精准 repo 路径", "返回下钻调用指令"]},
                    {"tag": "MICRO · CodeGraph", "title": "微观符号穿透", "accent": C_TEAL_2, "lines": ["AST 级函数定义读取", "动态派发调用链追踪", "远超 grep 正则盲搜"]},
                    {"tag": "PREVENT DRIFT", "title": "滞后自动告警", "accent": C_GOLD, "lines": ["比对源码 mtime vs db", "落后时在 Brief 提醒", "一键 codegraph sync"]},
                ],
                [
                    {
                        "title": "为什么「宏微联邦」比单打独斗强一个量级？",
                        "accent": C_TEAL,
                        "bg": C_TEAL_L,
                        "lines": [
                            "CodeGraph 只懂单个仓库内部的函数调用，不知道机器上还有哪些仓库、端口和定时任务",
                            "Agent Body 懂整台宿主机的跨仓/进程/服务拓扑，但不需要自己维护几十种语言的 AST 解析器",
                            "两者联邦：Agent Body 0.4ms 把大脑带到正确仓库门口，CodeGraph 一跳直达目标函数体",
                        ],
                    },
                    {
                        "title": "本仓实测状态",
                        "accent": C_GREEN,
                        "bg": C_GREEN_L,
                        "lines": [
                            "agent-body 自身已建 .codegraph/ 索引，增量同步 52 文件仅耗时 1.5s（1,707 nodes）",
                            "Systemd 守护进程自动探测 NVM/Cargo 路径下的 codegraph 二进制",
                            ("body_codegraph_bridge(repo_path=\"/root/new/work/agent-body\")", True),
                        ],
                    },
                ],
                animated=True,
                mid="ah208",
            ),
            "anim_caps": [
                "① 宏观层：Agent Body 管理整机仓库与服务拓扑",
                "② 联邦桥：body_codegraph_bridge 检测与路由",
                "③ 微观层：CodeGraph 穿透函数与动态派发链",
                "④ 防漂移：自动检测 .codegraph.db 索引滞后",
                "⑤ 联邦协同价值与本仓 1.5s 增量同步实测",
            ],
            "next": "文件在磁盘上每分每秒都在被改、被删，感官图谱怎么保持实时同步而不被 I/O 打爆？",
            "script": (
                "有的朋友问：你为什么不在 Agent Body 里顺手把 Tree-sitter AST 解析也做了？\n"
                "答案还是递弱代偿。CodeGraph 专门干单仓代码符号地图，已经做得极好；Agent Body 专门干宿主机全局物理现实。我们用 `body_codegraph_bridge` 把两者接起来：Agent Body 先帮大脑在 110 个仓库里找到正确项目、顺便检查 `.codegraph/codegraph.db` 有没有过期，再交棒给 CodeGraph 精确读函数源码。"
            ),
        },
        {
            "eyebrow": "08 · 实时神经反射",
            "title": "inotify 文件监听 + 60s 折叠防洪 + 后台定时深度巡检",
            "lead": "感官不能只靠手动触发。<b>被动事件驱动 + 主动周期巡检</b>双轨运转，兼顾实时性与低功耗。",
            "svg": svg_columns(
                [
                    {
                        "tag": "REACTIVE · 毫秒感知",
                        "title": "FileWatcher (inotify)",
                        "accent": C_TEAL_D,
                        "lines": [
                            "监听白名单根目录改/增/删",
                            "命中黑名单/浅层目录立即丢弃",
                            "写穿重置旧向量与 L2 标注",
                            ("删除时级联清理 FTS/图谱/记忆", True),
                        ],
                        "foot_lbl": "源码触点",
                        "foot_val": "src/watcher/handler.rs",
                    },
                    {
                        "tag": "ANTI-FLOOD · 60s 折叠",
                        "title": "防编译/格式化洪泛",
                        "accent": C_CLAY,
                        "lines": [
                            "曾实测 watcher 占 changes 表 94.7%",
                            "同一路径 60 秒内高频改写自动折叠",
                            "保留精确变更计数与最终态哈希",
                            ("结果：审计与变更表瘦身 90%+", True),
                        ],
                        "foot_lbl": "源码触点",
                        "foot_val": "src/audit/mod.rs",
                    },
                    {
                        "tag": "PROACTIVE · 每 1800s",
                        "title": "BackgroundScheduler",
                        "accent": C_GREEN,
                        "lines": [
                            "同步 /proc 进程与端口快照",
                            "增量刷新属性图与向量补全",
                            "刷新 MCP/SKILL 目录 + 新鲜度对账",
                            ("巡检五维稳态并触发 L1 自愈", True),
                        ],
                        "foot_lbl": "源码触点",
                        "foot_val": "src/daemon/scheduler.rs",
                    },
                ],
                ("现网 agent-body.service 常驻运行，单二进制内存极低，零人工干预维持全机感官鲜活。", "systemctl is-active -> active", "teal"),
            ),
            "next": "感官数据收集齐了，但不能一股脑全倒进上下文——我们如何做到「索引先行，按需下钻」？",
            "script": (
                "感官系统最怕两个极端：要么不更新变成死数据，要么一保存代码就触发几十次事件把数据库写爆。\n"
                "我们采用了动静双轨：静的是每 1800 秒跑一次的 `BackgroundScheduler`，做全机进程同步、属性图更新、新鲜度对账和向量补全；动的是 `inotify` 实时监听，文件一删立刻级联清理 FTS 和幽灵节点，同时用 60 秒窗口折叠高频编译事件，彻底消灭日志洪泛。"
            ),
        },
        {
            "eyebrow": "09 · 索引先行原则",
            "title": "Brief 索引先行 + Skill 防爆路由：为什么能省下 99% Token",
            "lead": "把 120+ SKILL 和两万个资产的全文直接塞进 Prompt 是灾难。<b>先给轻量地图，命中再按需加载</b>。",
            "svg": svg_metrics_table(
                [
                    ("599,695 → 358", "W7 百级 SKILL 路由 Token", "节省 99.9% · 质量 100.0"),
                    ("838.0x", "W7 有效信息密度倍数", "从全量注入到 Top-K 预算"),
                    ("772 μs", "M2 FTS5 全文检索均值", "P99 = 914 μs (SLA 1500)"),
                    ("298 μs", "M1 3 跳图谱遍历均值", "P99 = 430 μs (SLA 1500)"),
                ],
                ["负载 ID", "感官与图谱实测场景", "Token (基线 → 赋能)", "质量 (基线 → 赋能)", "状态"],
                [
                    ["W1", "宿主工具能力发现", "21,079 → 131 (-99.4%)", "60.5 → 100.0 (5/5)", "PASS"],
                    ["W2", "活跃端口防撞与空闲选址", "350 → 16 (-95.4%)", "42.0 → 100.0 (4/4)", "PASS"],
                    ["W3", "分级扫描与黑名单噪音阻断", "6,375 → 266 (-95.8%)", "51.0 → 100.0 (10/10)", "PASS"],
                    ["W5", "3 跳爆炸半径与断链自愈", "613 → 23 (-96.2%)", "25.5 → 100.0 (5/5)", "PASS"],
                    ["W10", "复杂模板依赖抽取抗假阳性", "172 → 19 (-89.0%)", "56.0 → 100.0 (2/2)", "PASS"],
                ],
                [18, 130, 560, 920, 1290],
                footnote="数据来源：Bench v3.9 全量实测 run-6a0a4995-7302-4bc1-86b6-554a784a6156",
            ),
            "next": "对标热门开源项目 OpenViking 与 AgentFS，这套感官方案在同机同数据集下表现如何？",
            "script": (
                "大家看 W7 这行数字：机器上装了上百个 SKILL，如果静态全量注入，一次对话就要吞掉近 60 万 Token；经过我们的 `ContextBudgetRouter` 按意图和 Token 预算精准召回 Top-K，只需要 358 Token，节省 99.9%，信息密度放大 838 倍。\n"
                "而且底层 Rust + SQLite FTS5 与属性图遍历都在几百微秒内完成：M1 三跳图谱遍历 P99 仅 430 微秒，M2 全文检索 P99 仅 914 微秒。"
            ),
        },
        {
            "eyebrow": "10 · 竞品实测对标",
            "title": "同机同数据对标 OpenViking、AgentFS 与原生 CLI 组合",
            "lead": "不靠 PPT 嘴炮，我们在 <b>COMP-C2 / C3 / C4</b> 中写好真实对照代码，同机实跑验收差距。",
            "svg": svg_compare_rows(
                ("对标赛道 (LIVE)", "开源竞品 / 传统基线实测", "Agent Body 实测优势"),
                [
                    ("COMP-C2 上下文路由", "OpenViking 虚拟摘要 + 静态全量技能：144,730 tok", "0 副本搬迁 · 931 tok (-99.4%) · 质量 45.5→100.0"),
                    ("COMP-C3 工作区治理", "AgentFS SQLite VFS 全量导入 290 文件", "白黑灰阻断 4 噪音目录 · 仅 10 净入库 (-96.5% tok)"),
                    ("COMP-C4 宿主冷启动", "rg + psutil + ss 派生 3 子进程 · 0 跳跨服务图 (176ms)", "0 子进程 · 2 个上游影响点 · 46ms · 质量 37.3→97.5"),
                ],
                ("零副本搬迁 + 白黑灰门禁 + 零子进程物理图谱，是感觉神经的三大硬壁垒。", "Bench v3.9 COMP-C2~C4 全 PASS"),
            ),
            "next": "如何在 30 秒内亲自在终端里体验一遍这套宿主机物理感官？",
            "script": (
                "我们在 Benchmark 轨道 C 里把常见替代方案全拉上同一个擂台：\n"
                "对标 OpenViking，我们不做虚拟目录副本搬迁，省了 99.4% Token；对标 AgentFS，我们靠白黑灰分级门禁挡住垃圾文件，只净入库核心资产；对标传统 `rg + psutil + ss` 拼装脚本，我们零子进程派生、自带跨服务拓扑，速度快将近 4 倍，上下文质量从 37.3 提到 97.5。"
            ),
        },
        {
            "eyebrow": "11 · 现场实操命令",
            "title": "工程师 6 条高频感官命令：黑屏敲完即刻验证",
            "lead": "所有感官能力全部开放为确定性 CLI 与等价 MCP 工具，<b>零门槛复现本讲全部结论</b>。",
            "svg": svg_code_and_flow(
                "宿主机物理感官 CLI 实战清单",
                f"agent-body v{APP_VER} CLI",
                [
                    "# 1. 意图级工具能力发现（返回路径、版本与推荐参数）",
                    "agent-body fact \"提取 PDF 文本\" -f capability",
                    "",
                    "# 2. 0.2ms 端口查重与空闲推荐",
                    "agent-body port check 8756",
                    "",
                    "# 3. 分级颗粒度扫盘 + 本地 Ollama L2 语义精炼",
                    "agent-body asset scan /root/new/work/agent-body --enrich",
                    "",
                    "# 4. 3 跳逆向爆炸半径分析 & 断链健康审计",
                    "agent-body links impact src/storage/spi.rs --depth 3",
                    "agent-body links audit",
                    "",
                    "# 5. 宏微联邦：一键检测并桥接 CodeGraph",
                    "agent-body codegraph bridge /root/new/work/agent-body",
                ],
                "等价 MCP 工具（大脑自动调用）",
                [
                    ("body_fact_query / body_port_check", "只读物理事实入口，零幻觉、亚毫秒返回", "L0_OBSERVE"),
                    ("body_asset_scan / body_asset_search", "白黑灰扫盘与 micro/standard/full 渐进召回", "DQ1=100% · DQ2=99.5%"),
                    ("body_links_impact / body_links_audit", "L1/L2 加权爆炸半径与抗假阳性断链自检", "DQ3=100% · DQ6=100%"),
                    ("body_codegraph_bridge", "跨仓定位后平滑交棒 CodeGraph 做符号级分析", "1.5s 增量同步"),
                ],
            ),
            "next": "感官让我们「看得见」，但接下来马上会遇到一个尖锐问题：每次干活前都先查一次 Agent Body，会不会太慢、太啰嗦？",
            "script": (
                "这六条命令大家散会后可以直接在终端敲一遍，每一条都能在毫秒到秒级看到真实宿主机反馈，而且在 Web 2.0 驾驶舱底部的黑屏命令镜像器里，也能实时看到对应的 CLI 和 MCP JSON-RPC。"
            ),
        },
        {
            "eyebrow": "12 · 边界与诚实清单",
            "title": "感官图谱的适用边界与已知权衡",
            "lead": "做工程必须讲清前提假设：<b>物理感官不是魔法，以下三种场景需要明确认知边界</b>。",
            "svg": svg_columns(
                [
                    {
                        "tag": "BOUNDARY 01 · 容器与远程",
                        "title": "单机视图 vs 跨机联邦",
                        "accent": C_TEAL,
                        "lines": [
                            "单实例 /proc 只看本宿主机命名空间",
                            "跨容器/跨服务器需接入 Mesh Peer",
                            ("解法：Phase 41/44 多体联邦协议", True),
                        ],
                    },
                    {
                        "tag": "BOUNDARY 02 · 向量依赖",
                        "title": "Ollama 离线降级策略",
                        "accent": C_GOLD,
                        "lines": [
                            "无本地 Ollama 时 L3 语义边暂停",
                            "L1 启发元数据与 FTS5 仍 100% 可用",
                            ("解法：优雅降级，绝不阻塞主流程", True),
                        ],
                    },
                    {
                        "tag": "BOUNDARY 03 · 极高频写入",
                        "title": "构建目录与大仓冷启动",
                        "accent": C_CLAY,
                        "lines": [
                            "未配置黑名单的超大构建目录会拖慢扫描",
                            "需通过 placement/policy 明确 Shallow Tier",
                            ("解法：白黑灰策略 + 断点状态机", True),
                        ],
                    },
                ],
                ("诚实标明边界，用户才能放心地把生产宿主机交给它守护。", "第一性原理 · 边界透明", "teal"),
            ),
            "next": "进入第 2 讲收束，顺便抛出引爆第 3 讲的关键悬念。",
            "script": (
                "我们从不把自己的产品吹成银弹。感官图谱有三条清晰边界：跨机器要走 Mesh 联邦；没装 Ollama 时会自动降级到 L1 启发加 FTS5；遇到新的巨型构建目录要纳入黑名单浅层阻断。把这些边界说透，落地才稳。"
            ),
        },
        {
            "eyebrow": "12 · 收束",
            "title": "先让 Agent 睁开眼睛，再谈一切智能协同",
            "lead": "没有物理感官图谱，大模型永远在黑暗中试错；有了它，<b>每一次动手前都站在客观事实之上</b>。",
            "svg": svg_closing(
                kicker="KEY TAKEAWAYS · 第 02 讲收束",
                headline="宿主机物理感官图谱：零幻觉 · 零噪音 · 零云端 Token 税",
                pillars=[
                    ("只读物理真值", "/proc 0.2ms 探测 + 写前脱敏 + 三端探针", "DQ4 100% · W1 -99.4%"),
                    ("敲门不进屋", "白黑灰分级扫描 + L1/L2/L3 语义管道", "DQ1 100% · DQ2 99.5%"),
                    ("全域拓扑连通", "L1/L2/L3 加权属性图 × CodeGraph 宏微联邦", "DQ3 100% · DQ6 100%"),
                ],
                open_question="开放思考：既然感官查一次只要 1ms，那 Agent 每次连本机 PostgreSQL 前都强制查一次，真的是最优解吗？",
                next_talk="下一讲预告 → 第 03 讲：小脑反射、痛觉门禁与自适应并发控制 AOPCC（03-cerebellum-and-aopcc.html）",
            ),
            "script": (
                "第 2 讲到这里收束。我们给 Agent 装上了一双零幻觉的眼睛。\n"
                "但最后抛一个反直觉的问题，也是第 3 讲的引子：就算查一次感官只要 1 毫秒，Agent 每次连本机 5432 端口的 PostgreSQL 前都强制停下来查一次 Agent Body，真的划算吗？\n"
                "下一讲，我们把数据库内核的悲观锁与乐观锁搬进 Agent 执行回路，彻底算清这笔账！"
            ),
        },
    ]
    render_deck(
        doc_title="第 02 讲 · 宿主机物理感官图谱与零污染扫盘 — Agent Body",
        brand_label="SERIES 02 / 06 · 物理感官",
        foot_label=f"AGENT BODY · 第 02 讲：宿主机物理感官图谱与零污染扫盘 (v{APP_VER})",
        slides_data=slides,
        out_path=OUT / "02-host-reality-graph.html",
    )


def build_talk_03() -> None:
    slides = [
        {
            "cover": True,
            "eyebrow": "封面 · 第 03 讲",
            "title": "小脑反射、痛觉门禁与自适应并发控制 AOPCC",
            "svg": svg_cover(
                kicker=f"SERIES 03 / 06 · AGENT BODY · v{APP_VER} · 30 MIN",
                title="小脑反射与自适应并发控制 AOPCC",
                en_sub="Cerebellar Reflexes, Pre-Hook Guardrails & Adaptive Concurrency",
                subtitle1="把数据库悲观锁与乐观锁，精准搬进 Agent 执行回路",
                subtitle2="消灭整轮对话税 · 乐观直通租约 · 毫秒级碰壁自愈 · WFG 死锁破局",
                spec_line="W6 27.70x · W15 -83.9% tok · 反射维度 99.7/100 · PR-11 实证",
                strata=[
                    ("核心痛点 · 整轮对话税 C_turn", "1ms 工具调用 ≠ 1ms 成本，背后是 5k~30k tok 重发", "#EBF1EE", C_INK),
                    ("三态分类器 · ConcurrencyClassifier", "OPTIMISTIC 直通 · CAUTIOUS_CAS 校验 · PESSIMISTIC_LOCK 强锁", "#CFE0DC", C_INK),
                    ("MVCC 乐观租约 · OptimisticLease", "SessionStart 签发 <=120 tok 租约，自信连接 :5432", C_TEAL_D, C_PAPER),
                    ("痛觉门禁 · PreToolUse 硬拦截", "端口防撞 · 目录层级锁 · L3 敏感路径红线", C_TEAL_2, C_PAPER),
                    ("碰壁秒级自愈 · FailureDiagnoser", "PostToolUse 捕获 ECONNREFUSED/EADDRINUSE 自动给处方", C_TEAL_3, C_PAPER),
                    ("多 Agent 协作锁 · WFG 死锁破局", "共享/排他/目录树锁 + Wait-For Graph 成环拦截 + 推送唤醒", "#A4C4C6", C_INK),
                ],
                right_tag="第 3 讲 · 30 分钟单点说透 · 地层剖面规范",
            ),
            "script": (
                "欢迎来到第 3 讲。这一讲的灵感直接来自数据库内核——悲观并发控制（PCC）与乐观并发控制（OCC）。\n"
                "30 分钟里，我们只把 Agent 本体的「小脑反射」讲透：为什么强制事前检查有时反而是负优化？什么时候该大胆乐观直通，什么时候必须悲观上锁？万一乐观猜错了碰壁，怎么在 2 毫秒内自愈？多 Agent 抢资源形成死锁又怎么破局？"
            ),
        },
        {
            "eyebrow": "01 · 反直觉发现",
            "title": "1ms 的事前查询，为什么让 Agent 付出 4 秒 + 2 万 Token 的代价？",
            "lead": "工具本身只跑 1ms，但它迫使大模型多走一整个推理回合——这就是隐形的<b>「整轮对话税 C_turn」</b>。",
            "svg": svg_pipeline(
                [
                    {"tag": "PCC 悲观路径", "title": "1. 模型决定先查端口", "accent": C_CLAY, "bg": C_CLAY_L, "lines": ["重发 15,000 tok 历史", "云端推理等待 2.5s", "输出 tool_call JSON"]},
                    {"tag": "LOCAL 1ms", "title": "2. Body 返回 5432 OK", "accent": C_TEAL, "lines": ["本地探测仅耗时 0.2ms", "但必须把结果塞回消息流"]},
                    {"tag": "EXTRA TURN", "title": "3. 模型再跑一轮执行 psql", "accent": C_CLAY, "bg": C_CLAY_L, "lines": ["再次重发 15,500 tok", "再次云端等待 2.5s", "才真正连上 PostgreSQL"]},
                    {"tag": "OCC 乐观直通", "title": "凭开工租约一步直连", "accent": C_GREEN, "bg": C_GREEN_L, "lines": ["Brief 已签发 5432 租约", "省去整整 1 轮 LLM 往返", "耗时减半 · Token 省 15k"]},
                ],
                [
                    {
                        "title": "核心公式：什么时候该乐观，什么时候该悲观？",
                        "accent": C_TEAL,
                        "bg": C_TEAL_L,
                        "lines": [
                            "悲观期望成本 = C_turn（整轮对话税，必然支付）",
                            "乐观期望成本 = P_fail × (C_rollback + C_diagnose)",
                            "当目标高置信、动作可逆、回滚代价极低（如 psql 连本机 5432）时：乐观完胜！",
                        ],
                    },
                    {
                        "title": "反之：破坏性写操作绝不允许赌乐观",
                        "accent": C_CLAY,
                        "bg": C_CLAY_L,
                        "lines": [
                            "服务端绑定端口、覆写共享配置、批量清理文件：P_fail × C_rollback 极大",
                            "此时必须由 PreToolUse 硬 Hook 在执行前拦截，零妥协执行 PESSIMISTIC_LOCK",
                        ],
                    },
                ],
                animated=True,
                mid="ah302",
            ),
            "anim_caps": [
                "① 悲观路径：为了查 1ms 事实，先付一轮推理税",
                "② 本地查询本身极快，瓶颈不在本体探测而在往返轮次",
                "③ 模型被迫再走一轮才真正干活",
                "④ 乐观路径：凭开工 MVCC 租约直接一步到位",
                "⑤ 数学判据：整轮对话税 vs 失败回滚期望代价",
            ],
            "next": "道理通了，怎么让代码自动精准分辨「连 5432 客户端」和「绑 8756 服务端」？",
            "script": (
                "这个洞察是我在做 PostgreSQL 验证时突然想明白的。\n"
                "我明明知道本机 5432 端口默认就跑着一个免密 PG 实例，结果如果规则死板地要求每次动手前先查一遍 Agent Body，虽然 0.2 毫秒就回答了，但大模型被迫多走一整轮对话，白白重发一两万 Token 上下文、干等三四秒。\n"
                "数据库几十年早就把这事算透了：读多冲突少、回滚成本低的，用乐观并发控制（OCC）；写冲突大、回滚昂贵的，用悲观并发控制（PCC）。我们把它做成自适应引擎，就叫 AOPCC。"
            ),
        },
        {
            "eyebrow": "02 · 三态分类引擎",
            "title": "AOPCC 三态决策矩阵：精确区分客户端连接与服务端绑定",
            "lead": "同样出现端口号，<b>`psql -p 5432` 是只读/客户端试探（乐观直通），`uvicorn --port 8756` 是服务端占坑（悲观门禁）</b>。",
            "svg": svg_columns(
                [
                    {
                        "tag": "MODE 01 · OPTIMISTIC",
                        "title": "乐观直通（零打断）",
                        "accent": C_GREEN,
                        "bg": C_GREEN_L,
                        "lines": [
                            "典型：psql -p 5432 / redis-cli",
                            "典型：curl 本地健康检查 / 只读查库",
                            "行为：Pre-Hook 直接放行，0 额外轮次",
                            ("兜底：真碰壁时 Post-Hook 毫秒自愈", True),
                        ],
                        "foot_lbl": "预期收益",
                        "foot_val": "省去 100% 事前查询对话税",
                    },
                    {
                        "tag": "MODE 02 · CAUTIOUS_CAS",
                        "title": "轻量条件校验",
                        "accent": C_GOLD,
                        "bg": C_GOLD_L,
                        "lines": [
                            "典型：编辑项目代码、写新脚本",
                            "行为：核验放置规范与软依赖提醒",
                            "无硬冲突直接放行，有风险注入警示",
                            ("特性：低摩擦 + 防路径漂移", True),
                        ],
                        "foot_lbl": "检查维度",
                        "foot_val": "Placement + 3 跳影响提示",
                    },
                    {
                        "tag": "MODE 03 · PESSIMISTIC_LOCK",
                        "title": "悲观硬门禁",
                        "accent": C_CLAY,
                        "bg": C_CLAY_L,
                        "lines": [
                            "典型：bind 监听端口、改共享配置",
                            "典型：多 Agent 同目录写入、危险命令",
                            "行为：强制查锁/查端口/查 L3 禁区",
                            ("底线：冲突即刻阻断，绝不事后擦屁股", True),
                        ],
                        "foot_lbl": "拦截通道",
                        "foot_val": "PreToolUse 硬 Hook",
                    },
                ],
                ("由 src/hooks/diagnoser.rs 中的 ConcurrencyPolicyClassifier 确定性判定，零 LLM 推理开销。", "src/hooks/diagnoser.rs", "teal"),
            ),
            "next": "乐观直通不是「盲目自信」，它手里握着一张开工时签发的 MVCC 短时租约。",
            "script": (
                "早期版本里我们只要在命令里看到端口号就一刀切拦下来，结果连 `psql -p 5432` 都被误伤。\n"
                "Phase 42 上线 `ConcurrencyPolicyClassifier` 之后，精确区分客户端连接命令和服务端监听绑定命令。客户端连高置信服务，直接放行；改代码走轻量 `CAUTIOUS_CAS`；一旦涉及起服务绑端口、抢锁或碰高危路径，立刻切到 `PESSIMISTIC_LOCK` 铁面无私。"
            ),
        },
        {
            "eyebrow": "03 · 开工租约",
            "title": "OptimisticLeaseEngine：开工简报附带 <=120 tok 乐观直通契约",
            "lead": "在 `SessionStart` 组装 `body_brief` 时，顺手把本机高置信服务的 <b>MVCC 租约</b> 塞进简报尾部。",
            "svg": svg_code_and_flow(
                "src/brief/optimistic.rs",
                "src/brief/optimistic.rs · assembler.rs",
                [
                    "pub struct OptimisticLease {",
                    "  pub resource_key: String,   // e.g. \"port:5432:postgres\"",
                    "  pub confidence: f64,        // 0.0 ~ 1.0 (历史成功滑窗)",
                    "  pub ttl_secs: u64,          // 短时有效期，防陈旧误判",
                    "  pub hint: String,           // \"本机 PG 5432 在线，可直连\"",
                    "}",
                    "",
                    "// 严控预算：租约区块总长 <= 120 tokens",
                    "// 闭环演进：",
                    "//   1. 碰壁一次 -> confidence 自动降级，切回悲观核验",
                    "//   2. 审计连续成功 -> audit::evolution 自动康复升权",
                ],
                "MVCC 乐观租约的生命周期",
                [
                    ("签发：零额外调用", "搭车 SessionStart 简报一次性下发，大模型开局就知道哪些本机服务可直接用", "Budget <= 120 tok"),
                    ("失效：碰壁即降级", "一旦 PostToolUse 捕获连接失败，租约置信度立刻下调，避免下个会话重复踩坑", "test_optimistic_miss_downgrades"),
                    ("康复：审计自愈升权", "当服务恢复正常且审计记录稳定成功，AuditEvolution 自动恢复租约直通资格", "test_optimistic_lease_rehabilitates"),
                ],
                animated=True,
            ),
            "anim_caps": [
                "① OptimisticLease 数据结构与 120 tok 预算硬上限",
                "② 签发阶段：搭车开工简报零额外轮次",
                "③ 降级阶段：碰壁自动降置信度防连环踩坑",
                "④ 康复阶段：双域审计确认稳定后自动恢复直通",
            ],
            "next": "那万一真碰到 1% 的意外——比如 5432 刚好重启了或者密码变了——怎么收场？",
            "script": (
                "租约机制最妙的地方在于它是活的，不是写死的静态配置。\n"
                "开工时简报里多花不到 120 个 Token，告诉大脑「本机 5432 PG 在线，置信度 0.98，可乐观直连」。如果今天刚好有人把 PG 停了，大脑直连碰壁，系统不仅马上给替代方案，还会自动把这条租约降级；等哪天 PG 恢复稳定运行，审计自进化引擎又会自动给它平反升权。"
            ),
        },
        {
            "eyebrow": "04 · 碰壁秒级自愈",
            "title": "OptimisticFailureDiagnoser：PostToolUse 毫秒级反查替代处方",
            "lead": "乐观猜错并不可怕，可怕的是大脑在黑暗里瞎重试。<b>Post-Hook 在失败瞬间反查宿主机，直接递上精确处方</b>。",
            "svg": svg_compare_rows(
                ("工具执行报错指纹", "裸奔大脑的盲目重试", "Agent Body Post-Hook 毫秒级诊断处方"),
                [
                    ("ECONNREFUSED (:5432)", "反复换 localhost/127.0.0.1 重试 3 轮", "反查 /proc 发现 PG 实际跑在 :5433，直接返回正确端口"),
                    ("password auth failed", "瞎猜默认密码或重写连接脚本", "检索服务通讯录提示环境变量名与脱敏认证方式"),
                    ("command not found", "立刻 apt/pip 盲装或放弃方案", "反查 capability 图谱给出本机等价工具及绝对路径"),
                    ("EADDRINUSE (:8756)", "手动试 8757/8758 碰运气", "指出占用 PID/服务名，并附带实测空闲推荐端口"),
                ],
                ("W15 实测：AOPCC 自适应控制节省 83.9% Token，加速 3.71x，质量从 52.0 升至 100.0。", "Bench v3.9 W15 PASS"),
            ),
            "next": "单 Agent 的快慢问题解决了；当三个 Agent 同时在一台机器上干活，怎么防止它们互相覆盖甚至死锁？",
            "script": (
                "我们为乐观直通兜了最硬的一张网：`OptimisticFailureDiagnoser`。\n"
                "只要命令返回非零退出码，`PostToolUse` 钩子立刻匹配四类高频错误指纹：连接拒绝、认证失败、命令不存在、端口被占。在 2 毫秒内反查宿主机最新物理真值，把「正确端口 / 替代工具 / 占用进程」直接注入到工具返回结果里。大模型下一轮一步就能修好，绝不原地打转。"
            ),
        },
        {
            "eyebrow": "05 · 硬 Hook 痛觉神经",
            "title": "PreToolUse + PostToolUse：确定性硬 Hook 与 MCP 的黄金分工",
            "lead": "靠提示词劝大模型「记得调工具」总会遗忘；<b>把防撞红线沉到客户端生命周期硬 Hook，100% 确定触发</b>。",
            "svg": svg_pipeline(
                [
                    {"tag": "SESSION START", "title": "开工自动注简报", "accent": C_TEAL_D, "lines": ["agent-body brief 自动运行", "注入宿主索引+乐观租约", "注入最近跨脑接力胶囊"]},
                    {"tag": "PRE TOOL USE", "title": "动手前痛觉拦截", "accent": C_CLAY, "bg": C_CLAY_L, "lines": ["校验端口占用与目录锁", "拦截 L3 危险路径/命令", "冲突即刻返回硬阻断"]},
                    {"tag": "EXECUTION", "title": "真实工具执行", "accent": C_TEAL_2, "lines": ["写入文件 / 运行命令", "高置信读动作零拦截", "主战心流丝滑无感"]},
                    {"tag": "POST TOOL USE", "title": "事后记账与自愈", "accent": C_GREEN, "bg": C_GREEN_L, "lines": ["成功：异步建档+写审计", "失败：毫秒注入诊断处方", "驱动 AOPCC 租约更新"]},
                ],
                [
                    {
                        "title": "一键生成多客户端硬 Hook 配置",
                        "accent": C_TEAL,
                        "bg": C_TEAL_L,
                        "lines": [
                            "支持 Claude Code / Cursor / Antigravity / Codex 一键生成 hooks.json",
                            "内建防循环与 fail-open 保护：Agent Body 异常时绝不卡死用户正常编码",
                            ("agent-body hooks generate --client antigravity --apply", True),
                        ],
                    },
                    {
                        "title": "为什么必须「硬 Hook + MCP」双轨？",
                        "accent": C_GREEN,
                        "bg": C_GREEN_L,
                        "lines": [
                            "硬 Hook 管确定性底线（开工 brief、写前防撞、写后建档）——不靠模型自觉",
                            "MCP 管高语义主动探索（搜资产、查爆炸半径、保存接力胶囊）——按需思考",
                        ],
                    },
                ],
                animated=True,
                mid="ah306",
            ),
            "anim_caps": [
                "① SessionStart：开工自动注入物理简报与租约",
                "② PreToolUse：写入与绑端口前的确定性痛觉门禁",
                "③ 执行期：合法操作丝滑通过，高置信直通无感",
                "④ PostToolUse：异步建档、审计与失败秒级处方",
                "⑤ 多客户端一键部署与双轨协同心法",
            ],
            "next": "现在把并发拉高：如果 Agent A 和 Agent B 同时抢两个目录或端口锁，怎么防死锁？",
            "script": (
                "这是我们从大量真实使用中总结出来的核心经验：不要把安全底线寄托在大模型的自觉性上。\n"
                "所以我们在 `~/.gemini/config/hooks.json` 里用硬 Hook 接管了三个确定性切面：SessionStart 自动注简报，PreToolUse 自动防撞，PostToolUse 自动建档与碰壁诊断。而深度检索、图谱影响分析这些需要高语义判断的，留给大模型主动调 MCP。"
            ),
        },
        {
            "eyebrow": "06 · 多 Agent 协作锁",
            "title": "共享/排他锁 + 目录树层级冲突传播",
            "lead": "锁住父目录 `/root/work/proj`，<b>任何子路径 `/root/work/proj/src/a.rs` 的排他写请求都会被自动感知与拦截</b>。",
            "svg": svg_columns(
                [
                    {
                        "tag": "SHARED (S 锁)",
                        "title": "多读并行不互斥",
                        "accent": C_TEAL,
                        "lines": [
                            "多个研究型 Agent 同时读同一仓库",
                            "支持同 owner 可重入续租 (TTL)",
                            "过期锁由 GC 与 L1 自愈自动回收",
                        ],
                    },
                    {
                        "tag": "EXCLUSIVE (X 锁)",
                        "title": "写操作严格排他",
                        "accent": C_CLAY,
                        "lines": [
                            "独占端口绑定或关键文件重构",
                            "阻止第二个 Agent 并发覆写",
                            "支持管理员显式 force_release",
                        ],
                    },
                    {
                        "tag": "HIERARCHY · 目录树传播",
                        "title": "父子路径自动冲突判定",
                        "accent": C_GREEN,
                        "lines": [
                            "路径规范化防 ../ 绕过",
                            "父目录 X 锁自动覆盖全部子文件",
                            "子文件持锁时拒绝父目录抢占 X 锁",
                        ],
                    },
                ],
                ("支持 SQLite 事务锁与 PostgreSQL Advisory Lock 跨连接强一致，8 项并发集成测试全绿。", "tests/coordination_integration.rs", "teal"),
            ),
            "next": "有了排他锁和等待队列，经典的 AB-BA 死锁就会出现——Agent Body 怎么在微秒内破局？",
            "script": (
                "多 Agent 协作最隐蔽的冲突是目录层级冲突：Agent A 锁了整个 `src/storage/` 目录在做重构，Agent B 去锁 `src/storage/spi.rs`，如果只做字符串相等比对，两个锁居然都拿到了，结果代码互相覆盖。\n"
                "我们的 `LockRegistry` 在归一化路径后做祖先/子孙树冲突传播，而且兼容 SQLite 和 PostgreSQL Advisory Lock，彻底堵死这个漏洞。"
            ),
        },
        {
            "eyebrow": "07 · WFG 死锁破局",
            "title": "Wait-For Graph 成环检测 + Wait-Queue 推送唤醒",
            "lead": "拒绝傻等超时干烧云端计费！<b>一旦等待图成环，立即精确选出牺牲者毫秒退避，解锁瞬间推送唤醒</b>。",
            "svg": svg_code_and_flow(
                "src/coordination/deadlock.rs",
                "src/coordination/deadlock.rs · tests/deadlock_wait_integration.rs",
                [
                    "// Agent A 持有 port:8080，等待 dir:/work/api",
                    "// Agent B 持有 dir:/work/api，请求 port:8080 (body_lock_wait)",
                    "",
                    "pub fn detect_cycle(wait_edges: &[(AgentId, AgentId)]) -> Option<Vec<AgentId>> {",
                    "  // DFS 三色标记实时检测 2-Agent / 3-Agent 及目录层级环路",
                    "  // 命中环路 -> 立即拒绝入队，返回结构化 DeadlockVictimAdvice",
                    "}",
                    "",
                    "// 无死锁时进入事件驱动等待队列 (EventBus 推送唤醒)：",
                    "// 锁一释放立即通知等待方，零轮询空转！",
                ],
                "W6 实测与集成测试验证",
                [
                    ("实时环路拦截", "覆盖 2-Agent 互锁、3-Agent 环形等待及目录层级引发的隐式环路", "test_three_agent_cycle_with_directory_hierarchy"),
                    ("事件推送零轮询", "依托内部 EventBus 在锁释放瞬间唤醒等待方，彻底告别 sleep 盲等", "test_wait_and_acquire_push_wakeup"),
                    ("W6 基准实证", "多 Agent 并发死锁破局节省 68.8% Token，加速比高达 27.70x，质量 100.0", "Bench v3.9 W6 PASS"),
                ],
                animated=True,
            ),
            "anim_caps": [
                "① Wait-For Graph DFS 三色成环检测源码逻辑",
                "② 支持 2/3 Agent 与目录层级隐式死锁拦截",
                "③ EventBus 事件推送唤醒告别轮询",
                "④ W6 实测 27.70x 加速比与质量满分",
            ],
            "next": "除了防撞与加锁，小脑还得管住 Agent 别把文件乱扔——8 大放置规则引擎怎么做到 83 微秒出建议？",
            "script": (
                "把数据库教科书里的 Wait-For Graph 搬到多 Agent 协作上，效果立竿见影。\n"
                "Agent A 占着端口等目录，Agent B 占着目录来等端口，以前两个 Agent 会一直傻等到超时，浪费几分钟。现在入队前做一次微秒级 DFS 成环检测，立刻把死锁环打出来退避，无环则挂进 EventBus 等待队列，前脚释放后脚瞬间唤醒。W6 实测加速 27.7 倍。"
            ),
        },
        {
            "eyebrow": "08 · 规约脊柱",
            "title": "83 μs 放置建议引擎 + 纠偏自演化学习",
            "lead": "新建脚本放哪、临时文件放哪、开源克隆放哪——<b>把主人的目录秩序编译成确定性规则，还能从纠偏中自己长出新规矩</b>。",
            "svg": svg_columns(
                [
                    {
                        "tag": "8 大目录规范 · 83 μs",
                        "title": "PlacementEngine v2",
                        "accent": C_TEAL_D,
                        "lines": [
                            "脚本 → /root/new/shared_proj/",
                            "技能 → /root/new/skills/",
                            "临时 → /root/new/tmp/",
                            "开源克隆 → /root/new/src/",
                            ("M3 微基准均值仅 83 μs (SLA 400)", True),
                        ],
                    },
                    {
                        "tag": "PROJECT OVERRIDE",
                        "title": "全局规约 + 项目覆盖",
                        "accent": C_TEAL_2,
                        "lines": [
                            "支持项目级配置与工作区动态覆盖",
                            "自动清洗非法文件名与路径穿越",
                            "按上下文动态过滤无关规约省 Token",
                            ("W4 实测质量 16.5 → 100.0 (+83.5)", True),
                        ],
                    },
                    {
                        "tag": "RULE LEARNER · 自演化",
                        "title": "PlacementLearner 聚类",
                        "accent": C_GREEN,
                        "lines": [
                            "记录用户每次手动挪动文件的纠偏",
                            "同模式纠偏聚类达阈值自动提炼新规",
                            "默认建议审核后应用，全程记入审计",
                            ("越用越懂主人习惯，告别反复叮嘱", True),
                        ],
                    },
                ],
                ("W4 8 类资产放置合规门禁：有效信息密度提升 4,519.8 倍，8/8 规则 100% 命中。", "body_placement_suggest / learn", "teal"),
            ),
            "next": "把整套小脑反射体系放到 Bench v3.9 里，看看硬指标表现如何。",
            "script": (
                "每个开发者都有自己的目录洁癖：临时脚本放 `tmp/`、复用工具放 `shared_proj/`、克隆源码放 `src/`。没有规则引擎时，Agent 随手就在根目录拉一堆 `test1.py`。\n"
                "我们的 `PlacementEngine` 只需 83 微秒就能给出合规落盘路径，而且如果你把某类文件多次挪到另一个目录，`PlacementLearner` 会聚类发现规律，自动提炼出一条新规则。"
            ),
        },
        {
            "eyebrow": "09 · 性能与实证",
            "title": "小脑反射与并发控制实证成绩单（Bench v3.9）",
            "lead": "反射弧必须比大脑思考快三个数量级——<b>所有微基准全部在百微秒级，所有负载全部帕累托双赢</b>。",
            "svg": svg_metrics_table(
                [
                    ("83 μs", "M3 放置规则决策均值", "P99 = 103 μs (SLA 400)"),
                    ("27.70x", "W6 死锁破局加速比", "Token -68.8% · 质量 100"),
                    ("3.71x", "W15 AOPCC 加速比", "Token -83.9% · 质量 100"),
                    ("99.7/100", "现网 24h 反射维度分", "HomeostasisVitals 实测"),
                ],
                ["评测项", "验证核心能力", "基线 → 赋能", "质量 / 延迟", "状态"],
                [
                    ["W4", "8 类资产放置合规门禁", "657 → 177 tok (-73.1%)", "16.5 → 100.0 (8/8)", "PASS"],
                    ["W6", "多 Agent 并发死锁破局", "400 → 125 tok (-68.8%)", "33.3 → 100.0 (27.7x)", "PASS"],
                    ["W15", "AOPCC 乐观直通与碰壁自愈", "174 → 28 tok (-83.9%)", "52.0 → 100.0 (3.71x)", "PASS"],
                    ["M3", "micro_placement_suggest (200x)", "P50=81μs · P95=100μs", "P99=103μs <= 400μs", "PASS"],
                    ["Int Tests", "coordination + deadlock 集成套件", "8 + 2 项并发与死锁测试", "10 / 10 passed", "PASS"],
                ],
                [18, 150, 560, 930, 1290],
                footnote="数据来源：Bench v3.9 run-6a0a4995 与 cargo test 实测",
            ),
            "next": "我们在终端里实操一遍锁、死锁检测与 AOPCC 钩子测试。",
            "script": (
                "看这组数字：M3 放置决策 P99 103 微秒，W6 死锁破局加速 27.7 倍，W15 AOPCC 省了 83.9% Token 还加速 3.71 倍，10 项并发与死锁集成测试全部一次通过。这就是小脑反射该有的反应速度。"
            ),
        },
        {
            "eyebrow": "10 · 实操命令清单",
            "title": "小脑反射与协作锁现场演练命令",
            "lead": "一套命令同时覆盖 <b>AOPCC 钩子模拟、自治包络裁决、协作锁申请与死锁排查</b>。",
            "svg": svg_code_and_flow(
                "小脑反射与协作锁 CLI 实操",
                f"agent-body v{APP_VER} CLI",
                [
                    "# 1. 模拟 PreToolUse 检查（区分客户端连 5432 vs 服务端绑端口）",
                    "agent-body hooks pre --tool run_command --command \"psql -p 5432\"",
                    "agent-body hooks pre --tool run_command --command \"uvicorn --port 8756\"",
                    "",
                    "# 2. 测试四级自治包络裁决（L0/L1/L2/L3）",
                    "agent-body body envelope --action write_file --target /etc/shadow",
                    "",
                    "# 3. 申请排他目录锁、查看锁表与死锁环路检测",
                    "agent-body lock acquire --resource /root/new/work/demo --mode exclusive",
                    "agent-body lock list",
                    "agent-body lock deadlock-check",
                    "",
                    "# 4. 83μs 获取合规文件放置建议",
                    "agent-body placement suggest --kind script --name pg_health.py",
                ],
                "等价 MCP 工具（大脑无缝协同）",
                [
                    ("body_hook_pre / body_hook_post", "硬 Hook 自动触发，同时开放为 MCP 工具供显式预演", "AOPCC 三态引擎"),
                    ("body_lock_acquire / wait / release", "共享/排他/目录树锁 + EventBus 唤醒 + WFG 检测", "tests/deadlock_wait_integration.rs"),
                    ("body_placement_suggest / learn", "8 大放置规则与纠偏自演化闭环", "M3 = 83 μs"),
                    ("body_state (action=envelope)", "L0~L3 自治边界实时裁决", "DQ8 L3 拦截 = true"),
                ],
            ),
            "next": "小脑反射解决了「当下这一步怎么又快又安全」，但长任务跨天、跨模型切换时，大脑还是会失忆。",
            "script": (
                "把这两条 `hooks pre` 对比着敲一遍特别有意思：敲 `psql -p 5432` 会告诉你乐观直通放行；敲 `uvicorn --port 8756` 会立刻触发悲观门禁并报出 8756 正被 `agent-body` 占用。这种毫秒级的精准区分，就是 AOPCC 的灵魂。"
            ),
        },
        {
            "eyebrow": "11 · 常见误区",
            "title": "并发控制落地中最容易踩的三个坑",
            "lead": "把数据库理论搬到 Agent 世界，<b>最怕生搬硬套而不看实际约束</b>。",
            "svg": svg_columns(
                [
                    {
                        "tag": "PITFALL 01 · 一刀切悲观",
                        "title": "把所有只读动作全拦下来",
                        "accent": C_CLAY,
                        "lines": [
                            "后果：每轮多烧 15k tok 对话税",
                            "体验：用户觉得工具反而拖慢节奏",
                            ("解药：AOPCC 乐观租约放行高置信读", True),
                        ],
                    },
                    {
                        "tag": "PITFALL 02 · 僵尸锁不回收",
                        "title": "Agent 崩溃后锁永远不放",
                        "accent": C_GOLD,
                        "lines": [
                            "后果：后续会话全部被永久阻塞",
                            "根因：缺少 TTL 与后台代谢回收",
                            ("解药：强制 TTL + Scheduler GC + L1 自愈", True),
                        ],
                    },
                    {
                        "tag": "PITFALL 03 · 锁粒度失真",
                        "title": "只按字符串相等判冲突",
                        "accent": C_TEAL,
                        "lines": [
                            "后果：父目录锁不住子文件写操作",
                            "根因：未做规范化路径与前缀树判定",
                            ("解药：LockRegistry 目录层级冲突传播", True),
                        ],
                    },
                ],
                ("真正的工程优雅，不是机制最复杂，而是在「零摩擦」与「零事故」之间找到精准平衡点。", "Phase 42 实战复盘", "teal"),
            ),
            "next": "收束第 3 讲，我们顺理成章走向第 4 讲——海马体、信息新鲜度与跨大脑 1ms 接力。",
            "script": (
                "这三个坑我们自己全踩过，也全都写进了集成测试里。特别是僵尸锁：AI 会话经常中途被用户 Ctrl+C 打断，如果锁没有 TTL 和后台自动回收，半小时后全机器卡死。现在所有锁强制带 TTL，且纳入五维稳态的代谢维度监控。"
            ),
        },
        {
            "eyebrow": "12 · 收束",
            "title": "快的地方乐观直通，险的地方悲观铁闸",
            "lead": "小脑反射让 Agent 拥有了接近人类老工程师的肌肉记忆：<b>熟路直接走，险路先探灯，撞墙秒换道</b>。",
            "svg": svg_closing(
                kicker="KEY TAKEAWAYS · 第 03 讲收束",
                headline="AOPCC 自适应并发控制：把数据库内核智慧注入 Agent 反射弧",
                pillars=[
                    ("算清对话税", "识别 1ms 工具背后的整轮 LLM 成本，按风险三态分流", "W15 -83.9% tok · 3.71x"),
                    ("租约 + 自愈", "开工签发 <=120 tok 租约，碰壁 2ms 反查处方并自动降级", "反射稳态 99.7/100"),
                    ("协同零死锁", "目录层级锁 + WFG 成环检测 + EventBus 推送唤醒", "W6 27.70x · 10 集成测试"),
                ],
                open_question="开放思考：当你在上午用 Claude Code 重构到一半，下午想换 Antigravity 继续，怎么让新大脑 1 毫秒无缝接盘且绝不泄露密钥？",
                next_talk="下一讲预告 → 第 04 讲：海马体、信息新鲜度治理与跨大脑 1ms 接力（04-hippocampus-freshness-continuity.html）",
            ),
            "script": (
                "总结第 3 讲就十二个字：快的地方乐观直通，险的地方悲观铁闸。\n"
                "下一讲，我们进入 Agent 本体的第三大器官——海马体，聊聊怎么治理最难缠的「陈旧信息与僵尸记忆」，以及 Phase 44 全新落地的「跨大脑 1 毫秒接力胶囊」。我们第 4 讲见！"
            ),
        },
    ]
    render_deck(
        doc_title="第 03 讲 · 小脑反射、痛觉门禁与自适应并发控制 AOPCC — Agent Body",
        brand_label="SERIES 03 / 06 · 小脑反射",
        foot_label=f"AGENT BODY · 第 03 讲：小脑反射、痛觉门禁与自适应并发控制 AOPCC (v{APP_VER})",
        slides_data=slides,
        out_path=OUT / "03-cerebellum-and-aopcc.html",
    )


if __name__ == "__main__":
    build_talk_01()
    build_talk_02()
    build_talk_03()
    print("✅ Built lectures 01~03 in presentation/")
