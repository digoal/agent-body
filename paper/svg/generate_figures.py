#!/usr/bin/env python3
"""Deterministic academic SVG generator for Agent Body top-tier conference paper suite.

Enforces:
- Mandatory CJK font-family on root <svg>
- Light warm-paper academic palette (#FAF8F4), zero harsh black backgrounds
- Self-contained <defs> (arrow markers) injected in every file
- Strict coordinate bounds assertions (zero viewBox overflow, >=14px inner card padding)
- Zero italic CJK synthesis
"""

from __future__ import annotations
import html
from pathlib import Path

OUT_DIR = Path(__file__).resolve().parent
OUT_DIR.mkdir(parents=True, exist_ok=True)

FONT = "'Noto Sans CJK SC', 'PingFang SC', 'Microsoft YaHei', sans-serif"

PALETTE = {
    "bg": "#FAF8F4",
    "panel": "#F3EFE8",
    "card": "#FFFFFF",
    "line": "#D6D0C4",
    "ink": "#162A33",
    "ink2": "#425863",
    "muted": "#6B7F88",
    "teal_fill": "#E7F2F5",
    "teal_stroke": "#1D5B70",
    "teal_dark": "#134252",
    "green_fill": "#E6F4EC",
    "green_stroke": "#1F7053",
    "green_dark": "#15533C",
    "amber_fill": "#FDF2E0",
    "amber_stroke": "#B56917",
    "amber_dark": "#8A4E0D",
    "red_fill": "#FCEBE8",
    "red_stroke": "#B43C2C",
    "red_dark": "#89281B",
    "purple_fill": "#EFEBF8",
    "purple_stroke": "#5A418E",
    "purple_dark": "#3F2B69",
}


def esc(s: str) -> str:
    return html.escape(s, quote=True)


class SvgCanvas:
    def __init__(self, width: int, height: int, title: str, subtitle: str):
        self.w = width
        self.h = height
        self.elems: list[str] = []
        # Background & outer frame
        self.rect(0, 0, width, height, fill=PALETTE["bg"], stroke="none", rx=10)
        self.rect(10, 10, width - 20, height - 20, fill="none", stroke=PALETTE["line"], sw=1.5, rx=8)
        # Header bar
        self.rect(24, 22, width - 48, 58, fill=PALETTE["panel"], stroke=PALETTE["line"], sw=1, rx=6)
        self.text(42, 47, title, size=18, weight="700", fill=PALETTE["ink"])
        self.text(42, 68, subtitle, size=12, weight="400", fill=PALETTE["ink2"])

    def rect(
        self,
        x: float,
        y: float,
        w: float,
        h: float,
        fill: str = "#FFFFFF",
        stroke: str = "#D6D0C4",
        sw: float = 1.5,
        rx: float = 6,
        dash: str = "",
    ) -> None:
        assert x >= 0 and y >= 0 and x + w <= self.w and y + h <= self.h, (
            f"Rect out of viewBox: ({x},{y},{w},{h}) vs ({self.w},{self.h})"
        )
        dash_attr = f' stroke-dasharray="{dash}"' if dash else ""
        self.elems.append(
            f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" '
            f'fill="{fill}" stroke="{stroke}" stroke-width="{sw}"{dash_attr}/>'
        )

    def text(
        self,
        x: float,
        y: float,
        content: str,
        size: int = 13,
        weight: str = "400",
        fill: str = "#162A33",
        anchor: str = "start",
    ) -> None:
        assert 12 <= y <= self.h - 12, f"Text y={y} too close to viewBox H={self.h}: {content}"
        self.elems.append(
            f'<text x="{x}" y="{y}" font-size="{size}" font-weight="{weight}" '
            f'fill="{fill}" text-anchor="{anchor}">{esc(content)}</text>'
        )

    def card(
        self,
        x: float,
        y: float,
        w: float,
        h: float,
        header: str,
        lines: list[str],
        theme: str = "teal",
        header_size: int = 14,
        body_size: int = 12,
        line_gap: int = 20,
    ) -> None:
        fill = PALETTE[f"{theme}_fill"]
        stroke = PALETTE[f"{theme}_stroke"]
        dark = PALETTE[f"{theme}_dark"]
        self.rect(x, y, w, h, fill="#FFFFFF", stroke=stroke, sw=1.6, rx=7)
        self.rect(x, y, w, 30, fill=fill, stroke=stroke, sw=1.2, rx=7)
        self.text(x + 14, y + 20, header, size=header_size, weight="700", fill=dark)
        cy = y + 48
        for ln in lines:
            assert cy <= y + h - 12, f"Card text overflow in '{header}': cy={cy} > bottom={y+h-12}"
            self.text(x + 14, cy, ln, size=body_size, weight="400", fill=PALETTE["ink"])
            cy += line_gap

    def pill(self, x: float, y: float, w: float, h: float, label: str, theme: str = "teal", size: int = 12) -> None:
        fill = PALETTE[f"{theme}_fill"]
        stroke = PALETTE[f"{theme}_stroke"]
        dark = PALETTE[f"{theme}_dark"]
        self.rect(x, y, w, h, fill=fill, stroke=stroke, sw=1.3, rx=h / 2)
        self.text(x + w / 2, y + h / 2 + size * 0.36, label, size=size, weight="600", fill=dark, anchor="middle")

    def line(
        self,
        pts: list[tuple[float, float]],
        color: str = "#1D5B70",
        sw: float = 1.8,
        marker: str = "arr-teal",
        dash: str = "",
    ) -> None:
        d = "M " + " L ".join(f"{px},{py}" for px, py in pts)
        m_attr = f' marker-end="url(#{marker})"' if marker else ""
        d_attr = f' stroke-dasharray="{dash}"' if dash else ""
        self.elems.append(
            f'<path d="{d}" fill="none" stroke="{color}" stroke-width="{sw}"{d_attr}{m_attr}/>'
        )

    def circle(self, cx: float, cy: float, r: float, fill: str, stroke: str, sw: float = 1.8) -> None:
        self.elems.append(
            f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>'
        )

    def footer(self, note: str) -> None:
        self.text(28, self.h - 18, note, size=11, weight="400", fill=PALETTE["muted"])

    def save(self, filename: str) -> None:
        defs = """  <defs>
    <marker id="arr-teal" viewBox="0 0 10 10" refX="8.5" refY="5" markerWidth="6.5" markerHeight="6.5" orient="auto-start-reverse">
      <path d="M 0 1 L 10 5 L 0 9 z" fill="#1D5B70"/>
    </marker>
    <marker id="arr-green" viewBox="0 0 10 10" refX="8.5" refY="5" markerWidth="6.5" markerHeight="6.5" orient="auto-start-reverse">
      <path d="M 0 1 L 10 5 L 0 9 z" fill="#1F7053"/>
    </marker>
    <marker id="arr-amber" viewBox="0 0 10 10" refX="8.5" refY="5" markerWidth="6.5" markerHeight="6.5" orient="auto-start-reverse">
      <path d="M 0 1 L 10 5 L 0 9 z" fill="#B56917"/>
    </marker>
    <marker id="arr-red" viewBox="0 0 10 10" refX="8.5" refY="5" markerWidth="6.5" markerHeight="6.5" orient="auto-start-reverse">
      <path d="M 0 1 L 10 5 L 0 9 z" fill="#B43C2C"/>
    </marker>
    <marker id="arr-purple" viewBox="0 0 10 10" refX="8.5" refY="5" markerWidth="6.5" markerHeight="6.5" orient="auto-start-reverse">
      <path d="M 0 1 L 10 5 L 0 9 z" fill="#5A418E"/>
    </marker>
  </defs>"""
        svg = (
            f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {self.w} {self.h}" '
            f'width="{self.w}" height="{self.h}" font-family="{FONT}">\n'
            f"{defs}\n  " + "\n  ".join(self.elems) + "\n</svg>\n"
        )
        (OUT_DIR / filename).write_text(svg, encoding="utf-8")


