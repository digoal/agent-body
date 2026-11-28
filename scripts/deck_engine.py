#!/usr/bin/env python3
"""Visual-First Architectural Blueprint HTML slide deck engine for Agent Body.

Upgrades the entire presentation suite from text-heavy rectangular boxes into an
iconic, visual-first diagram engine (1472 x 640 main figure stage = 72%+ visual ratio):
  - Light, warm architectural-paper palette (zero harsh black backgrounds).
  - Full library of bespoke domain SVG illustrations via deck_visuals.py.
  - Upgraded general visual primitives (icon-led visual cards, flowing pipelines,
    visual versus arenas, linked code-to-diagram maps, radar/bar meters).
  - Zero external deps, 16:9 vector scaling, G TOC, N/T synced speaker drawer,
    P autoplay, F fullscreen with bottom control bar, A/S/R live step animation
    (defaults to FULL CLEAR VIEW on slide entry, animates step-by-step on play).
"""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any

from deck_visuals import (
    C_CARD,
    C_CLAY,
    C_CLAY_L,
    C_CLAY_M,
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
    C_RED,
    C_RED_L,
    C_TEAL,
    C_TEAL_2,
    C_TEAL_3,
    C_TEAL_D,
    C_TEAL_L,
    C_TEAL_M,
    C_WHITE,
    _arrow_defs,
    esc,
    icon,
    icon_badge,
    ring_gauge,
    vis_aopcc_highway,
    vis_awr_telemetry,
    vis_benchmark_pareto,
    vis_blind_agent_scene,
    vis_brain_body_anatomy,
    vis_command_console,
    vis_comparative_scale,
    vis_competitor_arena,
    vis_cover_visual,
    vis_do_dont_wall,
    vis_evolution_flywheel,
    vis_freshness_purifier,
    vis_handoff_capsule,
    vis_host_graph_federation,
    vis_lock_and_wfg,
    vis_mesh_network,
    vis_microkernel_hub,
    vis_parity_console,
    vis_quality_gates,
    vis_roadmap_journey,
    vis_security_dual_audit,
    vis_storage_tri_engine,
    vis_three_tier_pyramid,
    vis_tiered_scan_doors,
    vis_vitals_radar_autonomy,
)

__all__ = [
    "C_PAPER", "C_PAPER2", "C_LINE", "C_INK", "C_INK2", "C_MUTE",
    "C_TEAL", "C_TEAL_D", "C_TEAL_2", "C_TEAL_3", "C_TEAL_L", "C_TEAL_M",
    "C_CLAY", "C_CLAY_L", "C_GREEN", "C_GREEN_L", "C_GOLD", "C_GOLD_L",
    "C_RED", "C_RED_L", "C_WHITE", "esc", "icon", "icon_badge", "ring_gauge",
    "svg_cover", "svg_columns", "svg_pipeline", "svg_compare_rows",
    "svg_code_and_flow", "svg_matrix_2x2", "svg_metrics_table", "svg_bars", "svg_closing",
    "vis_cover_visual", "vis_brain_body_anatomy", "vis_comparative_scale",
    "vis_blind_agent_scene", "vis_host_graph_federation", "vis_tiered_scan_doors",
    "vis_aopcc_highway", "vis_lock_and_wfg", "vis_freshness_purifier",
    "vis_handoff_capsule", "vis_security_dual_audit", "vis_vitals_radar_autonomy",
    "vis_awr_telemetry", "vis_evolution_flywheel", "vis_benchmark_pareto",
    "vis_competitor_arena", "vis_microkernel_hub", "vis_mesh_network",
    "vis_do_dont_wall", "vis_storage_tri_engine", "vis_parity_console",
    "vis_quality_gates", "vis_three_tier_pyramid", "vis_command_console",
    "vis_roadmap_journey", "render_deck",
]


def _pick_icon(text: str, fallback: str = "cube") -> str:
    """Infer an expressive vector icon from Chinese/English title keywords."""
    t = text.lower()
    rules = [
        (("脑", "brain", "llm", "claude", "anti", "cursor"), "brain"),
        (("插", "plug", "接力", "spi", "桥", "总线"), "plug"),
        (("身体", "本体", "body", "器官"), "body"),
        (("感官", "探针", "/proc", "事实", "雷达", "睁眼", "radar"), "radar"),
        (("小脑", "aopcc", "乐观", "直通", "反射", "自愈", "毫秒", "快"), "bolt"),
        (("胶囊", "海马", "记忆", "memory", "checkpoint", "续跑"), "capsule"),
        (("脊柱", "放置", "规则", "路由", "防爆", "skill", "秩序"), "spine"),
        (("安全", "脱敏", "盾", "红线", "权限", "包络", "防御", "投毒"), "shield"),
        (("联邦", "mesh", "跨机", "多体", "协同"), "mesh"),
        (("微内核", "kernel", "contrib", "插件", "发行版", "systemd"), "gear"),
        (("sqlite", "postgres", "duckdb", "存储", "数据库", "表", "迁移"), "database"),
        (("锁", "lock", "wfg", "死锁", "互斥", "并发"), "lock"),
        (("目录", "扫描", "文件", "黑白", "敲门", "folder"), "folder"),
        (("禁", "不做", "黑名单", "红灯", "阻断", "误区", "坑"), "ban"),
        (("codegraph", "ast", "符号", "微观", "显微镜"), "microscope"),
        (("陈旧", "净", "保鲜", "写穿", "漏斗", "过滤", "折叠"), "filter"),
        (("命令", "cli", "终端", "黑屏", "演练", "实操", "hook"), "terminal"),
        (("awr", "性能", "等待", "体征", "遥测", "cpu", "sla", "指标"), "gauge"),
        (("bench", "实证", "成绩", "帕累托", "竞品", "对标", "pass", "门禁"), "trophy"),
        (("比较优势", "天平", "权衡", "边界"), "scale"),
        (("进化", "飞轮", "探针", "p1", "闭环"), "flywheel"),
        (("图谱", "图", "拓扑", "graph", "links", "爆炸"), "graph"),
        (("审计", "账本", "audit", "留痕", "trace"), "audit"),
        (("源码", "struct", "fn ", "代码", "rust"), "code"),
        (("cron", "定时", "时间", "60s", "10ms"), "clock"),
        (("架构", "分层", "tier", "三层", "四层"), "layers"),
        (("翻车", "痛点", "惨案", "事故", "盲"), "warning"),
        (("上手", "启动", "部署", "终局", "未来", "跃迁"), "rocket"),
    ]
    for kws, ic in rules:
        if any(k in t for k in kws):
            return ic
    return fallback


def _short(text: str, max_chars: int = 24) -> str:
    """Keep SVG visual labels concise and scannable so diagrams breathe."""
    s = str(text).strip()
    if len(s) <= max_chars:
        return s
    # Split cleanly at punctuation if possible
    for sep in ("：", "—", "（", "，", "；", "·", "|"):
        pos = s.find(sep)
        if 6 <= pos <= max_chars:
            return s[:pos].strip()
    return s[: max_chars - 1].rstrip(" ,;:/·—-") + "…"


def _extract_hero_num(lines: list[Any]) -> str | None:
    """Pull a standout metric/number from card lines for big-number visual display."""
    joined = " ".join(x[0] if isinstance(x, tuple) else str(x) for x in lines)
    for pat in (
        r"-?\d+(?:\.\d+)?%",
        r"\d+(?:\.\d+)?x",
        r"<=?\d+\s*(?:ms|μs|ns|tok|MB|KB)",
        r"\d+\s*/\s*\d+",
        r"\d+(?:,\d+)+",
    ):
        m = re.search(pat, joined)
        if m:
            return m.group(0).replace(" ", "")
    return None


# ==============================================================================
# UPGRADED GENERAL VISUAL PRIMITIVES (1472 x 640 — High Graphic, Low Text)
# ==============================================================================

def svg_cover(
    kicker: str,
    title: str,
    en_sub: str,
    subtitle1: str,
    subtitle2: str,
    spec_line: str,
    strata: list[tuple[str, str, str, str]],
    right_tag: str = "图解演说 · v1.1.0",
    theme: str | None = None,
) -> str:
    """Visual Cover upgrade — auto-selects bespoke vector cover illustration."""
    if theme is None:
        all_txt = (title + " " + en_sub + " " + kicker).lower()
        if "感官" in all_txt or "扫盘" in all_txt or "图谱" in all_txt:
            theme = "sensory"
        elif "小脑" in all_txt or "aopcc" in all_txt:
            theme = "cerebellum"
        elif "海马" in all_txt or "新鲜" in all_txt or "接力" in all_txt:
            theme = "hippocampus"
        elif "安全" in all_txt or "审计" in all_txt or "mesh" in all_txt:
            theme = "security"
        elif "bench" in all_txt or "实证" in all_txt:
            theme = "benchmark"
        elif "技术" in all_txt or "全景" in all_txt or "exchange" in all_txt:
            theme = "tech_arch"
        else:
            theme = "roadshow"

    # Extract 3 crisp metric badges from spec_line
    raw_parts = [p.strip() for p in spec_line.split("·") if p.strip()]
    cols = [C_TEAL, C_CLAY, C_GREEN, C_GOLD]
    badges: list[tuple[str, str, str]] = []
    for i, part in enumerate(raw_parts):
        m = re.search(r"(\d+(?:\.\d+)?(?:%|x|MB|ms|μs)?|v\d+\.\d+(?:\.\d+)?)", part)
        if m and len(badges) < 3:
            val = m.group(1)
            lbl = part.replace(val, "").strip(" ·:：") or part
            badges.append((val, _short(lbl, 10), cols[len(badges) % len(cols)]))
    if len(badges) < 3:
        badges = [
            ("27 模块", "Rust 单二进制", C_TEAL),
            ("43 MCP", "黑白屏同源", C_CLAY),
            ("-99.2%", "Bench v3.9", C_GREEN),
        ]
    return vis_cover_visual(
        kicker=kicker,
        title=title,
        en_sub=en_sub,
        tagline=_short(subtitle1, 30),
        badges=badges[:3],
        theme=theme,
        right_tag=right_tag,
    )


