#!/usr/bin/env python3
"""Shared '地层剖面 · 工程蓝图' (Geological Stratum / Architectural Blueprint)
HTML slide deck engine for Agent Body presentation series.

Matches presentation/presentation.html and presentation/agent-body-deck.html 100%
in visual tokens, 1600x900 stage layout, bottom control bar, right speaker-notes
drawer (N/T), TOC modal (G), step animations (P/A/S/R), and zero external deps.
"""

from __future__ import annotations

import html
import json
from pathlib import Path
from typing import Any

# Exact color tokens from presentation.html / agent-body-deck.html
C_PAPER = "#F7F6F2"
C_PAPER2 = "#F1EFE9"
C_LINE = "#D9D5CB"
C_INK = "#1B2B33"
C_INK2 = "#46585F"
C_MUTE = "#89938F"
C_TEAL = "#245A6B"
C_TEAL_D = "#1E4B5A"
C_TEAL_2 = "#2E6373"
C_TEAL_3 = "#47808D"
C_TEAL_L = "#E5EFF0"
C_TEAL_M = "#7BA6AF"
C_CLAY = "#BE653B"
C_CLAY_L = "#F7E9DE"
C_GREEN = "#2F7A66"
C_GREEN_L = "#E4F0EA"
C_GOLD = "#AE8A3E"
C_GOLD_L = "#F6F0DF"
C_WHITE = "#FFFFFF"


def esc(text: str) -> str:
    return html.escape(str(text), quote=True)


def svg_defs(marker_id: str) -> str:
    return (
        f'<defs><marker id="{marker_id}" viewBox="0 0 10 10" refX="8.4" refY="5" '
        f'markerWidth="9" markerHeight="9" orient="auto-start-reverse">'
        f'<path d="M 0 0.6 L 9.4 5 L 0 9.4 z" fill="{C_INK2}"/></marker></defs>'
    )


def svg_cover(
    kicker: str,
    title: str,
    en_sub: str,
    subtitle1: str,
    subtitle2: str,
    spec_line: str,
    strata: list[tuple[str, str, str, str]],
    right_tag: str = "地层剖面 · 工程蓝图 · 30 分钟专题",
) -> str:
    """Cover SVG (1600x900) matching presentation.html cover geometry."""
    title_fs = 46 if len(title) > 12 else 56
    en_fs = 16.5 if len(en_sub) > 44 else 20
    parts = [
        '<svg class="fig cover-svg" viewBox="0 0 1600 900" width="1600" height="900" xmlns="http://www.w3.org/2000/svg">',
        f'<line x1="96" y1="140" x2="1500" y2="140" stroke="{C_LINE}" stroke-width="1"/>',
        f'<text x="96" y="218" font-family="var(--f-m)" font-size="13" fill="{C_MUTE}" letter-spacing="3.0">{esc(kicker)}</text>',
        f'<text x="96" y="310" font-family="var(--f-r)" font-size="{title_fs}" fill="{C_INK}" font-weight="600" letter-spacing="1.0">{esc(title)}</text>',
        f'<text x="96" y="362" font-family="var(--f-m)" font-size="{en_fs}" fill="{C_TEAL}" letter-spacing="0.6">{esc(en_sub)}</text>',
        f'<line x1="96" y1="398" x2="760" y2="398" stroke="{C_LINE}" stroke-width="1"/>',
        f'<text x="96" y="446" font-family="var(--f-s)" font-size="21" fill="{C_INK}" font-weight="600">{esc(subtitle1)}</text>',
        f'<text x="96" y="484" font-family="var(--f-s)" font-size="18.5" fill="{C_INK2}">{esc(subtitle2)}</text>',
        f'<text x="96" y="562" font-family="var(--f-m)" font-size="13.5" fill="{C_MUTE}">{esc(spec_line)}</text>',
        f'<text x="96" y="806" font-family="var(--f-s)" font-size="14" fill="{C_MUTE}">digoal 德哥 · 2026-11</text>',
        f'<text x="1500" y="806" font-family="var(--f-s)" font-size="12" fill="{C_MUTE}" text-anchor="end">{esc(right_tag)}</text>',
    ]
    n = len(strata)
    gap = 10
    total_h = 566
    bh = (total_h - gap * (n - 1)) // n
    y = 148
    for idx, (st_title, st_sub, bg, fg) in enumerate(strata, 1):
        parts.append(f'<rect x="880" y="{y}" width="620" height="{bh}" fill="{bg}" stroke="{bg}" stroke-width="1"/>')
        parts.append(
            f'<text x="904" y="{y + int(bh * 0.44)}" font-family="var(--f-r)" font-size="19" fill="{fg}" font-weight="600">{esc(st_title)}</text>'
        )
        parts.append(
            f'<text x="904" y="{y + int(bh * 0.76)}" font-family="var(--f-m)" font-size="11.5" fill="{fg}" opacity="0.82">{esc(st_sub)}</text>'
        )
        parts.append(
            f'<text x="1476" y="{y + 28}" font-family="var(--f-m)" font-size="11" fill="{fg}" text-anchor="end" opacity="0.58">{idx:02d}</text>'
        )
        y += bh + gap
    parts.append("</svg>")
    return "".join(parts)