# ---------------------------------------------------------------------------
# 1. 00-paper-portfolio-matrix.svg
# ---------------------------------------------------------------------------
def gen_00_portfolio_matrix() -> None:
    c = SvgCanvas(
        1120,
        640,
        "Agent Body 顶会论文矩阵全景图 (1 旗舰奠基篇 + 4 正交学科专论 = 共 5 篇)",
        "遵循 One Paper, One Sharp Thesis 学术准则：拒绝大杂烩工程报告，以单一系统底座支撑五大 CCF-A 顶级会议赛道",
    )
    # Center flagship card
    c.card(
        330,
        98,
        460,
        136,
        "Paper 01 · 奠基旗舰系统论文 [OSDI / SOSP / EuroSys]",
        [
            "核心命题: Brain-Body Separation (脑体分离具身底座)",
            "理论突破: AOPCC 整轮对话税期望代价不等式 + <2ms 碰壁自愈",
            "系统原语: BodyManifest · ContinuityCapsule v2 · L0~L3 包络",
            "实证支撑: W1~W22 全栈帕累托净升 + COMP-C1~C4 同机实跑对标",
        ],
        theme="teal",
    )

    # Four orthogonal pillars below
    specs = [
        (
            28,
            290,
            252,
            286,
            "Paper 02 · 数据库 [SIGMOD]",
            [
                "命题: 事务级海马体 (Scheme A→B)",
                "矛盾: 6 个数量级时间不对称性",
                "机制 1: 双时态 MVCC + 同键微锁",
                "机制 2: STC 空闲睡眠整理 (3.73x)",
                "机制 3: 零 BLOB 快权重三门禁",
                "机制 4: QueryPredictabilityGate",
                "PG 18.6: 1,000,003 行 P99 4.42ms",
                "故障拦截: 3 类退化 100% 原子中止",
            ],
            "green",
        ),
        (
            296,
            290,
            252,
            286,
            "Paper 03 · 检索与图谱 [SIGIR/KDD]",
            [
                "命题: 端侧弱向量补偿与宏微联邦",
                "矛盾: 0.8B 弱向量塌缩与假硬边",
                "机制 1: L1/L2/L3 证据分层图谱",
                "机制 2: 对数饱和上卷 + Top-3 剪枝",
                "机制 3: 0.8B 原文锚定三元组抽取",
                "机制 4: 8 维重归一化 Omni-RRF",
                "连通率: 孤立率 99.5% -> 0.5%",
                "离线检索: 0 向量 <20ms 100% 召回",
            ],
            "amber",
        ),
        (
            564,
            290,
            252,
            286,
            "Paper 04 · 软件工程 [ICSE/FSE]",
            [
                "命题: 微内核防腐与零重扫自进化",
                "矛盾: 递弱代偿膨胀 + 毛细血管漂移",
                "机制 1: 类 PG 8 张 body_cat_* 目录",
                "机制 2: 11 个标准五件套 contrib",
                "机制 3: 16 类一等公民 O(1) 路由",
                "机制 4: 运行时-工程态双飞轮闭环",
                "裁剪力: 3.9MB 纯内核 / 0 C++ 探针",
                "工程度: 78 阶段 435 测 0 Panic",
            ],
            "purple",
        ),
        (
            832,
            290,
            260,
            286,
            "Paper 05 · 安全与评测 [USENIX Sec]",
            [
                "命题: 具身纵深防御与防作弊基准",
                "矛盾: 密钥外泄/黑盒自治/丢事实刷分",
                "机制 1: L1~L5 脱敏与抗提示词投毒",
                "机制 2: IN/OUT 双域 60s 折叠审计",
                "机制 3: MeshEnvelope v1.0 零信任",
                "机制 4: 四维质量 Q 帕累托双门禁",
                "现网审计: 7d 9,431 条 0% 明文泄露",
                "可溯账本: PR-01~16 SHA-256 存证",
            ],
            "red",
        ),
    ]
    for x, y, w, h, hdr, lns, thm in specs:
        c.card(x, y, w, h, hdr, lns, theme=thm, header_size=13, body_size=11, line_gap=24)

    # Orthogonal fan-out lines from Paper 01 bottom center (560, 234)
    c.line([(560, 234), (560, 262), (154, 262), (154, 290)], color=PALETTE["green_stroke"], marker="arr-green")
    c.line([(560, 234), (560, 262), (422, 262), (422, 290)], color=PALETTE["amber_stroke"], marker="arr-amber")
    c.line([(560, 234), (560, 262), (690, 262), (690, 290)], color=PALETTE["purple_stroke"], marker="arr-purple")
    c.line([(560, 234), (560, 262), (962, 262), (962, 290)], color=PALETTE["red_stroke"], marker="arr-red")

    c.footer("数据溯源: Agent Body v1.1.0 (commit 641b71d) · 27 顶层模块 / 3 微内核 Crates / 11 Contrib 扩展 / 435 项工作区测试全绿")
    c.save("00-paper-portfolio-matrix.svg")


# ---------------------------------------------------------------------------
# 2. 00-publication-roadmap-flow.svg
# ---------------------------------------------------------------------------
def gen_00_roadmap_flow() -> None:
    c = SvgCanvas(
        1120,
        580,
        "顶会投稿梯队排期、实证复用与 Artifact Evaluation (AE) 交付流水线",
        "以统一可复现开源底座驱动分波次双盲投稿，确保每篇论文拥有独立理论纵深与零重叠核心贡献声明",
    )
    # Wave 1, Wave 2, Wave 3 columns
    c.card(
        32,
        104,
        332,
        205,
        "第一波次 · 核心架构与数据底座双响",
        [
            "1) Paper 01 -> OSDI / SOSP 主赛道",
            "   锚定脑体分离范式与 AOPCC 理论模型",
            "2) Paper 02 -> SIGMOD / VLDB 主赛道",
            "   锚定 PG 18 双时态海马体与快权重三门禁",
            "协同关系: Paper 01 建立全局系统抽象,",
            "Paper 02 深钻存储事务与百万事件流证明",
        ],
        theme="teal",
    )
    c.card(
        394,
        104,
        332,
        205,
        "第二波次 · 图谱检索与软件工程突破",
        [
            "3) Paper 03 -> SIGIR / KDD 主赛道",
            "   聚焦 L1~L3 证据分层图与 8 维 Omni-RRF",
            "4) Paper 04 -> ICSE / FSE 主赛道",
            "   聚焦类 PG 微内核与 O(1) 一等公民传导",
            "协同关系: 分别向 IR 社区与 SE 社区输出",
            "端侧零向量召回与 78 阶段自维护工程方法论",
        ],
        theme="purple",
    )
    c.card(
        756,
        104,
        332,
        205,
        "第三波次 · 纵深安全与评测标准收口",
        [
            "5) Paper 05 -> USENIX Security / CCS",
            "   聚焦五层具身安全、双域防洪存证审计、",
            "   MeshEnvelope v1.0 与防作弊帕累托基准",
            "生态价值: 将 Benchmark v4.1 (W1~W22)",
            "与 COMP-C1~C4 开源为社区公共评测套件,",
            "确立具身 Agent 安全与效能度量衡事实标准",
        ],
        theme="amber",
    )

    c.line([(364, 206), (394, 206)], color=PALETTE["teal_stroke"], marker="arr-teal")
    c.line([(726, 206), (756, 206)], color=PALETTE["purple_stroke"], marker="arr-purple")

    # Shared AE bar at bottom
    c.rect(32, 352, 1056, 178, fill="#FFFFFF", stroke=PALETTE["green_stroke"], sw=1.6, rx=8)
    c.rect(32, 352, 1056, 32, fill=PALETTE["green_fill"], stroke=PALETTE["green_stroke"], sw=1.2, rx=8)
    c.text(50, 373, "五大顶会共享 Artifact Evaluation (AE) 三枚徽章冲刺底座 (Available · Functional · Reproduced)", size=14, weight="700", fill=PALETTE["green_dark"])

    ae_boxes = [
        (52, 400, 240, 110, "确定性单二进制与容器", ["Rust Workspace 零 Panic", "Dockerfile --target probe", "一键复现全部图表脚本"], "green"),
        (310, 400, 240, 110, "三轨基准套件 v4.1", ["W1~W22 宏负载 + M1~M6", "DQ1~DQ12 + COMP-C1~C4", "SHA-256 存证签名防篡改"], "teal"),
        (568, 400, 240, 110, "真机 PG 18.6 隔离压测", ["TempPgDatabase RAII 沙盒", "1,000,003 行事件流自动注入", "3 类快权重故障注入回归"], "purple"),
        (826, 400, 242, 110, "双盲匿名审查脱敏包", ["剥离作者/域名/本地特定路径", "内置合成数据集与离线种子", "15 分钟审稿人快速核验脚本"], "amber"),
    ]
    for x, y, w, h, hdr, lns, thm in ae_boxes:
        c.card(x, y, w, h, hdr, lns, theme=thm, header_size=12, body_size=11, line_gap=18)

    c.line([(198, 309), (198, 352)], color=PALETTE["green_stroke"], marker="arr-green")
    c.line([(560, 309), (560, 352)], color=PALETTE["green_stroke"], marker="arr-green")
    c.line([(922, 309), (922, 352)], color=PALETTE["green_stroke"], marker="arr-green")

    c.footer("验收标准: 每篇论文独立具备形式化定理/代价模型 + >=4 个对比基线 + 完整消融实验 + 显式失效边界分析")
    c.save("00-publication-roadmap-flow.svg")