def svg_columns(
    cols: list[dict[str, Any]],
    bottom_banner: tuple[str, str, str] | None = None,
    animated: bool = False,
) -> str:
    """Visual Icon-Led Pillars (1472x640) — large vector badges, hero numbers, concise chips."""
    n = len(cols)
    gap = 22 if n <= 4 else 16
    w = (1440 - gap * (n - 1)) // n
    h = 520 if bottom_banner else 602
    cls = "fig anim" if animated else "fig"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("vcol")]

    for i, c in enumerate(cols):
        x = 16 + i * (w + gap)
        acc = c.get("accent", C_TEAL)
        bg = c.get("bg", C_WHITE)
        ic = c.get("icon") or _pick_icon(c.get("title", "") + " " + c.get("tag", ""))
        lines = c.get("lines", [])
        hero_num = c.get("metric") or _extract_hero_num(lines)

        parts.append(f'<g data-step="{i + 1}">' if animated else "<g>")
        parts.append(f'<rect x="{x}" y="18" width="{w}" height="{h}" rx="20" fill="{bg}" stroke="{acc}" stroke-width="1.8" filter="url(#vcol_sh)"/>')
        parts.append(f'<rect x="{x}" y="18" width="{w}" height="10" rx="5" fill="{acc}"/>')

        # Large visual icon emblem centered at top of card
        cx = x + w / 2
        parts.append(icon_badge(ic, cx, 98, 36, acc, C_WHITE if bg != C_WHITE else None))
        tag = _short(c.get("tag", f"0{i + 1}"), 22)
        parts.append(f'<text x="{cx:.1f}" y="160" font-family="var(--f-m)" font-size="11.5" fill="{acc}" font-weight="700" text-anchor="middle" letter-spacing="1.2">{esc(tag)}</text>')

        title_fs = 22 if n <= 3 else 18.5
        parts.append(f'<text x="{cx:.1f}" y="198" font-family="var(--f-r)" font-size="{title_fs}" fill="{C_INK}" font-weight="700" text-anchor="middle">{esc(_short(c["title"], 16 if n <= 3 else 12))}</text>')

        # Big visual metric pill if present
        cur_y = 224
        if hero_num:
            pw = min(w - 56, 200)
            parts.append(f'<rect x="{cx - pw/2:.1f}" y="{cur_y}" width="{pw}" height="46" rx="23" fill="{acc}"/>')
            parts.append(f'<text x="{cx:.1f}" y="{cur_y + 31}" font-family="var(--f-m)" font-size="22" fill="{C_PAPER}" font-weight="700" text-anchor="middle">{esc(hero_num)}</text>')
            cur_y += 70
        else:
            parts.append(f'<line x1="{x + 32}" y1="{cur_y + 8}" x2="{x + w - 32}" y2="{cur_y + 8}" stroke="{C_LINE}" stroke-width="1.2"/>')
            cur_y += 28

        # Max 3 concise visual bullet chips (drastically cutting prose density)
        max_chars = 21 if n <= 3 else 15
        shown_lines = lines[:3]
        chip_h = 46
        chip_gap = 12
        for line in shown_lines:
            is_hi = False
            txt = line
            if isinstance(line, tuple):
                txt, is_hi = line
            if cur_y + chip_h > 18 + h - 72:
                break
            cbg = C_TEAL_L if is_hi and acc == C_TEAL else (
                C_CLAY_L if is_hi and acc == C_CLAY else (
                C_GREEN_L if is_hi and acc == C_GREEN else C_PAPER))
            parts.append(f'<rect x="{x + 22}" y="{cur_y}" width="{w - 44}" height="{chip_h}" rx="12" fill="{cbg}" stroke="{acc if is_hi else C_LINE}" stroke-width="{1.4 if is_hi else 1}"/>')
            parts.append(f'<circle cx="{x + 42}" cy="{cur_y + chip_h / 2:.1f}" r="4.5" fill="{acc}"/>')
            parts.append(f'<text x="{x + 58}" y="{cur_y + 28}" font-family="var(--f-s)" font-size="{14 if n <= 3 else 13}" fill="{acc if is_hi else C_INK}" font-weight="{700 if is_hi else 600}">{esc(_short(txt, max_chars))}</text>')
            cur_y += chip_h + chip_gap

        foot_val = c.get("foot_val") or c.get("foot_lbl")
        if foot_val:
            fy = 18 + h - 38
            parts.append(f'<rect x="{x + 22}" y="{fy - 14}" width="{w - 44}" height="34" rx="10" fill="{C_PAPER2}"/>')
            parts.append(f'<text x="{cx:.1f}" y="{fy + 8}" font-family="var(--f-m)" font-size="12" fill="{C_INK2}" font-weight="600" text-anchor="middle">{esc(_short(foot_val, 26 if n <= 3 else 19))}</text>')
        parts.append("</g>")

    if bottom_banner:
        b_txt, b_right, b_style = bottom_banner
        bar = C_CLAY if b_style == "clay" else (C_GREEN if b_style == "green" else C_TEAL_D)
        parts.append(f'<g data-step="{n + 1}">' if animated else "<g>")
        parts.append(f'<rect x="16" y="562" width="1440" height="60" rx="14" fill="{bar}"/>')
        parts.append(f'<text x="48" y="598" font-family="var(--f-r)" font-size="18.5" fill="{C_PAPER}" font-weight="700">{esc(_short(b_txt, 54))}</text>')
        if b_right:
            parts.append(f'<text x="1424" y="598" font-family="var(--f-m)" font-size="13" fill="#CFE4E7" font-weight="600" text-anchor="end">{esc(_short(b_right, 28))}</text>')
        parts.append("</g>")

    parts.append("</svg>")
    return "".join(parts)