def svg_columns(
    cols: list[dict[str, Any]],
    bottom_banner: tuple[str, str, str] | None = None,
    animated: bool = False,
) -> str:
    """2~5 equal-width vertical blueprint cards + optional bottom takeaway banner."""
    n = len(cols)
    gap = 20 if n <= 4 else 16
    w = (1400 - gap * (n - 1)) // n
    h = 336 if bottom_banner else 396
    cls = "fig anim" if animated else "fig"
    parts = [f'<svg class="{cls}" viewBox="0 0 1400 424" width="1400" height="424" xmlns="http://www.w3.org/2000/svg">']
    for i, c in enumerate(cols):
        x = i * (w + gap)
        accent = c.get("accent", C_TEAL_D)
        bg = c.get("bg", C_WHITE)
        border = c.get("border", C_LINE)
        step_open = f'<g data-step="{i + 1}">' if animated else "<g>"
        parts.append(step_open)
        parts.append(f'<rect x="{x}" y="14" width="{w}" height="{h}" fill="{bg}" stroke="{border}" stroke-width="1"/>')
        parts.append(f'<rect x="{x}" y="14" width="{w}" height="5" fill="{accent}"/>')
        tag = c.get("tag", f"{i + 1:02d}")
        parts.append(
            f'<text x="{x + 18}" y="46" font-family="var(--f-m)" font-size="11" fill="{C_MUTE}" letter-spacing="1.4">{esc(tag)}</text>'
        )
        title_size = 18 if n <= 3 else 16
        parts.append(
            f'<text x="{x + 18}" y="78" font-family="var(--f-r)" font-size="{title_size}" fill="{C_INK}" font-weight="600">{esc(c["title"])}</text>'
        )
        parts.append(f'<line x1="{x + 18}" y1="94" x2="{x + w - 18}" y2="94" stroke="#EAE6DD" stroke-width="1"/>')
        ly = 124
        line_step = 30 if n <= 3 else 28
        body_fs = 13.5 if n <= 3 else 12.5
        for line in c.get("lines", []):
            is_hi = False
            txt = line
            if isinstance(line, tuple):
                txt, is_hi = line
            fill = accent if is_hi else C_INK2
            fw = "600" if is_hi else "400"
            parts.append(
                f'<text x="{x + 18}" y="{ly}" font-family="var(--f-s)" font-size="{body_fs}" fill="{fill}" font-weight="{fw}">{esc(txt)}</text>'
            )
            ly += line_step
        foot_lbl = c.get("foot_lbl")
        foot_val = c.get("foot_val")
        if foot_lbl or foot_val:
            fy = 14 + h - 54
            parts.append(f'<line x1="{x + 18}" y1="{fy - 10}" x2="{x + w - 18}" y2="{fy - 10}" stroke="{C_LINE}" stroke-width="0.8"/>')
            if foot_lbl:
                parts.append(
                    f'<text x="{x + 18}" y="{fy + 10}" font-family="var(--f-s)" font-size="11" fill="{C_GOLD}" font-weight="600">{esc(foot_lbl)}</text>'
                )
            if foot_val:
                parts.append(
                    f'<text x="{x + 18}" y="{fy + 32}" font-family="var(--f-m)" font-size="11.5" fill="{C_INK2}">{esc(foot_val)}</text>'
                )
        parts.append("</g>")
    if bottom_banner:
        b_txt, b_right, b_style = bottom_banner
        bg = C_CLAY_L if b_style == "clay" else (C_GREEN_L if b_style == "green" else C_TEAL_L)
        bar = C_CLAY if b_style == "clay" else (C_GREEN if b_style == "green" else C_TEAL)
        step_open = f'<g data-step="{n + 1}">' if animated else "<g>"
        parts.append(step_open)
        parts.append(f'<rect x="0" y="368" width="1400" height="46" fill="{bg}"/>')
        parts.append(f'<rect x="0" y="368" width="4" height="46" fill="{bar}"/>')
        parts.append(f'<text x="20" y="397" font-family="var(--f-r)" font-size="18.5" fill="{C_INK}" font-weight="600">{esc(b_txt)}</text>')
        if b_right:
            parts.append(f'<text x="1380" y="397" font-family="var(--f-m)" font-size="12" fill="{bar}" text-anchor="end">{esc(b_right)}</text>')
        parts.append("</g>")
    parts.append("</svg>")
    return "".join(parts)