# ---------------------------------------------------------------------------
# 3. 01-brain-body-six-organs-arch.svg (Paper 01 Fig 1)
# ---------------------------------------------------------------------------
def gen_01_brain_body_arch() -> None:
    c = SvgCanvas(
        1120,
        650,
        "Paper 01 图 1 · 脑体分离架构：瞬态大脑皮层与永驻宿主机六器官本体",
        "严格坚守递弱代偿红线：本体零内置主战 ReAct 规划循环，以 <90MB 常驻内存与微秒级原语服务任意异构大模型客户端",
    )
    # Top: Ephemeral Cortices
    c.rect(28, 94, 1064, 78, fill=PALETTE["purple_fill"], stroke=PALETTE["purple_stroke"], sw=1.5, rx=8)
    c.text(46, 118, "瞬态大脑皮层 (Ephemeral LLM Cortices · 无状态 / 可随时热切换 / 零本体锁定)", size=14, weight="700", fill=PALETTE["purple_dark"])
    brains = [
        (46, 130, 230, 30, "Antigravity / Claude Code"),
        (296, 130, 230, 30, "Cursor / Codex / Aider"),
        (546, 130, 250, 30, "Hermes Cron / 批处理 Agent"),
        (816, 130, 254, 30, "本地轻量模型 / 自动化脚本"),
    ]
    for bx, by, bw, bh, bl in brains:
        c.pill(bx, by, bw, bh, bl, theme="purple", size=12)

    # Protocol bus
    c.rect(180, 194, 760, 32, fill=PALETTE["panel"], stroke=PALETTE["teal_stroke"], sw=1.4, rx=6)
    c.text(560, 215, "43 MCP Tools (支持 core 10 工具减税 78%) · 30 CLI · Pre/Post Hooks · Web REST/SSE (:8765)", size=12, weight="600", fill=PALETTE["teal_dark"], anchor="middle")
    c.line([(560, 172), (560, 194)], color=PALETTE["purple_stroke"], marker="arr-purple")
    c.line([(560, 226), (560, 248)], color=PALETTE["teal_stroke"], marker="arr-teal")

    # Middle: Agent Body Substrate (6 organs in 2x3 grid)
    c.rect(28, 248, 1064, 274, fill="#FFFFFF", stroke=PALETTE["teal_stroke"], sw=1.8, rx=8)
    c.text(46, 272, "Agent 本体 (Agent Embodiment Substrate · Rust 单二进制 · <15ms 冷简报 · 六大生理器官)", size=14, weight="700", fill=PALETTE["teal_dark"])

    organs = [
        (46, 286, 330, 106, "1. 感官与本体感觉 (Proprioception)", ["PATH 能力图谱 · /proc 进程与活跃端口", "L1/L2/L3 加权事实图 · Omni-RRF 检索", "桥接 CodeGraph 穿透 AST 符号调用链"], "teal"),
        (395, 286, 330, 106, "2. 小脑与条件反射 (Cerebellum)", ["AOPCC 开工乐观直通租约 (P_valid>=0.85)", "Pre-Hook 悲观防撞 · 路径与敏感红线拦截", "Post-Hook <2ms 碰壁反查注入自愈处方"], "green"),
        (744, 286, 330, 106, "3. 海马体与连续性 (Hippocampus)", ["SQLite FTS5 + PG 18 双时态事务记忆", "<=300 tok 脱敏抗投毒 ContinuityCapsule v2", "同基座热挂快权重 / 异构基座回退文本胶囊"], "purple"),
        (46, 402, 330, 106, "4. 规约脊柱与包络 (Normative Spine)", ["8 大工作区规范放置推演 (~103 us P50)", "L0 只读 / L1 自主稳态 / L2 监督 / L3 禁区", "全链路 SecretRedactor + 抗提示词投毒"], "amber"),
        (395, 402, 330, 106, "5. 代谢免疫与稳态 (Homeostasis)", ["IN_AGENT / OUT_OF_AGENT 双域防洪审计", "18 Wait Events (<100ns) 内生 AWR/ADDM", "五维生命体征雷达 + Shadow A/B H1~H5 自愈"], "red"),
        (744, 402, 330, 106, "6. 社会协同与网格 (Social & Mesh)", ["父子目录层级互斥锁 + WFG 死锁环检测", "MeshEnvelope v1.0 (HMAC + ±300s + Nonce)", "<=10MB 0600 隔离收件箱与多体名片发现"], "teal"),
    ]
    for ox, oy, ow, oh, ohdr, olns, othm in organs:
        c.card(ox, oy, ow, oh, ohdr, olns, theme=othm, header_size=12, body_size=11, line_gap=18)

    # Bottom: Physical Host OS
    c.line([(560, 522), (560, 548)], color=PALETTE["teal_stroke"], marker="arr-teal")
    c.rect(28, 548, 1064, 56, fill=PALETTE["panel"], stroke=PALETTE["ink2"], sw=1.4, rx=8)
    c.text(560, 573, "物理宿主机内核与外部存储底座 (Linux /proc · inotify · Systemd · Cron · Socket · SQLite WAL · PostgreSQL 18 · DuckDB)", size=13, weight="700", fill=PALETTE["ink"], anchor="middle")
    c.text(560, 593, "零幻觉物理真值采集 (L0_OBSERVE)  ×  可逆稳态自愈与受控监督代谢 (L1_AUTONOMIC / L2_SUPERVISED)", size=11, weight="400", fill=PALETTE["ink2"], anchor="middle")

    c.footer("Paper 01 核心架构图 · 对应源码模块: src/body/, src/brief/, src/hooks/, src/fact/, src/coordination/, src/audit/")
    c.save("01-brain-body-six-organs-arch.svg")


# ---------------------------------------------------------------------------
# 4. 01-aopcc-cost-boundary-and-recovery.svg (Paper 01 Fig 2)
# ---------------------------------------------------------------------------
def gen_01_aopcc_diagram() -> None:
    c = SvgCanvas(
        1120,
        620,
        "Paper 01 图 2 · AOPCC 自适应并发期望代价边界与 <2ms 碰壁自愈回路",
        "消除 LLM Agent 每步前置探查引发的整轮对话税 C_turn，在低失败代价区间实现零额外轮次直通执行",
    )
    # Left panel: Cost boundary conceptual plot
    c.rect(28, 96, 510, 476, fill="#FFFFFF", stroke=PALETTE["teal_stroke"], sw=1.5, rx=8)
    c.text(48, 124, "A. 期望对话轮次代价相图 (p_valid × C_fail)", size=14, weight="700", fill=PALETTE["teal_dark"])
    c.text(48, 145, "切换条件: (1 - p_valid)(C_fail + C_rec) < C_turn", size=12, weight="600", fill=PALETTE["ink2"])

    # Plot box
    px, py, pw, ph = 86, 170, 416, 300
    c.rect(px, py, pw, ph, fill=PALETTE["bg"], stroke=PALETTE["line"], sw=1.2, rx=4)
    # Regions inside plot
    # Top band: high C_fail -> PESSIMISTIC_LOCK
    c.rect(px + 4, py + 4, pw - 8, 96, fill=PALETTE["red_fill"], stroke=PALETTE["red_stroke"], sw=1.2, rx=4)
    c.text(px + 204, py + 38, "PESSIMISTIC_LOCK (悲观强门禁区)", size=13, weight="700", fill=PALETTE["red_dark"], anchor="middle")
    c.text(px + 204, py + 60, "端口 bind / 写共享锁 / 高爆炸半径文件 / 敏感路径", size=11, fill=PALETTE["ink"], anchor="middle")
    c.text(px + 204, py + 80, "C_fail >> C_turn : 强制 PreToolUse 物理核验", size=11, weight="600", fill=PALETTE["red_dark"], anchor="middle")

    # Middle-left: low p_valid -> CAUTIOUS_CAS
    c.rect(px + 4, py + 108, 176, 188, fill=PALETTE["amber_fill"], stroke=PALETTE["amber_stroke"], sw=1.2, rx=4)
    c.text(px + 92, py + 156, "CAUTIOUS_CAS", size=13, weight="700", fill=PALETTE["amber_dark"], anchor="middle")
    c.text(px + 92, py + 180, "(条件静默校验区)", size=12, weight="600", fill=PALETTE["amber_dark"], anchor="middle")
    c.text(px + 92, py + 208, "p_valid < 0.85 或", size=11, fill=PALETTE["ink"], anchor="middle")
    c.text(px + 92, py + 228, "近期碰壁已降级租约", size=11, fill=PALETTE["ink"], anchor="middle")
    c.text(px + 92, py + 252, "本地 ~103us 规约校验", size=11, weight="600", fill=PALETTE["amber_dark"], anchor="middle")

    # Bottom-right: high p_valid & low C_fail -> OPTIMISTIC
    c.rect(px + 188, py + 108, 224, 188, fill=PALETTE["green_fill"], stroke=PALETTE["green_stroke"], sw=1.5, rx=4)
    c.text(px + 300, py + 156, "OPTIMISTIC (乐观直通区)", size=13, weight="700", fill=PALETTE["green_dark"], anchor="middle")
    c.text(px + 300, py + 180, "p_valid >= 0.85, C_fail ~ 0", size=11, weight="600", fill=PALETTE["green_dark"], anchor="middle")
    c.text(px + 300, py + 208, "客户端直连 :5432 / :11434", size=11, fill=PALETTE["ink"], anchor="middle")
    c.text(px + 300, py + 228, "已就绪 CLI / tmp 沙盒直写", size=11, fill=PALETTE["ink"], anchor="middle")
    c.text(px + 300, py + 254, "省去 100% 前置查询轮次!", size=11, weight="700", fill=PALETTE["green_dark"], anchor="middle")

    # Axes labels
    c.line([(px, py + ph), (px + pw - 16, py + ph)], color=PALETTE["ink"], sw=1.6, marker="arr-teal")
    c.line([(px, py + ph), (px, py + 16)], color=PALETTE["ink"], sw=1.6, marker="arr-teal")
    c.text(px + pw / 2, py + ph + 24, "宿主机事实先验稳定概率 p_valid (0.0  --->  0.85  --->  1.0)", size=12, weight="600", fill=PALETTE["ink"], anchor="middle")
    c.text(48, py + ph + 50, "W15 实测: 总轮次 20 -> 12 轮 (节省 40% 对话税, 加速 3.49x)", size=11, weight="600", fill=PALETTE["teal_dark"])

    # Right panel: Closed-loop execution & <2ms recovery
    c.rect(558, 96, 534, 476, fill="#FFFFFF", stroke=PALETTE["green_stroke"], sw=1.5, rx=8)
    c.text(578, 124, "B. 开工租约签发、碰壁秒级自愈与审计滑窗校准闭环", size=14, weight="700", fill=PALETTE["green_dark"])

    c.card(582, 142, 486, 84, "Step 1 · SessionStart (body_brief <= 120 tok 租约块)", ["混入 tx_epoch 与活跃端口/工具节点计算 8 位 epoch_hash", "签发高置信度 OptimisticLease (如 lease-cli-toolchain p=0.96)"], theme="teal", header_size=12, body_size=11, line_gap=18)
    c.card(582, 242, 486, 70, "Step 2 · 乐观直通执行主动作 (0 前置 MCP 查询)", ["90%+ 稳态命中直接完成任务，消除 5k~30k 上下文重发预填充延迟"], theme="green", header_size=12, body_size=11, line_gap=18)
    c.card(582, 328, 486, 106, "Step 3 · 乐观碰壁 PostToolUse(success=false) <2ms 处方自愈", ["OptimisticFailureDiagnoser 覆盖 5 类确定性故障模式:", "ECONNREFUSED / 认证失败 / exit 127 缺命令 / EADDRINUSE / MVCC_EPOCH_CONFLICT", "实时反查 /proc + CapabilityEngine 注入替代端口/二进制处方"], theme="amber", header_size=12, body_size=11, line_gap=18)
    c.card(582, 450, 486, 96, "Step 4 · 双域审计滑窗经验校准 (AuditEvolutionAnalyzer)", ["统计 24h 窗口命中率 p_valid_hat；连续 2 次碰壁自动压低置信度 <0.70", "撤回乐观租约切回 CAUTIOUS/PCC；待守护巡检确认恢复后平滑康复"], theme="purple", header_size=12, body_size=11, line_gap=18)

    c.line([(825, 226), (825, 242)], color=PALETTE["teal_stroke"], marker="arr-teal")
    c.line([(825, 312), (825, 328)], color=PALETTE["amber_stroke"], marker="arr-amber")
    c.line([(825, 434), (825, 450)], color=PALETTE["purple_stroke"], marker="arr-purple")

    c.footer("数据与源码出处: src/brief/optimistic.rs · src/hooks/diagnoser.rs · src/audit/evolution.rs · Benchmark W15 & PR-11")
    c.save("01-aopcc-cost-boundary-and-recovery.svg")