def svg_pipeline(
    steps: list[dict[str, Any]],
    bottom_panels: list[dict[str, Any]] | None = None,
    animated: bool = False,
    mid: str = "ahp",
) -> str:
    """Visual Flowing Pipeline with Large Icon Stations + Crisp Graphic Outcome Cards."""
    cls = "fig anim" if animated else "fig"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs(mid)]

    n = len(steps)
    arrow_w = 42
    box_w = (1440 - arrow_w * (n - 1)) // n
    box_h = 286 if bottom_panels else 450
    y0 = 26 if bottom_panels else 82

    for i, st in enumerate(steps):
        x = 16 + i * (box_w + arrow_w)
        cx = x + box_w / 2
        acc = st.get("accent", C_TEAL)
        bg = st.get("bg", C_WHITE)
        ic = st.get("icon") or _pick_icon(st.get("title", "") + " " + st.get("tag", ""))
        parts.append(f'<g data-step="{i + 1}">' if animated else "<g>")
        parts.append(f'<rect x="{x}" y="{y0}" width="{box_w}" height="{box_h}" rx="20" fill="{bg}" stroke="{acc}" stroke-width="2" filter="url(#{mid}_sh)"/>')
        parts.append(f'<rect x="{x}" y="{y0}" width="{box_w}" height="9" rx="4" fill="{acc}"/>')
        # Step number badge top-right
        parts.append(f'<circle cx="{x + box_w - 28}" cy="{y0 + 34}" r="14" fill="{acc}"/>')
        parts.append(f'<text x="{x + box_w - 28}" y="{y0 + 39}" font-family="var(--f-m)" font-size="12" fill="{C_PAPER}" font-weight="700" text-anchor="middle">{i + 1}</text>')
        # Big centered icon
        parts.append(icon_badge(ic, cx, y0 + 74, 30, acc, C_WHITE if bg != C_WHITE else None))
        parts.append(f'<text x="{cx:.1f}" y="{y0 + 132}" font-family="var(--f-m)" font-size="11.5" fill="{acc}" font-weight="700" text-anchor="middle">{esc(_short(st.get("tag", f"STEP {i+1}"), 18))}</text>')
        parts.append(f'<text x="{cx:.1f}" y="{y0 + 166}" font-family="var(--f-r)" font-size="20" fill="{C_INK}" font-weight="700" text-anchor="middle">{esc(_short(st["title"], 14))}</text>')

        # At most 2 short visual pills per pipeline station
        ly = y0 + 190
        for ln in st.get("lines", [])[:2]:
            parts.append(f'<rect x="{x + 18}" y="{ly}" width="{box_w - 36}" height="34" rx="10" fill="{C_PAPER}"/>')
            parts.append(f'<text x="{cx:.1f}" y="{ly + 22}" font-family="var(--f-s)" font-size="13" fill="{C_INK2}" font-weight="600" text-anchor="middle">{esc(_short(ln, 18 if n >= 4 else 24))}</text>')
            ly += 44

        if i < n - 1:
            ax1 = x + box_w + 4
            ax2 = x + box_w + arrow_w - 6
            ay = y0 + box_h // 2
            parts.append(f'<line x1="{ax1}" y1="{ay}" x2="{ax2}" y2="{ay}" stroke="{acc}" stroke-width="3.5" stroke-dasharray="7 4" class="flow-line" marker-end="url(#{mid}_t)"/>')
        parts.append("</g>")

    if bottom_panels:
        m = len(bottom_panels)
        pgap = 24
        pw = (1440 - pgap * (m - 1)) // m
        py = 352
        ph = 266
        parts.append(f'<g data-step="{n + 1}">' if animated else "<g>")
        for j, p in enumerate(bottom_panels):
            px = 16 + j * (pw + pgap)
            acc = p.get("accent", C_TEAL)
            ic = p.get("icon") or _pick_icon(p.get("title", ""), "trophy" if j == 1 else "gauge")
            lines = p.get("lines", [])
            hero = _extract_hero_num(lines)
            parts.append(f'<rect x="{px}" y="{py}" width="{pw}" height="{ph}" rx="20" fill="{C_WHITE}" stroke="{acc}" stroke-width="2" filter="url(#{mid}_sh)"/>')
            parts.append(f'<rect x="{px}" y="{py}" width="10" height="{ph}" rx="5" fill="{acc}"/>')
            parts.append(icon_badge(ic, px + 58, py + 58, 26, acc))
            parts.append(f'<text x="{px + 102}" y="{py + 52}" font-family="var(--f-r)" font-size="20" fill="{acc}" font-weight="700">{esc(_short(p["title"], 24))}</text>')
            if hero:
                parts.append(f'<rect x="{px + pw - 156}" y="{py + 28}" width="134" height="44" rx="22" fill="{acc}"/>')
                parts.append(f'<text x="{px + pw - 89}" y="{py + 56}" font-family="var(--f-m)" font-size="18" fill="{C_PAPER}" font-weight="700" text-anchor="middle">{esc(hero)}</text>')
            ly = py + 112
            for ln in lines[:3]:
                is_mono = False
                txt = ln
                if isinstance(ln, tuple):
                    txt, is_mono = ln
                cbg = C_TEAL_L if is_mono else C_PAPER
                parts.append(f'<rect x="{px + 30}" y="{ly}" width="{pw - 56}" height="38" rx="10" fill="{cbg}"/>')
                parts.append(f'<text x="{px + 48}" y="{ly + 24}" font-family="{"var(--f-m)" if is_mono else "var(--f-s)"}" font-size="{13 if is_mono else 14}" fill="{acc if is_mono else C_INK}" font-weight="600">{esc(_short(txt, 40 if m == 2 else 28))}</text>')
                ly += 48
        parts.append("</g>")

    parts.append("</svg>")
    return "".join(parts)