def svg_pipeline(
    steps: list[dict[str, Any]],
    bottom_panels: list[dict[str, Any]] | None = None,
    animated: bool = False,
    mid: str = "ahp",
) -> str:
    """Horizontal arrowed pipeline on top + 2 or 3 detail panels below."""
    cls = "fig anim" if animated else "fig"
    parts = [f'<svg class="{cls}" viewBox="0 0 1400 424" width="1400" height="424" xmlns="http://www.w3.org/2000/svg">', svg_defs(mid)]
    n = len(steps)
    arrow_w = 34
    box_w = (1400 - arrow_w * (n - 1)) // n
    box_h = 148 if bottom_panels else 260
    y0 = 18 if bottom_panels else 64
    for i, st in enumerate(steps):
        x = i * (box_w + arrow_w)
        accent = st.get("accent", C_TEAL)
        bg = st.get("bg", C_WHITE)
        g_open = f'<g data-step="{i + 1}">' if animated else "<g>"
        parts.append(g_open)
        parts.append(f'<rect x="{x}" y="{y0}" width="{box_w}" height="{box_h}" fill="{bg}" stroke="{C_LINE}" stroke-width="1"/>')
        parts.append(f'<rect x="{x}" y="{y0}" width="{box_w}" height="4" fill="{accent}"/>')
        parts.append(
            f'<text x="{x + 16}" y="{y0 + 30}" font-family="var(--f-m)" font-size="11" fill="{accent}" font-weight="600">{esc(st.get("tag", f"STEP {i+1}"))}</text>'
        )
        parts.append(
            f'<text x="{x + 16}" y="{y0 + 58}" font-family="var(--f-r)" font-size="17" fill="{C_INK}" font-weight="600">{esc(st["title"])}</text>'
        )
        ly = y0 + 88
        for ln in st.get("lines", []):
            parts.append(f'<text x="{x + 16}" y="{ly}" font-family="var(--f-s)" font-size="12.5" fill="{C_INK2}">{esc(ln)}</text>')
            ly += 24
        if i < n - 1:
            ax1 = x + box_w + 2
            ax2 = x + box_w + arrow_w - 4
            ay = y0 + box_h // 2
            parts.append(f'<path d="M {ax1},{ay} L {ax2},{ay}" fill="none" stroke="{C_TEAL}" stroke-width="1.5" marker-end="url(#{mid})"/>')
        parts.append("</g>")

    if bottom_panels:
        m = len(bottom_panels)
        pgap = 22
        pw = (1400 - pgap * (m - 1)) // m
        py = 190
        ph = 218
        g_open = f'<g data-step="{n + 1}">' if animated else "<g>"
        parts.append(g_open)
        for j, p in enumerate(bottom_panels):
            px = j * (pw + pgap)
            accent = p.get("accent", C_TEAL)
            bg = p.get("bg", C_PAPER2)
            parts.append(f'<rect x="{px}" y="{py}" width="{pw}" height="{ph}" fill="{bg}" stroke="{accent}" stroke-width="1"/>')
            parts.append(f'<rect x="{px}" y="{py}" width="4" height="{ph}" fill="{accent}"/>')
            parts.append(
                f'<text x="{px + 20}" y="{py + 32}" font-family="var(--f-r)" font-size="17" fill="{accent}" font-weight="600">{esc(p["title"])}</text>'
            )
            ly = py + 64
            for ln in p.get("lines", []):
                is_mono = False
                txt = ln
                if isinstance(ln, tuple):
                    txt, is_mono = ln
                ff = "var(--f-m)" if is_mono else "var(--f-s)"
                fs = 12 if is_mono else 13
                fc = C_TEAL if is_mono else C_INK2
                parts.append(f'<text x="{px + 20}" y="{ly}" font-family="{ff}" font-size="{fs}" fill="{fc}">{esc(txt)}</text>')
                ly += 27
        parts.append("</g>")
    parts.append("</svg>")
    return "".join(parts)