# ---------------------------------------------------------------------------
# 5. 02-temporal-asymmetry-and-dual-path.svg (Paper 02 Fig 1)
# ---------------------------------------------------------------------------
def gen_02_temporal_asymmetry() -> None:
    c = SvgCanvas(
        1120,
        640,
        "Paper 02 图 1 · 六数量级时间不对称性与 PostgreSQL 事务级海马体 (Scheme A → B)",
        "严禁将 2PL 悲观锁横跨在秒级 LLM 推理与快权重训练上：以 MVCC 逻辑纪元 tx_epoch 解耦亚毫秒热路径与异步冷整理",
    )
    # Top timeline bar: 6 orders of magnitude
    c.rect(28, 94, 1064, 112, fill="#FFFFFF", stroke=PALETTE["amber_stroke"], sw=1.5, rx=8)
    c.text(46, 118, "六个数量级物理延迟鸿沟 (10^1 us  --->  10^7 us) —— 多智能体共享记忆的核心系统矛盾", size=14, weight="700", fill=PALETTE["amber_dark"])

    tiers = [
        (46, 134, 240, 56, "10^1 ~ 10^2 us (宿主机微秒反射)", "放置 103us · 3跳图 244us · FTS 879us", "green"),
        (306, 134, 240, 56, "10^3 ~ 10^4 us (PG 事务提交/检索)", "MVCC 写穿 1~2ms · 百万行检索 2.66ms", "teal"),
        (566, 134, 248, 56, "10^6 ~ 10^7 us (LLM 多步推理)", "单轮 Prefill+Decode 1.7s ~ 10s", "amber"),
        (834, 134, 240, 56, "10^7 ~ 4.5x10^7 us (神经内化)", "快权重/LoRA 合并与门禁评测 10~45s", "red"),
    ]
    for tx, ty, tw, th, t1, t2, thm in tiers:
        c.rect(tx, ty, tw, th, fill=PALETTE[f"{thm}_fill"], stroke=PALETTE[f"{thm}_stroke"], sw=1.3, rx=6)
        c.text(tx + 12, ty + 23, t1, size=12, weight="700", fill=PALETTE[f"{thm}_dark"])
        c.text(tx + 12, ty + 43, t2, size=11, fill=PALETTE["ink"])

    # Hot path vs Cold path
    c.card(
        28,
        228,
        516,
        356,
        "热路径 (Hot Path · 亚毫秒~毫秒级无阻塞契约 · Scheme A M1)",
        [
            "1. 同键微序列化: pg_advisory_xact_lock(hash(body|scope|fact_key))",
            "   不同 fact_key 100% 并发；消除 16 线程热点 SQLSTATE 40001 惊群",
            "2. 双时态不可变追加表 ext_mem_pg_tx__events:",
            "   截断旧行 valid_range [t0, now()) + 置 stale；新行赋单调 tx_epoch",
            "3. LSM 解耦检索段 ext_mem_pg_tx__segments:",
            "   旧段同步置 is_deleted=TRUE，删改后僵尸向量召回率严格 == 0%",
            "4. 读路径 REPEATABLE READ READ ONLY 快照读:",
            "   WITH active_e AS MATERIALIZED 强制局部索引，零锁无阻塞读取",
            "5. 网络断连兜底: PG 不可达自动无损回退嵌入式 SQLite 并记审计",
        ],
        theme="teal",
        line_gap=32,
    )

    c.card(
        576,
        228,
        516,
        356,
        "冷路径 (Cold Path · L1 空闲窗口与旁路快权重控制面 · M2 & M3)",
        [
            "1. STC 突触显著性评分 (SalienceScorer):",
            "   融合惊喜度、跨会话复用频次与修正权重，分层高价值决策 vs 噪声",
            "2. SleepTimeCompactor + QueryPredictabilityGate (M2):",
            "   空闲窗口压缩 3.73x (事实保留 100%)；高熵探索查询自动旁路直查原表",
            "3. 旁路 Model Runtime 控制面 (M3 · 零权重 BLOB 入 PG 堆表):",
            "   JSON-RPC 2.0 调度外部 TTT/Atlas/Larimar/SCoL，拦截 data: 内联载荷",
            "4. 三道硬门禁单事务原子切换 (fast_weight_snapshots):",
            "   norm_ratio 合规 ∧ eval_glue_delta >= -0.03 ∧ unlearn_leak < 0.001",
            "5. selective_unlearn 墓碑擦除 + 按 tx_epoch 24ms 秒级回滚",
        ],
        theme="purple",
        line_gap=32,
    )

    c.line([(544, 406), (576, 406)], color=PALETTE["purple_stroke"], sw=2.0, marker="arr-purple")

    c.footer("源码锚点: contrib/mem_pg_tx/src/{engine,sleep_compactor,predictability,fast_weight,fast_weight_ops}.rs · kernel/body-sdk/src/fast_weight.rs")
    c.save("02-temporal-asymmetry-and-dual-path.svg")


