#!/usr/bin/env python3
"""Build presentation/presentation.html (17-slide Visual-First Executive Roadshow Deck).

Decks-as-Code generator using bespoke vector diagrams & iconic blueprints.
Grounded 100% in Agent Body v1.1.0 source code, live Benchmark v4.1,
and live AWR telemetry.
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
    vis_aopcc_highway,
    vis_awr_telemetry,
    vis_benchmark_pareto,
    vis_blind_agent_scene,
    vis_brain_body_anatomy,
    vis_comparative_scale,
    vis_competitor_arena,
    vis_cover_visual,
    vis_do_dont_wall,
    vis_handoff_capsule,
    vis_host_graph_federation,
    vis_microkernel_hub,
    vis_parity_console,
    vis_quality_gates,
    vis_security_dual_audit,
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
KERNEL_CNT = _COUNTS["kernel_crates"]
CONTRIB_CNT = _COUNTS["contrib_exts"]
DIST_CNT = _COUNTS["distributions"]
TEST_CNT = _COUNTS["verified_tests"]

ROOT = Path(__file__).resolve().parent.parent
PRES_DIR = ROOT / "presentation"


def build_roadshow_deck() -> None:
    slides = [
        # 01 视觉封面 · 脑体分离主插画
        {
            "cover": True,
            "eyebrow": "封面 · 路演总览",
            "title": "Agent Body · 脑体分离的宿主机具身底座",
            "svg": vis_cover_visual(
                kicker=f"AGENT BODY · EXECUTIVE ROADSHOW · v{APP_VER}",
                title="Agent 本体",
                en_sub="Brain-Body Separation · Embodied Host Substrate",
                tagline="瞬态大模型是可热切换的大脑 · 本系统是永驻宿主机的身体",
                badges=[
                    ("-99.23%", "Token 节省", C_TEAL),
                    ("98.0 分", "上下文质量", C_GREEN),
                    (f"{TOOL_CNT} MCP", f"{MOD_CNT} 模块 · {TEST_CNT} 测", C_CLAY),
                ],
                theme="roadshow",
                right_tag=f"图解蓝图 · 对外路演版 (17 页) · Bench v{BENCH_VER}",
            ),
            "script": (
                "各位好，今天用十五到二十分钟，向大家完整汇报一个从真实工程痛点里长出来的开源项目——Agent Body，中文我们叫它「Agent 本体」。\n"
                "先看封面右边这幅脑体分离解剖图：最上面是随时可热插拔的大模型大脑（Claude Code、Antigravity、Cursor、Cron）；中间是统一神经总线；下面永驻宿主机的六大生理器官，共同构成 Agent 贴地的「物理身体」。\n"
                f"在 Phase 70~75 我们完成了借鉴 PostgreSQL 插件体系的微内核重构并打通全栈可观测性：{KERNEL_CNT} 个纯机制微内核 crate、{CONTRIB_CNT} 个标准五件套周边扩展、外加 {DIST_CNT} 款零 C++/零 DuckDB 精简发行版，全工作区 {TEST_CNT} 项测试与门禁全绿，Benchmark v{BENCH_VER} 实现 Token 节省 99.23%，上下文质量升至 98.0 (+64.2)。"
            ),
        },
        # 02 比较优势 · 算力分工天平
        {
            "eyebrow": "01 · COMPARATIVE ADVANTAGE",
            "title": "比较优势：让昂贵大脑专攻创造，把物理杂务交给底座",
            "lead": "云端推理极贵且失忆 · <b>本地本体 0.2ms 确定性 0 Token</b>",
            "svg": vis_comparative_scale(animated=True),
            "anim_caps": [
                "Step 1/4：云端 LLM 大脑擅长高阶推理，但冷启动失忆、看不见宿主机物理态",
                "Step 2/4：物理探索延迟 3,000ms vs 0.2ms（快 15,000 倍），探索 Token 省 99.23%",
                "Step 3/4：Rust 单二进制永驻本机，零云端 Token 守候感官、防撞与连续状态",
                "Step 4/4：硅谷物业 → 决策幕僚 → 脑体分离，实现信息密度 94.5 倍跃升",
            ],
            "next": "当大脑没有这具身体时，每天在真实开发机上会发生什么？看翻车现场。",
            "script": (
                "这个项目的起心动念，来自经济学里最朴素的一个词：比较优势。\n"
                "硅谷创业者能一周沉浸写代码，是因为物业把吃住琐事全包了；主帅专攻拍板，是因为幕僚管好档案与避坑。\n"
                "转头看今天的 AI Agent：我们花着昂贵的 Token 请最聪明的大模型干活，结果它大量时间都在反复 ls、grep、查端口、猜路径。它缺的不是更高智商，而是一具永驻机器、0.2 毫秒看清环境并拦住低级失误的「身体」。"
            ),
        },
        # 03 痛点 · 蒙眼大脑五连翻车现场
        {
            "eyebrow": "02 · THE BLIND AGENT SCENE",
            "title": "PostgreSQL 调研惨案：蒙眼大脑的五连翻车现场",
            "lead": "50 个记忆项目只管「聊过什么」，<b>对物理现实视而不见</b>",
            "svg": vis_blind_agent_scene(animated=True),
            "anim_caps": [
                "Step 1/4：无本体的 LLM 就像戴着眼罩的大脑，只记得聊天记录，看不见宿主机",
                "Step 2/4：重复 git clone PG 源码爆磁盘，直接 bind 撞上已占用的 :5432 端口",
                "Step 3/4：误改共享脚本剪断上游 Cron 电缆，随手乱丢文件扫爆 60 万 Token 噪音",
                "Step 4/4：切换 IDE 或会话中断即 100% 失忆——Agent Body 专治这五大物理硬伤",
            ],
            "next": "为了彻底根治这五种翻车，我们把架构升维为「脑体分离」。",
            "script": (
                "看这幅真实开发机事故现场图：我经常让 Agent 调研 PostgreSQL 源码，以前每次开新会话，它不知道本地早就浅克隆好了源码、建好了 CodeGraph，上来又重 clone 编译浪费半小时。\n"
                "更致命的是另外四撞：撞端口、剪断上游 Cron 线、扫爆 node_modules 污染目录、换个客户端直接失忆。\n"
                "市面上几十个记忆项目都在卷「对话摘要」，却漏掉了宿主机物理世界这五件硬事。这正是 Agent Body 存在的根本理由。"
            ),
        },
        # 04 范式跃迁 · 脑体分离解剖图
        {
            "eyebrow": "03 · BRAIN-BODY ANATOMY",
            "title": "范式跃迁：可热插拔的瞬态大脑 × 永驻宿主的六大器官",
            "lead": f"Rust 单二进制 · <b>{MOD_CNT} 模块 · {TOOL_CNT} MCP · {SPI_CNT} SPI</b>",
            "svg": vis_brain_body_anatomy(animated=True),
            "anim_caps": [
                "Step 1/3：顶部 Claude / Antigravity / Cursor / Cron 均是可随时热插拔的无状态大脑",
                "Step 2/3：统一神经总线将 43 MCP、30 CLI、7 大 Hook 与 Web 指挥舱无缝桥接",
                "Step 3/3：永驻宿主机的六大生理器官：感官、小脑、海马体、脊柱、免疫与联邦",
            ],
            "next": "下面逐个解剖核心器官，先看第一器官：宿主机事实感官图谱。",
            "script": (
                "想明白痛点之后，我们确立了第一性原理：脑体分离。\n"
                "不论今天用 Antigravity、Claude Code、Cursor 还是凌晨 Cron，它们都是随时可拔插的大脑；而 Agent Body 通过统一神经总线常驻你的宿主机。\n"
                "机身内部严格划分为六大器官：感官图谱、小脑反射、海马体接力、规约脊柱、免疫代谢与多体联邦——全部收进一个零外部依赖的 Rust 单二进制。"
            ),
        },
        # 05 器官 1 · 宿主机三层证据图谱与 CodeGraph 显微镜
        {
            "eyebrow": "04 · ORGAN 1 · HOST REALITY GRAPH",
            "title": "感觉神经：L1 / L2 / L3 证据分层图谱 × CodeGraph 显微镜",
            "lead": "绝不把「语义相似」当「物理因果」 · <b>连通度 0.5% → 100%</b>",
            "svg": vis_host_graph_federation(animated=True),
            "anim_caps": [
                "Step 1/4：L1 物理硬依赖（实线粗边 w=0.95+）：Systemd → 活进程 PID → :5432 端口",
                "Step 2/4：L2 结构上卷（中虚线 w=0.72）：将 4,985 条文件引用聚合成宏观项目关系",
                "Step 3/4：L3 语义近邻（琥珀点线 Top-3）：仅作跨项目探索，严禁冒充硬因果",
                "Step 4/4：右侧 CodeGraph 显微镜直接穿透微观 AST 函数调用，3 跳推演仅 148 μs",
            ],
            "next": "看清了物理世界，但事事事前查询会不会太慢？看小脑反射 AOPCC 的破局。",
            "script": (
                "很多图谱方案把语义相似混成物理依赖，查爆炸半径全是误报。\n"
                "我们用三层线型彻底隔离证据强度：L1 实线是 Systemd 到进程到端口的物理硬依赖；L2 把近五千条文件引用上卷成项目拓扑，把 216 个工程的连通率从 0.5% 拉到 100%；L3 语义边严格限 Top-3 防污染。\n"
                "需要下钻到函数 AST 调用链时，右侧透镜直接桥接 CodeGraph，做到宏微联邦、各司其职。"
            ),
        },
        # 06 器官 2 · 小脑反射 AOPCC 高速 ETC 立交
        {
            "eyebrow": "05 · ORGAN 2 · CEREBELLUM & AOPCC",
            "title": "小脑反射：ETC 乐观直通 + 红灯悲观门禁，消灭整轮对话税",
            "lead": "W15 实测：<b>Token -83.9% · 质量 52 → 100 · 加速 3.22x</b>",
            "svg": vis_aopcc_highway(animated=True),
            "anim_caps": [
                "Step 1/4：事前全查虽仅 1ms，却迫使 LLM 多走一整轮推理（2~4s + 10k tok 对话税）",
                "Step 2/4：SessionStart 签发 <=120 tok MVCC 租约，由三态分类器实时分流",
                "Step 3/4：可逆客户端走 ETC 绿道 0 额外轮直飞；bind / 高危写走红灯铁闸 1ms 拦截",
                "Step 4/4：偶发碰壁时 PostToolUse <2ms 反查宿主吐出替代处方并自动降级旧租约",
            ],
            "next": "单次执行又快又稳，那切换大模型或会话中断时，如何做到「换脑不失忆」？",
            "script": (
                "这一页的灵感直接来自数据库并发控制。事前全查虽然本地只要 1 毫秒，但大模型为此多走一整轮对话要付 2 到 4 秒和上万 Token——这就是整轮对话税。\n"
                "所以我们把执行回路做成高速 ETC 立交：开工签发 120 Token 以内的 MVCC 租约，psql 直连和写 /tmp 走 ETC 绿道飞驰；bind 端口和高危写走红灯铁闸 1 毫秒拦死；万一绿道碰壁，底部应急环 2 毫秒反查宿主机塞回替代端口并自动降级租约。"
            ),
        },
        # 07 器官 3 · 海马体发光接力胶囊
        {
            "eyebrow": "06 · ORGAN 3 · HIPPOCAMPUS CAPSULE",
            "title": "海马体接力：50,000 tok 聊天山 vs 150 tok 发光胶囊",
            "lead": "DQ5 鲜活度 100% · <b>W17 接力省 92.7% Token · 10ms 唤醒</b>",
            "svg": vis_handoff_capsule(animated=True),
            "anim_caps": [
                "Step 1/2：传统换脑拖着 5 万 Token 历史噪音重扫 20 分钟；本体压缩为 150 tok 脱敏胶囊",
                "Step 2/2：上午 Claude 封存 → 底座保险箱 → 下午 Antigravity 10ms 续跑 → 凌晨 Cron 接棒",
            ],
            "next": "记忆保鲜且可接力，再看第四器官：规约脊柱如何一键粉碎 60 万 Token 上下文爆炸。",
            "script": (
                "传统记忆库既会发霉，又绑定在单一客户端壳里。我们不仅在底层用写穿失效把 589 项现网鲜活度做到 100%，更把跨大脑接力做成了一枚发光的 150 Token 胶囊。\n"
                "看上方体积对比：左边是五万 Token 的聊天垃圾山，右边是只含未竟目标、变更文件、避坑红线和脱敏锁印的微型胶囊。上午 Claude 写到一半，下午换 Antigravity 10 毫秒着陆直接续跑。"
            ),
        },
        # 08 器官 4 · 规约脊柱与 Context 防爆压缩柱状图
        {
            "eyebrow": "07 · ORGAN 4 · GOVERNANCE SPINE",
            "title": "规约脊柱：百级 SKILL 609k → 358 tok (-99.9%) 与 41 μs 放置",
            "lead": "三层渐进披露防爆路由 · <b>搬移纠偏自演化学习</b>",
            "svg": svg_bars(
                title_left=f"治理与防爆实测压缩率（Benchmark v{BENCH_VER} 真机数据）",
                rows=[
                    ("W7 百级 SKILL 防爆路由 (609k → 358t)", 99.9, "-99.9%", "密度 838x", C_TEAL_D),
                    ("W1 宿主工具语义发现 (21k → 119t)", 99.4, "-99.4%", "加速 565x", C_TEAL),
                    ("W5 三跳爆炸半径推演 (613 → 23t)", 96.2, "-96.2%", "M1 148 μs", C_TEAL_2),
                    ("W3 敲门不进屋阻断噪音 (534 文件)", 95.8, "-95.8%", "10/10 命中", C_GREEN),
                    ("W4 8 类资产放置合规门禁", 73.1, "-73.1%", "M3 41 μs", C_CLAY),
                ],
                footer_text="白名单深挖 · 黑名单敲门不进屋 · 用户手动搬移文件自动聚类沉淀新规约 (PlacementLearner)",
            ),
            "next": "能力越强，安全边界越必须铁板一块。看第五器官：纵深防御与双域审计。",
            "script": (
                "今天很多团队给 Agent 挂了一百多个 SKILL，一开工上下文就被塞掉几十万 Token。看图上第一根柱子：W7 场景下，Context 防爆路由按任务意图只披露最相关的微型索引，把 60.9 万 Token 压到 358，压缩率 99.9%，信息密度飙升 838 倍。\n"
                "同时 8 大资产放置引擎 41 微秒管住文件放哪儿，主人一搬移还能自动学会新习惯。"
            ),
        },
        # 09 器官 5 · 纵深安全盾牌与双域审计漏斗
        {
            "eyebrow": "08 · ORGAN 5 · SECURITY & DUAL AUDIT",
            "title": "免疫系统：四道纵深安全铁闸 × IN / OUT 双域审计黑匣子",
            "lead": "DQ4 凭据零落盘 100% · <b>DQ7 双域留痕 100% · 60s 折叠防洪</b>",
            "svg": vis_security_dual_audit(animated=True),
            "anim_caps": [
                "Step 1/2：左侧四道铁闸——全链路脱敏、抗提示词投毒、防命令注入、0600 最小权限",
                "Step 2/2：右侧双域监控——大脑调用 + 后台守护进程同步汇聚 60s 漏斗并盖 SHA-256 钢印",
            ],
            "next": "单机安全稳如磐石后，看本体自身如何像数据库内核一样内置 AWR 自我体检。",
            "script": (
                "把一台机器托付给 Agent 本体，安全与透明是底线。左边是四道安全铁闸：密钥零落盘、清洗外部仓库提示词投毒、POSIX 转义防注入、/etc/shadow 和 ~/.ssh 物理禁区 100% 拦停。\n"
                "右边是双域审计黑匣子：不只审大模型，连本体自己的后台巡检、GC 和自愈也 100% 经过 60 秒折叠防洪后盖上 SHA-256 指纹记账，彻底消灭黑盒自治。"
            ),
        },
        # 10 器官 6 · 内生数据库级 AWR 与 600% CPU 根治曲线
        {
            "eyebrow": "09 · ORGAN 6 · BUILT-IN AWR & TELEMETRY",
            "title": "内生代谢：把数据库级 AWR 装进本体，根治 600% CPU 尖峰",
            "lead": "6 类 18 等待事件 (&lt;100ns) · <b>Prometheus /metrics + W3C Trace</b>",
            "svg": vis_awr_telemetry(animated=True),
            "anim_caps": [
                "Step 1/2：左侧 6 类 18 等待事件无锁直方图 (<100ns) + /proc/self 零特权实时探针",
                "Step 2/2：右侧真实事故曲线——ADDM 秒级定位 600% CPU / 1.3G 尖峰并 L1 压降至 <2%",
            ],
            "next": f"架构图讲得再透，最终要拿真机实测说话。看 Benchmark v{BENCH_VER} 帕累托成绩单。",
            "script": (
                "作为 PostgreSQL 老兵，我们在生产排查过一次 DuckDB 并发把 CPU 打到 600%、内存冲到 1.3G 的事故后，决定把数据库最精华的 Wait Events、AWR 黄金基线和 ADDM 诊断原生写进本体。\n"
                "看右边这条真实压降曲线：不到 100 纳秒的无锁埋点精准定位锁与内存瓶颈，一键 L1 调优触发内存收缩与堆页归还，常驻压回 2% CPU 与 90MB 以内。"
            ),
        },
        # 11 实证一 · Benchmark v4.1 帕累托二维象限跃迁图
        {
            "eyebrow": f"10 · EMPIRICAL PROOF · BENCHMARK v{BENCH_VER}",
            "title": "实证成绩单：省 99.23% Token，上下文质量反升 +64.2 分",
            "lead": "帕累托双门禁：<b>严禁靠丢事实骗压缩率 · 全维门禁 PASS</b>",
            "svg": vis_benchmark_pareto(animated=True),
            "anim_caps": [
                "Step 1/3：二维帕累托象限——裸奔大脑落左下红区 (35.6 分)，本体跃迁右上黄金区 (98.0 分)",
                f"Step 2/3：三大核心仪表：Token 省 99.23%、质量飙升 +64.2 分、{WL_CNT}+{MICRO_CNT}+{DQ_CNT} 全维通过",
                "Step 3/3：内核微秒 SLA——无锁探针 7μs、放置 33μs、3 跳图 139μs、FTS5 259μs",
            ],
            "next": "跟裸奔的自己比完，再拉上四大开源热门赛道同机真跑对决！",
            "script": (
                "我们定下死规矩：严禁靠删信息刷压缩率。看左边这张二维帕累托象限图，右下角是「丢事实骗省钱」的红区，会被门禁直接熔断。\n"
                f"真机跑出来的 Benchmark v{BENCH_VER} 成绩单上，Agent Body 从左下角 35.6 分一路跃迁到右上角黄金区 98.0 分，同时把 66.25 万 Token 压到 5,114，有效信息密度暴涨 94.5 倍，全部内核微基准跑在微秒级；PG 18.6 百万事件流检索 P99 稳定在 4.42 ms。"
            ),
        },
        # 12 实证二 · 四大赛道同机 VS 擂台图
        {
            "eyebrow": "11 · COMPETITIVE ARENA · COMP-C1 ~ C4",
            "title": "四赛道同机横向对决：数量级代差一目了然，拒绝拉踩",
            "lead": "同机同数据真跑 (LIVE_BENCHMARKED) · <b>记忆生态 SPI 开放兼容</b>",
            "svg": vis_competitor_arena(animated=True),
            "anim_caps": [
                "Step 1/4：C1 长期记忆——对标裸流水 5/5 明文泄露，本体 0 泄露且省 97.8% Token",
                "Step 2/4：C2 上下文路由——OpenViking 144,730 tok vs 本体零副本 931 tok (-99.4%)",
                "Step 3/4：C3 文件治理——AgentFS 290 全量入库 vs 本体 10 净入库拦黑名单 (-96.5%)",
                "Step 4/4：C4 宿主探针——多子进程 89ms 0 跳 vs 本体 20ms 单进程 + 3 跳影响图",
            ],
            "next": "内核与跑分都硬核，开发者和决策者平时怎么直观地操控与演示它？",
            "script": (
                "四条红绿对比长条一眼看清差异：脱敏赛道 5/5 明文泄露 vs 0 泄露；路由赛道 14.4 万 Token vs 931 Token；文件治理 290 噪音全吞 vs 10 份净入库；宿主探针 89ms 盲扫 vs 20ms 带 3 跳上游影响图。\n"
                "同时态度极度开放：Mem0、Letta、PowerContext 通过 SPI 一行配置兼容拥抱。"
            ),
        },
        # 13 产品形态 · 43/43 黑白屏双屏同源与 7 幕剧场
        {
            "eyebrow": "12 · PRODUCT EXPERIENCE · 43/43 PARITY",
            "title": "产品形态：43/43 黑白屏双向镜像 × 7 幕真引擎路演剧场",
            "lead": "黑屏给大脑极速调用 · <b>白屏 Web 指挥舱实时治理与一键路演</b>",
            "svg": vis_parity_console(animated=True),
            "anim_caps": [
                "Step 1/3：左侧黑屏接口——43 MCP 工具、30 CLI 命令、7 大硬 Hook 与 3 套内置 SKILL",
                "Step 2/3：中央 /api/v1/parity/matrix 自动化门禁锁死 43/43 1:1:1 零漂移同源",
                "Step 3/3：右侧 Web 指挥舱 (:8765) 含 9 大工作台与 7 幕沙盒真引擎剧场",
            ],
            "next": "面向政务云与轻量服务器，我们如何借鉴 PostgreSQL 拆出微内核与四大发行版？",
            "script": (
                "看这幅左右镜像图：左边黑屏是给 AI 大脑的高速神经接口，右边白屏是单二进制内嵌的 Web 指挥舱，中间由 Parity 校验锁死 43/43 完全同源。\n"
                "网页上每点一个按钮，旁边同步回显等价 CLI 和 MCP 请求；内置的 7 幕路演剧场全是真实引擎隔离真跑，演完一键无痕复原。"
            ),
        },
        # 14 微内核与四大发行版 · 星型插件与积木塔
        {
            "eyebrow": "13 · POSTGRESQL-STYLE MICROKERNEL",
            "title": "微内核与周边分离：3 核 × 10 磁吸扩展 → 4 大发行版积木",
            "lead": f"Phase 70~75 · <b>Probe 6.7MB / DBA 6.4MB / Minimal 3.9MB (零 C++)</b>",
            "svg": vis_microkernel_hub(animated=True),
            "anim_caps": [
                f"Step 1/3：中心六边形纯机制微内核（{KERNEL_CNT} Crates · 8 Catalog · 6 Routine · 7 Hook）",
                f"Step 2/3：{CONTRIB_CNT} 个独立 Contrib 扩展（Linux/PG/JVM/mTLS/SIEM/OTel）磁吸插入 Hook 槽",
                "Step 3/3：右侧四座积木塔按需装配——服务器与政务云直接上 6.7MB 零 C++ 探针",
            ],
            "next": "微内核拆干净之后，看这套大仓如何靠确定性流水线门禁守住零腐烂。",
            "script": (
                f"Phase 70 到 75 我们借鉴 PostgreSQL 做了一场彻底的微内核手术：左边中心六边形只留最纯粹的机制，{CONTRIB_CNT} 个行业与运维能力全部做成外围磁吸 Contrib 扩展，受 arch-check 严防跨层污染。\n"
                "看右边四座积木高度对比：开发机继续用全功能旗舰版，上服务器和政务云直接用 6.7MB 的 probe 或 6.4MB 的 dba，零 libc++、零 DuckDB 运行时依赖。"
            ),
        },
        # 15 工程账单 · 七重确定性安检流水线
        {
            "eyebrow": "14 · ENGINEERING BILL & QUALITY GATES",
            "title": "工程账单：7 道确定性机器门禁 × 极致轻量单二进制",
            "lead": f"零生产 Panic · <b>{TEST_CNT}/{TEST_CNT} 全工作区测试全绿 · 位置无关交付</b>",
            "svg": vis_quality_gates(test_cnt=TEST_CNT, mod_cnt=MOD_CNT, tool_cnt=TOOL_CNT, animated=True),
            "anim_caps": [
                f"Step 1/2：顶部 7 道全自动质量安检门（零漂移/零污染/零 unwrap/零>1000L/{TEST_CNT} 测试/Playwright）",
                "Step 2/2：底部三项硬核运行账单——6.7MB 零 C++ 探针、100% 位置无关、2 轮零重扫定位",
            ],
            "next": "做底层架构，克制比堆功能更难。看我们的递弱代偿红线墙。",
            "script": (
                f"我们按数据库内核的纪律来打磨 AI 基础设施：上方 7 道机器门禁把零漂移、零内核污染、零 Panic、零超长文件、{TEST_CNT} 项全测和 Playwright 像素级防重叠检查全写进了自动化脚本。\n"
                "底部三个大数字是最终交付账单：6.7MB 极简探针、100% 搬移即跑的位置无关单二进制，以及 2 轮精准锁触点的零重扫演进体系。"
            ),
        },
        # 16 架构克制 · 递弱代偿边界城墙
        {
            "eyebrow": "15 · ARCHITECTURAL RESTRAINT",
            "title": "递弱代偿边界墙：把护城河挖深，坚决不越界抢大脑",
            "lead": "越基础的能力越被深度依赖 · <b>绝不膨胀成臃肿全栈框架</b>",
            "svg": vis_do_dont_wall(animated=True),
            "anim_caps": [
                "Step 1/3：左侧蓝绿堡垒——死磕物理感官、反射门禁、保鲜海马体、AWR 与轻量联邦",
                "Step 2/3：中央警戒红线——递弱代偿克制法则，严防底座沦为大泥球框架",
                "Step 3/3：右侧红叉禁区——不做主战大脑、不替代编排器、不造重型 Raft 与沙箱",
            ],
            "next": "最后，一页看清终局愿景与 5 分钟三步上手路径。",
            "script": (
                "看中间这道醒目的警戒红线：左边蓝绿堡垒是我们死磕到底的物理具身护城河；右边红叉禁区写进规约 #2 坚决不碰——不内置 LLM ReAct 循环、不替代 LangGraph、不造容器沙箱和 Raft。\n"
                "正因为边界极其纯粹，它才能零冲突地装进任何 AI 开发与运维体系。"
            ),
        },
        # 17 终局与 5 分钟三步上手
        {
            "eyebrow": "16 · VISION & 5-MIN QUICK START",
            "title": "终局：智能体时代的下一代 OS 原生具身底座",
            "lead": "今天是模块化微内核单二进制 · <b>明天是智能体 OS 原生标配</b>",
            "svg": svg_closing(
                kicker="NEXT-GEN OS EMBODIED SUBSTRATE FOR AI AGENTS",
                headline="让每个瞬态大脑，一着陆就拥有耳聪目明的物理身体",
                pillars=[
                    ("1. 编译自检 (1 分钟)", "cargo build --release -p agent-body-probe", "旗舰 / probe 6.7M / dba 6.4M"),
                    ("2. 接入大脑 (2 分钟)", "挂载 MCP + 自动生成 7 大 Hook", "Antigravity / Claude / Cursor"),
                    ("3. 真机验真 (2 分钟)", "agent-body bench run --suite full", "Web :8765 体验 7 幕剧场"),
                ],
                open_question="想深挖内核与源码？请移步「深度技术交流版 agent-body-deck.html (25 页)」或「6 讲专题 index.html」",
                next_talk="开源仓库：github.com/digoal/agent-body · 完整手册见 docs/manual/01~10",
            ),
            "script": (
                "过去五十年的操作系统是为人类设计的；在智能体时代，操作系统需要一层原生面向 AI 大脑的具身底座，管好感官、反射、连续性和代谢稳态。\n"
                "只需三步五分钟：编出旗舰版或 6.7MB 零依赖探针、挂载 MCP 与 Hooks、跑一遍 Benchmark 和 Web 指挥舱，就能亲眼看到省下 99% 探索 Token 且质量飙升的实测效果。感谢聆听！"
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