def svg_compare_rows(
    headers: tuple[str, str, str],
    rows: list[tuple[str, str, str]],
    takeaway: tuple[str, str] | None = None,
) -> str:
    """Visual Versus Split Arena (replacing dry 3-column text table with graphic contrast tracks)."""
    parts = ['<svg class="fig" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("vcr")]

    # Column visual header pills
    parts.append(f'<rect x="16" y="10" width="278" height="36" rx="18" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="155" y="33" font-family="var(--f-s)" font-size="13.5" fill="{C_PAPER}" font-weight="700" text-anchor="middle">{esc(_short(headers[0], 14))}</text>')

    parts.append(f'<rect x="314" y="10" width="540" height="36" rx="18" fill="{C_RED_L}" stroke="{C_RED}" stroke-width="1.4"/>')
    parts.append(f'<text x="584" y="33" font-family="var(--f-s)" font-size="13.5" fill="{C_RED}" font-weight="700" text-anchor="middle">✕ {esc(_short(headers[1], 24))}</text>')

    parts.append(f'<rect x="916" y="10" width="540" height="36" rx="18" fill="{C_GREEN_L}" stroke="{C_GREEN}" stroke-width="1.4"/>')
    parts.append(f'<text x="1186" y="33" font-family="var(--f-s)" font-size="13.5" fill="{C_GREEN}" font-weight="700" text-anchor="middle">✓ {esc(_short(headers[2], 24))}</text>')

    n = min(5, max(1, len(rows)))
    avail_h = 484
    gap = 14
    rh = min(88, (avail_h - gap * (n - 1)) // n)
    y = 64
    for c1, c2, c3 in rows[:5]:
        ic = _pick_icon(c1 + " " + c3)
        cy = y + rh / 2
        # Left dimension icon badge card
        parts.append(f'<rect x="16" y="{y}" width="278" height="{rh}" rx="14" fill="{C_TEAL_L}" stroke="{C_TEAL_M}" stroke-width="1.3"/>')
        parts.append(icon_badge(ic, 56, cy, 20, C_TEAL, C_WHITE))
        parts.append(f'<text x="90" y="{cy + 6:.1f}" font-family="var(--f-r)" font-size="17.5" fill="{C_TEAL_D}" font-weight="700">{esc(_short(c1, 11))}</text>')
        # Middle red-cross contrast card
        parts.append(f'<rect x="314" y="{y}" width="540" height="{rh}" rx="14" fill="{C_WHITE}" stroke="{C_RED_L}" stroke-width="2"/>')
        parts.append(icon("cross", 346, cy, 22, C_RED))
        parts.append(f'<text x="372" y="{cy + 6:.1f}" font-family="var(--f-s)" font-size="15" fill="{C_INK2}" font-weight="500">{esc(_short(c2, 29))}</text>')
        # Arrow bridge
        parts.append(f'<line x1="860" y1="{cy:.1f}" x2="906" y2="{cy:.1f}" stroke="{C_TEAL}" stroke-width="2.6" marker-end="url(#vcr_t)"/>')
        # Right green-check outcome card
        parts.append(f'<rect x="916" y="{y}" width="540" height="{rh}" rx="14" fill="{C_GREEN_L}" stroke="{C_GREEN}" stroke-width="1.6"/>')
        parts.append(icon("check", 948, cy, 22, C_GREEN))
        parts.append(f'<text x="974" y="{cy + 6:.1f}" font-family="var(--f-s)" font-size="15.5" fill="{C_INK}" font-weight="700">{esc(_short(c3, 29))}</text>')
        y += rh + gap

    if takeaway:
        parts.append(f'<rect x="16" y="566" width="1440" height="58" rx="14" fill="{C_TEAL_D}"/>')
        parts.append(f'<text x="48" y="601" font-family="var(--f-r)" font-size="19" fill="{C_PAPER}" font-weight="700">{esc(_short(takeaway[0], 48))}</text>')
        parts.append(f'<text x="1426" y="601" font-family="var(--f-m)" font-size="12.5" fill="#A6CFD5" text-anchor="end">{esc(_short(takeaway[1], 32))}</text>')

    parts.append("</svg>")
    return "".join(parts)


def svg_code_and_flow(
    left_title: str,
    file_ref: str,
    code_lines: list[str],
    right_title: str,
    right_cards: list[tuple[str, str, str]],
    animated: bool = False,
) -> str:
    """Visual Code-to-Mechanism Blueprint: crisp slate code window connected by arrows to iconic cards."""
    cls = "fig anim" if animated else "fig"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("vcf")]

    # Left crisp code panel (soft architectural slate, light-on-slate high readability, trimmed to <=12 core lines)
    parts.append('<g data-step="1">' if animated else "<g>")
    parts.append(f'<rect x="16" y="16" width="746" height="608" rx="20" fill="#223640" stroke="{C_TEAL}" stroke-width="2" filter="url(#vcf_sh)"/>')
    parts.append(f'<rect x="16" y="16" width="746" height="52" rx="18" fill="#2C4652"/>')
    parts.append(f'<circle cx="46" cy="42" r="6" fill="#E36D5D"/><circle cx="68" cy="42" r="6" fill="#E6B95E"/><circle cx="90" cy="42" r="6" fill="#63BA8B"/>')
    parts.append(f'<text x="116" y="47" font-family="var(--f-s)" font-size="14" fill="{C_PAPER}" font-weight="700">{esc(_short(left_title, 28))}</text>')
    parts.append(f'<text x="742" y="47" font-family="var(--f-m)" font-size="12" fill="#9AC7CE" text-anchor="end">{esc(_short(file_ref, 30))}</text>')

    trimmed = [ln for ln in code_lines if ln.strip()][:12]
    ly = 104
    step_y = min(42, 500 // max(1, len(trimmed)))
    for idx, cl in enumerate(trimmed):
        # Line number gutter
        parts.append(f'<text x="46" y="{ly}" font-family="var(--f-m)" font-size="12" fill="#6D93A0" text-anchor="end">{idx + 1:02d}</text>')
        col = "#85C2CC" if cl.strip().startswith(("//", "#")) else "#F5F3EE"
        if any(kw in cl for kw in ("fn ", "pub struct", "pub enum", "SELECT", "CREATE TABLE", "$ ", "make ")):
            col = "#F3CE7A"
        parts.append(f'<text x="64" y="{ly}" font-family="var(--f-m)" font-size="13.5" fill="{col}">{esc(cl[:64])}</text>')
        ly += step_y
    parts.append("</g>")

    # Right iconic mechanism cards linked with flowing arrows
    parts.append(f'<text x="822" y="38" font-family="var(--f-m)" font-size="12.5" fill="{C_TEAL}" font-weight="700" letter-spacing="1.6">{esc(_short(right_title, 36))}</text>')
    n = min(4, max(1, len(right_cards)))
    gap = 20
    ch = (566 - gap * (n - 1)) // n
    ry = 56
    accents = [C_TEAL_D, C_GREEN, C_CLAY, C_GOLD]
    for i, (ct, c1, c2) in enumerate(right_cards[:4]):
        acc = accents[i % len(accents)]
        ic = _pick_icon(ct + " " + c1)
        cy = ry + ch / 2
        parts.append(f'<g data-step="{i + 2}">' if animated else "<g>")
        # Connector arrow from code panel to card
        parts.append(f'<line x1="762" y1="{cy:.1f}" x2="814" y2="{cy:.1f}" stroke="{acc}" stroke-width="3" stroke-dasharray="6 4" class="flow-line" marker-end="url(#vcf_t)"/>')
        parts.append(f'<rect x="822" y="{ry}" width="634" height="{ch}" rx="18" fill="{C_WHITE}" stroke="{acc}" stroke-width="2" filter="url(#vcf_sh)"/>')
        parts.append(f'<rect x="822" y="{ry}" width="9" height="{ch}" rx="4" fill="{acc}"/>')
        parts.append(icon_badge(ic, 882, cy, 28, acc))
        parts.append(f'<text x="928" y="{ry + int(ch * 0.32)}" font-family="var(--f-r)" font-size="20" fill="{C_INK}" font-weight="700">{esc(_short(ct, 22))}</text>')
        parts.append(f'<text x="928" y="{ry + int(ch * 0.58)}" font-family="var(--f-s)" font-size="14.5" fill="{C_INK2}">{esc(_short(c1, 30))}</text>')
        if c2:
            parts.append(f'<rect x="928" y="{ry + int(ch * 0.70)}" width="504" height="30" rx="15" fill="{C_PAPER}"/>')
            parts.append(f'<text x="944" y="{ry + int(ch * 0.70) + 20}" font-family="var(--f-m)" font-size="12.5" fill="{acc}" font-weight="700">{esc(_short(c2, 38))}</text>')
        parts.append("</g>")
        ry += ch + gap

    parts.append("</svg>")
    return "".join(parts)


def svg_matrix_2x2(
    quads: list[dict[str, Any]],
    footer_banner: str | None = None,
    animated: bool = False,
) -> str:
    """Visual 2x2 Quadrant Diagram with Large Icon Medallions & Center Cross Hub."""
    cls = "fig anim" if animated else "fig"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("vmx")]

    qw = 702
    qh = 258 if footer_banner else 292
    coords = [(16, 16), (754, 16), (16, 16 + qh + 20), (754, 16 + qh + 20)]
    for i, (q, (x, y)) in enumerate(zip(quads[:4], coords)):
        acc = q.get("accent", C_TEAL)
        bg = q.get("bg", C_WHITE)
        ic = q.get("icon") or _pick_icon(q.get("title", "") + " " + q.get("tag", ""))
        parts.append(f'<g data-step="{i + 1}">' if animated else "<g>")
        parts.append(f'<rect x="{x}" y="{y}" width="{qw}" height="{qh}" rx="20" fill="{bg}" stroke="{acc}" stroke-width="2" filter="url(#vmx_sh)"/>')
        parts.append(f'<rect x="{x}" y="{y}" width="10" height="{qh}" rx="5" fill="{acc}"/>')
        parts.append(icon_badge(ic, x + 68, y + 66, 30, acc, C_WHITE if bg != C_WHITE else None))
        parts.append(f'<text x="{x + 114}" y="{y + 52}" font-family="var(--f-m)" font-size="12" fill="{acc}" font-weight="700" letter-spacing="1.2">{esc(_short(q["tag"], 32))}</text>')
        parts.append(f'<text x="{x + 114}" y="{y + 88}" font-family="var(--f-r)" font-size="22" fill="{C_INK}" font-weight="700">{esc(_short(q["title"], 22))}</text>')

        # 2 or 3 crisp pill bullets
        ly = y + 118
        for ln in q.get("lines", [])[:3]:
            if ly + 36 > y + qh - 10:
                break
            parts.append(f'<rect x="{x + 34}" y="{ly}" width="{qw - 60}" height="36" rx="10" fill="{C_WHITE if bg != C_WHITE else C_PAPER}" stroke="{C_LINE}" stroke-width="1"/>')
            parts.append(f'<circle cx="{x + 54}" cy="{ly + 18}" r="4.5" fill="{acc}"/>')
            parts.append(f'<text x="{x + 70}" y="{ly + 23}" font-family="var(--f-s)" font-size="14" fill="{C_INK}" font-weight="600">{esc(_short(ln, 34))}</text>')
            ly += 44
        parts.append("</g>")

    if footer_banner:
        parts.append(f'<rect x="16" y="568" width="1440" height="56" rx="14" fill="{C_TEAL_D}"/>')
        parts.append(f'<text x="48" y="602" font-family="var(--f-r)" font-size="18.5" fill="{C_PAPER}" font-weight="700">{esc(_short(footer_banner, 58))}</text>')
    parts.append("</svg>")
    return "".join(parts)


def svg_metrics_table(
    kpis: list[tuple[str, str, str]],
    headers: list[str],
    rows: list[list[str]],
    col_xs: list[int],
    footnote: str = "",
) -> str:
    """Visual KPI Hero Strip (with mini accent meters) + Clean High-Contrast Visual Scoreboard."""
    parts = ['<svg class="fig" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("vmt")]

    nk = len(kpis)
    kgap = 20
    kw = (1440 - kgap * (nk - 1)) // nk
    accents = [C_TEAL, C_GREEN, C_CLAY, C_GOLD]
    ics = ["bolt", "trophy", "gauge", "check"]
    for i, (val, lbl, sub) in enumerate(kpis):
        x = 16 + i * (kw + kgap)
        acc = accents[i % len(accents)]
        parts.append(f'<rect x="{x}" y="12" width="{kw}" height="156" rx="18" fill="{C_WHITE}" stroke="{acc}" stroke-width="2" filter="url(#vmt_sh)"/>')
        parts.append(f'<rect x="{x}" y="12" width="{kw}" height="8" rx="4" fill="{acc}"/>')
        parts.append(icon_badge(ics[i % len(ics)], x + kw - 46, 62, 22, acc))
        parts.append(f'<text x="{x + 22}" y="76" font-family="var(--f-m)" font-size="36" fill="{acc}" font-weight="700">{esc(val)}</text>')
        parts.append(f'<text x="{x + 22}" y="114" font-family="var(--f-r)" font-size="17" fill="{C_INK}" font-weight="700">{esc(_short(lbl, 16))}</text>')
        parts.append(f'<text x="{x + 22}" y="144" font-family="var(--f-m)" font-size="12" fill="{C_MUTE}">{esc(_short(sub, 26))}</text>')

    # Visual Scoreboard rows (expanded spacing, PASS green pills, visual highlight)
    ty = 192
    parts.append(f'<rect x="16" y="{ty}" width="1440" height="44" rx="12" fill="{C_TEAL_D}"/>')
    scale_x = [int(16 + cx * 1440 / 1400) for cx in col_xs]
    for htxt, cx in zip(headers, scale_x):
        parts.append(f'<text x="{cx}" y="{ty + 28}" font-family="var(--f-m)" font-size="13" fill="{C_PAPER}" font-weight="700">{esc(_short(htxt, 16))}</text>')

    ry = ty + 54
    shown_rows = rows[:6]
    rh = min(56, (320 - 8 * max(0, len(shown_rows) - 1)) // max(1, len(shown_rows)))
    for ri, row in enumerate(shown_rows):
        bg = C_WHITE if ri % 2 == 0 else C_PAPER
        parts.append(f'<rect x="16" y="{ry}" width="1440" height="{rh}" rx="12" fill="{bg}" stroke="{C_LINE}" stroke-width="1"/>')
        for ci, (cell, cx) in enumerate(zip(row, scale_x)):
            if cell.strip() == "PASS":
                parts.append(f'<rect x="{cx - 10}" y="{ry + rh // 2 - 14}" width="76" height="28" rx="14" fill="{C_GREEN}"/>')
                parts.append(f'<text x="{cx + 28}" y="{ry + rh // 2 + 5}" font-family="var(--f-m)" font-size="12.5" fill="{C_PAPER}" font-weight="700" text-anchor="middle">PASS ✓</text>')
            else:
                ff = "var(--f-m)" if ci == 0 or "%" in cell or "μs" in cell or "ms" in cell or "x" in cell else "var(--f-s)"
                fc = C_TEAL_D if ci == 0 else (C_GREEN if "100" in cell or "PASS" in cell else C_INK)
                fw = "700" if ci == 0 or ci == len(row) - 1 else "500"
                parts.append(f'<text x="{cx}" y="{ry + rh // 2 + 5}" font-family="{ff}" font-size="14" fill="{fc}" font-weight="{fw}">{esc(_short(cell, 26))}</text>')
        ry += rh + 8

    if footnote:
        parts.append(f'<rect x="16" y="576" width="1440" height="48" rx="12" fill="{C_TEAL_L}"/>')
        parts.append(f'<text x="42" y="606" font-family="var(--f-m)" font-size="13" fill="{C_TEAL_D}" font-weight="600">{esc(_short(footnote, 78))}</text>')

    parts.append("</svg>")
    return "".join(parts)


def svg_bars(
    title_left: str,
    rows: list[tuple[str, float, str, str, str]],
    footer_text: str,
) -> str:
    """High-Impact Visual Horizontal Bar Chart with Percentage Badges & Icon Anchors."""
    parts = ['<svg class="fig" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">']
    parts.append(f'<rect x="16" y="14" width="1440" height="536" rx="22" fill="{C_WHITE}" stroke="{C_LINE}" stroke-width="1.6"/>')
    parts.append(f'<text x="46" y="52" font-family="var(--f-m)" font-size="13" fill="{C_TEAL}" font-weight="700" letter-spacing="1.4">{esc(_short(title_left, 44))}</text>')
    parts.append(f'<text x="1156" y="52" font-family="var(--f-s)" font-size="12" fill="{C_MUTE}" text-anchor="end">压缩 / 达标率</text>')
    parts.append(f'<text x="1202" y="52" font-family="var(--f-s)" font-size="12" fill="{C_MUTE}">核心实证</text>')

    n = min(6, max(1, len(rows)))
    step = min(84, 456 // n)
    y = 104
    for label, pct, pct_str, tag, color in rows[:6]:
        ic = _pick_icon(label)
        bw = max(14, min(580, int(580 * pct / 100.0)))
        parts.append(icon_badge(ic, 66, y - 4, 20, color))
        parts.append(f'<text x="102" y="{y + 2}" font-family="var(--f-s)" font-size="16" fill="{C_INK}" font-weight="700">{esc(_short(label, 22))}</text>')
        parts.append(f'<rect x="490" y="{y - 18}" width="580" height="28" rx="14" fill="{C_PAPER2}"/>')
        parts.append(f'<rect x="490" y="{y - 18}" width="{bw}" height="28" rx="14" fill="{color}"/>')
        parts.append(f'<text x="1156" y="{y + 2}" font-family="var(--f-m)" font-size="18" fill="{color}" text-anchor="end" font-weight="700">{esc(pct_str)}</text>')
        parts.append(f'<rect x="1186" y="{y - 18}" width="240" height="32" rx="16" fill="{C_PAPER}" stroke="{color}" stroke-width="1.3"/>')
        parts.append(f'<text x="1306" y="{y + 3}" font-family="var(--f-m)" font-size="13" fill="{C_INK}" font-weight="700" text-anchor="middle">{esc(_short(tag, 18))}</text>')
        y += step

    parts.append(f'<rect x="16" y="568" width="1440" height="56" rx="14" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="48" y="602" font-family="var(--f-r)" font-size="18.5" fill="{C_PAPER}" font-weight="700">{esc(_short(footer_text, 62))}</text>')
    parts.append("</svg>")
    return "".join(parts)


def svg_closing(
    kicker: str,
    headline: str,
    pillars: list[tuple[str, str, str]],
    open_question: str,
    next_talk: str,
) -> str:
    """Visual Closing Slide with Hero Banner, 3 Large-Icon Pillar Cards, and Bold Action Strip."""
    parts = ['<svg class="fig" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("vcl")]

    # Hero Headline Banner
    parts.append(f'<rect x="16" y="14" width="1440" height="128" rx="22" fill="{C_TEAL_D}"/>')
    parts.append(icon_badge("rocket", 88, 78, 32, C_PAPER, "#27697D"))
    parts.append(f'<text x="142" y="58" font-family="var(--f-m)" font-size="12.5" fill="#A6CFD5" letter-spacing="2.2" font-weight="700">{esc(_short(kicker, 56))}</text>')
    parts.append(f'<text x="142" y="106" font-family="var(--f-r)" font-size="31" fill="{C_PAPER}" font-weight="700">{esc(_short(headline, 36))}</text>')

    # 3 Large Visual Icon Pillars
    n = len(pillars)
    gap = 24
    pw = (1440 - gap * (n - 1)) // n
    accents = [C_TEAL, C_GREEN, C_CLAY]
    ics = ["terminal", "plug", "trophy"]
    for i, (pt, p1, p2) in enumerate(pillars):
        px = 16 + i * (pw + gap)
        cx = px + pw / 2
        acc = accents[i % len(accents)]
        ic = _pick_icon(pt + " " + p1, ics[i % len(ics)])
        parts.append(f'<rect x="{px}" y="168" width="{pw}" height="266" rx="20" fill="{C_WHITE}" stroke="{acc}" stroke-width="2" filter="url(#vcl_sh)"/>')
        parts.append(f'<rect x="{px}" y="168" width="{pw}" height="9" rx="4" fill="{acc}"/>')
        parts.append(icon_badge(ic, cx, 236, 34, acc))
        parts.append(f'<text x="{cx:.1f}" y="302" font-family="var(--f-r)" font-size="21" fill="{C_INK}" font-weight="700" text-anchor="middle">{esc(_short(pt, 18))}</text>')
        parts.append(f'<rect x="{px + 24}" y="324" width="{pw - 48}" height="42" rx="10" fill="{C_PAPER}"/>')
        parts.append(f'<text x="{cx:.1f}" y="350" font-family="var(--f-m)" font-size="13" fill="{C_INK2}" font-weight="600" text-anchor="middle">{esc(_short(p1, 32))}</text>')
        parts.append(f'<text x="{cx:.1f}" y="402" font-family="var(--f-s)" font-size="13.5" fill="{acc}" font-weight="700" text-anchor="middle">{esc(_short(p2, 26))}</text>')

    # Open Question Highlight Ribbon
    parts.append(f'<rect x="16" y="460" width="1440" height="84" rx="18" fill="{C_CLAY_L}" stroke="{C_CLAY}" stroke-width="1.8"/>')
    parts.append(icon_badge("bolt", 70, 502, 22, C_CLAY, C_WHITE))
    parts.append(f'<text x="110" y="509" font-family="var(--f-r)" font-size="20" fill="{C_INK}" font-weight="700">{esc(_short(open_question, 54))}</text>')

    # Footer Repo & Next Talk Pointer
    parts.append(f'<rect x="16" y="566" width="1440" height="54" rx="14" fill="{C_PAPER2}" stroke="{C_LINE}" stroke-width="1.2"/>')
    parts.append(f'<text x="42" y="599" font-family="var(--f-m)" font-size="13.5" fill="{C_TEAL}" font-weight="700">{esc(_short(next_talk, 64))}</text>')
    parts.append(f'<text x="1426" y="599" font-family="var(--f-m)" font-size="12.5" fill="{C_MUTE}" text-anchor="end">github.com/digoal/agent-body · Apache-2.0</text>')
    parts.append("</svg>")
    return "".join(parts)


# ==============================================================================
# HTML STAGE CSS + INTERACTIVE JS ENGINE (Light Paper Blueprint, 72% Visual Stage)
# ==============================================================================

CSS_TEMPLATE = """\
:root{
  --paper:#FAF8F4; --paper2:#F2EFE9; --line:#DCD7CC; --ink:#162A33; --ink2:#445A63;
  --mute:#7D8B88; --teal:#1F5E73; --teal-l:#E6F2F4; --teal-m:#7DB0BC;
  --clay:#C85A32; --clay-l:#FCEEE8; --green:#257A60; --gold:#B5852A;
  --f-r:"Songti SC","STSong","Songti TC","Noto Serif CJK SC",Georgia,serif;
  --f-s:"PingFang SC","Hiragino Sans GB","Noto Sans CJK SC","Microsoft YaHei",sans-serif;
  --f-m:"SF Mono",Menlo,Consolas,"Courier New",monospace;
}
*{box-sizing:border-box;margin:0;padding:0}
html,body{height:100%;background:#E7E4DC;overflow:hidden;color:var(--ink);
  font-family:var(--f-s);-webkit-font-smoothing:antialiased}
#stageArea{position:absolute;left:0;top:0;height:100%;display:flex;align-items:center;
  justify-content:center;transition:width .34s cubic-bezier(.4,0,.2,1)}
#deck{width:1600px;height:958px;flex:0 0 auto;position:relative;transform-origin:center center;
  box-shadow:0 16px 48px rgba(22,42,51,.12)}
.slide{position:absolute;left:0;top:0;width:1600px;height:900px;background:var(--paper);
  background-image:radial-gradient(#DDD8CC .9px,transparent .9px);background-size:28px 28px;
  overflow:hidden;opacity:0;visibility:hidden;transition:opacity .28s ease}
.slide.on{opacity:1;visibility:visible}
.hd{position:absolute;left:64px;top:24px;width:1472px;display:flex;flex-direction:column;gap:4px}
.hd-top{display:flex;align-items:center;justify-content:space-between;gap:16px}
.eyebrow{display:inline-block;font-family:var(--f-m);font-size:11.5px;letter-spacing:2px;
  color:var(--teal);background:var(--teal-l);padding:3px 12px;border-radius:12px;font-weight:700}
.lead-pill{font-size:14px;color:var(--ink2);max-width:920px;white-space:nowrap;overflow:hidden;
  text-overflow:ellipsis;text-align:right}
.lead-pill b{color:var(--teal);font-weight:700}
.title{font-family:var(--f-r);font-size:35px;font-weight:700;line-height:1.18;letter-spacing:.3px;
  color:var(--ink);margin-top:4px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.rule{position:absolute;left:64px;top:120px;width:1472px;height:2px;
  background:linear-gradient(90deg,var(--teal) 0%,var(--line) 32%,var(--line) 100%)}
.fig{position:absolute;left:64px;top:134px;width:1472px;height:640px;overflow:visible}
.fig.cover-svg{left:0;top:0;width:1600px;height:900px}
@keyframes flowDash{to{stroke-dashoffset:-28}}
.flow-line{animation:flowDash 1.6s linear infinite}
.fig.anim [data-step]{transition:opacity .38s ease,filter .38s ease}
.fig.anim [data-step].off{opacity:.18;filter:saturate(.35)}
.animbar{position:absolute;left:64px;top:786px;width:1472px;height:38px;display:flex;
  align-items:center;justify-content:space-between;background:rgba(255,255,255,.88);
  border:1px solid var(--line);border-radius:12px;padding:0 14px}
.animcap{font-size:13px;color:var(--ink);display:flex;align-items:center;gap:10px;
  white-space:nowrap;overflow:hidden;text-overflow:ellipsis;max-width:1080px}
.animcap .atag{font-family:var(--f-m);font-size:10px;letter-spacing:1.4px;color:var(--paper);
  background:var(--teal);padding:2px 8px;border-radius:10px;font-style:normal;font-weight:700}
.animbtns{display:flex;align-items:center;gap:6px}
.stepno{font-family:var(--f-m);font-size:11.5px;color:var(--mute);margin-right:8px;
  display:flex;align-items:center;gap:4px}
.stepin{width:38px;font-family:var(--f-m);font-size:11.5px;color:var(--teal);text-align:center;
  border:1px solid var(--line);border-radius:6px;background:var(--paper);padding:2px 0;-moz-appearance:textfield}
.stepin::-webkit-outer-spin-button,.stepin::-webkit-inner-spin-button{-webkit-appearance:none;margin:0}
.animbtns button{font-family:var(--f-m);font-size:11px;padding:4px 11px;border:1px solid var(--line);
  border-radius:8px;background:var(--paper);color:var(--ink2);cursor:pointer;font-weight:600}
.animbtns button:hover{background:var(--teal-l);border-color:var(--teal-m);color:var(--teal)}
.animbtns button.on{background:var(--teal);border-color:var(--teal);color:var(--paper)}
.nextbar{position:absolute;left:64px;top:788px;width:1472px;height:34px;display:flex;
  align-items:center;gap:12px;padding:0 14px;background:rgba(255,255,255,.78);
  border:1px solid var(--line);border-left:4px solid var(--clay);border-radius:8px}
.nb-lbl{font-family:var(--f-m);font-size:10.5px;letter-spacing:1.8px;color:var(--clay);
  font-weight:700;white-space:nowrap}
.nextbar p{font-size:13.5px;color:var(--ink2);white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.foot{position:absolute;left:64px;top:840px;width:1472px;display:flex;
  justify-content:space-between;align-items:center;border-top:1px solid var(--line);
  padding-top:10px;font-family:var(--f-m);font-size:11px;color:var(--mute);letter-spacing:1.2px}
.foot .pnum{color:var(--teal);font-weight:700}
#ctrl{position:absolute;left:0;top:900px;width:1600px;height:58px;background:var(--paper2);
  border-top:1px solid var(--line);display:flex;align-items:center;gap:20px;padding:0 28px;
  font-family:var(--f-m);font-size:11.5px;color:var(--ink2);letter-spacing:.6px}
.brand{color:var(--teal);letter-spacing:2px;font-size:11px;font-weight:700;text-decoration:none}
.brand:hover{text-decoration:underline}
.pos{color:var(--mute)}
.pos b{color:var(--ink);font-weight:700}
.prog{flex:1;height:4px;background:#E2DED5;border-radius:2px;position:relative;min-width:120px;overflow:hidden}
.prog i{position:absolute;left:0;top:0;height:4px;background:var(--teal);width:7%;
  border-radius:2px;transition:width .3s ease}
.keys{display:flex;align-items:center;gap:14px}
.k{display:flex;align-items:center;gap:6px;cursor:pointer;color:var(--ink2);white-space:nowrap}
.k:hover{color:var(--teal)}
.kbd{display:inline-block;min-width:22px;text-align:center;border:1px solid var(--line);
  border-radius:5px;background:var(--paper);padding:2px 6px;font-size:10.5px;color:var(--ink);font-weight:700}
.k.on .kbd{background:var(--teal);border-color:var(--teal);color:var(--paper)}
#panel{position:fixed;right:0;top:0;height:100%;width:500px;background:var(--paper2);
  border-left:1px solid var(--line);transform:translateX(100%);
  transition:transform .34s cubic-bezier(.4,0,.2,1);display:flex;flex-direction:column;z-index:40}
#panel.open{transform:translateX(0)}
.ph{padding:18px 24px 12px;border-bottom:1px solid var(--line);display:flex;
  justify-content:space-between;align-items:flex-end;gap:12px}
.ph h3{font-family:var(--f-r);font-size:18px;font-weight:700;letter-spacing:.4px}
.ph .tabs{display:flex;gap:12px;font-family:var(--f-m);font-size:11px;color:var(--mute)}
.ph .tabs button{background:none;border:none;font:inherit;color:inherit;cursor:pointer;
  padding:2px 0;border-bottom:2px solid transparent;font-weight:600}
.ph .tabs button.on{color:var(--teal);border-bottom-color:var(--teal)}
.pb{flex:1;overflow-y:auto;padding:22px 26px 80px;scroll-behavior:smooth}
.pb::-webkit-scrollbar{width:8px}
.pb::-webkit-scrollbar-thumb{background:#D3CEC3;border-radius:4px}
.pb p{font-size:14.5px;line-height:2.0;color:var(--ink);margin-bottom:14px;text-align:justify}
.pb .nxt{background:var(--clay-l);border-left:3px solid var(--clay);padding:10px 14px;
  border-radius:8px;font-size:13px;color:var(--ink);margin-top:16px}
.pb .sec{font-family:var(--f-m);font-size:10.5px;letter-spacing:2px;color:var(--mute);
  margin:24px 0 8px;padding-top:14px;border-top:1px solid var(--line)}
.pb .sec:first-child{margin-top:0;padding-top:0;border:none}
.pb .sec b{display:block;font-family:var(--f-r);font-size:16px;color:var(--ink);
  letter-spacing:0;margin-top:6px;font-weight:700}
.pb .sec.cur{background:var(--teal-l);padding:10px 12px;border-left:3px solid var(--teal);border-radius:6px}
.pb .meta{margin-bottom:16px}
.pb .meta .e{font-family:var(--f-m);font-size:10.5px;letter-spacing:1.8px;color:var(--teal);font-weight:700}
.pb .meta .t{font-family:var(--f-r);font-size:20px;font-weight:700;margin-top:6px;line-height:1.38}
#ov{position:fixed;inset:0;background:rgba(22,42,51,.42);display:none;
  align-items:center;justify-content:center;z-index:60;padding:40px}
#ov.open{display:flex}
.card{background:var(--paper);border:1px solid var(--line);border-radius:18px;padding:36px 42px;
  max-height:82vh;overflow:auto;box-shadow:0 24px 64px rgba(22,42,51,.2)}
.card h3{font-family:var(--f-r);font-size:25px;font-weight:700;margin-bottom:6px}
.card .sub{font-family:var(--f-m);font-size:11px;letter-spacing:2px;color:var(--mute);margin-bottom:20px}
.toc{width:1160px}
.toc-grid{display:grid;grid-template-columns:1fr 1fr;column-gap:48px}
.toc-item{display:flex;gap:14px;align-items:baseline;padding:9px 10px;border-bottom:1px solid #EAE6DD;
  cursor:pointer;font-size:14px;color:var(--ink2);border-radius:8px}
.toc-item:hover{background:var(--teal-l);color:var(--teal)}
.toc-item.cur{background:var(--teal-l)}
.toc-item.cur .no{color:var(--clay);font-weight:700}
.toc-item .no{font-family:var(--f-m);font-size:11px;color:var(--mute);width:24px;flex:0 0 auto}
.toc-item .tt{font-family:var(--f-r);font-size:15px;color:var(--ink)}
.help{width:760px}
.hrow{display:flex;gap:18px;align-items:baseline;padding:9px 0;border-bottom:1px solid #EAE6DD;
  font-size:13px;color:var(--ink2)}
.hrow .kk{font-family:var(--f-m);font-size:11px;color:var(--ink);min-width:112px;
  border:1px solid var(--line);border-radius:6px;padding:2px 7px;text-align:center;background:var(--paper2)}
"""

JS_TEMPLATE = """\
const slides = [...document.querySelectorAll('.slide')];
const deck = document.getElementById('deck');
const stageArea = document.getElementById('stageArea');
const posCur = document.getElementById('posCur');
const progBar = document.getElementById('progBar');
const panel = document.getElementById('panel');
const pTitle = document.getElementById('pTitle');
const pb = document.getElementById('pb');
const ov = document.getElementById('ov');
let cur = 0, panelMode = null, autoTimer = null;
const animState = {};

function layout(){
  const w = panelMode ? Math.max(320, window.innerWidth - 500) : window.innerWidth;
  stageArea.style.width = w + 'px';
  const s = Math.min(w / 1600, window.innerHeight / 958);
  deck.style.transform = 'scale(' + s + ')';
}
window.addEventListener('resize', layout);

function maxSteps(idx){
  const el = slides[idx].querySelector('.fig.anim');
  if(!el) return 0;
  const nums = [...el.querySelectorAll('[data-step]')].map(n => +n.dataset.step);
  return nums.length ? Math.max(...nums) : 0;
}
function setStep(idx, step){
  const el = slides[idx].querySelector('.fig.anim');
  if(!el) return;
  const max = maxSteps(idx);
  const s = Math.max(0, Math.min(max, step));
  animState[idx] = s;
  el.querySelectorAll('[data-step]').forEach(n => {
    n.classList.toggle('off', s > 0 && +n.dataset.step > s);
  });
  const inp = slides[idx].querySelector('.stepin');
  if(inp) inp.value = s || max;
  const cap = slides[idx].querySelector('.animcap');
  const list = CAPS[String(idx)] || [];
  if(cap){
    if(s === max && !animTimers[idx]){
      const txt = list[list.length - 1] || '全貌图解 · 点击「播放」或按 A 逐步演绎';
      cap.innerHTML = '<i class="atag">图解动画</i>' + txt;
    } else if(list.length){
      const txt = list[Math.max(0, (s || 1) - 1)] || list[0];
      cap.innerHTML = '<i class="atag">Step ' + (s || 1) + '/' + max + '</i>' + txt;
    }
  }
}
const animTimers = {};
function animPause(idx){
  if(animTimers[idx]){ clearInterval(animTimers[idx]); animTimers[idx] = null; }
  const b = slides[idx].querySelector('[data-act="play"]');
  if(b) b.classList.remove('on');
}
function animPlay(idx){
  const max = maxSteps(idx);
  if(!max) return;
  if(animTimers[idx]){ animPause(idx); return; }
  const b = slides[idx].querySelector('[data-act="play"]');
  if(b) b.classList.add('on');
  setStep(idx, 1);
  animTimers[idx] = setInterval(() => {
    const nxt = (animState[idx] || 1) + 1;
    if(nxt > max){ animPause(idx); setStep(idx, max); return; }
    setStep(idx, nxt);
  }, 1700);
}
function animStep(idx){
  animPause(idx);
  const max = maxSteps(idx);
  if(!max) return;
  const nxt = ((animState[idx] || max) % max) + 1;
  setStep(idx, nxt);
}
slides.forEach((sl, idx) => {
  const bar = sl.querySelector('.animbar');
  if(!bar) return;
  const max = maxSteps(idx);
  // Default to FULL CLEAR VIEW (no washed-out ghost slide on arrival)
  setStep(idx, max);
  bar.querySelector('[data-act="play"]').addEventListener('click', () => animPlay(idx));
  bar.querySelector('[data-act="pause"]').addEventListener('click', () => animPause(idx));
  bar.querySelector('[data-act="step"]').addEventListener('click', () => animStep(idx));
  bar.querySelector('[data-act="reset"]').addEventListener('click', () => { animPause(idx); setStep(idx, 1); });
  const inp = bar.querySelector('.stepin');
  inp.addEventListener('change', () => { animPause(idx); setStep(idx, +inp.value || 1); });
});

function fmtParas(s){
  return s.split('\\n').filter(Boolean).map(p => '<p>' + p + '</p>').join('');
}
function renderPanel(){
  if(!panelMode) return;
  document.querySelectorAll('.ph .tabs button[data-mode]').forEach(b => {
    b.classList.toggle('on', b.dataset.mode === panelMode);
  });
  document.querySelector('.k[data-key="n"]').classList.toggle('on', panelMode === 'page');
  document.querySelector('.k[data-key="t"]').classList.toggle('on', panelMode === 'all');
  if(panelMode === 'page'){
    pTitle.textContent = '本页讲稿';
    const m = META[cur];
    const nxt = m.n ? '<div class="nxt"><b>承接下一页：</b>' + m.n + '</div>' : '';
    pb.innerHTML = '<div class="meta"><div class="e">' + String(cur+1).padStart(2,'0') + ' / ' + String(slides.length).padStart(2,'0') + ' · ' + m.e + '</div><div class="t">' + m.t + '</div></div>' + fmtParas(m.s) + nxt;
    pb.scrollTop = 0;
  } else {
    pTitle.textContent = '全文讲稿';
    pb.innerHTML = META.map((m, i) => '<div class="sec' + (i===cur?' cur':'') + '" id="sec-' + i + '">' + String(i+1).padStart(2,'0') + ' · ' + m.e + '<b>' + m.t + '</b></div>' + fmtParas(m.s)).join('');
    const target = document.getElementById('sec-' + cur);
    if(target) target.scrollIntoView({block: 'start', behavior: 'smooth'});
  }
}
function openPanel(mode){
  panelMode = mode;
  panel.classList.add('open');
  renderPanel();
  layout();
}
function closePanel(){
  panelMode = null;
  panel.classList.remove('open');
  document.querySelector('.k[data-key="n"]').classList.remove('on');
  document.querySelector('.k[data-key="t"]').classList.remove('on');
  layout();
}
document.querySelectorAll('.ph .tabs button').forEach(b => {
  b.addEventListener('click', () => {
    if(b.dataset.mode === 'close') return closePanel();
    openPanel(b.dataset.mode);
  });
});

function go(idx){
  const nxt = Math.max(0, Math.min(slides.length - 1, idx));
  animPause(cur);
  slides[cur].classList.remove('on');
  cur = nxt;
  slides[cur].classList.add('on');
  posCur.textContent = String(cur + 1).padStart(2, '0');
  progBar.style.width = (((cur + 1) / slides.length) * 100).toFixed(2) + '%';
  renderPanel();
  layout();
}
function next(){ if(cur < slides.length - 1) go(cur + 1); }
function prev(){ if(cur > 0) go(cur - 1); }

function showOv(kind){
  if(kind === 'toc'){
    const half = Math.ceil(META.length / 2);
    ov.innerHTML = '<div class="card toc"><h3>目录</h3><div class="sub">共 ' + META.length + ' 页 · 点击跳转 · ESC 关闭</div><div class="toc-grid">'
      + [0, 1].map(c => '<div>' + META.slice(c * half, (c + 1) * half).map((m, k) => {
          const i = c * half + k;
          return '<div class="toc-item' + (i === cur ? ' cur' : '') + '" data-go="' + i + '"><span class="no">'
            + String(i + 1).padStart(2, '0') + '</span><span class="tt">' + m.t + '</span></div>';
        }).join('') + '</div>').join('')
      + '</div></div>';
    ov.querySelectorAll('[data-go]').forEach(el => el.addEventListener('click', () => {
      go(+el.dataset.go); hideOv();
    }));
  } else {
    const rows = [
      ['→ / 空格', '下一页'], ['←', '上一页'], ['G', '目录'],
      ['N', '本页讲稿（按一下显示，再按关闭）'], ['T', '全文讲稿（自动定位当前页，翻页联动）'],
      ['P', '自动播放'], ['A', '播放 / 暂停本页动画'], ['S', '动画单步'],
      ['R', '动画重置至第 1 步'], ['F', '全屏（底部控制条保持可见）'],
      ['1 – 9 · 0', '跳到第 1–10 页'], ['Home / End', '首页 / 末页'],
      ['Esc', '关闭浮层与讲稿'], ['?', '本帮助']
    ];
    ov.innerHTML = '<div class="card help"><h3>快捷键</h3><div class="sub">全屏后依然可用 · 控制条常驻页面底部</div>'
      + rows.map(r => '<div class="hrow"><span class="kk">' + r[0] + '</span><span>' + r[1] + '</span></div>').join('')
      + '</div>';
  }
  ov.classList.add('open');
}
function hideOv(){ ov.classList.remove('open'); }
ov.addEventListener('click', e => { if(e.target === ov) hideOv(); });

function toggleFS(){
  const k = document.querySelector('.k[data-key="f"]');
  if(!document.fullscreenElement){
    (document.documentElement.requestFullscreen || document.documentElement.webkitRequestFullscreen || function(){}).call(document.documentElement);
    k.classList.add('on');
  } else {
    (document.exitFullscreen || document.webkitExitFullscreen || function(){}).call(document);
    k.classList.remove('on');
  }
}
document.addEventListener('fullscreenchange', () => {
  const k = document.querySelector('.k[data-key="f"]');
  if(k) k.classList.toggle('on', !!document.fullscreenElement);
  layout();
});
function toggleAuto(){
  const k = document.querySelector('.k[data-key="p"]');
  if(autoTimer){ clearInterval(autoTimer); autoTimer = null; k.classList.remove('on'); return; }
  k.classList.add('on');
  autoTimer = setInterval(() => {
    if(cur < slides.length - 1) next();
    else { clearInterval(autoTimer); autoTimer = null; k.classList.remove('on'); }
  }, 18000);
}
function jumpFromKey(e){
  if(e.key === '0'){ go(9); return true; }
  const n = parseInt(e.key, 10);
  if(n >= 1 && n <= 9){ go(n - 1); return true; }
  return false;
}
window.addEventListener('keydown', e => {
  if(e.metaKey || e.ctrlKey || e.altKey) return;
  if(e.target && e.target.tagName === 'INPUT') return;
  const k = e.key.toLowerCase();
  if(k === 'arrowright' || k === 'arrowdown' || k === 'pagedown' || k === ' '){ e.preventDefault(); next(); return; }
  if(k === 'arrowleft' || k === 'arrowup' || k === 'pageup'){ e.preventDefault(); prev(); return; }
  if(k === 'home'){ go(0); return; }
  if(k === 'end'){ go(slides.length - 1); return; }
  if(k === 'f'){ e.preventDefault(); toggleFS(); return; }
  if(k === 'g'){ e.preventDefault(); (ov.classList.contains('open') ? hideOv() : showOv('toc')); return; }
  if(k === 'n'){ e.preventDefault(); (panelMode === 'page' ? closePanel() : openPanel('page')); return; }
  if(k === 't'){ e.preventDefault(); (panelMode === 'all' ? closePanel() : openPanel('all')); return; }
  if(k === 'p'){ e.preventDefault(); toggleAuto(); return; }
  if(k === 'a'){ e.preventDefault(); animPlay(cur); return; }
  if(k === 's'){ e.preventDefault(); animStep(cur); return; }
  if(k === 'r'){ e.preventDefault(); animPause(cur); setStep(cur, 1); return; }
  if(k === '?' || (k === '/' && e.shiftKey)){ e.preventDefault(); (ov.classList.contains('open') ? hideOv() : showOv('help')); return; }
  if(k === 'escape'){ hideOv(); if(panelMode) closePanel(); return; }
  if(jumpFromKey(e)){ e.preventDefault(); }
});
document.querySelectorAll('.k').forEach(el => {
  el.addEventListener('click', () => {
    const k = el.dataset.key;
    if(k === 'g') (ov.classList.contains('open') ? hideOv() : showOv('toc'));
    if(k === 'n') panelMode === 'page' ? closePanel() : openPanel('page');
    if(k === 't') panelMode === 'all' ? closePanel() : openPanel('all');
    if(k === 'p') toggleAuto();
    if(k === 'f') toggleFS();
    if(k === '?') (ov.classList.contains('open') ? hideOv() : showOv('help'));
  });
});
const qs = new URLSearchParams(location.search);
const qp = qs.get('p') || (location.hash.startsWith('#p=') ? location.hash.slice(3) : null);
go(qp ? Math.max(0, +qp - 1) : 0);
if(qs.get('panel')) setTimeout(() => openPanel(qs.get('panel')), 120);
if(qs.get('ov')) setTimeout(() => showOv(qs.get('ov')), 120);
"""


def _strip_html_short(raw: str, max_len: int = 44) -> str:
    """Shorten header lead to a clean single-line pill so it never crowds the visual."""
    plain = re.sub(r"<[^>]+>", "", raw or "").strip()
    if len(plain) <= max_len:
        return raw or ""
    # Take first crisp clause
    for sep in ("；", "。", "——", "："):
        pos = plain.find(sep)
        if 10 <= pos <= max_len:
            return esc(plain[:pos])
    return esc(plain[: max_len - 1] + "…")


def render_deck(
    doc_title: str,
    brand_label: str,
    foot_label: str,
    slides_data: list[dict[str, Any]],
    out_path: Path,
) -> None:
    """Render a complete self-contained Visual-First Blueprint HTML slide deck."""
    total = len(slides_data)
    sections: list[str] = []
    meta_list: list[dict[str, str]] = []
    caps_map: dict[str, list[str]] = {}

    for idx, sl in enumerate(slides_data):
        on_cls = " on" if idx == 0 else ""
        is_cover = sl.get("cover", False)
        pnum = f"{idx + 1:02d} / {total:02d}"
        meta_list.append({
            "e": sl.get("eyebrow", "封面"),
            "t": sl.get("title", doc_title),
            "s": sl.get("script", ""),
            "n": sl.get("next", ""),
        })
        if sl.get("anim_caps"):
            caps_map[str(idx)] = sl["anim_caps"]

        if is_cover:
            sec = (
                f'<section class="slide{on_cls}" data-i="{idx}">'
                f'{sl["svg"]}'
                f'<div class="foot"><span>{esc(foot_label)}</span><span class="pnum">{pnum}</span></div>'
                f"</section>"
            )
        else:
            has_anim = bool(sl.get("anim_caps"))
            anim_html = ""
            next_html = ""
            if has_anim:
                caps = sl["anim_caps"]
                anim_html = (
                    f'<div class="animbar">'
                    f'<div class="animcap"><i class="atag">图解动画</i>{esc(caps[-1])}</div>'
                    f'<div class="animbtns">'
                    f'<span class="stepno"><input class="stepin" type="number" min="1" max="{len(caps)}" value="{len(caps)}"> / {len(caps)}</span>'
                    f'<button data-act="play">播放</button><button data-act="pause">暂停</button>'
                    f'<button data-act="step">单步</button><button data-act="reset">重置</button>'
                    f"</div></div>"
                )
            elif sl.get("next"):
                next_html = f'<div class="nextbar"><span class="nb-lbl">NEXT →</span><p>{esc(sl["next"])}</p></div>'

            lead_html = ""
            if sl.get("lead"):
                lead_html = f'<div class="lead-pill">{_strip_html_short(sl["lead"], 54)}</div>'

            sec = (
                f'<section class="slide{on_cls}" data-i="{idx}">'
                f'<div class="hd">'
                f'<div class="hd-top"><span class="eyebrow">{esc(sl["eyebrow"])}</span>{lead_html}</div>'
                f'<h2 class="title">{esc(sl["title"])}</h2>'
                f"</div>"
                f'<div class="rule"></div>'
                f'{sl["svg"]}'
                f"{next_html}"
                f"{anim_html}"
                f'<div class="foot"><span>{esc(foot_label)}</span><span class="pnum">{pnum}</span></div>'
                f"</section>"
            )
        sections.append(sec)

    html_doc = f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{esc(doc_title)}</title>
<style>
{CSS_TEMPLATE}
</style>
</head>
<body>
<div id="stageArea"><div id="deck">
{"".join(sections)}
<div id="ctrl">
  <a class="brand" href="index.html" title="返回系列总览">{esc(brand_label)}</a>
  <div class="pos"><b id="posCur">01</b> / {total:02d}</div>
  <div class="prog"><i id="progBar"></i></div>
  <div class="keys">
    <div class="k" data-key="g"><span class="kbd">G</span>目录</div>
    <div class="k" data-key="n"><span class="kbd">N</span>讲稿</div>
    <div class="k" data-key="t"><span class="kbd">T</span>全文</div>
    <div class="k" data-key="p"><span class="kbd">P</span>自动</div>
    <div class="k" data-key="f"><span class="kbd">F</span>全屏</div>
    <div class="k" data-key="?"><span class="kbd">?</span>帮助</div>
  </div>
</div>
</div></div>
<aside id="panel">
  <div class="ph">
    <h3 id="pTitle">本页讲稿</h3>
    <div class="tabs">
      <button data-mode="page" class="on">本页</button>
      <button data-mode="all">全文</button>
      <button data-mode="close">关闭</button>
    </div>
  </div>
  <div class="pb" id="pb"></div>
</aside>
<div id="ov"></div>
<script>
const META = {json.dumps(meta_list, ensure_ascii=False)};
const CAPS = {json.dumps(caps_map, ensure_ascii=False)};
{JS_TEMPLATE}
</script>
</body>
</html>
"""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(html_doc, encoding="utf-8")