# ---------------------------------------------------------------------------
# 6. 02-pg18-million-events-and-gates.svg (Paper 02 Fig 2)
# ---------------------------------------------------------------------------
def gen_02_pg18_results() -> None:
    c = SvgCanvas(
        1120,
        620,
        "Paper 02 图 2 · PostgreSQL 18.6 真机百万事件流延迟实测与三门禁故障拦截验证",
        "实测环境: Linux x86_64 (8 Cores) · PostgreSQL 18.6 · 真实注入 1,000,003 条双时态历史事件与 3 类神经退化故障注入",
    )
    # Left card: Latency bar chart (log/scaled visual bars with exact ms labels)
    c.rect(28, 96, 530, 470, fill="#FFFFFF", stroke=PALETTE["teal_stroke"], sw=1.5, rx=8)
    c.text(48, 124, "A. 1,000,003 行事件规模下核心操作延迟与 SLA 对比", size=14, weight="700", fill=PALETTE["teal_dark"])
    c.text(48, 145, "局部唯一索引 + WITH active_e AS MATERIALIZED 消除全表 Merge Join", size=11, fill=PALETTE["ink2"])

    metrics = [
        ("active_snap 查询 (P99)", 0.75, 10.0, "0.75 ms  (SLA < 10 ms)", "green"),
        ("WITH RECURSIVE 血缘 (P99)", 1.94, 5.0, "1.94 ms  (SLA < 5 ms)", "teal"),
        ("1M 行 search_tx (P50)", 2.66, 20.0, "2.66 ms  (SLA < 20 ms)", "green"),
        ("1M 行 search_tx (P99)", 4.42, 20.0, "4.42 ms  (SLA < 20 ms)", "teal"),
        ("200 行基线 search_tx (P99)", 4.34, 20.0, "4.34 ms  (O(1) 规模无关!)", "purple"),
        ("M2 整理期热路径 (P99)", 10.65, 20.0, "10.65 ms (后台整理零阻塞)", "amber"),
        ("按 tx_epoch 原子回滚", 24.0, 1000.0, "24.00 ms (SLA < 1000 ms)", "green"),
    ]
    by = 172
    for label, val, sla, note, thm in metrics:
        c.text(48, by + 13, label, size=12, weight="600", fill=PALETTE["ink"])
        c.text(534, by + 13, note, size=11, weight="700", fill=PALETTE[f"{thm}_dark"], anchor="end")
        ratio = min(0.92, max(0.06, val / sla))
        bar_max = 486
        c.rect(48, by + 20, bar_max, 11, fill=PALETTE["panel"], stroke=PALETTE["line"], sw=1, rx=4)
        c.rect(48, by + 20, round(bar_max * ratio, 1), 11, fill=PALETTE[f"{thm}_stroke"], stroke="none", rx=4)
        by += 50

    c.text(48, 542, "并发写压测 (M1): 16 线程 × 5 热点事务 = 80 tx -> 丢写率 0.0%, 40001 重试 0 次", size=11, weight="700", fill=PALETTE["green_dark"])

    # Right card: Three Hard Gates State Machine & Fault Injection Matrix
    c.rect(578, 96, 514, 470, fill="#FFFFFF", stroke=PALETTE["purple_stroke"], sw=1.5, rx=8)
    c.text(598, 124, "B. 方案 B 快权重三道硬门禁与故障注入实测 (100% 拦截)", size=14, weight="700", fill=PALETTE["purple_dark"])

    c.pill(604, 146, 136, 34, "warming 快照", theme="amber", size=12)
    c.pill(768, 146, 146, 34, "三门禁联合校验", theme="purple", size=12)
    c.pill(942, 146, 126, 34, "active 激活", theme="green", size=12)
    c.line([(740, 163), (768, 163)], color=PALETTE["amber_stroke"], marker="arr-amber")
    c.line([(914, 163), (942, 163)], color=PALETTE["green_stroke"], marker="arr-green")

    gates = [
        (
            598,
            204,
            474,
            96,
            "门禁 1 · 范数防雪崩门禁 (Norm Explosion Gate)",
            [
                "判据: norm_ratio <= max_norm_ratio (默认阈值上限保护参数稳定性)",
                "故障注入: norm_ratio = 2.40 -> 状态置 aborted, 原 snap1 保持 active",
            ],
            "red",
        ),
        (
            598,
            314,
            474,
            96,
            "门禁 2 · 通用能力防灾难遗忘门禁 (General Capability Gate)",
            [
                "判据: eval_glue_delta >= -0.03 (连续神经合并通用基准衰减 < 3%)",
                "故障注入: eval_glue_delta = -0.08 -> 100% 阻断切换并无损回退方案 A",
            ],
            "amber",
        ),
        (
            598,
            424,
            474,
            122,
            "门禁 3 · 合规定向遗忘防泄露门禁 (Selective Unlearning Gate)",
            [
                "判据: unlearn_leak_rate < 0.001 (<0.1%) ∧ 严拒 data: 内联权重 URI",
                "故障注入: unlearn_leak_rate = 0.02 -> 阻断激活; 检索层墓碑前置过滤",
                "确保擦除作业排队与执行全周期 ComplianceToken 僵尸召回率 == 0%",
            ],
            "purple",
        ),
    ]
    for gx, gy, gw, gh, ghdr, glns, gthm in gates:
        c.card(gx, gy, gw, gh, ghdr, glns, theme=gthm, header_size=12, body_size=11, line_gap=20)

    c.footer("实测复现入口: cargo test --test pg_tx_memory_integration && cargo test --test sleep_compaction_integration && cargo test --test fast_weight_m3_integration")
    c.save("02-pg18-million-events-and-gates.svg")


# ---------------------------------------------------------------------------
# 7. 03-evidence-stratified-graph-federation.svg (Paper 03 Fig 1)
# ---------------------------------------------------------------------------
def gen_03_graph_federation() -> None:
    c = SvgCanvas(
        1120,
        620,
        "Paper 03 图 1 · L1/L2/L3 证据分层有向加权图谱与 CodeGraph 宏微代码联邦",
        "严禁将隐式向量近邻伪装成物理硬依赖：分层赋权 + 跳数阻尼衰减 (gamma=0.80) + 宏观文件/服务拓扑穿透微观 AST 符号图",
    )
    # Left 3 tiers stacked
    c.card(
        28,
        98,
        610,
        142,
        "Tier L1 · L1_EXPLICIT 物理显式硬依赖层 (w in [0.85, 1.00])",
        [
            "证据源: Systemd ExecStart · Cron 目标脚本 · /proc Socket · AST import",
            "工程语义: 确定性物理硬边 (listens_on / scheduled_by)，改动直接触发故障",
            "治理保证: 爆炸半径默认强纳入；FreshnessReconciler 实时剪枝失效节点",
        ],
        theme="red",
        header_size=13,
        body_size=11,
        line_gap=22,
    )
    c.card(
        28,
        258,
        610,
        148,
        "Tier L2 · L2_STRUCTURAL & KNOWHOW 结构与因果层 (0.55~0.88)",
        [
            "结构上卷: 5,116 条文件引用对数饱和上卷 w_L2 = min(0.84, 0.55+0.08*ln(1+N))",
            "同体桥接: runs_as 消解 Systemd 与 /proc 分裂；PMI>1.2 术语共现 (Top-5)",
            "锚定经验: 0.8B 小模型抽取 causes/mitigates 三元组 (>=8 字符原文核验)",
        ],
        theme="teal",
        header_size=13,
        body_size=11,
        line_gap=22,
    )
    c.card(
        28,
        424,
        610,
        142,
        "Tier L3 · L3_SEMANTIC 稀疏语义亲和层 (0.35~0.65, Top-K<=3)",
        [
            "混合打分: S = 0.75*cos(u,v) + 0.25*J(T_u,T_v) (门槛 cos>=0.78, S>=0.62)",
            "防毛线球: 每节点限 Top-3 隐式边，术语总度数 <=32；爆炸半径默认隔离 L3",
            "现网成效: 宿主机工程节点孤立率由 99.5% 压降至 0.5% (连通率 221/222)",
        ],
        theme="purple",
        header_size=13,
        body_size=11,
        line_gap=22,
    )

    # Right: Macro-Micro CodeGraph Federation & Blast Radius
    c.rect(666, 98, 426, 468, fill="#FFFFFF", stroke=PALETTE["green_stroke"], sw=1.6, rx=8)
    c.text(686, 126, "宏微两级联邦推演 (Macro-Micro Federation)", size=14, weight="700", fill=PALETTE["green_dark"])
    c.text(686, 148, "PathStrength(P) = prod( w_e_i * 0.80^(i-1) )", size=12, weight="600", fill=PALETTE["ink2"])

    c.card(688, 168, 382, 114, "宏观层 (Agent Body Host Graph)", ["跨介质拓扑: Cron -> Python/Rust 脚本", "-> 监听端口 :8765 -> Systemd 守护单元", "微基准 M1: 100节点 3跳遍历 P50 = 244 us"], theme="teal", header_size=12, body_size=11, line_gap=20)
    c.card(688, 308, 382, 114, "联邦桥接器 (CodeGraphBridge)", ["按需穿透仓库 .codegraph/ 符号级索引", "定位修改函数 StorageProvider::upsert_asset", "追踪跨模块动态派发调用链 (Callers/Callees)"], theme="green", header_size=12, body_size=11, line_gap=20)
    c.card(688, 446, 382, 102, "端到端爆炸半径输出 (Impact Report)", ["精确合并: 微观受影响 Rust 函数签名", "+ 宏观受波及 Systemd 服务与定时任务"], theme="amber", header_size=12, body_size=11, line_gap=20)

    c.line([(638, 169), (666, 169)], color=PALETTE["red_stroke"], marker="arr-red")
    c.line([(638, 332), (666, 332)], color=PALETTE["teal_stroke"], marker="arr-teal")
    c.line([(879, 282), (879, 308)], color=PALETTE["green_stroke"], marker="arr-green")
    c.line([(879, 422), (879, 446)], color=PALETTE["amber_stroke"], marker="arr-amber")

    c.footer("源码与实测依据: src/fact/property_graph.rs · src/links/{impact,codegraph}.rs · Benchmark W5, W10, W13, M1 & DQ6")
    c.save("03-evidence-stratified-graph-federation.svg")