def svg_compare_rows(
    headers: tuple[str, str, str],
    rows: list[tuple[str, str, str]],
    takeaway: tuple[str, str] | None = None,
) -> str:
    """3-column contrast matrix (left teal tag, middle white, right clay/green outcome)."""
    parts = ['<svg class="fig" viewBox="0 0 1400 424" width="1400" height="424" xmlns="http://www.w3.org/2000/svg">']
    parts.append(f'<text x="14" y="16" font-family="var(--f-s)" font-size="12" fill="{C_MUTE}" font-weight="600" letter-spacing="1.4">{esc(headers[0])}</text>')
    parts.append(f'<text x="326" y="16" font-family="var(--f-s)" font-size="12" fill="{C_MUTE}" font-weight="600" letter-spacing="1.4">{esc(headers[1])}</text>')
    parts.append(f'<text x="850" y="16" font-family="var(--f-s)" font-size="12" fill="{C_MUTE}" font-weight="600" letter-spacing="1.4">{esc(headers[2])}</text>')
    n = len(rows)
    row_h = min(60, (340 - 8 * (n - 1)) // max(1, n))
    y = 30
    for c1, c2, c3 in rows:
        parts.append(f'<rect x="0" y="{y}" width="296" height="{row_h}" fill="{C_TEAL_L}"/>')
        parts.append(f'<text x="16" y="{y + row_h // 2 + 5}" font-family="var(--f-s)" font-size="15" fill="{C_TEAL}" font-weight="600">{esc(c1)}</text>')
        parts.append(f'<rect x="312" y="{y}" width="508" height="{row_h}" fill="{C_WHITE}" stroke="{C_LINE}" stroke-width="1"/>')
        parts.append(f'<text x="328" y="{y + row_h // 2 + 5}" font-family="var(--f-s)" font-size="14" fill="{C_INK}">{esc(c2)}</text>')
        parts.append(f'<rect x="836" y="{y}" width="564" height="{row_h}" fill="{C_CLAY_L}"/>')
        parts.append(f'<text x="852" y="{y + row_h // 2 + 5}" font-family="var(--f-s)" font-size="14" fill="{C_CLAY}" font-weight="500">{esc(c3)}</text>')
        y += row_h + 8
    if takeaway:
        parts.append(f'<text x="0" y="404" font-family="var(--f-r)" font-size="19" fill="{C_INK}" font-weight="600">{esc(takeaway[0])}</text>')
        parts.append(f'<text x="1400" y="404" font-family="var(--f-s)" font-size="12.5" fill="{C_MUTE}" text-anchor="end">{esc(takeaway[1])}</text>')
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
    """Left grounded code block (file:line) + right architectural mechanism cards."""
    cls = "fig anim" if animated else "fig"
    parts = [f'<svg class="{cls}" viewBox="0 0 1400 424" width="1400" height="424" xmlns="http://www.w3.org/2000/svg">']
    # Left code panel
    parts.append('<g data-step="1">' if animated else "<g>")
    parts.append(f'<rect x="0" y="10" width="730" height="404" fill="#1B2B33" stroke="#1B2B33" stroke-width="1"/>')
    parts.append(f'<rect x="0" y="10" width="730" height="38" fill="#243842"/>')
    parts.append(f'<text x="18" y="34" font-family="var(--f-s)" font-size="13" fill="#E5EFF0" font-weight="600">{esc(left_title)}</text>')
    parts.append(f'<text x="712" y="34" font-family="var(--f-m)" font-size="11.5" fill="#7BA6AF" text-anchor="end">{esc(file_ref)}</text>')
    ly = 74
    step_y = min(24, 330 // max(1, len(code_lines)))
    for cl in code_lines:
        col = "#7BA6AF" if cl.strip().startswith("//") or cl.strip().startswith("#") else "#F7F6F2"
        if "fn " in cl or "pub struct" in cl or "pub enum" in cl or "SELECT" in cl or "CREATE TABLE" in cl:
            col = "#E8C98A"
        parts.append(f'<text x="18" y="{ly}" font-family="var(--f-m)" font-size="12" fill="{col}">{esc(cl)}</text>')
        ly += step_y
    parts.append("</g>")

    # Right mechanism cards
    parts.append(f'<text x="760" y="28" font-family="var(--f-m)" font-size="12" fill="{C_MUTE}" letter-spacing="1.6">{esc(right_title)}</text>')
    n = len(right_cards)
    gap = 14
    ch = (370 - gap * (n - 1)) // max(1, n)
    ry = 44
    accents = [C_TEAL_D, C_GREEN, C_CLAY, C_GOLD]
    for i, (ct, c1, c2) in enumerate(right_cards):
        acc = accents[i % len(accents)]
        g_open = f'<g data-step="{i + 2}">' if animated else "<g>"
        parts.append(g_open)
        parts.append(f'<rect x="760" y="{ry}" width="640" height="{ch}" fill="{C_WHITE}" stroke="{C_LINE}" stroke-width="1"/>')
        parts.append(f'<rect x="760" y="{ry}" width="4" height="{ch}" fill="{acc}"/>')
        parts.append(f'<text x="780" y="{ry + 30}" font-family="var(--f-r)" font-size="16.5" fill="{C_INK}" font-weight="600">{esc(ct)}</text>')
        parts.append(f'<text x="780" y="{ry + 58}" font-family="var(--f-s)" font-size="13" fill="{C_INK2}">{esc(c1)}</text>')
        if c2:
            parts.append(f'<text x="780" y="{ry + 84}" font-family="var(--f-m)" font-size="11.5" fill="{acc}">{esc(c2)}</text>')
        parts.append("</g>")
        ry += ch + gap
    parts.append("</svg>")
    return "".join(parts)


def svg_matrix_2x2(
    quads: list[dict[str, Any]],
    footer_banner: str | None = None,
    animated: bool = False,
) -> str:
    """2x2 architectural quadrant grid (e.g., L0~L3 envelope or scope matrix)."""
    cls = "fig anim" if animated else "fig"
    parts = [f'<svg class="{cls}" viewBox="0 0 1400 424" width="1400" height="424" xmlns="http://www.w3.org/2000/svg">']
    qw, qh = 686, 166 if footer_banner else 192
    coords = [(0, 12), (714, 12), (0, 12 + qh + 16), (714, 12 + qh + 16)]
    for i, (q, (x, y)) in enumerate(zip(quads, coords)):
        acc = q.get("accent", C_TEAL)
        bg = q.get("bg", C_WHITE)
        parts.append(f'<g data-step="{i + 1}">' if animated else "<g>")
        parts.append(f'<rect x="{x}" y="{y}" width="{qw}" height="{qh}" fill="{bg}" stroke="{C_LINE}" stroke-width="1"/>')
        parts.append(f'<rect x="{x}" y="{y}" width="5" height="{qh}" fill="{acc}"/>')
        parts.append(f'<text x="{x + 22}" y="{y + 30}" font-family="var(--f-m)" font-size="11.5" fill="{acc}" font-weight="600">{esc(q["tag"])}</text>')
        parts.append(f'<text x="{x + 22}" y="{y + 58}" font-family="var(--f-r)" font-size="19" fill="{C_INK}" font-weight="600">{esc(q["title"])}</text>')
        ly = y + 90
        for ln in q.get("lines", []):
            parts.append(f'<text x="{x + 22}" y="{ly}" font-family="var(--f-s)" font-size="13.5" fill="{C_INK2}">{esc(ln)}</text>')
            ly += 26
        parts.append("</g>")
    if footer_banner:
        parts.append(f'<rect x="0" y="370" width="1400" height="44" fill="{C_TEAL_L}"/>')
        parts.append(f'<rect x="0" y="370" width="4" height="44" fill="{C_TEAL}"/>')
        parts.append(f'<text x="20" y="398" font-family="var(--f-r)" font-size="18" fill="{C_TEAL}" font-weight="600">{esc(footer_banner)}</text>')
    parts.append("</svg>")
    return "".join(parts)


def svg_metrics_table(
    kpis: list[tuple[str, str, str]],
    headers: list[str],
    rows: list[list[str]],
    col_xs: list[int],
    footnote: str = "",
) -> str:
    """Top KPI strip + crisp engineering table."""
    parts = ['<svg class="fig" viewBox="0 0 1400 424" width="1400" height="424" xmlns="http://www.w3.org/2000/svg">']
    nk = len(kpis)
    kgap = 18
    kw = (1400 - kgap * (nk - 1)) // nk
    accents = [C_TEAL, C_GREEN, C_CLAY, C_GOLD]
    for i, (val, lbl, sub) in enumerate(kpis):
        x = i * (kw + kgap)
        acc = accents[i % len(accents)]
        parts.append(f'<rect x="{x}" y="8" width="{kw}" height="106" fill="{C_WHITE}" stroke="{C_LINE}" stroke-width="1"/>')
        parts.append(f'<rect x="{x}" y="8" width="{kw}" height="4" fill="{acc}"/>')
        parts.append(f'<text x="{x + 18}" y="52" font-family="var(--f-m)" font-size="30" fill="{acc}" font-weight="600">{esc(val)}</text>')
        parts.append(f'<text x="{x + 18}" y="78" font-family="var(--f-s)" font-size="13.5" fill="{C_INK}" font-weight="600">{esc(lbl)}</text>')
        parts.append(f'<text x="{x + 18}" y="100" font-family="var(--f-m)" font-size="11" fill="{C_MUTE}">{esc(sub)}</text>')

    # Table header
    ty = 134
    parts.append(f'<rect x="0" y="{ty}" width="1400" height="34" fill="{C_TEAL_D}"/>')
    for htxt, cx in zip(headers, col_xs):
        parts.append(f'<text x="{cx}" y="{ty + 22}" font-family="var(--f-m)" font-size="12" fill="{C_PAPER}" font-weight="600">{esc(htxt)}</text>')
    ry = ty + 34
    rh = min(38, 224 // max(1, len(rows)))
    for ri, row in enumerate(rows):
        bg = C_WHITE if ri % 2 == 0 else C_PAPER2
        parts.append(f'<rect x="0" y="{ry}" width="1400" height="{rh}" fill="{bg}" stroke="{C_LINE}" stroke-width="0.6"/>')
        for ci, (cell, cx) in enumerate(zip(row, col_xs)):
            ff = "var(--f-m)" if ci == 0 or cell.endswith("%") or cell.endswith("x") or cell.endswith("μs") else "var(--f-s)"
            fc = C_GREEN if "PASS" in cell or "100.0%" in cell else (C_TEAL if ci == 0 else C_INK)
            fw = "600" if ci == 0 or "PASS" in cell else "400"
            parts.append(f'<text x="{cx}" y="{ry + rh // 2 + 5}" font-family="{ff}" font-size="12.5" fill="{fc}" font-weight="{fw}">{esc(cell)}</text>')
        ry += rh
    if footnote:
        parts.append(f'<text x="0" y="412" font-family="var(--f-m)" font-size="11.5" fill="{C_MUTE}">{esc(footnote)}</text>')
    parts.append("</svg>")
    return "".join(parts)


def svg_bars(
    title_left: str,
    rows: list[tuple[str, float, str, str, str]],
    footer_text: str,
) -> str:
    """Horizontal percentage/coverage bars matching presentation.html slide 4."""
    parts = ['<svg class="fig" viewBox="0 0 1400 424" width="1400" height="424" xmlns="http://www.w3.org/2000/svg">']
    parts.append(f'<text x="0" y="16" font-family="var(--f-s)" font-size="12" fill="{C_MUTE}" font-weight="600">{esc(title_left)}</text>')
    parts.append(f'<text x="1128" y="16" font-family="var(--f-s)" font-size="11" fill="{C_MUTE}" text-anchor="end">实测覆盖 / 达标度</text>')
    parts.append(f'<text x="1178" y="16" font-family="var(--f-s)" font-size="11" fill="{C_MUTE}">归属 / 状态</text>')
    y = 52
    step = min(56, 310 // max(1, len(rows)))
    for label, pct, pct_str, tag, color in rows:
        bw = max(0, min(600, int(600 * pct / 100.0)))
        parts.append(f'<text x="0" y="{y}" font-family="var(--f-s)" font-size="14.5" fill="{C_INK}" font-weight="500">{esc(label)}</text>')
        parts.append(f'<rect x="500" y="{y - 12}" width="600" height="16" fill="{C_PAPER2}" stroke="{C_LINE}" stroke-width="1"/>')
        if bw > 0:
            parts.append(f'<rect x="500" y="{y - 12}" width="{bw}" height="16" fill="{color}"/>')
        parts.append(f'<text x="1128" y="{y + 1}" font-family="var(--f-m)" font-size="13" fill="{color}" text-anchor="end" font-weight="600">{esc(pct_str)}</text>')
        parts.append(f'<text x="1178" y="{y + 1}" font-family="var(--f-s)" font-size="12.5" fill="{C_INK2}" font-weight="600">{esc(tag)}</text>')
        parts.append(f'<line x1="0" y1="{y + 20}" x2="1400" y2="{y + 20}" stroke="{C_LINE}" stroke-width="0.7"/>')
        y += step
    parts.append(f'<rect x="0" y="374" width="1400" height="42" fill="{C_TEAL_L}"/>')
    parts.append(f'<rect x="0" y="374" width="4" height="42" fill="{C_TEAL}"/>')
    parts.append(f'<text x="20" y="401" font-family="var(--f-s)" font-size="15.5" fill="{C_TEAL}" font-weight="600">{esc(footer_text)}</text>')
    parts.append("</svg>")
    return "".join(parts)


def svg_closing(
    kicker: str,
    headline: str,
    pillars: list[tuple[str, str, str]],
    open_question: str,
    next_talk: str,
) -> str:
    """Closing slide with 3 takeaway pillars + open question + next talk pointer."""
    parts = ['<svg class="fig" viewBox="0 0 1400 424" width="1400" height="424" xmlns="http://www.w3.org/2000/svg">']
    parts.append(f'<rect x="0" y="10" width="1400" height="92" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="28" y="42" font-family="var(--f-m)" font-size="11.5" fill="#A4C4C6" letter-spacing="2.4">{esc(kicker)}</text>')
    parts.append(f'<text x="28" y="80" font-family="var(--f-r)" font-size="28" fill="{C_PAPER}" font-weight="600">{esc(headline)}</text>')
    n = len(pillars)
    gap = 20
    pw = (1400 - gap * (n - 1)) // n
    accents = [C_TEAL, C_GREEN, C_CLAY]
    for i, (pt, p1, p2) in enumerate(pillars):
        px = i * (pw + gap)
        acc = accents[i % len(accents)]
        parts.append(f'<rect x="{px}" y="122" width="{pw}" height="158" fill="{C_WHITE}" stroke="{C_LINE}" stroke-width="1"/>')
        parts.append(f'<rect x="{px}" y="122" width="{pw}" height="4" fill="{acc}"/>')
        parts.append(f'<text x="{px + 20}" y="158" font-family="var(--f-r)" font-size="18.5" fill="{C_INK}" font-weight="600">{esc(pt)}</text>')
        parts.append(f'<text x="{px + 20}" y="196" font-family="var(--f-s)" font-size="13.5" fill="{C_INK2}">{esc(p1)}</text>')
        parts.append(f'<text x="{px + 20}" y="226" font-family="var(--f-m)" font-size="12" fill="{acc}">{esc(p2)}</text>')
    parts.append(f'<rect x="0" y="302" width="1400" height="54" fill="{C_CLAY_L}"/>')
    parts.append(f'<rect x="0" y="302" width="4" height="54" fill="{C_CLAY}"/>')
    parts.append(f'<text x="22" y="335" font-family="var(--f-r)" font-size="18.5" fill="{C_INK}" font-weight="600">{esc(open_question)}</text>')
    parts.append(f'<text x="0" y="396" font-family="var(--f-m)" font-size="13" fill="{C_TEAL}">{esc(next_talk)}</text>')
    parts.append(f'<text x="1400" y="396" font-family="var(--f-m)" font-size="12" fill="{C_MUTE}" text-anchor="end">github.com/digoal/agent-body · Apache-2.0</text>')
    parts.append("</svg>")
    return "".join(parts)


CSS_TEMPLATE = """\
:root{
  --paper:#F7F6F2; --paper2:#F1EFE9; --line:#D9D5CB; --ink:#1B2B33; --ink2:#46585F;
  --mute:#89938F; --teal:#245A6B; --teal-l:#E5EFF0; --teal-m:#7BA6AF;
  --clay:#BE653B; --clay-l:#F7E9DE; --green:#2F7A66; --gold:#AE8A3E;
  --f-r:"Songti SC","STSong","Songti TC","Noto Serif CJK SC",Georgia,serif;
  --f-s:"PingFang SC","Hiragino Sans GB","Noto Sans CJK SC","Microsoft YaHei",sans-serif;
  --f-m:"SF Mono",Menlo,Consolas,"Courier New",monospace;
}
*{box-sizing:border-box;margin:0;padding:0}
html,body{height:100%;background:#E9E7E1;overflow:hidden;color:var(--ink);
  font-family:var(--f-s);-webkit-font-smoothing:antialiased}
#stageArea{position:absolute;left:0;top:0;height:100%;display:flex;align-items:center;
  justify-content:center;transition:width .34s cubic-bezier(.4,0,.2,1)}
#deck{width:1600px;height:958px;flex:0 0 auto;position:relative;transform-origin:center center}
.slide{position:absolute;left:0;top:0;width:1600px;height:900px;background:var(--paper);
  overflow:hidden;opacity:0;visibility:hidden;transition:opacity .3s ease}
.slide.on{opacity:1;visibility:visible}
.hd{position:absolute;left:64px;top:50px;width:1472px}
.eyebrow{font-family:var(--f-m);font-size:13px;letter-spacing:3.2px;color:var(--mute)}
.title{font-family:var(--f-r);font-size:42px;font-weight:600;line-height:1.18;
  letter-spacing:.4px;margin-top:12px}
.lead{font-size:18.5px;line-height:1.58;color:var(--ink2);margin-top:10px;width:1420px}
.lead b{color:var(--teal);font-weight:600}
.rule{position:absolute;left:64px;top:178px;width:1472px;height:1px;background:var(--line)}
.fig{position:absolute;left:100px;top:192px;overflow:visible}
.fig.cover-svg{left:0;top:0}
.nextbar{position:absolute;left:64px;top:664px;width:1120px;padding-left:16px;
  border-left:3px solid var(--clay)}
.nb-lbl{font-family:var(--f-m);font-size:11px;letter-spacing:2.4px;color:var(--mute);
  display:block;margin-bottom:7px}
.nextbar p{font-family:var(--f-r);font-size:19.5px;line-height:1.5;color:var(--ink)}
.foot{position:absolute;left:64px;top:836px;width:1472px;display:flex;
  justify-content:space-between;align-items:center;border-top:1px solid var(--line);
  padding-top:11px;font-family:var(--f-m);font-size:11px;color:var(--mute);letter-spacing:1.2px}
.foot .pnum{color:var(--ink2)}
.animbar{position:absolute;left:100px;top:614px;width:1400px;height:28px;display:flex;
  align-items:center;justify-content:space-between}
.animcap{font-size:12.5px;color:var(--ink2);letter-spacing:.2px;display:flex;align-items:center;gap:9px}
.animcap .atag{font-family:var(--f-m);font-size:9.5px;letter-spacing:1.6px;color:var(--teal);
  border:1px solid var(--teal-m);padding:2px 6px;font-style:normal;background:var(--teal-l)}
.animbtns{display:flex;align-items:center;gap:6px}
.stepno{font-family:var(--f-m);font-size:11px;color:var(--mute);margin-right:10px;
  display:flex;align-items:center;gap:4px}
.stepin{width:38px;font-family:var(--f-m);font-size:11px;color:var(--teal);text-align:center;
  border:1px solid var(--line);background:var(--paper);padding:2px 0;-moz-appearance:textfield}
.stepin::-webkit-outer-spin-button,.stepin::-webkit-inner-spin-button{-webkit-appearance:none;margin:0}
.animbtns button{font-family:var(--f-m);font-size:11px;padding:3px 10px;border:1px solid var(--line);
  background:var(--paper2);color:var(--ink2);cursor:pointer;letter-spacing:.5px}
.animbtns button:hover{background:var(--teal-l);border-color:var(--teal-m);color:var(--teal)}
.animbtns button.on{background:var(--teal);border-color:var(--teal);color:var(--paper)}
.fig.anim [data-step]{transition:opacity .38s ease}
.fig.anim [data-step].off{opacity:.12}
#ctrl{position:absolute;left:0;top:900px;width:1600px;height:58px;background:var(--paper2);
  border-top:1px solid var(--line);display:flex;align-items:center;gap:20px;padding:0 28px;
  font-family:var(--f-m);font-size:11.5px;color:var(--ink2);letter-spacing:.6px}
.brand{color:var(--teal);letter-spacing:2.4px;font-size:10.5px;text-decoration:none}
.brand:hover{text-decoration:underline}
.pos{color:var(--mute)}
.pos b{color:var(--ink);font-weight:500}
.prog{flex:1;height:2px;background:#E2DED5;position:relative;min-width:120px}
.prog i{position:absolute;left:0;top:0;height:2px;background:var(--teal);width:7%;
  transition:width .3s ease}
.keys{display:flex;align-items:center;gap:14px}
.k{display:flex;align-items:center;gap:6px;cursor:pointer;color:var(--ink2);white-space:nowrap}
.k:hover{color:var(--teal)}
.kbd{display:inline-block;min-width:20px;text-align:center;border:1px solid var(--line);
  background:var(--paper);padding:1px 5px;font-size:10px;color:var(--ink)}
.k.on .kbd{background:var(--teal);border-color:var(--teal);color:var(--paper)}
#panel{position:fixed;right:0;top:0;height:100%;width:520px;background:var(--paper2);
  border-left:1px solid var(--line);transform:translateX(100%);
  transition:transform .34s cubic-bezier(.4,0,.2,1);display:flex;flex-direction:column;z-index:40}
#panel.open{transform:translateX(0)}
.ph{padding:20px 26px 12px;border-bottom:1px solid var(--line);display:flex;
  justify-content:space-between;align-items:flex-end;gap:12px}
.ph h3{font-family:var(--f-r);font-size:18px;font-weight:600;letter-spacing:.4px}
.ph .tabs{display:flex;gap:12px;font-family:var(--f-m);font-size:10.5px;color:var(--mute)}
.ph .tabs button{background:none;border:none;font:inherit;color:inherit;cursor:pointer;
  padding:2px 0;border-bottom:1px solid transparent}
.ph .tabs button.on{color:var(--teal);border-bottom-color:var(--teal)}
.pb{flex:1;overflow-y:auto;padding:22px 28px 80px;scroll-behavior:smooth}
.pb::-webkit-scrollbar{width:8px}
.pb::-webkit-scrollbar-thumb{background:#D3CEC3}
.pb p{font-size:14.5px;line-height:2.05;color:var(--ink);margin-bottom:15px;text-align:justify}
.pb .sec{font-family:var(--f-m);font-size:10.5px;letter-spacing:2.2px;color:var(--mute);
  margin:26px 0 8px;padding-top:14px;border-top:1px solid var(--line)}
.pb .sec:first-child{margin-top:0;padding-top:0;border:none}
.pb .sec b{display:block;font-family:var(--f-r);font-size:16px;color:var(--ink);
  letter-spacing:0;margin-top:6px;font-weight:600}
.pb .sec.cur{background:var(--teal-l);padding:10px 12px;border-left:3px solid var(--teal)}
.pb .meta{margin-bottom:18px}
.pb .meta .e{font-family:var(--f-m);font-size:10.5px;letter-spacing:2px;color:var(--mute)}
.pb .meta .t{font-family:var(--f-r);font-size:21px;font-weight:600;margin-top:6px;line-height:1.4}
#ov{position:fixed;inset:0;background:rgba(27,43,51,.44);display:none;
  align-items:center;justify-content:center;z-index:60;padding:40px}
#ov.open{display:flex}
.card{background:var(--paper);border:1px solid var(--line);padding:40px 46px;
  max-height:80vh;overflow:auto}
.card h3{font-family:var(--f-r);font-size:25px;font-weight:600;margin-bottom:6px}
.card .sub{font-family:var(--f-m);font-size:10.5px;letter-spacing:2.4px;color:var(--mute);
  margin-bottom:22px}
.toc{width:1140px}
.toc-grid{display:grid;grid-template-columns:1fr 1fr;column-gap:56px}
.toc-item{display:flex;gap:14px;align-items:baseline;padding:8px 8px;border-bottom:1px solid #EAE6DD;
  cursor:pointer;font-size:14px;color:var(--ink2)}
.toc-item:hover{background:var(--teal-l);color:var(--teal)}
.toc-item.cur{background:var(--teal-l)}
.toc-item.cur .no{color:var(--clay)}
.toc-item .no{font-family:var(--f-m);font-size:10.5px;color:var(--mute);width:22px;flex:0 0 auto}
.toc-item .tt{font-family:var(--f-r);font-size:15px;color:var(--ink)}
.help{width:760px}
.hrow{display:flex;gap:18px;align-items:baseline;padding:9px 0;border-bottom:1px solid #EAE6DD;
  font-size:13px;color:var(--ink2)}
.hrow .kk{font-family:var(--f-m);font-size:11px;color:var(--ink);min-width:112px;
  border:1px solid var(--line);padding:2px 7px;text-align:center;background:var(--paper2)}
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
  const w = panelMode ? Math.max(320, window.innerWidth - 520) : window.innerWidth;
  stageArea.style.width = w + 'px';
  const s = Math.min(w / 1600, window.innerHeight / 958);
  deck.style.transform = 'scale(' + s + ')';
}
window.addEventListener('resize', layout);

function maxSteps(idx){
  const el = slides[idx].querySelector('.fig.anim');
  if(!el) return 0;
  return Math.max(...[...el.querySelectorAll('[data-step]')].map(n => +n.dataset.step));
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
  if(inp) inp.value = s || 1;
  const cap = slides[idx].querySelector('.animcap');
  const list = CAPS[String(idx)] || [];
  if(cap && list.length){
    const txt = list[Math.max(0, (s || 1) - 1)] || list[0];
    cap.innerHTML = '<i class="atag">动画</i>' + txt;
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
  if((animState[idx] || 0) >= max) setStep(idx, 1);
  else if(!animState[idx]) setStep(idx, 1);
  animTimers[idx] = setInterval(() => {
    const nxt = (animState[idx] || 1) + 1;
    if(nxt > max){ animPause(idx); return; }
    setStep(idx, nxt);
  }, 1700);
}
function animStep(idx){
  animPause(idx);
  const max = maxSteps(idx);
  if(!max) return;
  const nxt = ((animState[idx] || 1) % max) + 1;
  setStep(idx, nxt);
}
slides.forEach((sl, idx) => {
  const bar = sl.querySelector('.animbar');
  if(!bar) return;
  setStep(idx, 1);
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
    pb.innerHTML = '<div class="meta"><div class="e">' + String(cur+1).padStart(2,'0') + ' / ' + String(slides.length).padStart(2,'0') + ' · ' + m.e + '</div><div class="t">' + m.t + '</div></div>' + fmtParas(m.s);
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
      ['R', '动画重置'], ['F', '全屏（底部控制条保持可见）'],
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


def render_deck(
    doc_title: str,
    brand_label: str,
    foot_label: str,
    slides_data: list[dict[str, Any]],
    out_path: Path,
) -> None:
    """Render a complete self-contained 地层剖面 HTML slide deck."""
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
            anim_html = ""
            if sl.get("anim_caps"):
                caps = sl["anim_caps"]
                anim_html = (
                    f'<div class="animbar">'
                    f'<div class="animcap"><i class="atag">动画</i>{esc(caps[0])}</div>'
                    f'<div class="animbtns">'
                    f'<span class="stepno"><input class="stepin" type="number" min="1" max="{len(caps)}" value="1"> / {len(caps)}</span>'
                    f'<button data-act="play">播放</button><button data-act="pause">暂停</button>'
                    f'<button data-act="step">单步</button><button data-act="reset">重置</button>'
                    f"</div></div>"
                )
            next_html = ""
            if sl.get("next"):
                next_html = f'<div class="nextbar"><span class="nb-lbl">承接 · NEXT</span><p>{esc(sl["next"])}</p></div>'
            sec = (
                f'<section class="slide{on_cls}" data-i="{idx}">'
                f'<div class="hd">'
                f'<div class="eyebrow">{esc(sl["eyebrow"])}</div>'
                f'<h2 class="title">{esc(sl["title"])}</h2>'
                f'<p class="lead">{sl["lead"]}</p>'
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