# ---------------------------------------------------------------------------
# 8. 03-omni-rrf-eight-channel-pipeline.svg (Paper 03 Fig 2)
# ---------------------------------------------------------------------------
def gen_03_omni_rrf() -> None:
    c = SvgCanvas(
        1120,
        630,
        "Paper 03 图 2 · 自适应权重重归一化 8 维全信号融合检索管线 (Omni-RRF)",
        "Score(d) = [ sum_{c in C_active} (w_tilde_c / (k_c + rank_c(d))) ] * Phi_scalar(d) —— 向量离线零塌缩，端到端 <20ms",
    )
    # Left: Query Preprocessor
    c.card(
        28,
        180,
        210,
        240,
        "查询解析与归一入口",
        [
            "1. 驼峰/蛇形标识符拆词:",
            "   ShadowAbEngine ->",
            "   Shadow + Ab + Engine",
            "2. 行业术语别名归一:",
            "   PG/Postgres -> postgresql",
            "3. 字符 2/3-gram 切片",
            "4. 探测向量通道在线态",
            "   动态激活 C_active",
        ],
        theme="teal",
        header_size=13,
        body_size=11,
        line_gap=22,
    )

    # Middle: 6 Retrieval Rankers + 2 Scalar Modulators (= 8 Signal Dimensions)
    channels = [
        (276, 98, 450, 56, "Ch1 · BM25 + FTS5 全文加权 (w=1.20, <3ms)", "标题/路径 3.0x · 章节/KV 2.0x · 正文 1.0x", "teal"),
        (276, 164, 450, 56, "Ch2 · Pure-Rust Fuzzy 模糊容错 (w=0.75, <4ms)", "Bigram/Trigram Jaccard + 有界 Levenshtein (<=2) 纠拼写误差", "green"),
        (276, 230, 450, 56, "Ch3 · 可选本地语义向量 Cosine (w=1.00, 可离线摘除)", "Ollama 离线时自动从 C_active 移除并重归一化其余通道权重", "purple"),
        (276, 296, 450, 56, "Ch4 · Know-How 属性图 1~2 跳扩散 (w=1.10, <6ms)", "沿 causes/mitigates/co_occurs 扩散 (gamma=0.65)，输出推理路径", "amber"),
        (276, 362, 450, 56, "Ch5 · Hierarchy 章节与目录局部性 (w=0.60, <2ms)", "HeadingChunker 父/兄面包屑回溯，消除 600~1000 字切片断章取义", "teal"),
        (276, 428, 450, 56, "Ch6 · Doc-to-Doc 显式互链传递 (w=0.65, <2ms)", "Markdown 引用 / documented_by 双向联动长文 MD <-> 架构图 SVG", "green"),
        (276, 496, 450, 72, "Ch7 & Ch8 · 事实标量硬过滤 + 软调制因子 Phi_scalar(d) (<1ms)", "Ch7 硬过滤: status='active' & modality/domain/kv_filter 零僵尸准入", "red"),
    ]
    for cx, cy, cw, ch, t1, t2, thm in channels:
        c.rect(cx, cy, cw, ch, fill="#FFFFFF", stroke=PALETTE[f"{thm}_stroke"], sw=1.4, rx=6)
        c.rect(cx, cy, 8, ch, fill=PALETTE[f"{thm}_stroke"], stroke="none", rx=3)
        c.text(cx + 20, cy + 22, t1, size=12, weight="700", fill=PALETTE[f"{thm}_dark"])
        c.text(cx + 20, cy + 43, t2, size=11, fill=PALETTE["ink"])
    c.text(296, 556, "Ch8 软调制: clip(0.75 + 0.25*imp + 0.12*exp(-dt/30d) + auth - stale, 0.4, 1.4)", size=10, weight="600", fill=PALETTE["red_dark"])

    # Right: Fusion & Grounded Output
    c.card(
        764,
        156,
        328,
        310,
        "Omni-RRF 融合与可解释回执",
        [
            "权重重归一化:",
            "w_tilde_c = w_c / sum_{j in active} w_j",
            "平滑常数 k_c = 60",
            "------------------------------------",
            "结构化可解释输出:",
            "- matched_channels (命中信号源)",
            "- matched_sections (含祖先面包屑)",
            "- reasoning_paths (<=40 tok 因果链)",
            "------------------------------------",
            "W19 离线零向量实测:",
            "跨词因果 + 拼写错误 ShadwAbEngine",
            "4/4 事实 100% 命中, 密度提升 2580x",
        ],
        theme="purple",
        header_size=13,
        body_size=11,
        line_gap=20,
    )

    # Connections
    for cy_mid in (126, 192, 258, 324, 390, 456, 532):
        c.line([(238, 300), (256, 300), (256, cy_mid), (276, cy_mid)], color=PALETTE["teal_stroke"], sw=1.3, marker="arr-teal")
        c.line([(726, cy_mid), (746, cy_mid), (746, 310), (764, 310)], color=PALETTE["purple_stroke"], sw=1.3, marker="arr-purple")

    c.footer("源码锚点: src/scanner/asset_search.rs · src/fact/{knowledge_graph,knowhow_extractor}.rs · Benchmark W19 & DQ10")
    c.save("03-omni-rrf-eight-channel-pipeline.svg")


# ---------------------------------------------------------------------------
# 9. 04-pg-inspired-microkernel-isolation.svg (Paper 04 Fig 1)
# ---------------------------------------------------------------------------
def gen_04_microkernel() -> None:
    c = SvgCanvas(
        1120,
        630,
        "Paper 04 图 1 · 借鉴 PostgreSQL 的纯机制微内核、11 标准五件套与 4 发行版裁剪架构",
        "以编译期单向依赖 + ScopedSqlExecutor 前缀沙盒 + 静态业务词清零门禁，终结 AI 自维护软件的递弱代偿膨胀",
    )
    # Top: 4 Distributions
    c.rect(28, 94, 1064, 96, fill=PALETTE["panel"], stroke=PALETTE["teal_stroke"], sw=1.4, rx=8)
    c.text(46, 118, "按需装配四大发行版谱系 (Distributions · 同一代码库零分叉构建)", size=13, weight="700", fill=PALETTE["teal_dark"])
    dists = [
        (46, 130, 248, 48, "personal-flagship (agent-body)", "全功能单二进制 · 43 MCP / Web / DuckDB", "teal"),
        (308, 130, 248, 48, "aiops-probe (~6.7 MB)", "政务云探针 · 纯 Rust 0 C++ / mTLS / SIEM", "green"),
        (570, 130, 248, 48, "dba-sentinel (~6.4 MB)", "DBA 哨兵 · PG/达梦/金仓诊断 + OBS 自检", "amber"),
        (832, 130, 242, 48, "kernel-minimal (~3.9 MB)", "0 contrib 纯微内核 · CI 防腐基线底座", "purple"),
    ]
    for dx, dy, dw, dh, d1, d2, thm in dists:
        c.rect(dx, dy, dw, dh, fill="#FFFFFF", stroke=PALETTE[f"{thm}_stroke"], sw=1.4, rx=6)
        c.text(dx + 10, dy + 20, d1, size=12, weight="700", fill=PALETTE[f"{thm}_dark"])
        c.text(dx + 10, dy + 38, d2, size=10, fill=PALETTE["ink2"])

    # Middle: contrib/ 11 standard 5-piece extensions
    c.rect(28, 212, 1064, 166, fill="#FFFFFF", stroke=PALETTE["green_stroke"], sw=1.6, rx=8)
    c.text(46, 236, "周边扩展层 contrib/ (11 个物理隔离标准五件套: Cargo.toml + .control.toml + sql/ext_<id>__* + body_module_magic! + regress.rs)", size=13, weight="700", fill=PALETTE["green_dark"])

    exts = [
        "1. idx_fts_fuzzy", "2. gov_placement", "3. probe_linux_ops", "4. probe_jvm_middleware",
        "5. probe_db_engines", "6. hook_auth_mtls_rbac", "7. hook_gov_compliance", "8. hook_audit_siem",
        "9. fdw_cmdb_ticket (轨B)", "10. obs_telemetry", "11. mem_pg_tx (PG海马体)", "严禁兄弟 contrib 互调!",
    ]
    for idx, name in enumerate(exts):
        row = idx // 4
        col = idx % 4
        ex = 46 + col * 258
        ey = 250 + row * 38
        thm = "red" if idx == 11 else ("purple" if idx >= 9 else "green")
        c.pill(ex, ey, 244, 30, name, theme=thm, size=11)

    # Unidirectional dependency arrow down to kernel
    c.line([(560, 378), (560, 404)], color=PALETTE["red_stroke"], sw=2.2, marker="arr-red")
    c.text(574, 396, "唯一单向依赖: contrib -> body-sdk (arch-check 拦截逆向引用与业务词)", size=11, weight="700", fill=PALETTE["red_dark"])

    # Bottom: Pure Mechanism Microkernel (kernel/ 3 crates)
    c.rect(28, 406, 1064, 172, fill=PALETTE["teal_fill"], stroke=PALETTE["teal_stroke"], sw=1.8, rx=8)
    c.text(46, 430, "纯机制微内核底座 kernel/ (3 Crates · 0 业务/观测协议词污染 · 0 C++ 依赖)", size=14, weight="700", fill=PALETTE["teal_dark"])

    kcrates = [
        (46, 444, 330, 120, "kernel/body-sdk (SDK v2.0 稳定契约)", ["6 大 Routine: Type / Operator / TableAM /", "  IndexAM / FdwRoutine / Proc & BgWorker", "7 大 Hook: ClientAuth / ObjectAccess /", "  Planner / ExecStart/End / Redact / Audit"], "teal"),
        (395, 444, 330, 120, "kernel/body-kernel (目录驱动内核)", ["8 张系统目录表 body_cat_{extension,type,", "  opclass,am,fdw,proc,hook,bgworker}", "5 张通用基表 + ScopedSqlExecutor 沙盒", "ObservabilityProvider 只读遥测注册表"], "green"),
        (744, 444, 330, 120, "kernel/body-protocol (轨 B 进程外协议)", ["JSON-RPC 2.0 over stdio / Unix Socket", "FdwRoutine 超时熔断与最小权限隔离", "支撑外部动态语言/企业 CMDB 安全接入", "崩溃零连带影响主内核进程"], "purple"),
    ]
    for kx, ky, kw, kh, khdr, klns, kthm in kcrates:
        c.card(kx, ky, kw, kh, khdr, klns, theme=kthm, header_size=12, body_size=11, line_gap=18)

    c.footer("验证入口: make arch-check (5 项架构防腐规则 0 违规) · cargo build --release -p agent-body-kernel-minimal -p agent-body-probe -p agent-body-dba")
    c.save("04-pg-inspired-microkernel-isolation.svg")


# ---------------------------------------------------------------------------
# 10. 04-dual-flywheel-and-citizen-routing.svg (Paper 04 Fig 2)
# ---------------------------------------------------------------------------
def gen_04_dual_flywheel() -> None:
    c = SvgCanvas(
        1120,
        620,
        "Paper 04 图 2 · 四圈层 16 类一等公民 O(1) 变更传导与双引擎自进化飞轮",
        "解决 Coding Agent 跨 78 个演进阶段的上下文爆炸与契约静默漂移：动态 SSoT + <100ms 增量路由省 90%+ 重扫 Token",
    )
    # Left: 4 Rings of 16 First-Class Citizens
    c.rect(28, 96, 540, 474, fill="#FFFFFF", stroke=PALETTE["purple_stroke"], sw=1.6, rx=8)
    c.text(48, 124, "A. 四圈层 16 类一等公民平权模型 (C01 ~ C16)", size=14, weight="700", fill=PALETTE["purple_dark"])

    rings = [
        (48, 142, 500, 96, "Ring 1 · 内核与契约源头 (C01 ~ C06)", ["C01 Rustdoc/内联注释 · C02 19套集成/E2E测试 (435项)", "C03 CLI 30命令树 · C04 43 MCP/Web Parity · C05 配置模板 · C06 三后端Schema+自愈"], "teal"),
        (48, 250, 500, 88, "Ring 2 · 自动化验证与门禁 (C07 ~ C09)", ["C07 Benchmark v4.1 & PR-01~16 · C08 六重确定性门禁 & 双平台 CI", "C09 EVOLUTION_GUIDE.md (MANIFEST 机器锚点) & .codegraph 符号索引"], "green"),
        (48, 350, 500, 96, "Ring 3 · 工程知识与操作手册 (C10 ~ C13)", ["C10 22份架构设计规格书 & 产品边界准则 · C11 ISSUE/反馈总账/活跃路线图", "C12 开发/安装/三视角指南 · C13 10卷官方使用手册"], "amber"),
        (48, 458, 500, 96, "Ring 4 · 对外阵地与智能体认知 (C14 ~ C16)", ["C14 README & 发版说明 · C15 3套内置 Agent SKILL.md", "C16 Decks-as-Code 9份动态演说 HTML (Playwright 验真) & 技术长文/SVG"], "purple"),
    ]
    for rx, ry, rw, rh, rhdr, rlns, rthm in rings:
        c.card(rx, ry, rw, rh, rhdr, rlns, theme=rthm, header_size=12, body_size=11, line_gap=20)

    # Right: O(1) Citizen Sync & Runtime-Engineering Dual Loop
    c.rect(590, 96, 502, 474, fill="#FFFFFF", stroke=PALETTE["teal_stroke"], sw=1.6, rx=8)
    c.text(610, 124, "B. O(1) 变更传导路由与双引擎闭环验证", size=14, weight="700", fill=PALETTE["teal_dark"])

    c.card(610, 142, 462, 118, "工程态飞轮 (Engineering Self-Evolution)", ["1. Tier-0 零重扫开工: 仅读 2 份入口文档 + codegraph explore", "2. make citizen-impact (<100ms): 基于 git diff 划分 L1/L2/L3", "   精准列出受影响毛细血管与最小测试集，告别全仓盲目 grep", "3. make quality-gate: CC-1~CC-7 动态交叉审计 0 漂移拦截"], theme="teal", header_size=12, body_size=11, line_gap=19)

    c.card(610, 282, 462, 122, "运行时飞轮 (Runtime Self-Evolution)", ["1. schema_reconcile.rs: 启动事务式老库补列/改名 + FTS5 自愈", "2. ShadowAbEngine: 线上 shadow_ab 捕获 H1~H5 偏差并写穿修复", "3. P1~P5 五点探针 + 双域审计驱动租约校准与规则聚类", "4. 真实痛点自动去重写入 user_feedback.md 触发下一轮工程迭代"], theme="green", header_size=12, body_size=11, line_gap=19)

    c.rect(610, 424, 462, 128, fill=PALETTE["amber_fill"], stroke=PALETTE["amber_stroke"], sw=1.4, rx=7)
    c.text(626, 448, "78 个演进阶段量化工程纪律实证结果:", size=13, weight="700", fill=PALETTE["amber_dark"])
    c.text(626, 472, "- 生产代码 .unwrap() / .expect() 数量:  0 处 (lint-panic 守护)", size=11, weight="600", fill=PALETTE["ink"])
    c.text(626, 494, "- >1000 行生产 Rust 文件数量:          0 个 (模块化红线)", size=11, weight="600", fill=PALETTE["ink"])
    c.text(626, 516, "- 核心路径静默 let _ = 吞错数量:        0 / 15 (18/18 WaitEvents)", size=11, weight="600", fill=PALETTE["ink"])
    c.text(626, 538, "- MCP / CLI / Web 动作级对齐度:        43 / 43 (100% Parity)", size=11, weight="600", fill=PALETTE["ink"])

    c.line([(548, 294), (590, 294)], color=PALETTE["purple_stroke"], sw=1.8, marker="arr-purple")
    c.line([(841, 260), (841, 282)], color=PALETTE["green_stroke"], sw=1.8, marker="arr-green")

    c.footer("源码锚点: scripts/{evolution_manifest,citizen_sync,code_hygiene}.py · src/storage/sqlite_provider/schema_reconcile.rs")
    c.save("04-dual-flywheel-and-citizen-routing.svg")


# ---------------------------------------------------------------------------
# 11. 05-five-layer-defense-and-dual-audit.svg (Paper 05 Fig 1)
# ---------------------------------------------------------------------------
def gen_05_defense_audit() -> None:
    c = SvgCanvas(
        1120,
        630,
        "Paper 05 图 1 · 宿主机具身智能体五层纵深防御、双域防洪审计与零信任 Mesh 信封",
        "面向拥有真实宿主机 Shell/文件/网络权限的自主 Agent：实现采集零泄密、自治零黑盒、跨机零直写覆盖",
    )
    # Left: 5 Defense-in-Depth Layers
    c.rect(28, 96, 520, 480, fill="#FFFFFF", stroke=PALETTE["red_stroke"], sw=1.6, rx=8)
    c.text(46, 124, "A. 五层具身安全纵深防御矩阵 (L1 ~ L5)", size=14, weight="700", fill=PALETTE["red_dark"])

    layers = [
        ("L1 采集准入与全链路脱敏", "敏感路径浅层阻断 + SecretRedactor 过滤 /proc/记忆/审计 (DQ4: 0% 泄露)", "red"),
        ("L2 动作门禁与命令沙盒", "Pre-Hook 拦截 /etc/shadow & ~/.ssh + POSIX 转义 + DuckDB 导出白名单", "amber"),
        ("L3 本地回环与令牌鉴权", "Web/Mesh 默认绑定 127.0.0.1 + 全写端点 Bearer 校验 (未授权 100% 401)", "teal"),
        ("L4 存储最小权限与预检", "~/.agent-body 0700 & body.db 0600 + PreflightChecker 权限越界告警", "green"),
        ("L5 抗提示词投毒清洗", "PromptInjectionScanner 隔离外部 README/胶囊越狱指令 (W11/W22 验证)", "purple"),
    ]
    ly = 142
    for title, desc, thm in layers:
        c.rect(46, ly, 484, 76, fill=PALETTE[f"{thm}_fill"], stroke=PALETTE[f"{thm}_stroke"], sw=1.4, rx=6)
        c.text(62, ly + 26, title, size=13, weight="700", fill=PALETTE[f"{thm}_dark"])
        c.text(62, ly + 52, desc, size=11, fill=PALETTE["ink"])
        ly += 84

    # Right top: Dual-Domain Audit
    c.card(
        572,
        96,
        520,
        232,
        "B. IN_AGENT × OUT_OF_AGENT 双域因果存证审计 (<80 us)",
        [
            "IN_AGENT 域: 捕获 Brief 租约 / Pre&Post Hook / 43 MCP / 30 CLI / Web",
            "OUT_OF_AGENT 域: 终结后台黑盒! 强制留痕 Daemon 巡检、HostGC、",
            "  LinkRepair 源码补丁、睡眠整理及 Watcher 60s 滑动窗防洪折叠",
            "防篡改载体: W3C trace_id + 脱敏 before_after_delta (<=4KB) + SHA-256",
            "现网 7 天实测: 9,431 条审计 (IN=6,236 / OUT=3,195), 明文泄露 == 0%",
        ],
        theme="teal",
        line_gap=28,
    )

    # Right bottom: MeshEnvelope v1.0
    c.card(
        572,
        348,
        520,
        228,
        "C. MeshEnvelope v1.0 零信任跨机联邦协同协议",
        [
            "规范签名: HMAC-SHA256(protocol|msg_id|type|from|to|ts|nonce|sha256)",
            "双因子防重放: |T_srv - ts| <= 300s 时间窗 + 并发 NonceCache (重放返 409)",
            "最小权限四作用域: read:facts / write:locks / dialogue / transfer",
            "受控文件沙盒: <=10MB 流式限额 + 禁敏感扩展名 -> 隔离落盘 mesh_inbox/",
            "  (0600 权限, status='quarantined')，须人工或本机显式审批方可消费",
        ],
        theme="purple",
        line_gap=28,
    )

    c.line([(548, 212), (572, 212)], color=PALETTE["teal_stroke"], marker="arr-teal")
    c.line([(548, 462), (572, 462)], color=PALETTE["purple_stroke"], marker="arr-purple")

    c.footer("源码与实测依据: src/scanner/redactor.rs · src/audit/mod.rs · src/mesh/{protocol,auth,transfer}.rs · Benchmark W11, W14, W16, DQ4 & DQ7")
    c.save("05-five-layer-defense-and-dual-audit.svg")


# ---------------------------------------------------------------------------
# 12. 05-pareto-quality-vs-token-frontier.svg (Paper 05 Fig 2)
# ---------------------------------------------------------------------------
def gen_05_pareto_frontier() -> None:
    c = SvgCanvas(
        1120,
        630,
        "Paper 05 图 2 · Benchmark v4.1 防作弊帕累托前沿实测 (Token 节省率 vs 质量分 Q)",
        "硬性双门禁: 严禁靠删减关键事实换取高压缩率! 强制要求 Fact Recall == 100% 且 Q_body >= Q_baseline",
    )
    # Scatter plot area on left
    px, py, pw, ph = 84, 130, 560, 410
    c.rect(28, 96, 640, 484, fill="#FFFFFF", stroke=PALETTE["green_stroke"], sw=1.5, rx=8)
    c.text(48, 120, "代表性宏负载与同机竞品对标帕累托跃迁轨迹 (实测数据)", size=13, weight="700", fill=PALETTE["green_dark"])

    c.rect(px, py, pw, ph, fill=PALETTE["bg"], stroke=PALETTE["line"], sw=1.2, rx=4)
    # Pareto target zone highlight (top-right: savings >= 55%, Q >= 90)
    c.rect(px + 308, py + 8, 244, 78, fill=PALETTE["green_fill"], stroke=PALETTE["green_stroke"], sw=1.3, rx=6, dash="5,4")
    c.text(px + 430, py + 72, "帕累托靶区 (Q>=95, 省 59%~99.9%)", size=11, weight="700", fill=PALETTE["green_dark"], anchor="middle")

    # Axes: X = Token Savings % (0% -> 100%), Y = Context Quality Score Q (0 -> 100)
    c.line([(px, py + ph), (px + pw - 14, py + ph)], color=PALETTE["ink"], sw=1.6, marker="arr-teal")
    c.line([(px, py + ph), (px, py + 14)], color=PALETTE["ink"], sw=1.6, marker="arr-teal")
    c.text(px + pw / 2, py + ph + 28, "Token 压缩节省率 Token Savings %  (0%  -------->  50%  -------->  100%)", size=12, weight="600", fill=PALETTE["ink"], anchor="middle")

    # Y ticks
    for q_val in (0, 25, 50, 75, 100):
        yy = py + ph - (q_val / 100.0) * (ph - 30) - 10
        c.line([(px - 5, yy), (px + pw - 20, yy)], color=PALETTE["line"], sw=0.8, marker="", dash="3,3")
        c.text(px - 10, yy + 4, f"Q={q_val}", size=10, fill=PALETTE["ink2"], anchor="end")

    def map_pt(sav_pct: float, q: float) -> tuple[float, float]:
        xx = px + 20 + (sav_pct / 100.0) * (pw - 52)
        yy = py + ph - 10 - (q / 100.0) * (ph - 30)
        return round(xx, 1), round(yy, 1)

    # Plot baseline (0% savings, baseline Q) -> Body (savings%, body Q) with leader labels
    pairs = [
        ("COMP-C1 (vs 裸记忆)", 0.0, 24.1, 97.8, 100.0, -90, 24),
        ("COMP-C2 (vs OpenViking)", 0.0, 45.5, 99.4, 100.0, -108, 44),
        ("COMP-C3 (vs AgentFS)", 0.0, 26.4, 96.5, 100.0, -92, 64),
        ("COMP-C4 (vs CLI组合)", 0.0, 37.3, 46.0, 97.5, -18, 28),
        ("W1 能力发现 (99.4%)", 0.0, 60.5, 99.4, 100.0, -104, -12),
        ("W8 冷简报 (59.0%)", 0.0, 40.0, 59.0, 96.6, 8, 26),
        ("W15 AOPCC (83.9%)", 0.0, 52.0, 83.9, 100.0, -70, 82),
    ]
    for name, s0, q0, s1, q1, dx_lbl, dy_lbl in pairs:
        x0, y0 = map_pt(s0, q0)
        x1, y1 = map_pt(s1, q1)
        c.line([(x0, y0), (x1, y1)], color=PALETTE["line"], sw=1.2, marker="", dash="4,3")
        c.circle(x0, y0, 4.5, fill=PALETTE["red_fill"], stroke=PALETTE["red_stroke"])
        c.circle(x1, y1, 5.5, fill=PALETTE["green_fill"], stroke=PALETTE["green_stroke"])

    # Clean non-overlapping callout legend inside plot middle-right
    callouts = [
        (210, 270, "W8 冷启动简报: Q 40.0 -> 96.6 (省 59.0%)"),
        (210, 294, "COMP-C4 vs CLI: Q 37.3 -> 97.5 (省 46.0%, 0子进程)"),
        (210, 318, "W15 AOPCC 回路: Q 52.0 -> 100.0 (省 83.9%, 3.49x)"),
        (210, 342, "COMP-C3 vs AgentFS: Q 26.4 -> 100.0 (省 96.5%)"),
        (210, 366, "COMP-C1 vs 裸记忆: Q 24.1 -> 100.0 (0% 凭据泄露)"),
        (210, 390, "COMP-C2 vs OpenViking: Q 45.5 -> 100.0 (省 99.4%)"),
        (210, 414, "W1/W7 能力&路由: Q -> 100.0 (省 99.4% ~ 99.9%)"),
    ]
    c.rect(198, 248, 392, 182, fill="#FFFFFF", stroke=PALETTE["line"], sw=1.1, rx=6)
    for lx, ly_c, txt in callouts:
        c.text(lx, ly_c, txt, size=11, weight="600", fill=PALETTE["ink"])

    # Right card: Formula & Track B Provenance Ledger
    c.card(
        688,
        96,
        404,
        232,
        "四维确定性上下文质量函数 Q in [0, 100]",
        [
            "Q = 100 * (0.40*R_fact + 0.30*P_signal",
            "         + 0.20*A_action + 0.10*S_safety)",
            "1. R_fact (40%): 必含物理真值事实召回完整率",
            "2. P_signal (30%): 有效信号字节信噪纯净度",
            "3. A_action (20%): 可直接执行的路径/端口/补丁",
            "4. S_safety (10%): 零密钥残留与抗注入合规度",
            "信息密度增益: rho_gain = rho_body / rho_base",
        ],
        theme="teal",
        line_gap=22,
    )
    c.card(
        688,
        348,
        404,
        232,
        "生产效能存证账本 (EffectivenessLedger)",
        [
            "杜绝拍脑袋估算: 每次收益强制绑定 PR-01 ~ PR-16",
            "- PR-01~08: 技能路由/分层扫描/Pre-Hook/简报",
            "- PR-11~13: AOPCC 租约直通/审计演化/跨脑胶囊",
            "- PR-15~16: 睡眠整理省 Token / 快权重门禁验证",
            "开销与防伪: 异步记账 <100 us；批次报告携带",
            "宿主机硬件指纹、Git Hash 与 SHA-256 签名",
        ],
        theme="amber",
        line_gap=24,
    )

    c.footer("数据出处: 本机真机实跑 run-310cf715 (W1~W19, COMP-C1~C4) 及 Phase 76~78 (W20~W22) · src/benchmark/")
    c.save("05-pareto-quality-vs-token-frontier.svg")


def main() -> None:
    gen_00_portfolio_matrix()
    gen_00_roadmap_flow()
    gen_01_brain_body_arch()
    gen_01_aopcc_diagram()
    gen_02_temporal_asymmetry()
    gen_02_pg18_results()
    gen_03_graph_federation()
    gen_03_omni_rrf()
    gen_04_microkernel()
    gen_04_dual_flywheel()
    gen_05_defense_audit()
    gen_05_pareto_frontier()
    print(f"Generated {len(list(OUT_DIR.glob('*.svg')))} SVG diagrams in {OUT_DIR}")


if __name__ == "__main__":
    main()
