#!/usr/bin/env python3
"""Visual-first SVG icon library and domain-specific diagram illustrations for Agent Body decks.

Canvas coordinate system: 1472 x 640 (for content figures) and 1600 x 900 (for visual covers).
Design principles:
  - Light, eye-friendly warm-paper blueprint aesthetic (NEVER dark/black slide backgrounds).
  - Visual-first: icons, topology graphs, anatomical diagrams, traffic intersections, radars,
    purifiers, capsules, shields, Pareto scatter plots, and microkernel hubs.
  - Ultra-low text density: crisp short labels & hero numbers only; long prose belongs in N/T notes.
  - Guaranteed zero text-overflow and zero text-overlap by deterministic coordinate geometry.
"""

from __future__ import annotations

import html
import math
from typing import Any

# Palette — Warm Architectural Blueprint (light, crisp, high-contrast, never dark)
C_PAPER = "#FAF8F4"
C_PAPER2 = "#F2EFE9"
C_CARD = "#FFFFFF"
C_LINE = "#DCD7CC"
C_LINE_S = "#EAE6DC"
C_INK = "#162A33"
C_INK2 = "#445A63"
C_MUTE = "#7D8B88"

C_TEAL = "#1F5E73"
C_TEAL_D = "#154454"
C_TEAL_2 = "#2D7187"
C_TEAL_3 = "#4D8FA1"
C_TEAL_L = "#E6F2F4"
C_TEAL_M = "#7DB0BC"

C_CLAY = "#C85A32"
C_CLAY_L = "#FCEEE8"
C_CLAY_M = "#E49A7D"

C_GREEN = "#257A60"
C_GREEN_L = "#E6F4EE"
C_GREEN_M = "#79BCA6"

C_GOLD = "#B5852A"
C_GOLD_L = "#FBF3E0"
C_GOLD_M = "#DDBB72"

C_RED = "#C23E3B"
C_RED_L = "#FBEAEA"

C_WHITE = "#FFFFFF"


def esc(text: str) -> str:
    return html.escape(str(text), quote=True)


def _arrow_defs(uid: str = "va") -> str:
    return (
        "<defs>"
        f'<marker id="{uid}_t" viewBox="0 0 10 10" refX="8.2" refY="5" markerWidth="8" markerHeight="8" orient="auto-start-reverse">'
        f'<path d="M 0 1 L 9 5 L 0 9 z" fill="{C_TEAL}"/></marker>'
        f'<marker id="{uid}_g" viewBox="0 0 10 10" refX="8.2" refY="5" markerWidth="8" markerHeight="8" orient="auto-start-reverse">'
        f'<path d="M 0 1 L 9 5 L 0 9 z" fill="{C_GREEN}"/></marker>'
        f'<marker id="{uid}_c" viewBox="0 0 10 10" refX="8.2" refY="5" markerWidth="8" markerHeight="8" orient="auto-start-reverse">'
        f'<path d="M 0 1 L 9 5 L 0 9 z" fill="{C_CLAY}"/></marker>'
        f'<marker id="{uid}_r" viewBox="0 0 10 10" refX="8.2" refY="5" markerWidth="8" markerHeight="8" orient="auto-start-reverse">'
        f'<path d="M 0 1 L 9 5 L 0 9 z" fill="{C_RED}"/></marker>'
        f'<marker id="{uid}_o" viewBox="0 0 10 10" refX="8.2" refY="5" markerWidth="8" markerHeight="8" orient="auto-start-reverse">'
        f'<path d="M 0 1 L 9 5 L 0 9 z" fill="{C_GOLD}"/></marker>'
        f'<marker id="{uid}_m" viewBox="0 0 10 10" refX="8.2" refY="5" markerWidth="8" markerHeight="8" orient="auto-start-reverse">'
        f'<path d="M 0 1 L 9 5 L 0 9 z" fill="{C_INK2}"/></marker>'
        f'<filter id="{uid}_sh" x="-6%" y="-6%" width="112%" height="118%">'
        f'<feDropShadow dx="0" dy="3" stdDeviation="4" flood-color="#162A33" flood-opacity="0.06"/></filter>'
        "</defs>"
    )


def icon(name: str, cx: float, cy: float, size: float = 32, color: str = C_TEAL) -> str:
    """Crisp flat+stroke SVG vector icon centered at (cx, cy) scaled from 24x24 viewbox."""
    s = size / 24.0
    tx = cx - size / 2.0
    ty = cy - size / 2.0
    st = f'stroke="{color}" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" fill="none"'
    sf = f'fill="{color}" opacity="0.16"'

    shapes: dict[str, str] = {
        "brain": (
            f'<path d="M9.5 4a3.5 3.5 0 0 0-3.3 2.4A3 3 0 0 0 4 9.3c0 1.1.6 2.1 1.5 2.6A3.2 3.2 0 0 0 5 13.7a3.3 3.3 0 0 0 3.2 3.3H11V4H9.5z" {sf}/>'
            f'<path d="M14.5 4a3.5 3.5 0 0 1 3.3 2.4A3 3 0 0 1 20 9.3c0 1.1-.6 2.1-1.5 2.6a3.2 3.2 0 0 1 .5 1.8 3.3 3.3 0 0 1-3.2 3.3H13V4h1.5z" {sf}/>'
            f'<path d="M12 4v16M9.5 4a3.5 3.5 0 0 0-3.3 2.4A3 3 0 0 0 4 9.3c0 1.1.6 2.1 1.5 2.6A3.2 3.2 0 0 0 5 13.7a3.3 3.3 0 0 0 3.2 3.3H12" {st}/>'
            f'<path d="M14.5 4a3.5 3.5 0 0 1 3.3 2.4A3 3 0 0 1 20 9.3c0 1.1-.6 2.1-1.5 2.6a3.2 3.2 0 0 1 .5 1.8 3.3 3.3 0 0 1-3.2 3.3H12" {st}/>'
            f'<path d="M8 9.5c1 0 1.5.5 2 1.2M16 9.5c-1 0-1.5.5-2 1.2M8.5 14c1 0 1.8-.4 2.2-1M15.5 14c-1 0-1.8-.4-2.2-1" {st}/>'
        ),
        "plug": (
            f'<rect x="7" y="9" width="10" height="6" rx="2" {sf}/>'
            f'<path d="M9 3v6M15 3v6M7 9h10v4a5 5 0 0 1-10 0V9zM12 18v3" {st}/>'
        ),
        "body": (
            f'<rect x="6" y="7" width="12" height="12" rx="3" {sf}/>'
            f'<circle cx="12" cy="4" r="2" {st}/>'
            f'<rect x="6" y="7" width="12" height="12" rx="3" {st}/>'
            f'<path d="M9 12h6M12 9v6M3 10v4M21 10v4" {st}/>'
        ),
        "eye": (
            f'<path d="M2 12s3.8-6 10-6 10 6 10 6-3.8 6-10 6-10-6-10-6z" {sf}/>'
            f'<path d="M2 12s3.8-6 10-6 10 6 10 6-3.8 6-10 6-10-6-10-6z" {st}/>'
            f'<circle cx="12" cy="12" r="3" {st}/>'
            f'<circle cx="12" cy="12" r="1.2" fill="{color}"/>'
        ),
        "bolt": (
            f'<polygon points="13 2 4 14 11.5 14 10.5 22 20 10 12.5 10 13 2" {sf}/>'
            f'<polygon points="13 2 4 14 11.5 14 10.5 22 20 10 12.5 10 13 2" {st}/>'
        ),
        "capsule": (
            f'<rect x="4" y="7" width="16" height="10" rx="5" {sf}/>'
            f'<rect x="4" y="7" width="16" height="10" rx="5" {st}/>'
            f'<line x1="12" y1="7" x2="12" y2="17" {st}/>'
            f'<circle cx="8.5" cy="12" r="1.3" fill="{color}"/>'
            f'<path d="M15 10.5h2.5M15 13.5h2" {st}/>'
        ),
        "spine": (
            f'<rect x="9" y="3" width="6" height="18" rx="2" {sf}/>'
            f'<line x1="12" y1="2" x2="12" y2="22" {st}/>'
            f'<path d="M7 6h10M6 10h12M6 14h12M7 18h10" {st}/>'
        ),
        "shield": (
            f'<path d="M12 2.5l8 3.2v5.8c0 5-3.4 8.6-8 10-4.6-1.4-8-5-8-10V5.7L12 2.5z" {sf}/>'
            f'<path d="M12 2.5l8 3.2v5.8c0 5-3.4 8.6-8 10-4.6-1.4-8-5-8-10V5.7L12 2.5z" {st}/>'
            f'<path d="M8.8 12.2l2.2 2.2 4.4-4.6" {st}/>'
        ),
        "mesh": (
            f'<circle cx="12" cy="5" r="2.5" {sf}/><circle cx="5" cy="18" r="2.5" {sf}/><circle cx="19" cy="18" r="2.5" {sf}/>'
            f'<circle cx="12" cy="5" r="2.5" {st}/><circle cx="5" cy="18" r="2.5" {st}/><circle cx="19" cy="18" r="2.5" {st}/>'
            f'<path d="M10.7 7.2L6.3 15.8M13.3 7.2l4.4 8.6M7.5 18h9" {st}/>'
        ),
        "gear": (
            f'<circle cx="12" cy="12" r="4" {sf}/>'
            f'<circle cx="12" cy="12" r="3.5" {st}/>'
            f'<path d="M12 2.5v2.5M12 19v2.5M2.5 12H5M19 12h2.5M5.3 5.3l1.8 1.8M16.9 16.9l1.8 1.8M18.7 5.3l-1.8 1.8M7.1 16.9l-1.8 1.8" {st}/>'
            f'<circle cx="12" cy="12" r="7.5" {st} stroke-dasharray="3 2.8"/>'
        ),
        "database": (
            f'<ellipse cx="12" cy="6" rx="7.5" ry="3" {sf}/>'
            f'<path d="M4.5 6v12c0 1.7 3.4 3 7.5 3s7.5-1.3 7.5-3V6" {sf}/>'
            f'<ellipse cx="12" cy="6" rx="7.5" ry="3" {st}/>'
            f'<path d="M4.5 6v12c0 1.7 3.4 3 7.5 3s7.5-1.3 7.5-3V6" {st}/>'
            f'<path d="M4.5 12c0 1.7 3.4 3 7.5 3s7.5-1.3 7.5-3" {st}/>'
        ),
        "server": (
            f'<rect x="3" y="4" width="18" height="6.5" rx="1.8" {sf}/>'
            f'<rect x="3" y="13.5" width="18" height="6.5" rx="1.8" {sf}/>'
            f'<rect x="3" y="4" width="18" height="6.5" rx="1.8" {st}/>'
            f'<rect x="3" y="13.5" width="18" height="6.5" rx="1.8" {st}/>'
            f'<circle cx="7" cy="7.2" r="1" fill="{color}"/><circle cx="7" cy="16.7" r="1" fill="{color}"/>'
            f'<path d="M11 7.2h6M11 16.7h6" {st}/>'
        ),
        "lock": (
            f'<rect x="5" y="11" width="14" height="10" rx="2" {sf}/>'
            f'<rect x="5" y="11" width="14" height="10" rx="2" {st}/>'
            f'<path d="M8 11V7.5a4 4 0 0 1 8 0V11" {st}/>'
            f'<circle cx="12" cy="16" r="1.4" fill="{color}"/>'
        ),
        "unlock": (
            f'<rect x="5" y="11" width="14" height="10" rx="2" {sf}/>'
            f'<rect x="5" y="11" width="14" height="10" rx="2" {st}/>'
            f'<path d="M8 11V7.5a4 4 0 0 1 7.6-1.7" {st}/>'
            f'<circle cx="12" cy="16" r="1.4" fill="{color}"/>'
        ),
        "folder": (
            f'<path d="M3 6.5A2 2 0 0 1 5 4.5h4.5l2 2.2H19a2 2 0 0 1 2 2V18a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6.5z" {sf}/>'
            f'<path d="M3 6.5A2 2 0 0 1 5 4.5h4.5l2 2.2H19a2 2 0 0 1 2 2V18a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6.5z" {st}/>'
        ),
        "ban": (
            f'<circle cx="12" cy="12" r="9" {sf}/>'
            f'<circle cx="12" cy="12" r="9" {st}/>'
            f'<line x1="6" y1="6" x2="18" y2="18" {st}/>'
        ),
        "radar": (
            f'<circle cx="12" cy="12" r="9" {sf}/>'
            f'<circle cx="12" cy="12" r="9" {st}/>'
            f'<circle cx="12" cy="12" r="5" {st} stroke-dasharray="2.5 2"/>'
            f'<line x1="12" y1="12" x2="18.2" y2="6.4" {st}/>'
            f'<circle cx="12" cy="12" r="1.5" fill="{color}"/>'
        ),
        "microscope": (
            f'<circle cx="10.5" cy="10.5" r="6" {sf}/>'
            f'<circle cx="10.5" cy="10.5" r="6" {st}/>'
            f'<line x1="15" y1="15" x2="20.5" y2="20.5" {st}/>'
            f'<path d="M8.5 10.5h4M10.5 8.5v4" {st}/>'
        ),
        "filter": (
            f'<polygon points="3 4 21 4 14 12.5 14 19 10 21 10 12.5 3 4" {sf}/>'
            f'<polygon points="3 4 21 4 14 12.5 14 19 10 21 10 12.5 3 4" {st}/>'
        ),
        "terminal": (
            f'<rect x="3" y="4" width="18" height="16" rx="2.5" {sf}/>'
            f'<rect x="3" y="4" width="18" height="16" rx="2.5" {st}/>'
            f'<path d="M7.5 10l3 2.5-3 2.5M12.5 15h4" {st}/>'
        ),
        "gauge": (
            f'<path d="M4 17a8.5 8.5 0 1 1 16 0" {sf}/>'
            f'<path d="M4 17a8.5 8.5 0 1 1 16 0" {st}/>'
            f'<line x1="12" y1="16" x2="16.5" y2="10.5" {st}/>'
            f'<circle cx="12" cy="16" r="1.8" fill="{color}"/>'
        ),
        "trophy": (
            f'<path d="M7 4h10v5a5 5 0 0 1-10 0V4z" {sf}/>'
            f'<path d="M7 4h10v5a5 5 0 0 1-10 0V4zM7 6H4v2a3 3 0 0 0 3 3M17 6h3v2a3 3 0 0 1-3 3M12 14v5M8 20h8" {st}/>'
        ),
        "check": (
            f'<circle cx="12" cy="12" r="9" {sf}/>'
            f'<circle cx="12" cy="12" r="9" {st}/>'
            f'<path d="M8.2 12.3l2.6 2.6 5.2-5.4" {st}/>'
        ),
        "cross": (
            f'<circle cx="12" cy="12" r="9" {sf}/>'
            f'<circle cx="12" cy="12" r="9" {st}/>'
            f'<path d="M8.8 8.8l6.4 6.4M15.2 8.8l-6.4 6.4" {st}/>'
        ),
        "warning": (
            f'<polygon points="12 3 21.5 19.5 2.5 19.5" {sf}/>'
            f'<polygon points="12 3 21.5 19.5 2.5 19.5" {st}/>'
            f'<line x1="12" y1="9.5" x2="12" y2="14" {st}/>'
            f'<circle cx="12" cy="16.8" r="1" fill="{color}"/>'
        ),
        "scale": (
            f'<path d="M12 4v16M6 20h12M4 8h16" {st}/>'
            f'<path d="M4 8l-2 6h4l-2-6zM20 8l-2 6h4l-2-6z" {sf}/>'
            f'<path d="M4 8l-2 6h4l-2-6zM20 8l-2 6h4l-2-6z" {st}/>'
        ),
        "flywheel": (
            f'<circle cx="12" cy="12" r="7.5" {sf}/>'
            f'<path d="M19.2 9.5A7.5 7.5 0 0 0 6.5 6.8M4.8 14.5a7.5 7.5 0 0 0 12.7 2.7" {st}/>'
            f'<polyline points="17 5 19.5 9.6 14.8 9.8" {st}/>'
            f'<polyline points="7 19 4.5 14.4 9.2 14.2" {st}/>'
        ),
        "rocket": (
            f'<path d="M12 2.5c3.5 2.5 5 6 5 10.5l-5 3-5-3c0-4.5 1.5-8 5-10.5z" {sf}/>'
            f'<path d="M12 2.5c3.5 2.5 5 6 5 10.5l-5 3-5-3c0-4.5 1.5-8 5-10.5zM7 12l-3 3v3l3-1M17 12l3 3v3l-3-1M10 18l2 3.5 2-3.5" {st}/>'
            f'<circle cx="12" cy="9.5" r="1.6" fill="{color}"/>'
        ),
        "clock": (
            f'<circle cx="12" cy="12" r="9" {sf}/>'
            f'<circle cx="12" cy="12" r="9" {st}/>'
            f'<path d="M12 7v5l3.5 2.2" {st}/>'
        ),
        "flame": (
            f'<path d="M12 2.5c2 3 6 5.5 6 10.5a6 6 0 0 1-12 0c0-2.5 1.4-4.8 3-6.5.2 2 1.2 3 2 3 0-2.5.4-5 1-7z" {sf}/>'
            f'<path d="M12 2.5c2 3 6 5.5 6 10.5a6 6 0 0 1-12 0c0-2.5 1.4-4.8 3-6.5.2 2 1.2 3 2 3 0-2.5.4-5 1-7z" {st}/>'
        ),
        "layers": (
            f'<polygon points="12 3 21 8 12 13 3 8 12 3" {sf}/>'
            f'<polygon points="12 3 21 8 12 13 3 8 12 3" {st}/>'
            f'<polyline points="3 12.5 12 17.5 21 12.5" {st}/>'
            f'<polyline points="3 16.5 12 21.5 21 16.5" {st}/>'
        ),
        "graph": (
            f'<circle cx="6" cy="7" r="2.4" {sf}/><circle cx="18" cy="6" r="2.4" {sf}/><circle cx="12" cy="18" r="2.4" {sf}/>'
            f'<circle cx="6" cy="7" r="2.4" {st}/><circle cx="18" cy="6" r="2.4" {st}/><circle cx="12" cy="18" r="2.4" {st}/>'
            f'<path d="M8.4 6.8l7.2-.6M7.3 9.1l3.5 6.8M16.8 8.1l-3.6 7.8" {st}/>'
        ),
        "audit": (
            f'<rect x="5" y="3" width="14" height="18" rx="2" {sf}/>'
            f'<rect x="5" y="3" width="14" height="18" rx="2" {st}/>'
            f'<path d="M8.5 8h7M8.5 12h7M8.5 16h4" {st}/>'
            f'<circle cx="15.5" cy="16" r="1.4" fill="{color}"/>'
        ),
        "cube": (
            f'<polygon points="12 3 20 7.5 20 16.5 12 21 4 16.5 4 7.5 12 3" {sf}/>'
            f'<polygon points="12 3 20 7.5 20 16.5 12 21 4 16.5 4 7.5 12 3" {st}/>'
            f'<path d="M12 12l8-4.5M12 12L4 7.5M12 12v9" {st}/>'
        ),
        "code": (
            f'<path d="M8.5 7L3.5 12l5 5M15.5 7l5 5-5 5M13.5 5l-3 14" {st}/>'
        ),
    }
    body = shapes.get(name, shapes["cube"])
    return f'<g transform="translate({tx:.1f},{ty:.1f}) scale({s:.3f})">{body}</g>'


def icon_badge(
    name: str,
    cx: float,
    cy: float,
    r: float = 26,
    color: str = C_TEAL,
    bg: str | None = None,
) -> str:
    """Circular badge housing a crisp vector icon."""
    fill = bg or (
        C_CLAY_L if color == C_CLAY else
        C_GREEN_L if color == C_GREEN else
        C_GOLD_L if color == C_GOLD else
        C_RED_L if color == C_RED else
        C_TEAL_L
    )
    return (
        f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{r:.1f}" fill="{fill}" stroke="{color}" stroke-width="1.5" stroke-opacity="0.35"/>'
        f'{icon(name, cx, cy, size=r * 1.15, color=color)}'
    )


def ring_gauge(
    cx: float,
    cy: float,
    r: float,
    pct: float,
    val_txt: str,
    lbl_txt: str,
    color: str = C_TEAL,
) -> str:
    """Clean SVG circular progress ring with center value and bottom label."""
    circ = 2 * math.pi * r
    dash = max(2.0, circ * max(0.0, min(100.0, pct)) / 100.0)
    return (
        f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{r:.1f}" fill="none" stroke="{C_LINE}" stroke-width="8"/>'
        f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{r:.1f}" fill="none" stroke="{color}" stroke-width="8" '
        f'stroke-dasharray="{dash:.1f} {circ:.1f}" stroke-linecap="round" transform="rotate(-90 {cx:.1f} {cy:.1f})"/>'
        f'<text x="{cx:.1f}" y="{cy + 7:.1f}" font-family="var(--f-m)" font-size="{int(r * 0.44)}" fill="{color}" '
        f'font-weight="700" text-anchor="middle">{esc(val_txt)}</text>'
        f'<text x="{cx:.1f}" y="{cy + r + 24:.1f}" font-family="var(--f-s)" font-size="13.5" fill="{C_INK}" '
        f'font-weight="600" text-anchor="middle">{esc(lbl_txt)}</text>'
    )


# ==============================================================================
# 1. VISUAL COVER HERO (1600 x 900) — Theme-Specific Hero Illustration
# ==============================================================================
def _cover_illustration(theme: str) -> str:
    """Right-side hero illustration (x: 760..1536, y: 100..780) tailored to deck theme."""
    parts = [
        '<g transform="translate(770,96)">',
        f'<rect x="0" y="0" width="760" height="676" rx="22" fill="{C_WHITE}" stroke="{C_LINE}" stroke-width="1.4"/>',
        # subtle blueprint grid inside illustration frame
        f'<g opacity="0.35" stroke="{C_LINE}" stroke-width="0.7">'
        + "".join(f'<line x1="{gx}" y1="0" x2="{gx}" y2="676"/>' for gx in range(76, 760, 76))
        + "".join(f'<line x1="0" y1="{gy}" x2="760" y2="{gy}"/>' for gy in range(76, 676, 76))
        + "</g>",
    ]

    if theme in ("roadshow", "brain_body"):
        # Brain-Body Separation Hero: 3 Hot-Swappable Brains plugging into 1 Persistent Host Body + 6 Organ Ring
        parts.append(f'<text x="380" y="42" font-family="var(--f-m)" font-size="12" fill="{C_MUTE}" text-anchor="middle" letter-spacing="2.4">TRANSIENT BRAINS · 热插拔瞬态大脑</text>')
        brains = [("Claude Code", 145, C_TEAL), ("Antigravity", 380, C_CLAY), ("Cursor / Cron", 615, C_GREEN)]
        for name, bx, col in brains:
            parts.append(f'<rect x="{bx - 88}" y="62" width="176" height="72" rx="14" fill="{C_PAPER}" stroke="{col}" stroke-width="2"/>')
            parts.append(icon_badge("brain", bx - 54, 98, 20, col))
            parts.append(f'<text x="{bx + 14}" y="103" font-family="var(--f-s)" font-size="14.5" fill="{C_INK}" font-weight="700" text-anchor="middle">{esc(name)}</text>')
            # Plug wire down into bus
            parts.append(f'<line x1="{bx}" y1="134" x2="{bx}" y2="196" stroke="{col}" stroke-width="2.4" stroke-dasharray="6 5" class="flow-line"/>')
            parts.append(f'<rect x="{bx - 12}" y="176" width="24" height="16" rx="4" fill="{col}"/>')
        # Unified Neural Bus
        parts.append(f'<rect x="52" y="196" width="656" height="46" rx="12" fill="{C_TEAL_D}"/>')
        parts.append(f'<text x="380" y="224" font-family="var(--f-m)" font-size="14" fill="{C_PAPER}" font-weight="600" text-anchor="middle">统一神经总线 · 43 MCP · 30 CLI · Hooks · Web</text>')
        # Host Body chassis with 6 organs orbiting central core
        parts.append(f'<rect x="52" y="274" width="656" height="366" rx="22" fill="{C_TEAL_L}" fill-opacity="0.45" stroke="{C_TEAL}" stroke-width="2"/>')
        parts.append(f'<line x1="380" y1="242" x2="380" y2="274" stroke="{C_TEAL_D}" stroke-width="3.5"/>')
        parts.append(f'<text x="380" y="304" font-family="var(--f-m)" font-size="12" fill="{C_TEAL}" font-weight="700" text-anchor="middle" letter-spacing="2">PERSISTENT HOST BODY · 永驻物理身体</text>')
        # 6 Organs in hex orbit (draw spokes first behind central core, then core, then organ cards)
        cx, cy = 380, 466
        organs = [
            ("感觉神经", "事实图谱", "eye", C_TEAL, 165, 376),
            ("小脑反射", "AOPCC / 锁", "bolt", C_CLAY, 595, 376),
            ("海马体", "记忆 / 胶囊", "capsule", C_GREEN, 130, 480),
            ("规约脊柱", "放置 / 防爆", "spine", C_GOLD, 630, 480),
            ("免疫代谢", "安全 / AWR", "shield", C_TEAL_2, 185, 582),
            ("多体联邦", "Mesh v1.0", "mesh", C_TEAL_D, 575, 582),
        ]
        for _, _, _, col, ox, oy in organs:
            parts.append(f'<line x1="{cx}" y1="{cy}" x2="{ox}" y2="{oy}" stroke="{col}" stroke-width="2" stroke-opacity="0.45"/>')
        # Central body core
        parts.append(f'<circle cx="{cx}" cy="{cy}" r="66" fill="{C_TEAL_D}"/>')
        parts.append(f'<circle cx="{cx}" cy="{cy}" r="76" fill="none" stroke="{C_TEAL_M}" stroke-width="1.6" stroke-dasharray="5 5"/>')
        parts.append(icon("body", cx, cy - 10, 38, C_PAPER))
        parts.append(f'<text x="{cx}" y="{cy + 34}" font-family="var(--f-r)" font-size="16" fill="{C_PAPER}" font-weight="700" text-anchor="middle">Agent 本体</text>')
        for ot, os, ic, col, ox, oy in organs:
            parts.append(f'<rect x="{ox - 84}" y="{oy - 32}" width="168" height="64" rx="14" fill="{C_WHITE}" stroke="{col}" stroke-width="1.6"/>')
            parts.append(icon_badge(ic, ox - 52, oy, 20, col))
            parts.append(f'<text x="{ox - 20}" y="{oy - 4}" font-family="var(--f-s)" font-size="14.5" fill="{C_INK}" font-weight="700">{esc(ot)}</text>')
            parts.append(f'<text x="{ox - 20}" y="{oy + 16}" font-family="var(--f-m)" font-size="11" fill="{col}">{esc(os)}</text>')

    elif theme == "tech_arch":
        # Microkernel + Layered Architecture Blueprint
        parts.append(f'<text x="380" y="42" font-family="var(--f-m)" font-size="12" fill="{C_MUTE}" text-anchor="middle" letter-spacing="2.2">MICROKERNEL &amp; LAYERED BLUEPRINT</text>')
        # Top interface pills
        for lbl, x, col in [("43 MCP", 130, C_TEAL), ("30 CLI", 296, C_TEAL_2), ("7 Hooks", 464, C_CLAY), ("Web UI", 630, C_GREEN)]:
            parts.append(f'<rect x="{x - 66}" y="64" width="132" height="44" rx="22" fill="{col}"/>')
            parts.append(f'<text x="{x}" y="91" font-family="var(--f-m)" font-size="14" fill="{C_PAPER}" font-weight="700" text-anchor="middle">{lbl}</text>')
            parts.append(f'<line x1="{x}" y1="108" x2="{x}" y2="148" stroke="{col}" stroke-width="2" stroke-dasharray="5 4" class="flow-line"/>')
        # Central Microkernel Hexagon + Orbiting Contrib Plugins
        cx, cy = 380, 326
        parts.append(f'<circle cx="{cx}" cy="{cy}" r="168" fill="{C_TEAL_L}" fill-opacity="0.35" stroke="{C_TEAL_M}" stroke-width="1.6" stroke-dasharray="7 5"/>')
        # 8 orbiting contrib chips
        contribs = ["Linux", "PG/DM", "JVM", "Nginx", "mTLS", "SIEM", "CMDB", "OTel"]
        for i, cname in enumerate(contribs):
            ang = math.radians(i * 45 - 90)
            px = cx + 168 * math.cos(ang)
            py = cy + 168 * math.sin(ang)
            parts.append(f'<line x1="{cx}" y1="{cy}" x2="{px:.1f}" y2="{py:.1f}" stroke="{C_TEAL_M}" stroke-width="1.6"/>')
            parts.append(f'<rect x="{px - 44:.1f}" y="{py - 18:.1f}" width="88" height="36" rx="10" fill="{C_WHITE}" stroke="{C_GREEN}" stroke-width="1.6"/>')
            parts.append(f'<text x="{px:.1f}" y="{py + 5:.1f}" font-family="var(--f-m)" font-size="12" fill="{C_GREEN}" font-weight="700" text-anchor="middle">{cname}</text>')
        # Hex center
        hex_pts = " ".join(f"{cx + 82 * math.cos(math.radians(60 * k - 30)):.1f},{cy + 82 * math.sin(math.radians(60 * k - 30)):.1f}" for k in range(6))
        parts.append(f'<polygon points="{hex_pts}" fill="{C_TEAL_D}"/>')
        parts.append(icon("gear", cx, cy - 16, 36, C_PAPER))
        parts.append(f'<text x="{cx}" y="{cy + 26}" font-family="var(--f-r)" font-size="17" fill="{C_PAPER}" font-weight="700" text-anchor="middle">Kernel</text>')
        parts.append(f'<text x="{cx}" y="{cy + 46}" font-family="var(--f-m)" font-size="11" fill="#B5D4D8" text-anchor="middle">3 Crates · 88 SPI</text>')
        # Bottom 3 storage engines (start stem cleanly below bottom mTLS chip at y=512)
        parts.append(f'<line x1="380" y1="514" x2="380" y2="538" stroke="{C_TEAL_D}" stroke-width="2.5"/>')
        parts.append(f'<line x1="160" y1="538" x2="600" y2="538" stroke="{C_TEAL_D}" stroke-width="2.5"/>')
        for db, sub, bx, col in [("SQLite WAL", "默认单机", 160, C_TEAL), ("PostgreSQL", "高并发向量", 380, C_GREEN), ("DuckDB", "列存 OLAP", 600, C_GOLD)]:
            parts.append(f'<line x1="{bx}" y1="536" x2="{bx}" y2="554" stroke="{col}" stroke-width="2.2"/>')
            parts.append(f'<rect x="{bx - 88}" y="554" width="176" height="82" rx="14" fill="{C_WHITE}" stroke="{col}" stroke-width="1.8"/>')
            parts.append(icon_badge("database", bx - 52, 595, 20, col))
            parts.append(f'<text x="{bx - 20}" y="590" font-family="var(--f-s)" font-size="14" fill="{C_INK}" font-weight="700">{db}</text>')
            parts.append(f'<text x="{bx - 20}" y="612" font-family="var(--f-s)" font-size="11.5" fill="{col}">{sub}</text>')

    elif theme == "sensory":
        # Host Radar + L1/L2/L3 Graph + CodeGraph Microscope
        cx, cy = 380, 328
        for r, op in [(230, 0.14), (165, 0.22), (100, 0.34)]:
            parts.append(f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="{C_TEAL_L}" fill-opacity="{op}" stroke="{C_TEAL}" stroke-width="1.3" stroke-dasharray="6 5"/>')
        parts.append(f'<line x1="{cx}" y1="{cy}" x2="{cx + 185}" y2="{cy - 135}" stroke="{C_TEAL}" stroke-width="2.6"/>')
        nodes = [
            ("Systemd", "L1 0.98", 180, 210, C_TEAL_D, "gear"),
            (":5432 PG", "L1 0.95", 570, 200, C_CLAY, "database"),
            ("/proc PID", "0.2 ms", 380, 328, C_TEAL, "radar"),
            ("Project", "L2 0.78", 190, 465, C_GREEN, "folder"),
            ("CodeGraph", "AST 联邦", 565, 465, C_GOLD, "microscope"),
        ]
        edges = [(0, 2, C_TEAL_D, ""), (2, 1, C_CLAY, ""), (3, 2, C_GREEN, "6 4"), (2, 4, C_GOLD, "4 4")]
        for a, b, col, da in edges:
            x1, y1 = nodes[a][2], nodes[a][3]
            x2, y2 = nodes[b][2], nodes[b][3]
            d_attr = f' stroke-dasharray="{da}"' if da else ""
            parts.append(f'<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{col}" stroke-width="2.8"{d_attr}/>')
        for nm, sb, nx, ny, col, ic in nodes:
            parts.append(f'<rect x="{nx - 78}" y="{ny - 34}" width="156" height="68" rx="16" fill="{C_WHITE}" stroke="{col}" stroke-width="2"/>')
            parts.append(icon_badge(ic, nx - 44, ny, 20, col))
            parts.append(f'<text x="{nx - 14}" y="{ny - 3}" font-family="var(--f-s)" font-size="14" fill="{C_INK}" font-weight="700">{nm}</text>')
            parts.append(f'<text x="{nx - 14}" y="{ny + 17}" font-family="var(--f-m)" font-size="11.5" fill="{col}" font-weight="600">{sb}</text>')
        parts.append(f'<rect x="110" y="578" width="540" height="58" rx="14" fill="{C_TEAL_D}"/>')
        parts.append(f'<text x="380" y="613" font-family="var(--f-m)" font-size="15" fill="{C_PAPER}" font-weight="600" text-anchor="middle">0.5% → 100% 连通 · 148 μs 三跳爆炸半径</text>')

    elif theme == "cerebellum":
        # Traffic Highway ETC Split + Reflex Arc
        parts.append(f'<text x="380" y="46" font-family="var(--f-m)" font-size="12" fill="{C_MUTE}" text-anchor="middle" letter-spacing="2.2">AOPCC · 自适应并发与反射弧</text>')
        # Left start node
        parts.append(f'<rect x="52" y="264" width="142" height="128" rx="18" fill="{C_TEAL_D}"/>')
        parts.append(icon("brain", 123, 306, 40, C_PAPER))
        parts.append(f'<text x="123" y="356" font-family="var(--f-s)" font-size="15" fill="{C_PAPER}" font-weight="700" text-anchor="middle">Agent 动作</text>')
        # 3 Lanes branching out
        lanes = [
            ("ETC 乐观直通", "psql / tmp · 0 额外轮", 142, C_GREEN, C_GREEN_L, "bolt"),
            ("CAS 谨慎校验", "常规新建 · 轻核验", 328, C_GOLD, C_GOLD_L, "check"),
            ("悲观红灯门禁", "bind / 锁 · 1ms 拦截", 514, C_RED, C_RED_L, "lock"),
        ]
        for lt, ls, ly, col, bg, ic in lanes:
            parts.append(f'<path d="M 194,328 C 254,328 264,{ly} 330,{ly}" fill="none" stroke="{col}" stroke-width="4" stroke-dasharray="8 6" class="flow-line"/>')
            parts.append(f'<rect x="330" y="{ly - 54}" width="376" height="108" rx="16" fill="{bg}" stroke="{col}" stroke-width="2"/>')
            parts.append(icon_badge(ic, 382, ly, 26, col, C_WHITE))
            parts.append(f'<text x="424" y="{ly - 8}" font-family="var(--f-r)" font-size="21" fill="{C_INK}" font-weight="700">{lt}</text>')
            parts.append(f'<text x="424" y="{ly + 20}" font-family="var(--f-m)" font-size="13" fill="{col}" font-weight="600">{ls}</text>')
        # Healing feedback arc on bottom
        parts.append(f'<path d="M 620,196 C 728,196 728,622 380,622 C 180,622 123,540 123,392" fill="none" stroke="{C_CLAY}" stroke-width="2.4" stroke-dasharray="6 5"/>')
        parts.append(f'<rect x="260" y="600" width="240" height="40" rx="20" fill="{C_CLAY}"/>')
        parts.append(f'<text x="380" y="625" font-family="var(--f-m)" font-size="13" fill="{C_PAPER}" font-weight="700" text-anchor="middle">Post-Hook &lt;2ms 碰壁秒愈</text>')

    elif theme == "hippocampus":
        # Glowing Continuity Capsule + Purifier Pipeline
        parts.append(f'<text x="380" y="46" font-family="var(--f-m)" font-size="12" fill="{C_MUTE}" text-anchor="middle" letter-spacing="2.2">HIPPOCAMPUS · 写穿保鲜 × 跨脑胶囊</text>')
        # Giant Capsule Hero in center
        parts.append(f'<rect x="130" y="138" width="500" height="176" rx="88" fill="{C_TEAL_L}" stroke="{C_TEAL}" stroke-width="3"/>')
        parts.append(f'<rect x="148" y="156" width="232" height="140" rx="70" fill="{C_TEAL_D}"/>')
        parts.append(icon("capsule", 226, 226, 56, C_PAPER))
        parts.append(f'<text x="316" y="218" font-family="var(--f-m)" font-size="28" fill="{C_PAPER}" font-weight="700" text-anchor="middle">150</text>')
        parts.append(f'<text x="316" y="246" font-family="var(--f-m)" font-size="12" fill="#B6D7DB" text-anchor="middle">TOKENS</text>')
        parts.append(f'<text x="498" y="206" font-family="var(--f-r)" font-size="22" fill="{C_INK}" font-weight="700" text-anchor="middle">跨脑接力胶囊</text>')
        parts.append(f'<text x="498" y="238" font-family="var(--f-s)" font-size="13.5" fill="{C_TEAL}" font-weight="600" text-anchor="middle">目标 · 变更 · 红线 · 脱敏</text>')
        parts.append(f'<text x="498" y="266" font-family="var(--f-m)" font-size="12" fill="{C_GREEN}" font-weight="700" text-anchor="middle">10ms 无缝唤醒</text>')
        # Brain relay track below
        for lbl, bx, col in [("上午 Claude", 145, C_TEAL), ("本体底座", 380, C_GREEN), ("下午 Antigravity", 615, C_CLAY)]:
            parts.append(f'<rect x="{bx - 88}" y="394" width="176" height="92" rx="16" fill="{C_WHITE}" stroke="{col}" stroke-width="2"/>')
            parts.append(icon_badge("database" if bx == 380 else "brain", bx, 426, 20, col))
            parts.append(f'<text x="{bx}" y="468" font-family="var(--f-s)" font-size="14" fill="{C_INK}" font-weight="700" text-anchor="middle">{lbl}</text>')
        parts.append(f'<line x1="233" y1="440" x2="292" y2="440" stroke="{C_TEAL}" stroke-width="3" stroke-dasharray="6 4" class="flow-line"/>')
        parts.append(f'<line x1="468" y1="440" x2="527" y2="440" stroke="{C_GREEN}" stroke-width="3" stroke-dasharray="6 4" class="flow-line"/>')
        # Bottom freshness ring meters
        parts.append(ring_gauge(210, 570, 40, 100.0, "100%", "写穿鲜活度", C_GREEN))
        parts.append(ring_gauge(380, 570, 40, 99.5, "99.5%", "本地向量覆盖", C_TEAL))
        parts.append(ring_gauge(550, 570, 40, 92.7, "-92.7%", "接力省 Token", C_CLAY))

    elif theme == "security":
        # Concentric Security Shields + Flywheel
        cx, cy = 380, 320
        rings = [
            (236, C_RED_L, C_RED, "L3 绝对禁区"),
            (182, C_GOLD_L, C_GOLD, "L2 审批防线"),
            (128, C_TEAL_L, C_TEAL, "L1 稳态自愈"),
            (74, C_GREEN_L, C_GREEN, "L0 零阻感知"),
        ]
        for r, fill, col, lbl in rings:
            parts.append(f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="{fill}" stroke="{col}" stroke-width="2"/>')
            if r > 80:
                parts.append(f'<text x="{cx}" y="{cy - r + 24}" font-family="var(--f-s)" font-size="13" fill="{col}" font-weight="700" text-anchor="middle">{lbl}</text>')
        parts.append(icon("shield", cx, cy - 10, 44, C_GREEN))
        parts.append(f'<text x="{cx}" y="{cy + 34}" font-family="var(--f-m)" font-size="12" fill="{C_GREEN}" font-weight="700" text-anchor="middle">DQ4 = 100%</text>')
        # 4 corner badges
        corners = [
            ("凭据零落盘", "SecretRedactor", 125, 118, C_CLAY, "shield"),
            ("双域全审计", "IN + OUT 留痕", 635, 118, C_TEAL, "audit"),
            ("P1~P5 飞轮", "探针自进化", 125, 560, C_GREEN, "flywheel"),
            ("Mesh v1.0", "HMAC 防重放", 635, 560, C_TEAL_D, "mesh"),
        ]
        for ct, cs, bx, by, col, ic in corners:
            parts.append(f'<rect x="{bx - 96}" y="{by - 38}" width="192" height="76" rx="14" fill="{C_WHITE}" stroke="{col}" stroke-width="1.8"/>')
            parts.append(icon_badge(ic, bx - 62, by, 20, col))
            parts.append(f'<text x="{bx - 30}" y="{by - 4}" font-family="var(--f-s)" font-size="14" fill="{C_INK}" font-weight="700">{ct}</text>')
            parts.append(f'<text x="{bx - 30}" y="{by + 18}" font-family="var(--f-m)" font-size="11.5" fill="{col}">{cs}</text>')

    else:  # benchmark
        # Pareto Quadrant Hero Chart
        parts.append(f'<text x="380" y="44" font-family="var(--f-m)" font-size="12" fill="{C_MUTE}" text-anchor="middle" letter-spacing="2.2">PARETO FRONTIER · BENCHMARK v3.9</text>')
        # Axes
        parts.append(f'<rect x="86" y="76" width="604" height="390" rx="12" fill="{C_PAPER}"/>')
        # Sweet spot corner
        parts.append(f'<rect x="440" y="76" width="250" height="156" rx="12" fill="{C_GREEN_L}"/>')
        parts.append(f'<text x="565" y="106" font-family="var(--f-m)" font-size="11.5" fill="{C_GREEN}" font-weight="700" text-anchor="middle">PARETO OPTIMUM</text>')
        parts.append(f'<line x1="86" y1="466" x2="690" y2="466" stroke="{C_INK}" stroke-width="2"/>')
        parts.append(f'<line x1="86" y1="466" x2="86" y2="76" stroke="{C_INK}" stroke-width="2"/>')
        parts.append(f'<text x="388" y="494" font-family="var(--f-s)" font-size="13" fill="{C_INK2}" text-anchor="middle">Prompt Token 节省率 (0% → 100%) →</text>')
        parts.append(f'<text x="54" y="270" font-family="var(--f-s)" font-size="13" fill="{C_INK2}" text-anchor="middle" transform="rotate(-90 54 270)">上下文质量分 Q (0 → 100) →</text>')
        # Baseline red dot -> Leap curve -> Agent Body green star
        parts.append(f'<circle cx="152" cy="364" r="18" fill="{C_RED_L}" stroke="{C_RED}" stroke-width="2.5"/>')
        parts.append(f'<circle cx="152" cy="364" r="7" fill="{C_RED}"/>')
        parts.append(f'<text x="152" y="404" font-family="var(--f-s)" font-size="13" fill="{C_RED}" font-weight="700" text-anchor="middle">裸奔大脑 · 35.6 分</text>')
        parts.append(f'<path d="M 168,354 C 280,260 410,160 618,142" fill="none" stroke="{C_GREEN}" stroke-width="4" stroke-dasharray="8 6" class="flow-line"/>')
        parts.append(f'<circle cx="628" cy="142" r="24" fill="{C_GREEN}" fill-opacity="0.18" stroke="{C_GREEN}" stroke-width="2.5"/>')
        parts.append(f'<circle cx="628" cy="142" r="10" fill="{C_GREEN}"/>')
        parts.append(f'<text x="588" y="196" font-family="var(--f-s)" font-size="15" fill="{C_GREEN}" font-weight="700" text-anchor="middle">Agent Body · 98.0 分</text>')
        parts.append(f'<text x="588" y="218" font-family="var(--f-m)" font-size="12.5" fill="{C_TEAL}" font-weight="700" text-anchor="middle">-99.23% Token · 94.5x 密度</text>')
        # Bottom KPI rings
        parts.append(ring_gauge(170, 568, 40, 99.2, "-99.2%", "Token 节省", C_TEAL))
        parts.append(ring_gauge(380, 568, 40, 98.0, "98.0", "质量得分 Q", C_GREEN))
        parts.append(ring_gauge(590, 568, 40, 100.0, "39/39", "全维门禁 PASS", C_CLAY))

    parts.append("</g>")
    return "".join(parts)


def vis_cover_visual(
    kicker: str,
    title: str,
    en_sub: str,
    tagline: str,
    badges: list[tuple[str, str, str]],
    theme: str = "roadshow",
    right_tag: str = "图解演说 · v1.1.0",
) -> str:
    """Visual-first 1600x900 Cover with left bold typography + right bespoke vector illustration."""
    title_fs = 44 if len(title) > 13 else (52 if len(title) > 8 else 64)
    parts = [
        '<svg class="fig cover-svg" viewBox="0 0 1600 900" width="1600" height="900" xmlns="http://www.w3.org/2000/svg">',
        # Left vertical architectural accent strip
        f'<rect x="64" y="102" width="6" height="62" rx="3" fill="{C_CLAY}"/>',
        f'<rect x="86" y="108" width="490" height="30" rx="15" fill="{C_TEAL_L}"/>',
        f'<text x="102" y="128" font-family="var(--f-m)" font-size="12.5" fill="{C_TEAL}" font-weight="700" letter-spacing="1.6">{esc(kicker)}</text>',
        f'<text x="64" y="262" font-family="var(--f-r)" font-size="{title_fs}" fill="{C_INK}" font-weight="700" letter-spacing="0.6">{esc(title)}</text>',
        f'<text x="64" y="314" font-family="var(--f-m)" font-size="15.5" fill="{C_TEAL}" letter-spacing="0.5">{esc(en_sub)}</text>',
        f'<line x1="64" y1="354" x2="680" y2="354" stroke="{C_LINE}" stroke-width="1.4"/>',
        f'<text x="64" y="410" font-family="var(--f-s)" font-size="22" fill="{C_INK}" font-weight="600">{esc(tagline)}</text>',
    ]
    # Hero metric badges (3 or 4 crisp cards in a 2x2 or 3x1 grid)
    nb = len(badges)
    if nb <= 3:
        bw = 204
        for i, (val, lbl, col) in enumerate(badges):
            bx = 64 + i * (bw + 16)
            parts.append(f'<rect x="{bx}" y="476" width="{bw}" height="114" rx="14" fill="{C_WHITE}" stroke="{C_LINE}" stroke-width="1.3"/>')
            parts.append(f'<rect x="{bx}" y="476" width="{bw}" height="5" rx="2" fill="{col}"/>')
            parts.append(f'<text x="{bx + 18}" y="532" font-family="var(--f-m)" font-size="28" fill="{col}" font-weight="700">{esc(val)}</text>')
            parts.append(f'<text x="{bx + 18}" y="566" font-family="var(--f-s)" font-size="14" fill="{C_INK2}" font-weight="600">{esc(lbl)}</text>')
    else:
        bw = 314
        for i, (val, lbl, col) in enumerate(badges[:4]):
            bx = 64 + (i % 2) * (bw + 18)
            by = 460 + (i // 2) * 116
            parts.append(f'<rect x="{bx}" y="{by}" width="{bw}" height="98" rx="14" fill="{C_WHITE}" stroke="{C_LINE}" stroke-width="1.3"/>')
            parts.append(f'<rect x="{bx}" y="{by}" width="6" height="98" rx="3" fill="{col}"/>')
            parts.append(f'<text x="{bx + 24}" y="{by + 48}" font-family="var(--f-m)" font-size="27" fill="{col}" font-weight="700">{esc(val)}</text>')
            parts.append(f'<text x="{bx + 24}" y="{by + 78}" font-family="var(--f-s)" font-size="14" fill="{C_INK2}" font-weight="600">{esc(lbl)}</text>')

    parts.append(f'<text x="64" y="794" font-family="var(--f-s)" font-size="14.5" fill="{C_MUTE}" font-weight="500">digoal 德哥 · 2026-11 · Apache-2.0</text>')
    parts.append(f'<text x="1530" y="810" font-family="var(--f-m)" font-size="12" fill="{C_MUTE}" text-anchor="end">{esc(right_tag)}</text>')
    parts.append(_cover_illustration(theme))
    parts.append("</svg>")
    return "".join(parts)


# ==============================================================================
# 2. DOMAIN BESPOKE VISUAL DIAGRAMS (1472 x 640)
# ==============================================================================

def vis_brain_body_anatomy(animated: bool = True) -> str:
    """Full-stage Brain-Body Separation Anatomical Diagram with hot-swappable plug sockets & 6 organs."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("bba")]

    # Step 1: Cloud Brains Row (Hot-swappable)
    parts.append(s(1))
    parts.append(f'<rect x="16" y="8" width="1440" height="136" rx="18" fill="{C_PAPER2}" stroke="{C_LINE}" stroke-width="1.3" stroke-dasharray="7 5"/>')
    parts.append(f'<text x="42" y="34" font-family="var(--f-m)" font-size="12" fill="{C_MUTE}" font-weight="700" letter-spacing="2">CLOUD BRAINS · 可热切换的瞬态大脑（无状态算力）</text>')
    brains = [
        ("Claude Code", "上午重构", 205, C_TEAL),
        ("Antigravity", "下午联调", 555, C_CLAY),
        ("Cursor / Codex", "随时接手", 905, C_GREEN),
        ("凌晨 Cron", "无头巡检", 1255, C_GOLD),
    ]
    for name, role, bx, col in brains:
        parts.append(f'<rect x="{bx - 132}" y="48" width="264" height="78" rx="14" fill="{C_WHITE}" stroke="{col}" stroke-width="2" filter="url(#bba_sh)"/>')
        parts.append(icon_badge("brain", bx - 84, 87, 23, col))
        parts.append(f'<text x="{bx - 48}" y="82" font-family="var(--f-s)" font-size="16" fill="{C_INK}" font-weight="700">{name}</text>')
        parts.append(f'<text x="{bx - 48}" y="106" font-family="var(--f-m)" font-size="12.5" fill="{col}" font-weight="600">{role}</text>')
        # plug connectors going down
        parts.append(f'<line x1="{bx}" y1="126" x2="{bx}" y2="192" stroke="{col}" stroke-width="2.8" stroke-dasharray="7 5" class="flow-line"/>')
        parts.append(f'<rect x="{bx - 16}" y="170" width="32" height="20" rx="5" fill="{col}"/>')
    parts.append("</g>")

    # Step 2: Neural Spine Bus
    parts.append(s(2))
    parts.append(f'<rect x="16" y="192" width="1440" height="60" rx="14" fill="{C_TEAL_D}"/>')
    parts.append(icon("plug", 62, 222, 28, C_PAPER))
    parts.append(f'<text x="92" y="228" font-family="var(--f-r)" font-size="20" fill="{C_PAPER}" font-weight="700">统一神经接口总线</text>')
    bus_pills = [("43 MCP 工具", 470), ("30 CLI 命令", 690), ("7 大硬 Hook", 910), ("Web 9 工作台", 1130), ("10ms 接力", 1340)]
    for lbl, px in bus_pills:
        parts.append(f'<rect x="{px - 88}" y="205" width="176" height="34" rx="17" fill="#27697D"/>')
        parts.append(f'<text x="{px}" y="227" font-family="var(--f-m)" font-size="13" fill="{C_PAPER}" font-weight="600" text-anchor="middle">{lbl}</text>')
    parts.append("</g>")

    # Step 3: Persistent Host Body Container + 6 Organs in 3x2 Anatomical Grid
    parts.append(s(3))
    parts.append(f'<rect x="16" y="282" width="1440" height="346" rx="22" fill="{C_TEAL_L}" fill-opacity="0.4" stroke="{C_TEAL}" stroke-width="2"/>')
    parts.append(f'<line x1="736" y1="252" x2="736" y2="282" stroke="{C_TEAL_D}" stroke-width="4"/>')
    parts.append(f'<text x="44" y="314" font-family="var(--f-m)" font-size="12.5" fill="{C_TEAL_D}" font-weight="700" letter-spacing="2">PERSISTENT HOST BODY · 永驻宿主机的六大生理器官（Rust 单二进制 · 0 云端 Token）</text>')
    organs = [
        ("01 感觉神经", "物理事实图谱", "/proc · 端口 · CodeGraph", "eye", C_TEAL_D, 44, 336),
        ("02 小脑反射", "AOPCC 与协作锁", "乐观直通 · <2ms 自愈 · WFG", "bolt", C_CLAY, 514, 336),
        ("03 海马体", "保鲜与跨脑接力", "写穿失效 · <=300t 胶囊", "capsule", C_GREEN, 984, 336),
        ("04 规约脊柱", "放置与防爆路由", "41μs 放置 · 60万→358t", "spine", C_GOLD, 44, 482),
        ("05 免疫代谢", "安全审计与 AWR", "脱敏 0 漏 · 18 等待事件", "shield", C_TEAL_2, 514, 482),
        ("06 多体联邦", "跨机零信任协同", "HMAC 签名 · 沙盒收件箱", "mesh", C_TEAL, 984, 482),
    ]
    for tag, title, metric, ic, col, ox, oy in organs:
        parts.append(f'<rect x="{ox}" y="{oy}" width="444" height="124" rx="16" fill="{C_WHITE}" stroke="{col}" stroke-width="1.8" filter="url(#bba_sh)"/>')
        parts.append(f'<rect x="{ox}" y="{oy}" width="8" height="124" rx="4" fill="{col}"/>')
        parts.append(icon_badge(ic, ox + 56, oy + 62, 30, col))
        parts.append(f'<text x="{ox + 104}" y="{oy + 34}" font-family="var(--f-m)" font-size="12" fill="{col}" font-weight="700">{tag}</text>')
        parts.append(f'<text x="{ox + 104}" y="{oy + 68}" font-family="var(--f-r)" font-size="22" fill="{C_INK}" font-weight="700">{title}</text>')
        parts.append(f'<rect x="{ox + 104}" y="{oy + 84}" width="314" height="26" rx="13" fill="{C_PAPER2}"/>')
        parts.append(f'<text x="{ox + 118}" y="{oy + 102}" font-family="var(--f-m)" font-size="12.5" fill="{C_INK2}" font-weight="600">{metric}</text>')
    parts.append("</g>")

    parts.append("</svg>")
    return "".join(parts)


def vis_comparative_scale(animated: bool = True) -> str:
    """Comparative Advantage Visual Balance: Expensive Cloud Brain vs Sub-ms Local Body + Cost Bars."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("cs")]

    # Step 1: Cloud Brain Card (Left)
    parts.append(s(1))
    parts.append(f'<rect x="16" y="20" width="446" height="420" rx="20" fill="{C_WHITE}" stroke="{C_CLAY}" stroke-width="2" filter="url(#cs_sh)"/>')
    parts.append(f'<rect x="16" y="20" width="446" height="68" rx="18" fill="{C_CLAY_L}"/>')
    parts.append(icon_badge("brain", 74, 54, 24, C_CLAY, C_WHITE))
    parts.append(f'<text x="114" y="50" font-family="var(--f-r)" font-size="23" fill="{C_INK}" font-weight="700">云端瞬态大脑 (LLM)</text>')
    parts.append(f'<text x="114" y="74" font-family="var(--f-m)" font-size="12" fill="{C_CLAY}" font-weight="600">稀缺资源：推理注意力 × 上下文窗口</text>')
    # Visual trait rows with icons
    b_rows = [
        ("check", C_GREEN, "高杠杆架构推理与代码创造"),
        ("cross", C_RED, "每次会话冷启动完全失忆"),
        ("cross", C_RED, "单轮探索税 2~4 秒 + 20k Token"),
        ("cross", C_RED, "看不见 /proc、端口与后台 Cron"),
    ]
    for i, (ic, col, txt) in enumerate(b_rows):
        ry = 136 + i * 72
        parts.append(f'<rect x="42" y="{ry - 24}" width="394" height="54" rx="12" fill="{C_PAPER}"/>')
        parts.append(icon(ic, 70, ry + 3, 24, col))
        parts.append(f'<text x="98" y="{ry + 9}" font-family="var(--f-s)" font-size="15.5" fill="{C_INK}" font-weight="600">{txt}</text>')
    parts.append("</g>")

    # Step 2: Center Visual Balance + Logarithmic Cost Contrast
    parts.append(s(2))
    cx = 736
    parts.append(f'<rect x="492" y="20" width="488" height="420" rx="20" fill="{C_TEAL_L}" fill-opacity="0.42" stroke="{C_TEAL}" stroke-width="2"/>')
    parts.append(icon_badge("scale", cx, 84, 34, C_TEAL, C_WHITE))
    parts.append(f'<text x="{cx}" y="148" font-family="var(--f-r)" font-size="22" fill="{C_INK}" font-weight="700" text-anchor="middle">比较优势 · 数量级代差</text>')
    # Visual Bar 1: Latency
    parts.append(f'<text x="530" y="198" font-family="var(--f-s)" font-size="14" fill="{C_INK2}" font-weight="700">物理感知延迟对比</text>')
    parts.append(f'<rect x="530" y="210" width="380" height="28" rx="8" fill="{C_CLAY}"/>')
    parts.append(f'<text x="544" y="229" font-family="var(--f-m)" font-size="13" fill="{C_PAPER}" font-weight="700">大脑盲探 ~3,000 ms</text>')
    parts.append(f'<rect x="530" y="246" width="24" height="28" rx="6" fill="{C_GREEN}"/>')
    parts.append(f'<text x="564" y="265" font-family="var(--f-m)" font-size="13.5" fill="{C_GREEN}" font-weight="700">本体直读 0.2 ms (快 15,000x)</text>')
    # Visual Bar 2: Token cost
    parts.append(f'<text x="530" y="316" font-family="var(--f-s)" font-size="14" fill="{C_INK2}" font-weight="700">探索期 Prompt Token 燃烧</text>')
    parts.append(f'<rect x="530" y="328" width="380" height="28" rx="8" fill="{C_CLAY}"/>')
    parts.append(f'<text x="544" y="347" font-family="var(--f-m)" font-size="13" fill="{C_PAPER}" font-weight="700">裸奔重扫 662,502 tok</text>')
    parts.append(f'<rect x="530" y="364" width="16" height="28" rx="6" fill="{C_TEAL}"/>')
    parts.append(f'<text x="556" y="383" font-family="var(--f-m)" font-size="13.5" fill="{C_TEAL}" font-weight="700">本体直供 5,114 tok (-99.23%)</text>')
    parts.append("</g>")

    # Step 3: Local Host Body Card (Right)
    parts.append(s(3))
    parts.append(f'<rect x="1010" y="20" width="446" height="420" rx="20" fill="{C_WHITE}" stroke="{C_TEAL}" stroke-width="2" filter="url(#cs_sh)"/>')
    parts.append(f'<rect x="1010" y="20" width="446" height="68" rx="18" fill="{C_TEAL_L}"/>')
    parts.append(icon_badge("body", 1068, 54, 24, C_TEAL, C_WHITE))
    parts.append(f'<text x="1108" y="50" font-family="var(--f-r)" font-size="23" fill="{C_INK}" font-weight="700">本地永驻本体 (Body)</text>')
    parts.append(f'<text x="1108" y="74" font-family="var(--f-m)" font-size="12" fill="{C_TEAL}" font-weight="600">核心优势：亚毫秒物理确定性 × 0 Token</text>')
    h_rows = [
        ("check", C_TEAL, "/proc 0.2ms 实时看清进程与端口"),
        ("check", C_TEAL, "Rust 单二进制 · 零云端 Token 税"),
        ("check", C_TEAL, "跨会话、跨大模型永驻状态不丢"),
        ("check", C_TEAL, "确定性防撞、脱敏、加锁与自愈"),
    ]
    for i, (ic, col, txt) in enumerate(h_rows):
        ry = 136 + i * 72
        parts.append(f'<rect x="1036" y="{ry - 24}" width="394" height="54" rx="12" fill="{C_PAPER}"/>')
        parts.append(icon(ic, 1064, ry + 3, 24, col))
        parts.append(f'<text x="1092" y="{ry + 9}" font-family="var(--f-s)" font-size="15.5" fill="{C_INK}" font-weight="600">{txt}</text>')
    parts.append("</g>")

    # Step 4: Bottom Synthesis Ribbon (3 Analogies + Outcome)
    parts.append(s(4))
    parts.append(f'<rect x="16" y="476" width="1440" height="142" rx="18" fill="{C_TEAL_D}"/>')
    analogs = [
        ("硅谷物业", "创业者写代码 · 物业管吃住", 205),
        ("决策幕僚", "主帅拍板 · 幕僚管档案避坑", 615),
        ("脑体分离", "大脑专攻创造 · 本体守护物理世界", 1075),
    ]
    for title, sub, ax in analogs:
        parts.append(f'<text x="{ax}" y="534" font-family="var(--f-r)" font-size="22" fill="{C_PAPER}" font-weight="700" text-anchor="middle">{title}</text>')
        parts.append(f'<text x="{ax}" y="570" font-family="var(--f-s)" font-size="14.5" fill="#B6D6DA" text-anchor="middle">{sub}</text>')
    parts.append(f'<line x1="410" y1="506" x2="410" y2="590" stroke="#397589" stroke-width="1.5"/>')
    parts.append(f'<line x1="820" y1="506" x2="820" y2="590" stroke="#397589" stroke-width="1.5"/>')
    parts.append(f'<rect x="1310" y="514" width="122" height="64" rx="12" fill="{C_GREEN}"/>')
    parts.append(f'<text x="1371" y="544" font-family="var(--f-m)" font-size="22" fill="{C_PAPER}" font-weight="700" text-anchor="middle">94.5x</text>')
    parts.append(f'<text x="1371" y="566" font-family="var(--f-s)" font-size="12" fill="{C_GREEN_L}" text-anchor="middle">信息密度跃升</text>')
    parts.append("</g>")

    parts.append("</svg>")
    return "".join(parts)


def vis_blind_agent_scene(animated: bool = True) -> str:
    """Visual Incident Scene: A blindfolded Brain crashing into 5 physical realities on the host."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("ba")]

    # Background Host Chassis
    parts.append(f'<rect x="20" y="16" width="1432" height="534" rx="24" fill="{C_PAPER2}" stroke="{C_LINE}" stroke-width="1.5"/>')
    parts.append(f'<text x="48" y="48" font-family="var(--f-m)" font-size="12" fill="{C_MUTE}" font-weight="700" letter-spacing="2">HOST REALITY SCENE · 无本体「蒙眼大脑」高频五连翻车现场</text>')

    # Central Blindfolded Brain
    cx, cy = 736, 286
    parts.append(s(1))
    parts.append(f'<circle cx="{cx}" cy="{cy}" r="96" fill="{C_RED_L}" stroke="{C_RED}" stroke-width="2.5" stroke-dasharray="7 5"/>')
    parts.append(f'<circle cx="{cx}" cy="{cy}" r="74" fill="{C_WHITE}" stroke="{C_RED}" stroke-width="2"/>')
    parts.append(icon("brain", cx, cy - 18, 46, C_RED))
    # Blindfold band across brain
    parts.append(f'<rect x="{cx - 42}" y="{cy - 26}" width="84" height="15" rx="7.5" fill="{C_INK}"/>')
    parts.append(f'<text x="{cx}" y="{cy + 28}" font-family="var(--f-r)" font-size="18.5" fill="{C_RED}" font-weight="700" text-anchor="middle">裸奔大脑</text>')
    parts.append(f'<text x="{cx}" y="{cy + 48}" font-family="var(--f-m)" font-size="11" fill="{C_INK2}" text-anchor="middle">只记聊天 · 不见宿主</text>')
    parts.append("</g>")

    # 5 Crash Nodes radiating outward from outer ring perimeter (never crossing center text)
    incidents = [
        ("01 重复造轮子", "不知已有 PG 源码 · 重 clone", "-30 分钟 / 爆磁盘", "server", 252, 142, 2),
        ("02 端口盲撞", "直撞 :5432 / :8080 占用", "进程崩溃 · +4 轮", "bolt", 1220, 142, 2),
        ("03 改崩上游", "误改共享脚本断 Cron 链", "后台静默宕机", "gear", 220, 408, 3),
        ("04 目录污染", "乱丢脚本 + 扫爆 node_modules", "60 万+ 噪音 Token", "folder", 1252, 408, 3),
        ("05 换脑失忆", "切 IDE / 断线即重头再来", "上下文 100% 清零", "clock", 736, 482, 4),
    ]
    for title, desc, cost, ic, ix, iy, stp in incidents:
        parts.append(s(stp))
        dx, dy = ix - cx, iy - cy
        dist = math.hypot(dx, dy) or 1.0
        sx = cx + (98.0 * dx / dist)
        sy = cy + (98.0 * dy / dist)
        ex = ix - (188.0 * dx / dist) if iy != 482 else ix
        ey = iy - (56.0 * dy / dist) if iy != 482 else (iy - 54.0)
        parts.append(f'<line x1="{sx:.1f}" y1="{sy:.1f}" x2="{ex:.1f}" y2="{ey:.1f}" stroke="{C_RED}" stroke-width="2.6" stroke-dasharray="6 5" marker-end="url(#ba_r)"/>')
        w = 376 if iy != 482 else 430
        h = 122 if iy != 482 else 98
        rx = ix - w / 2
        ry = iy - h / 2
        parts.append(f'<rect x="{rx:.1f}" y="{ry:.1f}" width="{w}" height="{h}" rx="16" fill="{C_WHITE}" stroke="{C_RED}" stroke-width="1.8" filter="url(#ba_sh)"/>')
        parts.append(icon_badge(ic, rx + 46, iy, 24, C_RED, C_RED_L))
        parts.append(f'<text x="{rx + 86}" y="{ry + 34}" font-family="var(--f-r)" font-size="19" fill="{C_INK}" font-weight="700">{title}</text>')
        parts.append(f'<text x="{rx + 86}" y="{ry + 62}" font-family="var(--f-s)" font-size="13.5" fill="{C_INK2}">{desc}</text>')
        badge_y = ry + 76 if iy != 482 else ry + 68
        if iy != 482:
            parts.append(f'<rect x="{rx + 86}" y="{badge_y}" width="210" height="26" rx="13" fill="{C_RED_L}"/>')
            parts.append(f'<text x="{rx + 100}" y="{badge_y + 18}" font-family="var(--f-m)" font-size="12" fill="{C_RED}" font-weight="700">{cost}</text>')
        else:
            parts.append(f'<rect x="{rx + 276}" y="{ry + 18}" width="136" height="28" rx="14" fill="{C_RED_L}"/>')
            parts.append(f'<text x="{rx + 344}" y="{ry + 37}" font-family="var(--f-m)" font-size="12" fill="{C_RED}" font-weight="700" text-anchor="middle">{cost}</text>')
        parts.append("</g>")

    # Bottom contrast banner
    parts.append(s(4))
    parts.append(f'<rect x="20" y="568" width="1432" height="58" rx="14" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="52" y="603" font-family="var(--f-r)" font-size="19.5" fill="{C_PAPER}" font-weight="700">记忆工具只管「聊过什么」—— Agent Body 让大脑睁眼看清整台宿主机。</text>')
    parts.append(f'<text x="1420" y="603" font-family="var(--f-m)" font-size="13" fill="#A6CFD5" text-anchor="end">50+ 记忆框架的共同盲区</text>')
    parts.append("</g>")
    parts.append("</svg>")
    return "".join(parts)


def vis_host_graph_federation(animated: bool = True) -> str:
    """True Directed Weighted Graph (L1/L2/L3 edges) + CodeGraph Microscope Lens."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("hg")]

    # Left Macro Topology Canvas (940 x 510)
    parts.append(f'<rect x="16" y="14" width="934" height="516" rx="20" fill="{C_WHITE}" stroke="{C_LINE}" stroke-width="1.5"/>')
    parts.append(f'<text x="42" y="46" font-family="var(--f-m)" font-size="12" fill="{C_TEAL}" font-weight="700" letter-spacing="1.8">MACRO GRAPH · 宿主机 L1 / L2 / L3 证据分层有向图</text>')

    # Step 1: L1 Physical Hard Edges (solid thick)
    parts.append(s(1))
    # Nodes: Systemd -> Process -> Port :5432, Cron -> Script
    parts.append(f'<line x1="208" y1="142" x2="386" y2="142" stroke="{C_TEAL_D}" stroke-width="4" marker-end="url(#hg_t)"/>')
    parts.append(f'<rect x="256" y="114" width="84" height="22" rx="11" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="298" y="129" font-family="var(--f-m)" font-size="11" fill="{C_PAPER}" font-weight="700" text-anchor="middle">L1 · 0.98</text>')

    parts.append(f'<line x1="566" y1="142" x2="736" y2="142" stroke="{C_TEAL_D}" stroke-width="4" marker-end="url(#hg_t)"/>')
    parts.append(f'<rect x="608" y="114" width="84" height="22" rx="11" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="650" y="129" font-family="var(--f-m)" font-size="11" fill="{C_PAPER}" font-weight="700" text-anchor="middle">L1 · 0.95</text>')

    for title, sub, nx, ny, col, ic in [
        ("Systemd Unit", "pg.service", 122, 142, C_TEAL_D, "gear"),
        ("活进程 PID", "/proc 直采", 476, 142, C_TEAL, "server"),
        ("端口 :5432", "Socket 监听", 826, 142, C_CLAY, "bolt"),
    ]:
        parts.append(f'<rect x="{nx - 86}" y="{ny - 38}" width="172" height="76" rx="14" fill="{C_TEAL_L}" stroke="{col}" stroke-width="2"/>')
        parts.append(icon_badge(ic, nx - 52, ny, 20, col, C_WHITE))
        parts.append(f'<text x="{nx - 22}" y="{ny - 4}" font-family="var(--f-s)" font-size="14" fill="{C_INK}" font-weight="700">{title}</text>')
        parts.append(f'<text x="{nx - 22}" y="{ny + 18}" font-family="var(--f-m)" font-size="11.5" fill="{col}">{sub}</text>')
    parts.append("</g>")

    # Step 2: L2 Structural Roll-up (medium dashed)
    parts.append(s(2))
    parts.append(f'<rect x="44" y="256" width="236" height="176" rx="14" fill="{C_PAPER}" stroke="{C_TEAL_2}" stroke-width="1.6" stroke-dasharray="6 4"/>')
    parts.append(f'<text x="162" y="282" font-family="var(--f-m)" font-size="11.5" fill="{C_TEAL_2}" font-weight="700" text-anchor="middle">4,985 条文件引用</text>')
    for fy, fname in [(312, "main.rs → schema.sql"), (352, "cron.sh → config.toml"), (392, "links.rs → storage")]:
        parts.append(f'<rect x="64" y="{fy - 14}" width="196" height="28" rx="6" fill="{C_WHITE}" stroke="{C_LINE}" stroke-width="1"/>')
        parts.append(f'<text x="162" y="{fy + 5}" font-family="var(--f-m)" font-size="11.5" fill="{C_INK2}" text-anchor="middle">{fname}</text>')
    # Roll-up arrow to Project node & bridge up to Process
    parts.append(f'<line x1="280" y1="344" x2="394" y2="344" stroke="{C_TEAL_2}" stroke-width="3" stroke-dasharray="8 4" marker-end="url(#hg_t)"/>')
    parts.append(f'<rect x="296" y="314" width="84" height="22" rx="11" fill="{C_TEAL_2}"/>')
    parts.append(f'<text x="338" y="329" font-family="var(--f-m)" font-size="11" fill="{C_PAPER}" font-weight="700" text-anchor="middle">L2 上卷</text>')
    parts.append(f'<line x1="476" y1="302" x2="476" y2="186" stroke="{C_TEAL_2}" stroke-width="2.6" stroke-dasharray="6 4" marker-end="url(#hg_t)"/>')
    parts.append(f'<rect x="394" y="302" width="166" height="84" rx="16" fill="{C_WHITE}" stroke="{C_GREEN}" stroke-width="2.2"/>')
    parts.append(icon_badge("folder", 434, 344, 21, C_GREEN))
    parts.append(f'<text x="466" y="338" font-family="var(--f-s)" font-size="15" fill="{C_INK}" font-weight="700">Project A</text>')
    parts.append(f'<text x="466" y="362" font-family="var(--f-m)" font-size="11.5" fill="{C_GREEN}" font-weight="600">L2 · 0.72</text>')
    parts.append("</g>")

    # Step 3: L3 Semantic Neighbors (dotted amber)
    parts.append(s(3))
    parts.append(f'<path d="M 560,344 Q 650,398 732,374" fill="none" stroke="{C_GOLD}" stroke-width="2.4" stroke-dasharray="3 5" marker-end="url(#hg_o)"/>')
    parts.append(f'<rect x="598" y="390" width="116" height="22" rx="11" fill="{C_GOLD_L}" stroke="{C_GOLD}" stroke-width="1"/>')
    parts.append(f'<text x="656" y="405" font-family="var(--f-m)" font-size="11" fill="{C_GOLD}" font-weight="700" text-anchor="middle">L3 cos&gt;=0.78</text>')
    parts.append(f'<rect x="738" y="330" width="184" height="80" rx="16" fill="{C_GOLD_L}" stroke="{C_GOLD}" stroke-width="1.8" stroke-dasharray="5 4"/>')
    parts.append(icon_badge("graph", 776, 370, 20, C_GOLD, C_WHITE))
    parts.append(f'<text x="806" y="365" font-family="var(--f-s)" font-size="14" fill="{C_INK}" font-weight="700">相似工程 Top3</text>')
    parts.append(f'<text x="806" y="387" font-family="var(--f-m)" font-size="11" fill="{C_GOLD}">严禁冒充硬依赖</text>')
    # Legend bar inside left canvas
    parts.append(f'<rect x="44" y="458" width="878" height="50" rx="12" fill="{C_PAPER}"/>')
    parts.append(f'<text x="66" y="488" font-family="var(--f-m)" font-size="13" fill="{C_TEAL_D}" font-weight="700">3 跳爆炸衰减公式：S = ∏ (w_e × 0.8^(hop-1))</text>')
    parts.append(f'<text x="900" y="488" font-family="var(--f-m)" font-size="13" fill="{C_GREEN}" font-weight="700" text-anchor="end">M1 P50 = 148 μs</text>')
    parts.append("</g>")

    # Step 4: Right Side — CodeGraph Microscope Lens + Live Connectivity Gauges
    parts.append(s(4))
    parts.append(f'<rect x="974" y="14" width="482" height="516" rx="20" fill="{C_WHITE}" stroke="{C_GREEN}" stroke-width="2"/>')
    parts.append(f'<text x="1002" y="46" font-family="var(--f-m)" font-size="12" fill="{C_GREEN}" font-weight="700" letter-spacing="1.8">MICRO FEDERATION · CodeGraph 显微镜</text>')
    # Bridge arrow from Project A to Microscope
    parts.append(f'<path d="M 560,322 C 740,260 880,224 1030,224" fill="none" stroke="{C_GREEN}" stroke-width="3" stroke-dasharray="7 5" class="flow-line" marker-end="url(#hg_g)"/>')
    # Microscope circular lens
    lcx, lcy = 1215, 224
    parts.append(f'<circle cx="{lcx}" cy="{lcy}" r="134" fill="{C_GREEN_L}" fill-opacity="0.55" stroke="{C_GREEN}" stroke-width="4"/>')
    parts.append(f'<circle cx="{lcx}" cy="{lcy}" r="122" fill="{C_WHITE}" stroke="{C_GREEN_M}" stroke-width="1.5"/>')
    # AST symbols inside lens
    ast_nodes = [("handle_req()", 1215, 146, C_TEAL_D), ("check_lock()", 1148, 226, C_CLAY), ("emit_audit()", 1282, 226, C_GREEN), ("sqlite_tx()", 1215, 302, C_TEAL)]
    parts.append(f'<line x1="1215" y1="162" x2="1148" y2="210" stroke="{C_INK2}" stroke-width="1.8"/>')
    parts.append(f'<line x1="1215" y1="162" x2="1282" y2="210" stroke="{C_INK2}" stroke-width="1.8"/>')
    parts.append(f'<line x1="1148" y1="242" x2="1215" y2="286" stroke="{C_INK2}" stroke-width="1.8"/>')
    parts.append(f'<line x1="1282" y1="242" x2="1215" y2="286" stroke="{C_INK2}" stroke-width="1.8"/>')
    for fn, fx, fy, fc in ast_nodes:
        parts.append(f'<rect x="{fx - 54}" y="{fy - 15}" width="108" height="30" rx="15" fill="{fc}"/>')
        parts.append(f'<text x="{fx}" y="{fy + 5}" font-family="var(--f-m)" font-size="11.5" fill="{C_PAPER}" font-weight="600" text-anchor="middle">{fn}</text>')
    # Bottom stats in right card
    parts.append(ring_gauge(1096, 434, 42, 100.0, "100%", "工程全连通 (216/216)", C_GREEN))
    parts.append(ring_gauge(1332, 434, 42, 99.5, "99.5%", "抗假阳性 DQ3", C_TEAL))
    parts.append("</g>")

    # Bottom takeaway ribbon
    parts.append(f'<rect x="16" y="556" width="1440" height="64" rx="14" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="48" y="594" font-family="var(--f-r)" font-size="19" fill="{C_PAPER}" font-weight="700">Agent Body 管宏观跨服务拓扑，CodeGraph 管微观 AST 调用链——绝不把「语义相似」当成「物理因果」。</text>')
    parts.append(f'<text x="1426" y="594" font-family="var(--f-m)" font-size="12.5" fill="#A6CFD5" text-anchor="end">DQ6 = 100.0%</text>')

    parts.append("</svg>")
    return "".join(parts)


def vis_tiered_scan_doors(animated: bool = True) -> str:
    """White / Black / Gray Tiered Scanning ('Knock, Don't Enter') + 3-Layer Metadata Pipeline."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("tsd")]

    # Left Host Disk Root
    parts.append(s(1))
    parts.append(f'<rect x="16" y="182" width="218" height="226" rx="20" fill="{C_TEAL_D}"/>')
    parts.append(icon("server", 125, 250, 56, C_PAPER))
    parts.append(f'<text x="125" y="318" font-family="var(--f-r)" font-size="22" fill="{C_PAPER}" font-weight="700" text-anchor="middle">宿主机磁盘</text>')
    parts.append(f'<text x="125" y="348" font-family="var(--f-m)" font-size="13" fill="#A6CFD5" text-anchor="middle">21,403 纳管资产</text>')
    parts.append(f'<text x="125" y="376" font-family="var(--f-m)" font-size="12" fill="#A6CFD5" text-anchor="middle">inotify 同步剪枝</text>')
    parts.append("</g>")

    # 3 Doors in center
    doors = [
        ("白名单 · 敞开深挖", "/shared_proj/  /work/", "全量元数据 + FTS5 + 向量", "DQ1=100% · DQ2=99.5%", 104, C_GREEN, C_GREEN_L, "folder", 2),
        ("黑名单 · 敲门不进屋", "node_modules/ .git/ target/", "仅记目录节点 · 严禁下钻", "挡下 7,825 无效句柄", 296, C_RED, C_RED_L, "ban", 3),
        ("灰名单 · 浅窗转交", "/src/ 克隆开源源码仓", "只读 README 与入口", "微观代码交 CodeGraph", 488, C_GOLD, C_GOLD_L, "microscope", 4),
    ]
    for title, path, act, stat, cy, col, bg, ic, stp in doors:
        parts.append(s(stp))
        parts.append(f'<path d="M 234,295 C 288,295 292,{cy} 352,{cy}" fill="none" stroke="{col}" stroke-width="3.5" stroke-dasharray="7 5" class="flow-line" marker-end="url(#tsd_m)"/>')
        parts.append(f'<rect x="358" y="{cy - 76}" width="576" height="152" rx="18" fill="{bg}" stroke="{col}" stroke-width="2" filter="url(#tsd_sh)"/>')
        # Door icon visual
        parts.append(f'<rect x="380" y="{cy - 54}" width="92" height="108" rx="14" fill="{C_WHITE}" stroke="{col}" stroke-width="2"/>')
        parts.append(icon(ic, 426, cy - 8, 42, col))
        parts.append(f'<circle cx="456" cy="{cy + 26}" r="5" fill="{col}"/>')
        parts.append(f'<text x="496" y="{cy - 28}" font-family="var(--f-r)" font-size="22" fill="{C_INK}" font-weight="700">{title}</text>')
        parts.append(f'<text x="496" y="{cy + 4}" font-family="var(--f-m)" font-size="13.5" fill="{col}" font-weight="700">{path}</text>')
        parts.append(f'<text x="496" y="{cy + 34}" font-family="var(--f-s)" font-size="14" fill="{C_INK2}">{act}</text>')
        parts.append(f'<rect x="748" y="{cy - 56}" width="170" height="32" rx="16" fill="{col}"/>')
        parts.append(f'<text x="833" y="{cy - 35}" font-family="var(--f-m)" font-size="12" fill="{C_PAPER}" font-weight="700" text-anchor="middle">{stat}</text>')
        parts.append("</g>")

    # Right panel: 3-Layer Zero-Cloud Metadata Enrichment
    parts.append(s(4))
    parts.append(f'<line x1="934" y1="104" x2="994" y2="180" stroke="{C_TEAL}" stroke-width="2.6" stroke-dasharray="6 4" marker-end="url(#tsd_t)"/>')
    parts.append(f'<rect x="1000" y="28" width="456" height="536" rx="20" fill="{C_WHITE}" stroke="{C_TEAL}" stroke-width="2"/>')
    parts.append(f'<rect x="1000" y="28" width="456" height="62" rx="18" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="1228" y="66" font-family="var(--f-r)" font-size="20" fill="{C_PAPER}" font-weight="700" text-anchor="middle">零云端 Token 三层标注流水线</text>')
    layers = [
        ("L0 溯源层", "content_hash + mtime", "驱动写穿失效", C_TEAL_D, 126),
        ("L1 通用层", "Rust 微秒静态提取", "0 网络依赖 · 入口/用途", C_TEAL, 266),
        ("L2 特化层", "本地 Ollama 0.8b 脱敏精炼", "script / project / doc", C_GREEN, 406),
    ]
    for lt, l1, l2, col, ly in layers:
        parts.append(f'<rect x="1028" y="{ly}" width="400" height="106" rx="14" fill="{C_PAPER}" stroke="{col}" stroke-width="1.6"/>')
        parts.append(f'<rect x="1028" y="{ly}" width="8" height="106" rx="4" fill="{col}"/>')
        parts.append(f'<text x="1054" y="{ly + 32}" font-family="var(--f-r)" font-size="19" fill="{col}" font-weight="700">{lt}</text>')
        parts.append(f'<text x="1054" y="{ly + 62}" font-family="var(--f-m)" font-size="13.5" fill="{C_INK}" font-weight="600">{l1}</text>')
        parts.append(f'<text x="1054" y="{ly + 88}" font-family="var(--f-s)" font-size="13" fill="{C_INK2}">{l2}</text>')
        if ly < 400:
            parts.append(f'<line x1="1228" y1="{ly + 106}" x2="1228" y2="{ly + 138}" stroke="{col}" stroke-width="2.2" marker-end="url(#tsd_t)"/>')
    parts.append(f'<text x="1228" y="544" font-family="var(--f-m)" font-size="13" fill="{C_CLAY}" font-weight="700" text-anchor="middle">W3 实测：阻断 534 噪音 · Token -95.8%</text>')
    parts.append("</g>")

    parts.append("</svg>")
    return "".join(parts)


def vis_aopcc_highway(animated: bool = True) -> str:
    """AOPCC Traffic ETC Highway vs Pessimistic Jam + <2ms Post-Hook Self-Healing Loop."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("aop")]

    # Step 1: Top contrast bar — Why Pure Pessimistic Fails (C_turn traffic jam)
    parts.append(s(1))
    parts.append(f'<rect x="16" y="10" width="1440" height="96" rx="16" fill="{C_RED_L}" stroke="{C_RED}" stroke-width="1.6"/>')
    parts.append(icon_badge("warning", 68, 58, 24, C_RED, C_WHITE))
    parts.append(f'<text x="108" y="48" font-family="var(--f-r)" font-size="20" fill="{C_RED}" font-weight="700">为什么不能「事事悲观前置查询」？—— 隐形杀手：整轮对话税 C_turn</text>')
    parts.append(f'<text x="108" y="78" font-family="var(--f-m)" font-size="14" fill="{C_INK}">即便本地查 MCP 只要 1ms，大模型重发上下文 + 推理也要 2~4 秒 + 10k~30k Token！</text>')
    parts.append(f'<rect x="1126" y="28" width="308" height="60" rx="12" fill="{C_WHITE}" stroke="{C_RED}" stroke-width="1.4"/>')
    parts.append(f'<text x="1280" y="54" font-family="var(--f-m)" font-size="13" fill="{C_RED}" font-weight="700" text-anchor="middle">数学判据：p · C_turn &gt; (1-p) · C_fail</text>')
    parts.append(f'<text x="1280" y="76" font-family="var(--f-s)" font-size="12" fill="{C_INK2}" text-anchor="middle">p &gt;= 0.90 且可逆 → 乐观直通完胜</text>')
    parts.append("</g>")

    # Step 2: Left Entry — Brief MVCC Lease + Classifier Hub
    parts.append(s(2))
    parts.append(f'<rect x="16" y="160" width="248" height="270" rx="18" fill="{C_TEAL_D}"/>')
    parts.append(icon("capsule", 140, 224, 48, C_PAPER))
    parts.append(f'<text x="140" y="282" font-family="var(--f-r)" font-size="21" fill="{C_PAPER}" font-weight="700" text-anchor="middle">开工 MVCC 租约</text>')
    parts.append(f'<text x="140" y="314" font-family="var(--f-m)" font-size="14" fill="#A6CFD5" text-anchor="middle">&lt;= 120 tokens</text>')
    parts.append(f'<rect x="40" y="338" width="200" height="32" rx="16" fill="#25667B"/>')
    parts.append(f'<text x="140" y="359" font-family="var(--f-m)" font-size="12" fill="{C_PAPER}" text-anchor="middle">p&gt;=0.90 + epoch_hash</text>')
    parts.append(f'<text x="140" y="402" font-family="var(--f-s)" font-size="13" fill="#CFE4E7" text-anchor="middle">SessionStart 自动签发</text>')

    # Classifier diamond / hub
    parts.append(f'<line x1="264" y1="295" x2="338" y2="295" stroke="{C_TEAL}" stroke-width="3.5" marker-end="url(#aop_t)"/>')
    parts.append(f'<polygon points="434,205 524,295 434,385 344,295" fill="{C_WHITE}" stroke="{C_TEAL}" stroke-width="2.5" filter="url(#aop_sh)"/>')
    parts.append(f'<text x="434" y="288" font-family="var(--f-r)" font-size="18" fill="{C_TEAL_D}" font-weight="700" text-anchor="middle">三态分流</text>')
    parts.append(f'<text x="434" y="314" font-family="var(--f-m)" font-size="12" fill="{C_INK2}" text-anchor="middle">Classifier</text>')
    parts.append("</g>")

    # Step 3: Three Highway Lanes
    lanes = [
        ("OPTIMISTIC · ETC 绿道直飞", "psql -p 5432 / 写 /tmp · 免查 0 额外轮", "直连成功率 >95%", 176, C_GREEN, C_GREEN_L, "bolt", "aop_g"),
        ("CAUTIOUS_CAS · 慢道轻核", "常规新建文件 · 检查路径归属", "41 μs 放置建议", 295, C_GOLD, C_GOLD_L, "check", "aop_o"),
        ("PESSIMISTIC · 悲观红灯铁闸", "服务端 bind / 高危写 / 共享锁", "Pre-Hook 1ms 强拦", 414, C_RED, C_RED_L, "lock", "aop_r"),
    ]
    parts.append(s(3))
    for title, sub, pill, ly, col, bg, ic, mid in lanes:
        parts.append(f'<path d="M 524,295 C 574,295 574,{ly} 628,{ly}" fill="none" stroke="{col}" stroke-width="3.8" stroke-dasharray="9 6" class="flow-line" marker-end="url(#{mid})"/>')
        parts.append(f'<rect x="634" y="{ly - 46}" width="570" height="92" rx="16" fill="{bg}" stroke="{col}" stroke-width="2"/>')
        parts.append(icon_badge(ic, 682, ly, 24, col, C_WHITE))
        parts.append(f'<text x="724" y="{ly - 8}" font-family="var(--f-r)" font-size="20" fill="{C_INK}" font-weight="700">{title}</text>')
        parts.append(f'<text x="724" y="{ly + 20}" font-family="var(--f-s)" font-size="13.5" fill="{C_INK2}">{sub}</text>')
        parts.append(f'<rect x="1038" y="{ly - 17}" width="150" height="34" rx="17" fill="{col}"/>')
        parts.append(f'<text x="1113" y="{ly + 5}" font-family="var(--f-m)" font-size="12" fill="{C_PAPER}" font-weight="700" text-anchor="middle">{pill}</text>')
    # Right side W15 Proof Card
    parts.append(f'<rect x="1232" y="130" width="224" height="330" rx="18" fill="{C_WHITE}" stroke="{C_TEAL}" stroke-width="2"/>')
    parts.append(f'<text x="1344" y="164" font-family="var(--f-m)" font-size="12" fill="{C_TEAL}" font-weight="700" text-anchor="middle">W15 实证战果</text>')
    parts.append(ring_gauge(1344, 244, 48, 83.9, "-83.9%", "Token 节省", C_GREEN))
    parts.append(f'<line x1="1256" y1="336" x2="1432" y2="336" stroke="{C_LINE}" stroke-width="1"/>')
    parts.append(f'<text x="1344" y="378" font-family="var(--f-m)" font-size="32" fill="{C_CLAY}" font-weight="700" text-anchor="middle">3.22x</text>')
    parts.append(f'<text x="1344" y="404" font-family="var(--f-s)" font-size="13.5" fill="{C_INK}" font-weight="600" text-anchor="middle">端到端加速比</text>')
    parts.append(f'<text x="1344" y="434" font-family="var(--f-m)" font-size="12.5" fill="{C_GREEN}" font-weight="700" text-anchor="middle">质量 52 → 100 分</text>')
    parts.append("</g>")

    # Step 4: Bottom <2ms PostToolUse Self-Healing Loop
    parts.append(s(4))
    parts.append(f'<rect x="16" y="492" width="1440" height="132" rx="18" fill="{C_CLAY_L}" stroke="{C_CLAY}" stroke-width="2"/>')
    parts.append(f'<text x="44" y="522" font-family="var(--f-m)" font-size="12" fill="{C_CLAY}" font-weight="700" letter-spacing="1.6">EMERGENCY HEAL LOOP · 乐观直通偶发碰壁 → PostToolUse 毫秒级自愈闭环</text>')
    heal_steps = [
        ("偶发碰壁", "ECONNREFUSED", "warning", C_RED, 160),
        ("反查宿主", "/proc 找真实端口", "radar", C_TEAL, 510),
        ("注入处方", "返回正确命令 (<2ms)", "bolt", C_GREEN, 860),
        ("降级租约", "防止下次再踩坑", "flywheel", C_CLAY, 1240),
    ]
    for idx, (ht, hs, ic, col, hx) in enumerate(heal_steps):
        parts.append(f'<rect x="{hx - 128}" y="538" width="256" height="72" rx="14" fill="{C_WHITE}" stroke="{col}" stroke-width="1.6"/>')
        parts.append(icon_badge(ic, hx - 88, 574, 20, col))
        parts.append(f'<text x="{hx - 54}" y="568" font-family="var(--f-s)" font-size="15.5" fill="{C_INK}" font-weight="700">{ht}</text>')
        parts.append(f'<text x="{hx - 54}" y="592" font-family="var(--f-m)" font-size="12" fill="{col}" font-weight="600">{hs}</text>')
        if idx < 3:
            nxt_x = heal_steps[idx + 1][4] - 134
            parts.append(f'<line x1="{hx + 128}" y1="574" x2="{nxt_x}" y2="574" stroke="{C_CLAY}" stroke-width="2.6" marker-end="url(#aop_c)"/>')
    parts.append("</g>")

    parts.append("</svg>")
    return "".join(parts)


def vis_lock_and_wfg(animated: bool = True) -> str:
    """Left: Directory Hierarchy Lock Propagation Tree | Right: Wait-For Graph Deadlock Cycle Breaker."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("lk")]

    # Left Panel: Directory Tree Lock Propagation
    parts.append(s(1))
    parts.append(f'<rect x="16" y="14" width="698" height="536" rx="20" fill="{C_WHITE}" stroke="{C_TEAL}" stroke-width="2"/>')
    parts.append(f'<text x="44" y="48" font-family="var(--f-m)" font-size="12.5" fill="{C_TEAL}" font-weight="700" letter-spacing="1.6">HIERARCHICAL LOCKS · 目录树层级互斥传播</text>')
    # Tree lines
    parts.append(f'<line x1="365" y1="146" x2="210" y2="254" stroke="{C_RED}" stroke-width="3" stroke-dasharray="6 4"/>')
    parts.append(f'<line x1="365" y1="146" x2="530" y2="254" stroke="{C_GREEN}" stroke-width="3"/>')
    parts.append(f'<line x1="210" y1="326" x2="140" y2="430" stroke="{C_RED}" stroke-width="3" stroke-dasharray="6 4"/>')
    parts.append(f'<line x1="210" y1="326" x2="296" y2="430" stroke="{C_RED}" stroke-width="3" stroke-dasharray="6 4"/>')
    # Root dir
    parts.append(f'<rect x="245" y="76" width="240" height="72" rx="14" fill="{C_PAPER2}" stroke="{C_INK}" stroke-width="1.8"/>')
    parts.append(icon_badge("folder", 285, 112, 20, C_TEAL))
    parts.append(f'<text x="316" y="108" font-family="var(--f-m)" font-size="15" fill="{C_INK}" font-weight="700">/work/repo/</text>')
    parts.append(f'<text x="316" y="130" font-family="var(--f-s)" font-size="12" fill="{C_INK2}">根工作目录</text>')
    # Locked branch (Exclusive)
    parts.append(f'<rect x="92" y="254" width="236" height="76" rx="14" fill="{C_RED_L}" stroke="{C_RED}" stroke-width="2.2"/>')
    parts.append(icon_badge("lock", 132, 292, 21, C_RED, C_WHITE))
    parts.append(f'<text x="164" y="286" font-family="var(--f-m)" font-size="14.5" fill="{C_RED}" font-weight="700">src/ (排他锁 X)</text>')
    parts.append(f'<text x="164" y="310" font-family="var(--f-s)" font-size="12.5" fill="{C_INK2}">Agent A 持有中</text>')
    # Shared branch
    parts.append(f'<rect x="412" y="254" width="256" height="76" rx="14" fill="{C_GREEN_L}" stroke="{C_GREEN}" stroke-width="2.2"/>')
    parts.append(icon_badge("unlock", 452, 292, 21, C_GREEN, C_WHITE))
    parts.append(f'<text x="484" y="286" font-family="var(--f-m)" font-size="14.5" fill="{C_GREEN}" font-weight="700">docs/ (共享锁 S)</text>')
    parts.append(f'<text x="484" y="310" font-family="var(--f-s)" font-size="12.5" fill="{C_INK2}">多 Agent 并行读取 ✓</text>')
    # Blocked child files
    for fx, fn in [(140, "lib.rs 🚫 阻断"), (308, "main.rs 🚫 阻断")]:
        parts.append(f'<rect x="{fx - 76}" y="428" width="152" height="54" rx="12" fill="{C_WHITE}" stroke="{C_RED}" stroke-width="1.6"/>')
        parts.append(f'<text x="{fx}" y="460" font-family="var(--f-m)" font-size="13" fill="{C_RED}" font-weight="700" text-anchor="middle">{fn}</text>')
    parts.append(f'<text x="365" y="524" font-family="var(--f-s)" font-size="14" fill="{C_INK2}" font-weight="600" text-anchor="middle">父锁子必阻 · 子锁父必阻 · 同级共享并发</text>')
    parts.append("</g>")

    # Right Panel: WFG Deadlock Cycle Detection + Wait-Queue Wakeup
    parts.append(s(2))
    parts.append(f'<rect x="742" y="14" width="714" height="536" rx="20" fill="{C_WHITE}" stroke="{C_CLAY}" stroke-width="2"/>')
    parts.append(f'<text x="770" y="48" font-family="var(--f-m)" font-size="12.5" fill="{C_CLAY}" font-weight="700" letter-spacing="1.6">WAIT-FOR GRAPH · 死锁环路检测 &amp; 推送唤醒</text>')

    # Cycle Diagram
    # Agent A (left), Agent B (right), Res 1 (top), Res 2 (bottom)
    parts.append(f'<path d="M 920,210 Q 980,140 1054,140" fill="none" stroke="{C_TEAL}" stroke-width="3" marker-end="url(#lk_t)"/>')
    parts.append(f'<path d="M 1184,140 Q 1258,140 1304,208" fill="none" stroke="{C_RED}" stroke-width="3.2" stroke-dasharray="7 5" class="flow-line" marker-end="url(#lk_r)"/>')
    parts.append(f'<path d="M 1304,284 Q 1254,352 1184,352" fill="none" stroke="{C_TEAL}" stroke-width="3" marker-end="url(#lk_t)"/>')
    parts.append(f'<path d="M 1054,352 Q 980,352 920,284" fill="none" stroke="{C_RED}" stroke-width="3.2" stroke-dasharray="7 5" class="flow-line" marker-end="url(#lk_r)"/>')

    # Resource pills
    for ry, rlbl in [(140, "🔒 Resource R1"), (352, "🔒 Resource R2")]:
        parts.append(f'<rect x="1044" y="{ry - 24}" width="152" height="48" rx="12" fill="{C_PAPER2}" stroke="{C_INK2}" stroke-width="1.6"/>')
        parts.append(f'<text x="1120" y="{ry + 5}" font-family="var(--f-m)" font-size="13.5" fill="{C_INK}" font-weight="700" text-anchor="middle">{rlbl}</text>')

    # Agent circles
    for ax, albl, col in [(888, "Agent A", C_TEAL_D), (1338, "Agent B", C_CLAY)]:
        parts.append(f'<circle cx="{ax}" cy="246" r="52" fill="{C_WHITE}" stroke="{col}" stroke-width="2.5" filter="url(#lk_sh)"/>')
        parts.append(icon("brain", ax, 236, 30, col))
        parts.append(f'<text x="{ax}" y="274" font-family="var(--f-s)" font-size="14" fill="{col}" font-weight="700" text-anchor="middle">{albl}</text>')

    # Center Cycle Breaker Thunderbolt
    parts.append(f'<circle cx="1120" cy="246" r="46" fill="{C_RED_L}" stroke="{C_RED}" stroke-width="2.2"/>')
    parts.append(icon("bolt", 1120, 236, 32, C_RED))
    parts.append(f'<text x="1120" y="274" font-family="var(--f-m)" font-size="11.5" fill="{C_RED}" font-weight="700" text-anchor="middle">成环即断</text>')
    parts.append("</g>")

    # Step 3: Bottom Wait-Queue + TTL Auto-Reap
    parts.append(s(3))
    parts.append(f'<rect x="774" y="424" width="650" height="102" rx="14" fill="{C_GREEN_L}" stroke="{C_GREEN}" stroke-width="1.6"/>')
    parts.append(icon_badge("check", 820, 475, 22, C_GREEN, C_WHITE))
    parts.append(f'<text x="858" y="464" font-family="var(--f-r)" font-size="18.5" fill="{C_GREEN}" font-weight="700">Wait-Queue 阻塞推送唤醒 + TTL 僵尸自动收割</text>')
    parts.append(f'<text x="858" y="494" font-family="var(--f-m)" font-size="13" fill="{C_INK2}">零轮询空烧 Token · 异常崩溃进程锁自动释放</text>')

    parts.append(f'<rect x="16" y="568" width="1440" height="56" rx="14" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="48" y="602" font-family="var(--f-r)" font-size="19" fill="{C_PAPER}" font-weight="700">多 Agent 并行干活绝不互相覆盖：41 μs 冲突判定 · 毫秒级死锁破环 · 0 轮询空转。</text>')
    parts.append(f'<text x="1426" y="602" font-family="var(--f-m)" font-size="12.5" fill="#A6CFD5" text-anchor="end">body_lock_acquire / wait / deadlock_check</text>')
    parts.append("</g>")

    parts.append("</svg>")
    return "".join(parts)


def vis_freshness_purifier(animated: bool = True) -> str:
    """7 Staleness Funnels -> 3-Stage Write-Through Purifier Pipeline -> 100% Freshness Gauge."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("fp")]

    # Step 1: Left Stale Inputs (4 visual stale hazards)
    parts.append(s(1))
    parts.append(f'<rect x="16" y="24" width="332" height="516" rx="20" fill="{C_RED_L}" fill-opacity="0.6" stroke="{C_RED}" stroke-width="1.8"/>')
    parts.append(f'<text x="182" y="58" font-family="var(--f-m)" font-size="12" fill="{C_RED}" font-weight="700" text-anchor="middle">STALE-01 ~ 07 · 七大陈旧漏斗</text>')
    hazards = [
        ("文件已改 · 向量未更", "旧语义误导检索", 122),
        ("文件已删 · FTS 僵尸", "搜出不存在幽灵", 232),
        ("路径重命名 · 断链", "图谱边指向黑洞", 342),
        ("重复记忆 · 矛盾堆积", "多版本真假难辨", 452),
    ]
    for ht, hs, hy in hazards:
        parts.append(f'<rect x="38" y="{hy - 36}" width="288" height="80" rx="14" fill="{C_WHITE}" stroke="{C_RED}" stroke-width="1.4"/>')
        parts.append(icon_badge("warning", 74, hy + 4, 18, C_RED, C_RED_L))
        parts.append(f'<text x="104" y="{hy - 2}" font-family="var(--f-s)" font-size="14.5" fill="{C_INK}" font-weight="700">{ht}</text>')
        parts.append(f'<text x="104" y="{hy + 22}" font-family="var(--f-s)" font-size="12.5" fill="{C_RED}">{hs}</text>')
    parts.append("</g>")

    # Step 2: Center 3-Stage Purifier Chambers
    parts.append(s(2))
    parts.append(f'<line x1="348" y1="282" x2="404" y2="282" stroke="{C_RED}" stroke-width="3.5" marker-end="url(#fp_r)"/>')
    parts.append(f'<rect x="410" y="24" width="668" height="516" rx="20" fill="{C_WHITE}" stroke="{C_TEAL}" stroke-width="2"/>')
    parts.append(f'<text x="744" y="58" font-family="var(--f-m)" font-size="12.5" fill="{C_TEAL}" font-weight="700" text-anchor="middle" letter-spacing="1.6">WRITE-THROUGH PURIFIER · 三级写穿净水保鲜引擎</text>')
    chambers = [
        ("01 哈希探针", "content_hash 写穿", "文件一变立即标 STALE", "radar", C_TEAL_D, 518),
        ("02 级联冲刷", "ChangeHandler", "删档同步清 FTS + 向量", "filter", C_CLAY, 744),
        ("03 现实对账", "FreshnessReconciler", "周期核验磁盘真值兜底", "check", C_GREEN, 970),
    ]
    for idx, (ct, c1, c2, ic, col, cx) in enumerate(chambers):
        parts.append(f'<rect x="{cx - 96}" y="96" width="192" height="328" rx="18" fill="{C_PAPER}" stroke="{col}" stroke-width="2"/>')
        parts.append(f'<rect x="{cx - 96}" y="96" width="192" height="10" rx="5" fill="{col}"/>')
        parts.append(icon_badge(ic, cx, 166, 32, col, C_WHITE))
        parts.append(f'<text x="{cx}" y="232" font-family="var(--f-r)" font-size="20" fill="{C_INK}" font-weight="700" text-anchor="middle">{ct}</text>')
        parts.append(f'<rect x="{cx - 80}" y="256" width="160" height="30" rx="15" fill="{col}"/>')
        parts.append(f'<text x="{cx}" y="276" font-family="var(--f-m)" font-size="11.5" fill="{C_PAPER}" font-weight="700" text-anchor="middle">{c1}</text>')
        parts.append(f'<text x="{cx}" y="330" font-family="var(--f-s)" font-size="13.5" fill="{C_INK2}" text-anchor="middle">{c2}</text>')
        if idx < 2:
            parts.append(f'<line x1="{cx + 96}" y1="260" x2="{cx + 128}" y2="260" stroke="{col}" stroke-width="3" marker-end="url(#fp_t)"/>')
    # Bottom SPI compatibility strip inside center box
    parts.append(f'<rect x="438" y="454" width="612" height="62" rx="12" fill="{C_TEAL_L}"/>')
    parts.append(f'<text x="744" y="490" font-family="var(--f-m)" font-size="13.5" fill="{C_TEAL_D}" font-weight="700" text-anchor="middle">Memory SPI 开放门面：内置 FTS5+RRF (335μs) · 可热插 Mem0 / Letta</text>')
    parts.append("</g>")

    # Step 3: Right Freshness Output Gauges
    parts.append(s(3))
    parts.append(f'<line x1="1078" y1="282" x2="1126" y2="282" stroke="{C_GREEN}" stroke-width="3.5" marker-end="url(#fp_g)"/>')
    parts.append(f'<rect x="1132" y="24" width="324" height="516" rx="20" fill="{C_GREEN_L}" fill-opacity="0.65" stroke="{C_GREEN}" stroke-width="2"/>')
    parts.append(f'<text x="1294" y="58" font-family="var(--f-m)" font-size="12" fill="{C_GREEN}" font-weight="700" text-anchor="middle">FRESHNESS VERIFIED</text>')
    parts.append(ring_gauge(1294, 174, 62, 100.0, "100%", "现网 589 项鲜活度", C_GREEN))
    parts.append(ring_gauge(1294, 370, 52, 99.5, "99.5%", "本地向量覆盖 DQ2", C_TEAL))
    parts.append(f'<rect x="1164" y="470" width="260" height="44" rx="22" fill="{C_GREEN}"/>')
    parts.append(f'<text x="1294" y="497" font-family="var(--f-m)" font-size="14" fill="{C_PAPER}" font-weight="700" text-anchor="middle">0 僵尸召回 · 0 过期误导</text>')
    parts.append("</g>")

    # Bottom banner
    parts.append(f'<rect x="16" y="566" width="1440" height="58" rx="14" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="48" y="601" font-family="var(--f-r)" font-size="19" fill="{C_PAPER}" font-weight="700">记忆不是「存得越多越好」，而是「文件一改一删，旧账瞬间清零」。</text>')
    parts.append(f'<text x="1426" y="601" font-family="var(--f-m)" font-size="12.5" fill="#A6CFD5" text-anchor="end">Phase 38 · DQ5 = 100.0%</text>')
    parts.append("</svg>")
    return "".join(parts)


def vis_handoff_capsule(animated: bool = True) -> str:
    """Visual Size Contrast (50k Chat Dump vs 150 tok Glowing Capsule) + Cross-Brain Relay Track."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("hc")]

    # Step 1: Top Visual Volume Contrast
    parts.append(s(1))
    # Left bloated dump
    parts.append(f'<rect x="16" y="16" width="600" height="248" rx="20" fill="{C_RED_L}" stroke="{C_RED}" stroke-width="1.8"/>')
    parts.append(f'<text x="44" y="50" font-family="var(--f-m)" font-size="12" fill="{C_RED}" font-weight="700">TRADITIONAL · 聊天记录裸搬</text>')
    # Piled messy blocks
    for i in range(6):
        bw = 190 + (i % 3) * 55
        parts.append(f'<rect x="{44 + (i % 2) * 26}" y="{70 + i * 24}" width="{bw}" height="18" rx="4" fill="{C_RED}" fill-opacity="{0.22 + i * 0.08:.2f}"/>')
    parts.append(f'<text x="466" y="138" font-family="var(--f-m)" font-size="44" fill="{C_RED}" font-weight="700" text-anchor="middle">50,000</text>')
    parts.append(f'<text x="466" y="168" font-family="var(--f-m)" font-size="14" fill="{C_RED}" font-weight="600" text-anchor="middle">TOKENS 历史噪音</text>')
    parts.append(f'<text x="466" y="216" font-family="var(--f-s)" font-size="14" fill="{C_INK2}" text-anchor="middle">重扫 20 分钟 · 含明文密钥</text>')

    # Center VS badge
    parts.append(f'<circle cx="672" cy="140" r="34" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="672" y="148" font-family="var(--f-m)" font-size="19" fill="{C_PAPER}" font-weight="700" text-anchor="middle">VS</text>')

    # Right Glowing 150-tok Capsule
    parts.append(f'<rect x="728" y="16" width="728" height="248" rx="20" fill="{C_GREEN_L}" stroke="{C_GREEN}" stroke-width="2"/>')
    parts.append(f'<text x="756" y="50" font-family="var(--f-m)" font-size="12" fill="{C_GREEN}" font-weight="700">AGENT BODY · 跨大脑接力胶囊 (&lt;=300 TOK)</text>')
    # Capsule visual pill with 4 colored micro-chips
    parts.append(f'<rect x="760" y="76" width="664" height="122" rx="61" fill="{C_WHITE}" stroke="{C_TEAL}" stroke-width="3" filter="url(#hc_sh)"/>')
    parts.append(f'<rect x="772" y="88" width="166" height="98" rx="49" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="855" y="136" font-family="var(--f-m)" font-size="30" fill="{C_PAPER}" font-weight="700" text-anchor="middle">150 t</text>')
    parts.append(f'<text x="855" y="162" font-family="var(--f-m)" font-size="11.5" fill="#A6CFD5" text-anchor="middle">-92.7% TOK</text>')
    chips = [("未竟目标", 1004, C_TEAL), ("变更文件", 1122, C_GREEN), ("避坑红线", 1240, C_CLAY), ("脱敏封印", 1358, C_GOLD)]
    for cl, cx, col in chips:
        w = 104
        parts.append(f'<rect x="{cx - w/2:.1f}" y="114" width="{w}" height="46" rx="12" fill="{C_PAPER}" stroke="{col}" stroke-width="1.6"/>')
        parts.append(f'<circle cx="{cx - 34}" cy="137" r="4.5" fill="{col}"/>')
        parts.append(f'<text x="{cx + 6}" y="142" font-family="var(--f-s)" font-size="13" fill="{col}" font-weight="700" text-anchor="middle">{cl}</text>')
    parts.append(f'<text x="1092" y="238" font-family="var(--f-m)" font-size="14" fill="{C_GREEN}" font-weight="700" text-anchor="middle">W17 实测：10ms 着陆唤醒 · 省 &gt;= 2 轮冷启对话</text>')
    parts.append("</g>")

    # Step 2: Bottom Relay Track across 4 stages
    parts.append(s(2))
    parts.append(f'<rect x="16" y="296" width="1440" height="328" rx="20" fill="{C_WHITE}" stroke="{C_LINE}" stroke-width="1.5"/>')
    parts.append(f'<text x="44" y="330" font-family="var(--f-m)" font-size="12" fill="{C_TEAL}" font-weight="700" letter-spacing="1.8">SEAMLESS RELAY TRACK · 换壳不换魂 · 换脑不失忆</text>')
    track = [
        ("上午 · Brain A", "Claude Code", "结项封存胶囊", "brain", C_TEAL, 194),
        ("永驻底座保险箱", "body_checkpoints", "SQLite / PG / DuckDB", "database", C_GREEN, 556),
        ("下午 · Brain B", "Antigravity", "10ms 自动唤醒续跑", "rocket", C_CLAY, 918),
        ("凌晨 · Brain C", "Cron Worker", "共享锁与规约巡检", "clock", C_GOLD, 1280),
    ]
    for idx, (t1, t2, t3, ic, col, tx) in enumerate(track):
        parts.append(f'<rect x="{tx - 140}" y="362" width="280" height="222" rx="18" fill="{C_PAPER}" stroke="{col}" stroke-width="2" filter="url(#hc_sh)"/>')
        parts.append(f'<rect x="{tx - 140}" y="362" width="280" height="8" rx="4" fill="{col}"/>')
        parts.append(icon_badge(ic, tx, 426, 30, col, C_WHITE))
        parts.append(f'<text x="{tx}" y="484" font-family="var(--f-m)" font-size="12" fill="{col}" font-weight="700" text-anchor="middle">{t1}</text>')
        parts.append(f'<text x="{tx}" y="516" font-family="var(--f-r)" font-size="21" fill="{C_INK}" font-weight="700" text-anchor="middle">{t2}</text>')
        parts.append(f'<rect x="{tx - 114}" y="534" width="228" height="32" rx="16" fill="{C_WHITE}" stroke="{C_LINE}" stroke-width="1"/>')
        parts.append(f'<text x="{tx}" y="555" font-family="var(--f-s)" font-size="13" fill="{C_INK2}" font-weight="600" text-anchor="middle">{t3}</text>')
        if idx < 3:
            parts.append(f'<line x1="{tx + 140}" y1="472" x2="{tx + 216}" y2="472" stroke="{col}" stroke-width="3.5" stroke-dasharray="7 5" class="flow-line" marker-end="url(#hc_t)"/>')
    parts.append("</g>")

    parts.append("</svg>")
    return "".join(parts)


def vis_security_dual_audit(animated: bool = True) -> str:
    """Left: 4-Layer Defense-in-Depth Security Gates | Right: Dual-Domain Audit Funnel + SHA-256 Ledger."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("sd")]

    # Step 1: Left 4 Security Shields
    parts.append(s(1))
    parts.append(f'<rect x="16" y="14" width="698" height="536" rx="20" fill="{C_WHITE}" stroke="{C_CLAY}" stroke-width="2"/>')
    parts.append(f'<text x="44" y="48" font-family="var(--f-m)" font-size="12.5" fill="{C_CLAY}" font-weight="700" letter-spacing="1.6">DEFENSE-IN-DEPTH · 四道纵深安全铁闸 (DQ4 = 100%)</text>')
    gates = [
        ("01 全链路脱敏", "SecretRedactor · 密钥/Token 0 落盘", "shield", C_CLAY, 108),
        ("02 抗提示词投毒", "PromptInjectionScanner · 清洗外部脏语料", "filter", C_GOLD, 218),
        ("03 防命令逃逸", "POSIX 严转义 · 杜绝 Shell 注入", "terminal", C_TEAL, 328),
        ("04 最小权限红线", "0600 落盘 · /etc/shadow &amp; ~/.ssh 100% 拦停", "lock", C_RED, 438),
    ]
    for title, desc, ic, col, gy in gates:
        parts.append(f'<rect x="44" y="{gy - 34}" width="642" height="86" rx="14" fill="{C_PAPER}" stroke="{col}" stroke-width="1.8"/>')
        parts.append(f'<rect x="44" y="{gy - 34}" width="8" height="86" rx="4" fill="{col}"/>')
        parts.append(icon_badge(ic, 96, gy + 9, 24, col, C_WHITE))
        parts.append(f'<text x="138" y="{gy + 2}" font-family="var(--f-r)" font-size="20" fill="{C_INK}" font-weight="700">{title}</text>')
        parts.append(f'<text x="138" y="{gy + 30}" font-family="var(--f-s)" font-size="13.5" fill="{C_INK2}">{desc}</text>')
        parts.append(f'<rect x="576" y="{gy - 8}" width="92" height="32" rx="16" fill="{col}"/>')
        parts.append(f'<text x="622" y="{gy + 13}" font-family="var(--f-m)" font-size="12" fill="{C_PAPER}" font-weight="700" text-anchor="middle">PASS</text>')
    parts.append("</g>")

    # Step 2: Right Dual-Domain Audit Funnel
    parts.append(s(2))
    parts.append(f'<rect x="742" y="14" width="714" height="536" rx="20" fill="{C_WHITE}" stroke="{C_TEAL}" stroke-width="2"/>')
    parts.append(f'<text x="770" y="48" font-family="var(--f-m)" font-size="12.5" fill="{C_TEAL}" font-weight="700" letter-spacing="1.6">DUAL-DOMAIN AUDIT · 拒绝黑盒自治 (DQ7 = 100%)</text>')

    # Two top domains: IN_AGENT & OUT_OF_AGENT
    domains = [
        ("IN_AGENT · 接入域", "43 MCP · Brief · Hook · Mesh", 928, C_TEAL, "brain"),
        ("OUT_OF_AGENT · 域外自治", "巡检 · GC · 自愈 · Watcher", 1272, C_GREEN, "gear"),
    ]
    for dt, ds, dx, col, ic in domains:
        parts.append(f'<rect x="{dx - 154}" y="82" width="308" height="114" rx="16" fill="{C_PAPER}" stroke="{col}" stroke-width="2"/>')
        parts.append(icon_badge(ic, dx - 110, 139, 24, col, C_WHITE))
        parts.append(f'<text x="{dx - 72}" y="130" font-family="var(--f-s)" font-size="16" fill="{col}" font-weight="700">{dt}</text>')
        parts.append(f'<text x="{dx - 72}" y="158" font-family="var(--f-s)" font-size="13" fill="{C_INK2}">{ds}</text>')
        parts.append(f'<line x1="{dx}" y1="196" x2="1100" y2="262" stroke="{col}" stroke-width="3" stroke-dasharray="6 4" class="flow-line" marker-end="url(#sd_t)"/>')

    # 60s Anti-Flood Funnel Trapezoid
    parts.append(f'<polygon points="940,266 1260,266 1200,356 1000,356" fill="{C_GOLD_L}" stroke="{C_GOLD}" stroke-width="2"/>')
    parts.append(f'<text x="1100" y="306" font-family="var(--f-r)" font-size="19" fill="{C_INK}" font-weight="700" text-anchor="middle">60s 折叠防洪漏斗</text>')
    parts.append(f'<text x="1100" y="334" font-family="var(--f-m)" font-size="13.5" fill="{C_GOLD}" font-weight="700" text-anchor="middle">10 条高频事件 → 1 条摘要 · 4KB 截断</text>')
    parts.append(f'<line x1="1100" y1="356" x2="1100" y2="404" stroke="{C_TEAL_D}" stroke-width="3" marker-end="url(#sd_t)"/>')

    # SHA-256 Sealed Ledger
    parts.append(f'<rect x="794" y="408" width="612" height="114" rx="16" fill="{C_TEAL_D}"/>')
    parts.append(icon_badge("audit", 854, 465, 26, C_TEAL, C_TEAL_L))
    parts.append(f'<text x="900" y="454" font-family="var(--f-r)" font-size="21" fill="{C_PAPER}" font-weight="700">SHA-256 不可抵赖审计账本</text>')
    parts.append(f'<text x="900" y="486" font-family="var(--f-m)" font-size="13" fill="#A6CFD5">delta 前后差值 · evidence_sha256 · DuckDB 极速 OLAP</text>')
    parts.append("</g>")

    # Bottom banner
    parts.append(f'<rect x="16" y="568" width="1440" height="56" rx="14" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="48" y="602" font-family="var(--f-r)" font-size="19" fill="{C_PAPER}" font-weight="700">不仅管住大模型的手，也给本体自己的后台守护进程装上 360° 黑匣子。</text>')
    parts.append(f'<text x="1426" y="602" font-family="var(--f-m)" font-size="12.5" fill="#A6CFD5" text-anchor="end">W16 实测：Token -83.5% · 质量 36 → 100</text>')

    parts.append("</svg>")
    return "".join(parts)


def vis_vitals_radar_autonomy(animated: bool = True) -> str:
    """Left: True 5-Axis Geometric Pentagon Radar Chart | Right: L0~L3 Concentric Autonomy Rings."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">']

    # Left: 5-Dimension Homeostasis Radar
    parts.append(s(1))
    parts.append(f'<rect x="16" y="14" width="698" height="536" rx="20" fill="{C_WHITE}" stroke="{C_TEAL}" stroke-width="2"/>')
    parts.append(f'<text x="44" y="48" font-family="var(--f-m)" font-size="12.5" fill="{C_TEAL}" font-weight="700" letter-spacing="1.6">HOMEOSTASIS VITALS · 五维生命体征雷达</text>')
    rcx, rcy, max_r = 365, 304, 172
    dims = [
        ("新鲜度 100", 1.00),
        ("图谱连通 100", 1.00),
        ("向量覆盖 99.5", 0.995),
        ("稳态遥测 93.9", 0.939),
        ("安全脱敏 100", 1.00),
    ]
    # Grid pentagons
    for frac in (0.33, 0.66, 1.0):
        pts = " ".join(
            f"{rcx + max_r * frac * math.cos(math.radians(-90 + i * 72)):.1f},{rcy + max_r * frac * math.sin(math.radians(-90 + i * 72)):.1f}"
            for i in range(5)
        )
        parts.append(f'<polygon points="{pts}" fill="none" stroke="{C_LINE}" stroke-width="1.3"/>')
    # Spokes + Labels
    poly_pts = []
    for i, (lbl, val) in enumerate(dims):
        ang = math.radians(-90 + i * 72)
        ox = rcx + max_r * math.cos(ang)
        oy = rcy + max_r * math.sin(ang)
        parts.append(f'<line x1="{rcx}" y1="{rcy}" x2="{ox:.1f}" y2="{oy:.1f}" stroke="{C_LINE}" stroke-width="1.2"/>')
        px = rcx + max_r * val * math.cos(ang)
        py = rcy + max_r * val * math.sin(ang)
        poly_pts.append((px, py))
        lx = rcx + (max_r + 42) * math.cos(ang)
        ly = rcy + (max_r + 34) * math.sin(ang)
        anchor = "middle" if abs(math.cos(ang)) < 0.25 else ("start" if math.cos(ang) > 0 else "end")
        parts.append(f'<text x="{lx:.1f}" y="{ly + 4:.1f}" font-family="var(--f-s)" font-size="14" fill="{C_INK}" font-weight="700" text-anchor="{anchor}">{lbl}</text>')
    pts_str = " ".join(f"{x:.1f},{y:.1f}" for x, y in poly_pts)
    parts.append(f'<polygon points="{pts_str}" fill="{C_TEAL}" fill-opacity="0.22" stroke="{C_TEAL}" stroke-width="3"/>')
    for px, py in poly_pts:
        parts.append(f'<circle cx="{px:.1f}" cy="{py:.1f}" r="6" fill="{C_WHITE}" stroke="{C_TEAL}" stroke-width="2.6"/>')
    parts.append(f'<circle cx="{rcx}" cy="{rcy}" r="36" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="{rcx}" y="{rcy + 7}" font-family="var(--f-m)" font-size="22" fill="{C_PAPER}" font-weight="700" text-anchor="middle">98.7</text>')
    parts.append(f'<text x="{rcx}" y="526" font-family="var(--f-m)" font-size="13" fill="{C_GREEN}" font-weight="700" text-anchor="middle">异常跌落阈值 → 自动触发 L1 自愈处方</text>')
    parts.append("</g>")

    # Right: L0 ~ L3 Concentric Autonomy Envelope
    parts.append(s(2))
    parts.append(f'<rect x="742" y="14" width="714" height="536" rx="20" fill="{C_WHITE}" stroke="{C_CLAY}" stroke-width="2"/>')
    parts.append(f'<text x="770" y="48" font-family="var(--f-m)" font-size="12.5" fill="{C_CLAY}" font-weight="700" letter-spacing="1.6">AUTONOMY ENVELOPE · L0 ~ L3 四级硬法律包络</text>')
    ecx, ecy = 984, 304
    envelopes = [
        (194, C_RED_L, C_RED, "L3 · 绝对痛觉禁区", "/etc/shadow · ~/.ssh 强拦"),
        (148, C_GOLD_L, C_GOLD, "L2 · 审批写边界", "业务修改强制 dry-run"),
        (102, C_TEAL_L, C_TEAL, "L1 · 可逆稳态自愈", "刷缓存 · 修断链 · 缩内存"),
        (56, C_GREEN_L, C_GREEN, "L0 · 只读永不阻塞", "查事实 · 查拓扑 · 0 副作用"),
    ]
    for r, fill, col, _, _ in envelopes:
        parts.append(f'<circle cx="{ecx}" cy="{ecy}" r="{r}" fill="{fill}" stroke="{col}" stroke-width="2.2"/>')
    parts.append(f'<text x="{ecx}" y="{ecy + 6}" font-family="var(--f-m)" font-size="18" fill="{C_GREEN}" font-weight="700" text-anchor="middle">L0</text>')
    parts.append(f'<text x="{ecx}" y="{ecy - 74}" font-family="var(--f-m)" font-size="14" fill="{C_TEAL}" font-weight="700" text-anchor="middle">L1</text>')
    parts.append(f'<text x="{ecx}" y="{ecy - 120}" font-family="var(--f-m)" font-size="14" fill="{C_GOLD}" font-weight="700" text-anchor="middle">L2</text>')
    parts.append(f'<text x="{ecx}" y="{ecy - 166}" font-family="var(--f-m)" font-size="14" fill="{C_RED}" font-weight="700" text-anchor="middle">L3</text>')
    # Clean callout legend cards on far right (wider 222px, zero border overflow)
    for i, (_, fill, col, lt, ld) in enumerate(reversed(envelopes)):
        ly = 108 + i * 106
        parts.append(f'<rect x="1214" y="{ly}" width="224" height="86" rx="12" fill="{fill}" stroke="{col}" stroke-width="1.6"/>')
        parts.append(f'<text x="1230" y="{ly + 32}" font-family="var(--f-s)" font-size="14.5" fill="{col}" font-weight="700">{lt}</text>')
        parts.append(f'<text x="1230" y="{ly + 60}" font-family="var(--f-s)" font-size="12" fill="{C_INK2}">{ld}</text>')
    parts.append("</g>")

    parts.append(f'<rect x="16" y="568" width="1440" height="56" rx="14" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="48" y="602" font-family="var(--f-r)" font-size="19" fill="{C_PAPER}" font-weight="700">自治边界刻进 Rust 类型系统：该自愈的秒级自愈，碰红线的 100% 物理拦截。</text>')
    parts.append(f'<text x="1426" y="602" font-family="var(--f-m)" font-size="12.5" fill="#A6CFD5" text-anchor="end">src/body/autonomy.rs · DQ8 = 93.9</text>')
    parts.append("</svg>")
    return "".join(parts)


def vis_awr_telemetry(animated: bool = True) -> str:
    """Database-grade Built-in AWR Dashboard: 6-Class 18 Wait Events + 600% CPU Postmortem Curve."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("awr")]

    # Step 1: Left Wait Events & /proc/self Dashboard
    parts.append(s(1))
    parts.append(f'<rect x="16" y="14" width="698" height="536" rx="20" fill="{C_WHITE}" stroke="{C_TEAL}" stroke-width="2"/>')
    parts.append(f'<text x="44" y="48" font-family="var(--f-m)" font-size="12.5" fill="{C_TEAL}" font-weight="700" letter-spacing="1.6">WAIT EVENTS (6 类 18 事件) · 无锁 RAII &lt;100 ns</text>')
    events = [
        ("Lock 等待", "SQLite 写锁 / 目录互斥", 22, C_RED),
        ("Disk IO", "WAL 刷盘 / 全盘扫描", 38, C_CLAY),
        ("Network", "Ollama 向量 / Mesh RPC", 64, C_GOLD),
        ("CPU 计算", "FTS5 / 图遍历 / 脱敏", 45, C_TEAL),
        ("IPC 通道", "MCP stdio / Hook 管道", 18, C_GREEN),
    ]
    for i, (en, es, pct, col) in enumerate(events):
        ey = 104 + i * 62
        parts.append(f'<text x="44" y="{ey}" font-family="var(--f-s)" font-size="14.5" fill="{C_INK}" font-weight="700">{en}</text>')
        parts.append(f'<text x="150" y="{ey}" font-family="var(--f-s)" font-size="12.5" fill="{C_MUTE}">{es}</text>')
        parts.append(f'<rect x="380" y="{ey - 14}" width="290" height="18" rx="9" fill="{C_PAPER2}"/>')
        parts.append(f'<rect x="380" y="{ey - 14}" width="{int(290 * pct / 100)}" height="18" rx="9" fill="{col}"/>')
    # /proc/self 3 mini KPI boxes
    parts.append(f'<line x1="44" y1="398" x2="684" y2="398" stroke="{C_LINE}" stroke-width="1"/>')
    parts.append(f'<text x="44" y="426" font-family="var(--f-m)" font-size="12" fill="{C_MUTE}" font-weight="700">/PROC/SELF 零特权实时探针 + 32 份黄金基线快照</text>')
    for i, (kv, kl, col) in enumerate([("<100 ns", "无锁埋点开销", C_GREEN), ("18 / 18", "WaitEvent 全埋点", C_TEAL), ("/metrics", "Prometheus + W3C", C_CLAY)]):
        kx = 44 + i * 218
        parts.append(f'<rect x="{kx}" y="444" width="204" height="84" rx="14" fill="{C_PAPER}" stroke="{col}" stroke-width="1.5"/>')
        parts.append(f'<text x="{kx + 102}" y="484" font-family="var(--f-m)" font-size="24" fill="{col}" font-weight="700" text-anchor="middle">{kv}</text>')
        parts.append(f'<text x="{kx + 102}" y="512" font-family="var(--f-s)" font-size="12.5" fill="{C_INK2}" text-anchor="middle">{kl}</text>')
    parts.append("</g>")

    # Step 2: Right 600% CPU Postmortem Curve -> ADDM + L1 Self-Tune
    parts.append(s(2))
    parts.append(f'<rect x="742" y="14" width="714" height="536" rx="20" fill="{C_WHITE}" stroke="{C_CLAY}" stroke-width="2"/>')
    parts.append(f'<text x="770" y="48" font-family="var(--f-m)" font-size="12.5" fill="{C_CLAY}" font-weight="700" letter-spacing="1.6">REAL POSTMORTEM · 600% CPU / 1.3G 内存尖峰根治曲线</text>')
    # Chart frame
    parts.append(f'<rect x="782" y="76" width="642" height="310" rx="14" fill="{C_PAPER}"/>')
    # Grid lines
    for gy, gl in [(116, "600%"), (204, "300%"), (292, "100%"), (358, "2%")]:
        parts.append(f'<line x1="836" y1="{gy}" x2="1400" y2="{gy}" stroke="{C_LINE}" stroke-width="1" stroke-dasharray="4 4"/>')
        parts.append(f'<text x="826" y="{gy + 4}" font-family="var(--f-m)" font-size="11.5" fill="{C_MUTE}" text-anchor="end">{gl}</text>')
    # Red spike area before fix
    parts.append(
        f'<path d="M 840,356 L 910,348 L 968,148 L 1018,108 L 1072,138 L 1110,348 L 1110,362 L 840,362 Z" fill="{C_RED}" fill-opacity="0.16"/>'
        f'<path d="M 840,356 L 910,348 L 968,148 L 1018,108 L 1072,138 L 1110,348" fill="none" stroke="{C_RED}" stroke-width="3.5"/>'
    )
    parts.append(f'<circle cx="1018" cy="108" r="8" fill="{C_RED}"/>')
    parts.append(f'<rect x="932" y="64" width="176" height="30" rx="15" fill="{C_RED}"/>')
    parts.append(f'<text x="1020" y="84" font-family="var(--f-m)" font-size="12.5" fill="{C_PAPER}" font-weight="700" text-anchor="middle">600% CPU · 1.3G</text>')
    # Intervention vertical line
    parts.append(f'<line x1="1114" y1="88" x2="1114" y2="368" stroke="{C_TEAL}" stroke-width="2.4" stroke-dasharray="6 4"/>')
    parts.append(f'<rect x="1054" y="218" width="124" height="34" rx="17" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="1116" y="240" font-family="var(--f-m)" font-size="12" fill="{C_PAPER}" font-weight="700" text-anchor="middle">ADDM 定位</text>')
    # Green flat stable line after fix
    parts.append(
        f'<path d="M 1114,348 L 1170,356 L 1260,358 L 1396,356 L 1396,364 L 1114,364 Z" fill="{C_GREEN}" fill-opacity="0.22"/>'
        f'<path d="M 1114,348 L 1170,356 L 1260,358 L 1396,356" fill="none" stroke="{C_GREEN}" stroke-width="4"/>'
    )
    parts.append(f'<rect x="1196" y="294" width="184" height="44" rx="12" fill="{C_GREEN_L}" stroke="{C_GREEN}" stroke-width="1.6"/>')
    parts.append(f'<text x="1288" y="321" font-family="var(--f-m)" font-size="14" fill="{C_GREEN}" font-weight="700" text-anchor="middle">稳态 &lt;2% · &lt;90MB</text>')
    # Bottom L1 self-tune prescription cards
    parts.append(f'<rect x="782" y="410" width="308" height="114" rx="14" fill="{C_TEAL_L}" stroke="{C_TEAL}" stroke-width="1.5"/>')
    parts.append(f'<text x="804" y="446" font-family="var(--f-r)" font-size="18" fill="{C_TEAL_D}" font-weight="700">L1 一键自调优</text>')
    parts.append(f'<text x="804" y="476" font-family="var(--f-m)" font-size="12.5" fill="{C_INK2}">PRAGMA shrink_memory</text>')
    parts.append(f'<text x="804" y="500" font-family="var(--f-m)" font-size="12.5" fill="{C_INK2}">glibc malloc_trim(0)</text>')

    parts.append(f'<rect x="1112" y="410" width="312" height="114" rx="14" fill="{C_GREEN_L}" stroke="{C_GREEN}" stroke-width="1.5"/>')
    parts.append(f'<text x="1134" y="446" font-family="var(--f-r)" font-size="18" fill="{C_GREEN}" font-weight="700">DuckDB 受控限流</text>')
    parts.append(f'<text x="1134" y="476" font-family="var(--f-m)" font-size="12.5" fill="{C_INK2}">threads = 2 · mem = 256MB</text>')
    parts.append(f'<text x="1134" y="500" font-family="var(--f-m)" font-size="12.5" fill="{C_INK2}">AWR Golden Baseline 差分</text>')
    parts.append("</g>")

    parts.append(f'<rect x="16" y="568" width="1440" height="56" rx="14" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="48" y="602" font-family="var(--f-r)" font-size="19" fill="{C_PAPER}" font-weight="700">把 Oracle / PostgreSQL 三十年数据库内核级 AWR 性能工程，原生装进 Agent 本体。</text>')
    parts.append(f'<text x="1426" y="602" font-family="var(--f-m)" font-size="12.5" fill="#A6CFD5" text-anchor="end">W18 实测：2,200→60 tok · M6 = 7 μs</text>')
    parts.append("</svg>")
    return "".join(parts)


def vis_evolution_flywheel(animated: bool = True) -> str:
    """True Circular Self-Evolution Flywheel with P1~P5 Probes orbiting the Tuning Core."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("ef")]

    cx, cy, R = 480, 316, 224
    # Step 1: Circular Ring Track + Center Engine
    parts.append(s(1))
    parts.append(f'<circle cx="{cx}" cy="{cy}" r="{R}" fill="none" stroke="{C_TEAL_M}" stroke-width="5" stroke-dasharray="12 8" class="flow-line"/>')
    parts.append(f'<circle cx="{cx}" cy="{cy}" r="96" fill="{C_TEAL_D}"/>')
    parts.append(icon("flywheel", cx, cy - 18, 44, C_PAPER))
    parts.append(f'<text x="{cx}" y="{cy + 28}" font-family="var(--f-r)" font-size="21" fill="{C_PAPER}" font-weight="700" text-anchor="middle">自进化中枢</text>')
    parts.append(f'<text x="{cx}" y="{cy + 52}" font-family="var(--f-m)" font-size="12" fill="#A6CFD5" text-anchor="middle">数据驱动闭环</text>')
    parts.append("</g>")

    # Step 2: 5 Probes orbiting on circle
    parts.append(s(2))
    probes = [
        ("P1 事实漂移", "磁盘真值 vs 快照", "radar", C_TEAL),
        ("P2 召回退化", "FTS / 向量命中率", "microscope", C_GREEN),
        ("P3 租约碰壁", "近 24h AOPCC 升降级", "bolt", C_CLAY),
        ("P4 规则摩擦", "手动搬移聚类学习", "spine", C_GOLD),
        ("P5 遥测稳态", "AWR 等待与内存峰值", "gauge", C_TEAL_2),
    ]
    for i, (pt, ps, ic, col) in enumerate(probes):
        ang = math.radians(-90 + i * 72)
        px = cx + R * math.cos(ang)
        py = cy + R * math.sin(ang)
        parts.append(f'<rect x="{px - 104:.1f}" y="{py - 40:.1f}" width="208" height="80" rx="16" fill="{C_WHITE}" stroke="{col}" stroke-width="2" filter="url(#ef_sh)"/>')
        parts.append(icon_badge(ic, px - 68, py, 20, col))
        parts.append(f'<text x="{px - 36:.1f}" y="{py - 4:.1f}" font-family="var(--f-s)" font-size="15" fill="{C_INK}" font-weight="700">{pt}</text>')
        parts.append(f'<text x="{px - 36:.1f}" y="{py + 20:.1f}" font-family="var(--f-s)" font-size="12" fill="{col}" font-weight="600">{ps}</text>')
    parts.append("</g>")

    # Step 3: Right Side Closed-Loop Output Pipeline
    parts.append(s(3))
    parts.append(f'<line x1="766" y1="316" x2="844" y2="316" stroke="{C_TEAL}" stroke-width="4" marker-end="url(#ef_t)"/>')
    parts.append(f'<rect x="854" y="42" width="600" height="546" rx="22" fill="{C_WHITE}" stroke="{C_LINE}" stroke-width="1.8"/>')
    parts.append(f'<text x="886" y="82" font-family="var(--f-m)" font-size="12.5" fill="{C_TEAL}" font-weight="700" letter-spacing="1.6">CLOSED-LOOP ACTION · 越用越稳的四大自动动作</text>')
    actions = [
        ("动态调租约", "高碰壁服务自动降级为悲观门禁", "bolt", C_CLAY, 152),
        ("自动沉淀规约", "高频目录搬移聚类成新放置规则", "spine", C_GOLD, 268),
        ("去重提反馈", "真实摩擦写入 user_feedback.md", "audit", C_TEAL, 384),
        ("Before / After", "每次自进化必带量化对比存证", "trophy", C_GREEN, 500),
    ]
    for at, ad, ic, col, ay in actions:
        parts.append(f'<rect x="886" y="{ay - 40}" width="536" height="90" rx="14" fill="{C_PAPER}" stroke="{col}" stroke-width="1.6"/>')
        parts.append(icon_badge(ic, 934, ay + 5, 24, col, C_WHITE))
        parts.append(f'<text x="976" y="{ay - 2}" font-family="var(--f-r)" font-size="20" fill="{C_INK}" font-weight="700">{at}</text>')
        parts.append(f'<text x="976" y="{ay + 26}" font-family="var(--f-s)" font-size="14" fill="{C_INK2}">{ad}</text>')
    parts.append("</g>")

    parts.append("</svg>")
    return "".join(parts)


def vis_benchmark_pareto(animated: bool = True) -> str:
    """Benchmark v3.9 Hero Visual: 2D Pareto Quadrant + 4 Ring Gauges + Microsecond SLA Bars."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("bp")]

    # Step 1: Left 2D Pareto Quadrant (810 x 540)
    parts.append(s(1))
    parts.append(f'<rect x="16" y="14" width="810" height="540" rx="20" fill="{C_WHITE}" stroke="{C_TEAL}" stroke-width="2"/>')
    parts.append(f'<text x="46" y="48" font-family="var(--f-m)" font-size="12.5" fill="{C_TEAL}" font-weight="700" letter-spacing="1.6">PARETO DUAL-GATE · 绝不靠丢信息骗压缩率</text>')

    # Plot Area: x=106..782 (w=676), y=82..474 (h=392)
    parts.append(f'<rect x="106" y="82" width="676" height="392" rx="10" fill="{C_PAPER}"/>')
    # Pareto Golden Zone (Top-Right)
    parts.append(f'<rect x="520" y="82" width="262" height="148" rx="10" fill="{C_GREEN_L}"/>')
    parts.append(f'<text x="651" y="108" font-family="var(--f-m)" font-size="11.5" fill="{C_GREEN}" font-weight="700" text-anchor="middle">帕累托双赢黄金区</text>')
    # Danger Lossy Zone (Bottom-Right)
    parts.append(f'<rect x="520" y="346" width="262" height="128" rx="10" fill="{C_RED_L}" fill-opacity="0.55"/>')
    parts.append(f'<text x="651" y="424" font-family="var(--f-s)" font-size="13" fill="{C_RED}" font-weight="700" text-anchor="middle">🚫 伪省 Token 失真区</text>')
    parts.append(f'<text x="651" y="446" font-family="var(--f-s)" font-size="11.5" fill="{C_INK2}" text-anchor="middle">丢事实骗压缩 → 门禁熔断</text>')
    # Axes
    parts.append(f'<line x1="106" y1="474" x2="782" y2="474" stroke="{C_INK}" stroke-width="2"/>')
    parts.append(f'<line x1="106" y1="474" x2="106" y2="82" stroke="{C_INK}" stroke-width="2"/>')
    parts.append(f'<text x="444" y="506" font-family="var(--f-s)" font-size="14" fill="{C_INK2}" font-weight="600" text-anchor="middle">Prompt Token 节省率 (0% → 100%) →</text>')
    parts.append(f'<text x="62" y="278" font-family="var(--f-s)" font-size="14" fill="{C_INK2}" font-weight="600" text-anchor="middle" transform="rotate(-90 62 278)">四维上下文质量分 Q (0 → 100) →</text>')

    # Baseline red dot (0% saving, Q=35.6 -> y = 474 - 392*0.356 = 334)
    parts.append(f'<circle cx="164" cy="334" r="22" fill="{C_RED_L}" stroke="{C_RED}" stroke-width="2.5"/>')
    parts.append(f'<circle cx="164" cy="334" r="8" fill="{C_RED}"/>')
    parts.append(f'<text x="234" y="328" font-family="var(--f-s)" font-size="15" fill="{C_RED}" font-weight="700">裸奔大脑基线</text>')
    parts.append(f'<text x="234" y="352" font-family="var(--f-m)" font-size="12.5" fill="{C_INK2}">662,502 tok · Q = 35.6</text>')

    # Leap Arc to Agent Body (99.23% saving, Q=98.0 -> x=734, y=132)
    parts.append(f'<path d="M 186,322 C 310,206 490,148 704,138" fill="none" stroke="{C_GREEN}" stroke-width="4.5" stroke-dasharray="10 6" class="flow-line" marker-end="url(#bp_g)"/>')
    parts.append(f'<rect x="336" y="198" width="166" height="36" rx="18" fill="{C_GREEN}"/>')
    parts.append(f'<text x="419" y="221" font-family="var(--f-m)" font-size="14" fill="{C_PAPER}" font-weight="700" text-anchor="middle">信息密度 94.5x</text>')

    # Scatter workload dots near optimum (clean spacing)
    for wx, wy, wl in [(566, 160, "W15"), (618, 148, "W16"), (668, 162, "W17")]:
        parts.append(f'<circle cx="{wx}" cy="{wy}" r="6" fill="{C_TEAL}"/>')
        parts.append(f'<text x="{wx}" y="{wy + 20}" font-family="var(--f-m)" font-size="11" fill="{C_TEAL}" font-weight="700" text-anchor="middle">{wl}</text>')
    parts.append(f'<circle cx="728" cy="134" r="18" fill="{C_GREEN}" fill-opacity="0.22" stroke="{C_GREEN}" stroke-width="2.6"/>')
    parts.append(f'<circle cx="728" cy="134" r="7" fill="{C_GREEN}"/>')
    parts.append(f'<text x="728" y="168" font-family="var(--f-m)" font-size="11" fill="{C_GREEN}" font-weight="700" text-anchor="middle">W7 / 均值</text>')
    parts.append(f'<text x="646" y="206" font-family="var(--f-s)" font-size="15" fill="{C_GREEN}" font-weight="700" text-anchor="middle">Agent Body v1.1 · Q = 98.0</text>')
    parts.append(f'<text x="444" y="538" font-family="var(--f-m)" font-size="12" fill="{C_MUTE}" text-anchor="middle">SHA-256 防篡改真机存证 · W1~W19 全 19 项帕累托双赢</text>')
    parts.append("</g>")

    # Step 2: Right Top 2x2 Ring Gauges + Bottom Microsecond SLA Bars
    parts.append(s(2))
    parts.append(f'<rect x="850" y="14" width="606" height="278" rx="20" fill="{C_WHITE}" stroke="{C_GREEN}" stroke-width="2"/>')
    parts.append(f'<text x="878" y="48" font-family="var(--f-m)" font-size="12" fill="{C_GREEN}" font-weight="700">BENCHMARK v3.9 核心实证</text>')
    parts.append(ring_gauge(960, 156, 52, 99.23, "-99.2%", "Token 节省", C_TEAL))
    parts.append(ring_gauge(1154, 156, 52, 98.0, "98.0", "质量分 (+64.2)", C_GREEN))
    parts.append(ring_gauge(1348, 156, 52, 100.0, "39/39", "全维门禁 PASS", C_CLAY))
    parts.append(f'<text x="1153" y="270" font-family="var(--f-m)" font-size="13" fill="{C_INK2}" text-anchor="middle">662,502 → 5,114 tok · 18/18 WaitEvent 全埋点</text>')
    parts.append("</g>")

    parts.append(s(3))
    parts.append(f'<rect x="850" y="314" width="606" height="240" rx="20" fill="{C_WHITE}" stroke="{C_LINE}" stroke-width="1.6"/>')
    parts.append(f'<text x="878" y="348" font-family="var(--f-m)" font-size="12" fill="{C_TEAL}" font-weight="700">M1 ~ M6 内核微秒 SLA（越短越快）</text>')
    slas = [
        ("M6 无锁探针", "7 μs", 5, C_GREEN),
        ("M3 放置建议", "33 μs", 16, C_TEAL),
        ("M1 3跳爆炸图", "139 μs", 46, C_TEAL_2),
        ("M2 FTS5 召回", "259 μs", 74, C_CLAY),
    ]
    for i, (mn, mv, pct, col) in enumerate(slas):
        my = 392 + i * 40
        parts.append(f'<text x="878" y="{my}" font-family="var(--f-s)" font-size="13.5" fill="{C_INK}" font-weight="600">{mn}</text>')
        parts.append(f'<rect x="1008" y="{my - 13}" width="320" height="16" rx="8" fill="{C_PAPER2}"/>')
        parts.append(f'<rect x="1008" y="{my - 13}" width="{int(320 * pct / 100)}" height="16" rx="8" fill="{col}"/>')
        parts.append(f'<text x="1426" y="{my}" font-family="var(--f-m)" font-size="13.5" fill="{col}" font-weight="700" text-anchor="end">{mv}</text>')
    parts.append("</g>")

    # Bottom banner
    parts.append(f'<rect x="16" y="572" width="1440" height="54" rx="14" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="48" y="605" font-family="var(--f-r)" font-size="19" fill="{C_PAPER}" font-weight="700">19 业务负载 + 6 微基准 + 10 宿主门禁 + 4 竞品同跑：用可复现的真机数字说话。</text>')
    parts.append(f'<text x="1426" y="605" font-family="var(--f-m)" font-size="12.5" fill="#A6CFD5" text-anchor="end">agent-body bench run --suite full</text>')
    parts.append("</svg>")
    return "".join(parts)


def vis_competitor_arena(animated: bool = True) -> str:
    """Visual 4-Track Head-to-Head Competitor Bar Arena (COMP-C1 ~ C4)."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">']

    tracks = [
        ("COMP-C1 · 长期记忆脱敏", "裸流水 (Mem0/JSON)", "5/5 明文泄露", 100, "0 泄露 · Top-5", 3, "+75.9 分 · -97.8%", "shield", C_CLAY),
        ("COMP-C2 · 上下文路由", "OpenViking 静态全注入", "144,730 tok", 100, "931 tok (0 副本)", 4, "+54.5 分 · -99.4%", "spine", C_TEAL),
        ("COMP-C3 · 文件系统治理", "AgentFS VFS 全量导入", "290 全量入库", 100, "10 净入 · 拦黑名单", 6, "+73.6 分 · -96.5%", "folder", C_GREEN),
        ("COMP-C4 · 宿主探针拓扑", "rg + psutil + ss 拼装", "89ms · 0 跳拓扑", 100, "20ms · 3 跳影响图", 22, "+60.2 分 · 4.4x 快", "radar", C_GOLD),
    ]
    for i, (tname, comp, c_txt, c_pct, b_txt, b_pct, delta, ic, col) in enumerate(tracks):
        ty = 18 + i * 134
        parts.append(s(i + 1))
        parts.append(f'<rect x="16" y="{ty}" width="1440" height="118" rx="18" fill="{C_WHITE}" stroke="{C_LINE}" stroke-width="1.5"/>')
        parts.append(f'<rect x="16" y="{ty}" width="8" height="118" rx="4" fill="{col}"/>')
        parts.append(icon_badge(ic, 74, ty + 59, 26, col))
        parts.append(f'<text x="118" y="{ty + 46}" font-family="var(--f-r)" font-size="20" fill="{C_INK}" font-weight="700">{tname}</text>')
        parts.append(f'<text x="118" y="{ty + 78}" font-family="var(--f-s)" font-size="13.5" fill="{C_MUTE}">对标：{comp}</text>')

        # Visual red vs green bars in middle (x=470..1130)
        parts.append(f'<rect x="470" y="{ty + 22}" width="640" height="28" rx="8" fill="{C_RED_L}"/>')
        parts.append(f'<rect x="470" y="{ty + 22}" width="{int(640 * c_pct / 100)}" height="28" rx="8" fill="{C_RED}" fill-opacity="0.78"/>')
        parts.append(f'<text x="486" y="{ty + 41}" font-family="var(--f-m)" font-size="13" fill="{C_PAPER}" font-weight="700">对标：{c_txt}</text>')

        parts.append(f'<rect x="470" y="{ty + 64}" width="640" height="28" rx="8" fill="{C_GREEN_L}"/>')
        bw = max(24, int(640 * b_pct / 100))
        parts.append(f'<rect x="470" y="{ty + 64}" width="{bw}" height="28" rx="8" fill="{C_GREEN}"/>')
        parts.append(f'<text x="{470 + bw + 14}" y="{ty + 83}" font-family="var(--f-m)" font-size="13.5" fill="{C_GREEN}" font-weight="700">Agent Body：{b_txt}</text>')

        # Right delta badge
        parts.append(f'<rect x="1152" y="{ty + 30}" width="276" height="58" rx="14" fill="{C_TEAL_L}" stroke="{col}" stroke-width="1.5"/>')
        parts.append(f'<text x="1290" y="{ty + 65}" font-family="var(--f-m)" font-size="16" fill="{col}" font-weight="700" text-anchor="middle">{delta}</text>')
        parts.append("</g>")

    parts.append(f'<rect x="16" y="566" width="1440" height="58" rx="14" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="48" y="601" font-family="var(--f-r)" font-size="18.5" fill="{C_PAPER}" font-weight="700">诚实开放边界：Mem0 / Letta / PowerContext 通过 Memory SPI 一行兼容拥抱，绝非零和替代。</text>')
    parts.append(f'<text x="1426" y="601" font-family="var(--f-m)" font-size="12.5" fill="#A6CFD5" text-anchor="end">LIVE_BENCHMARKED · 同机同数据</text>')
    parts.append("</svg>")
    return "".join(parts)


def vis_microkernel_hub(animated: bool = True) -> str:
    """PostgreSQL-style Microkernel Star Hub (3 Kernel + 10 Contrib) + 4 Editions Size Tower."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">']

    # Step 1 & 2: Left Star Microkernel + 10 Orbiting Contrib Extensions
    parts.append(f'<rect x="16" y="14" width="842" height="536" rx="20" fill="{C_WHITE}" stroke="{C_TEAL}" stroke-width="2"/>')
    parts.append(f'<text x="44" y="48" font-family="var(--f-m)" font-size="12.5" fill="{C_TEAL}" font-weight="700" letter-spacing="1.6">KERNEL / CONTRIB · 借鉴 PostgreSQL 插件微内核星型架构</text>')

    cx, cy = 437, 300
    parts.append(s(1))
    parts.append(f'<circle cx="{cx}" cy="{cy}" r="196" fill="{C_TEAL_L}" fill-opacity="0.35" stroke="{C_TEAL_M}" stroke-width="1.8" stroke-dasharray="7 5"/>')
    hex_pts = " ".join(f"{cx + 108 * math.cos(math.radians(60 * k - 30)):.1f},{cy + 108 * math.sin(math.radians(60 * k - 30)):.1f}" for k in range(6))
    parts.append(f'<polygon points="{hex_pts}" fill="{C_TEAL_D}"/>')
    parts.append(icon("gear", cx, cy - 34, 38, C_PAPER))
    parts.append(f'<text x="{cx}" y="{cy + 10}" font-family="var(--f-r)" font-size="22" fill="{C_PAPER}" font-weight="700" text-anchor="middle">微内核</text>')
    parts.append(f'<text x="{cx}" y="{cy + 36}" font-family="var(--f-m)" font-size="11.5" fill="#B6D6DA" text-anchor="middle">3 Crates · 8 Catalog</text>')
    parts.append(f'<text x="{cx}" y="{cy + 56}" font-family="var(--f-m)" font-size="11.5" fill="#B6D6DA" text-anchor="middle">6 Routine · 7 Hook</text>')
    parts.append("</g>")

    parts.append(s(2))
    contribs = [
        "Linux", "PG/DM", "JVM", "Nginx", "mTLS",
        "脱敏", "SIEM", "CMDB", "OTel", "Mesh",
    ]
    for i, name in enumerate(contribs):
        ang = math.radians(-90 + i * 36)
        px = cx + 196 * math.cos(ang)
        py = cy + 196 * math.sin(ang)
        col = C_GREEN if i % 2 == 0 else C_TEAL_2
        parts.append(f'<line x1="{cx + 108 * math.cos(ang):.1f}" y1="{cy + 108 * math.sin(ang):.1f}" x2="{px:.1f}" y2="{py:.1f}" stroke="{col}" stroke-width="2"/>')
        parts.append(f'<rect x="{px - 48:.1f}" y="{py - 20:.1f}" width="96" height="40" rx="12" fill="{C_WHITE}" stroke="{col}" stroke-width="2"/>')
        parts.append(f'<text x="{px:.1f}" y="{py + 5:.1f}" font-family="var(--f-m)" font-size="13" fill="{col}" font-weight="700" text-anchor="middle">{name}</text>')
    parts.append(f'<text x="{cx}" y="534" font-family="var(--f-m)" font-size="13" fill="{C_GREEN}" font-weight="700" text-anchor="middle">10 个独立 Contrib 扩展 · ext_*__ 前缀沙盒 · 0 内核业务污染</text>')
    parts.append("</g>")

    # Step 3: Right 4 Editions Visual Building Blocks (Height proportional to binary footprint)
    parts.append(s(3))
    parts.append(f'<rect x="882" y="14" width="574" height="536" rx="20" fill="{C_WHITE}" stroke="{C_CLAY}" stroke-width="2"/>')
    parts.append(f'<text x="910" y="48" font-family="var(--f-m)" font-size="12.5" fill="{C_CLAY}" font-weight="700" letter-spacing="1.6">4 EDITIONS · 一套底座按需拼装四大发行版</text>')

    editions = [
        ("Flagship", "全功能旗舰", "全功能 · Web", 310, C_TEAL_D, 954),
        ("Probe", "政务云探针", "6.7 MB · 0 C++", 190, C_GREEN, 1086),
        ("DBA", "数据库哨兵", "6.4 MB · 0 C++", 174, C_CLAY, 1218),
        ("Minimal", "纯微内核", "3.9 MB · 极简", 120, C_GOLD, 1350),
    ]
    base_y = 444
    for ename, erole, esub, bh, col, ex in editions:
        by = base_y - bh
        parts.append(f'<rect x="{ex - 54}" y="{by}" width="108" height="{bh}" rx="14" fill="{col}"/>')
        # Stacked block lines
        for sy in range(by + 36, base_y - 16, 36):
            parts.append(f'<line x1="{ex - 42}" y1="{sy}" x2="{ex + 42}" y2="{sy}" stroke="{C_PAPER}" stroke-opacity="0.28" stroke-width="1.5"/>')
        parts.append(f'<text x="{ex}" y="{by - 12}" font-family="var(--f-m)" font-size="13.5" fill="{col}" font-weight="700" text-anchor="middle">{esub.split("·")[0].strip()}</text>')
        parts.append(f'<text x="{ex}" y="{base_y + 28}" font-family="var(--f-m)" font-size="14" fill="{C_INK}" font-weight="700" text-anchor="middle">{ename}</text>')
        parts.append(f'<text x="{ex}" y="{base_y + 52}" font-family="var(--f-s)" font-size="12.5" fill="{C_INK2}" text-anchor="middle">{erole}</text>')
    parts.append(f'<line x1="904" y1="{base_y}" x2="1428" y2="{base_y}" stroke="{C_LINE}" stroke-width="2"/>')
    parts.append(f'<text x="1169" y="532" font-family="var(--f-m)" font-size="12.5" fill="{C_CLAY}" font-weight="700" text-anchor="middle">Probe / DBA / Minimal 均零 libc++ · 零 DuckDB 运行时</text>')
    parts.append("</g>")

    parts.append(f'<rect x="16" y="568" width="1440" height="56" rx="14" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="48" y="602" font-family="var(--f-r)" font-size="19" fill="{C_PAPER}" font-weight="700">新业务/新行业不改内核 1 行代码，全通过 Contrib 扩展热插拔装配。</text>')
    parts.append(f'<text x="1426" y="602" font-family="var(--f-m)" font-size="12.5" fill="#A6CFD5" text-anchor="end">make arch-check PASS · ISSUE #52~#54</text>')
    parts.append("</svg>")
    return "".join(parts)


def vis_mesh_network(animated: bool = True) -> str:
    """Multi-Host Zero-Trust MeshEnvelope v1.0 Topology with HMAC Envelopes & 10MB Sandbox Inbox."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("mn")]

    parts.append(f'<rect x="16" y="14" width="1440" height="536" rx="22" fill="{C_PAPER2}" stroke="{C_LINE}" stroke-width="1.5"/>')
    parts.append(f'<text x="44" y="48" font-family="var(--f-m)" font-size="12.5" fill="{C_TEAL}" font-weight="700" letter-spacing="1.8">MESHENVELOPE v1.0 · 多宿主机零信任联邦协同（拒绝重型 Raft）</text>')

    # 3 Host Nodes
    hosts = [
        ("宿主机 A · 开发机", "Body-Alpha", "主战代码 + 本机 PG", 242, 294, C_TEAL, "server"),
        ("宿主机 B · GPU 算力机", "Body-GPU", "模型推理 + 训练任务", 1230, 174, C_CLAY, "rocket"),
        ("宿主机 C · 运维跳板机", "Body-Ops", "监控巡检 + 沙盒接收", 1230, 424, C_GREEN, "shield"),
    ]
    parts.append(s(1))
    for ht, hb, hs, hx, hy, col, ic in hosts:
        parts.append(f'<rect x="{hx - 178}" y="{hy - 84}" width="356" height="168" rx="20" fill="{C_WHITE}" stroke="{col}" stroke-width="2.2" filter="url(#mn_sh)"/>')
        parts.append(f'<rect x="{hx - 178}" y="{hy - 84}" width="356" height="10" rx="5" fill="{col}"/>')
        parts.append(icon_badge(ic, hx - 118, hy - 8, 28, col))
        parts.append(f'<text x="{hx - 74}" y="{hy - 22}" font-family="var(--f-r)" font-size="20" fill="{C_INK}" font-weight="700">{ht}</text>')
        parts.append(f'<text x="{hx - 74}" y="{hy + 8}" font-family="var(--f-m)" font-size="13.5" fill="{col}" font-weight="700">{hb}</text>')
        parts.append(f'<rect x="{hx - 150}" y="{hy + 32}" width="300" height="34" rx="10" fill="{C_PAPER}"/>')
        parts.append(f'<text x="{hx}" y="{hy + 54}" font-family="var(--f-s)" font-size="13.5" fill="{C_INK2}" text-anchor="middle">{hs}</text>')
    parts.append("</g>")

    # Center Envelopes flying with HMAC lock + Nonce clock
    parts.append(s(2))
    parts.append(f'<path d="M 420,254 Q 736,154 1044,174" fill="none" stroke="{C_TEAL}" stroke-width="3.5" stroke-dasharray="9 6" class="flow-line" marker-end="url(#mn_t)"/>')
    parts.append(f'<path d="M 420,334 Q 736,434 1044,424" fill="none" stroke="{C_GREEN}" stroke-width="3.5" stroke-dasharray="9 6" class="flow-line" marker-end="url(#mn_g)"/>')
    parts.append(f'<path d="M 1230,258 L 1230,334" fill="none" stroke="{C_CLAY}" stroke-width="3" stroke-dasharray="7 5" class="flow-line" marker-end="url(#mn_c)"/>')

    # Envelope Anatomy Card in middle
    parts.append(f'<rect x="524" y="198" width="424" height="196" rx="18" fill="{C_TEAL_D}" filter="url(#mn_sh)"/>')
    parts.append(f'<text x="736" y="234" font-family="var(--f-m)" font-size="12.5" fill="#A6CFD5" font-weight="700" text-anchor="middle" letter-spacing="1.6">ZERO-TRUST ENVELOPE</text>')
    env_rows = [
        ("🔒 HMAC-SHA256 规范签名", 272),
        ("⏱️ ±300s 时间窗 + Nonce 防重放", 308),
        ("📦 双向脱敏 + 10MB 沙盒收件箱", 344),
        ("🎯 Peer Scopes 最小权限白名单", 376),
    ]
    for et, ey in env_rows:
        parts.append(f'<text x="560" y="{ey}" font-family="var(--f-s)" font-size="14.5" fill="{C_PAPER}" font-weight="600">{et}</text>')
    parts.append("</g>")

    parts.append(f'<rect x="16" y="568" width="1440" height="56" rx="14" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="48" y="602" font-family="var(--f-r)" font-size="19" fill="{C_PAPER}" font-weight="700">每台机器自治独立，跨机只传签名信封——绝不引入分布式共识的运维泥潭。</text>')
    parts.append(f'<text x="1426" y="602" font-family="var(--f-m)" font-size="12.5" fill="#A6CFD5" text-anchor="end">W14 实测：防重放 100% 拦截 · Q 12.5→100</text>')
    parts.append("</svg>")
    return "".join(parts)


def vis_do_dont_wall(animated: bool = True) -> str:
    """Visual Architectural Boundary Wall: What Agent Body DOES vs NEVER DOES."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">']

    # Left Green Fortress: DO
    parts.append(s(1))
    parts.append(f'<rect x="16" y="16" width="676" height="534" rx="22" fill="{C_GREEN_L}" fill-opacity="0.58" stroke="{C_GREEN}" stroke-width="2.2"/>')
    parts.append(f'<rect x="16" y="16" width="676" height="64" rx="20" fill="{C_GREEN}"/>')
    parts.append(icon("check", 62, 48, 30, C_PAPER))
    parts.append(f'<text x="94" y="55" font-family="var(--f-r)" font-size="24" fill="{C_PAPER}" font-weight="700">坚决做深 · 宿主机具身护城河</text>')
    dos = [
        ("零幻觉物理感官", "/proc · 端口 · L1/L2/L3 图谱", "eye", C_TEAL_D),
        ("毫秒反射防撞", "AOPCC 租约 · 锁 · 放置规约", "bolt", C_TEAL),
        ("保鲜海马体", "写穿失效 · <=300t 跨脑胶囊", "capsule", C_GREEN),
        ("稳态与审计", "双域留痕 · AWR · 轻量 Mesh", "shield", C_TEAL_2),
    ]
    for i, (dt, ds, ic, col) in enumerate(dos):
        dy = 112 + i * 106
        parts.append(f'<rect x="44" y="{dy}" width="620" height="86" rx="14" fill="{C_WHITE}" stroke="{col}" stroke-width="1.6"/>')
        parts.append(icon_badge(ic, 94, dy + 43, 24, col))
        parts.append(f'<text x="136" y="{dy + 36}" font-family="var(--f-r)" font-size="20" fill="{C_INK}" font-weight="700">{dt}</text>')
        parts.append(f'<text x="136" y="{dy + 64}" font-family="var(--f-m)" font-size="13" fill="{col}" font-weight="600">{ds}</text>')
    parts.append("</g>")

    # Center Red Boundary Wall
    parts.append(s(2))
    parts.append(f'<rect x="712" y="16" width="48" height="534" rx="12" fill="{C_CLAY}"/>')
    for wy in range(44, 530, 38):
        parts.append(f'<line x1="720" y1="{wy}" x2="752" y2="{wy + 16}" stroke="{C_PAPER}" stroke-opacity="0.35" stroke-width="3"/>')
    parts.append(f'<rect x="698" y="236" width="76" height="92" rx="16" fill="{C_INK}"/>')
    parts.append(f'<text x="736" y="276" font-family="var(--f-r)" font-size="18" fill="{C_PAPER}" font-weight="700" text-anchor="middle">红线</text>')
    parts.append(f'<text x="736" y="304" font-family="var(--f-m)" font-size="11.5" fill="#F2BE9E" text-anchor="middle">克制</text>')
    parts.append("</g>")

    # Right Forbidden Zone: DO NOT
    parts.append(s(3))
    parts.append(f'<rect x="780" y="16" width="676" height="534" rx="22" fill="{C_RED_L}" fill-opacity="0.58" stroke="{C_RED}" stroke-width="2.2"/>')
    parts.append(f'<rect x="780" y="16" width="676" height="64" rx="20" fill="{C_RED}"/>')
    parts.append(icon("ban", 826, 48, 30, C_PAPER))
    parts.append(f'<text x="858" y="55" font-family="var(--f-r)" font-size="24" fill="{C_PAPER}" font-weight="700">坚决不做 · 递弱代偿边界</text>')
    donts = [
        ("不做主战大脑", "不抢 ReAct 规划与推理循环", "brain", C_RED),
        ("不做多 Agent 编排", "不替代 LangGraph / CrewAI", "mesh", C_CLAY),
        ("不造重型基础轮子", "不造容器沙箱 / Vault / Raft", "server", C_GOLD),
        ("不独占记忆与 AST", "记忆开放 SPI · 代码交 CodeGraph", "microscope", C_INK2),
    ]
    for i, (dt, ds, ic, col) in enumerate(donts):
        dy = 112 + i * 106
        parts.append(f'<rect x="808" y="{dy}" width="620" height="86" rx="14" fill="{C_WHITE}" stroke="{C_RED}" stroke-width="1.5" stroke-dasharray="5 4"/>')
        parts.append(icon_badge("ban", 858, dy + 43, 24, C_RED, C_RED_L))
        parts.append(f'<text x="900" y="{dy + 36}" font-family="var(--f-r)" font-size="20" fill="{C_INK}" font-weight="700">{dt}</text>')
        parts.append(f'<text x="900" y="{dy + 64}" font-family="var(--f-s)" font-size="13.5" fill="{C_INK2}">{ds}</text>')
    parts.append("</g>")

    parts.append(f'<rect x="16" y="568" width="1440" height="56" rx="14" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="48" y="602" font-family="var(--f-r)" font-size="19" fill="{C_PAPER}" font-weight="700">越底层的基建越要纯粹：把 100% 创造力还给大脑，把 100% 确定性留给身体。</text>')
    parts.append(f'<text x="1426" y="602" font-family="var(--f-m)" font-size="12.5" fill="#A6CFD5" text-anchor="end">规约 #2 · 递弱代偿原则</text>')
    parts.append("</svg>")
    return "".join(parts)


def vis_storage_tri_engine(animated: bool = True) -> str:
    """4-Layer Topology + 88-Method StorageProvider SPI + SQLite / PostgreSQL / DuckDB Hot Migration."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("st")]

    # Layer 1 & 2 top bars
    parts.append(s(1))
    parts.append(f'<rect x="16" y="14" width="1440" height="92" rx="16" fill="{C_WHITE}" stroke="{C_TEAL}" stroke-width="1.8"/>')
    parts.append(f'<text x="44" y="46" font-family="var(--f-m)" font-size="12" fill="{C_TEAL}" font-weight="700">LAYER 1 · 零业务逻辑接入层</text>')
    for lbl, lx, col in [("43 MCP Tools", 220, C_TEAL_D), ("30 CLI Commands", 540, C_TEAL), ("7 Hard Hooks", 860, C_CLAY), ("Web HTTP/SSE", 1180, C_GREEN)]:
        parts.append(f'<rect x="{lx - 110}" y="54" width="220" height="38" rx="19" fill="{col}"/>')
        parts.append(f'<text x="{lx}" y="78" font-family="var(--f-m)" font-size="13.5" fill="{C_PAPER}" font-weight="700" text-anchor="middle">{lbl}</text>')
    parts.append(f'<line x1="736" y1="106" x2="736" y2="142" stroke="{C_TEAL}" stroke-width="3" marker-end="url(#st_t)"/>')
    parts.append("</g>")

    # Layer 2: 27 Modules Organ Engine
    parts.append(s(2))
    parts.append(f'<rect x="16" y="146" width="1440" height="106" rx="16" fill="{C_TEAL_L}" stroke="{C_TEAL}" stroke-width="1.8"/>')
    parts.append(f'<text x="44" y="178" font-family="var(--f-m)" font-size="12" fill="{C_TEAL_D}" font-weight="700">LAYER 2 · 六大器官引擎（src/lib.rs · 27 个 Rust 顶层模块）</text>')
    mods = ["fact / scanner", "links / graph", "brief / hooks", "coord / lock", "memory / body", "audit / mesh"]
    for i, m in enumerate(mods):
        mx = 44 + i * 234
        parts.append(f'<rect x="{mx}" y="194" width="218" height="44" rx="10" fill="{C_WHITE}" stroke="{C_TEAL_M}" stroke-width="1.4"/>')
        parts.append(f'<text x="{mx + 109}" y="221" font-family="var(--f-m)" font-size="13.5" fill="{C_INK}" font-weight="700" text-anchor="middle">{m}</text>')
    parts.append(f'<line x1="736" y1="252" x2="736" y2="286" stroke="{C_GOLD}" stroke-width="3.5" marker-end="url(#st_o)"/>')
    parts.append("</g>")

    # Layer 3: StorageProvider SPI Bus
    parts.append(s(3))
    parts.append(f'<rect x="16" y="290" width="1440" height="68" rx="16" fill="{C_TEAL_D}"/>')
    parts.append(icon("plug", 64, 324, 30, C_PAPER))
    parts.append(f'<text x="98" y="331" font-family="var(--f-r)" font-size="22" fill="{C_PAPER}" font-weight="700">StorageProvider SPI 统一门面</text>')
    parts.append(f'<rect x="520" y="306" width="220" height="36" rx="18" fill="#27697D"/>')
    parts.append(f'<text x="630" y="329" font-family="var(--f-m)" font-size="14" fill="{C_PAPER}" font-weight="700" text-anchor="middle">88 强类型方法</text>')
    parts.append(f'<rect x="768" y="306" width="220" height="36" rx="18" fill="#27697D"/>')
    parts.append(f'<text x="878" y="329" font-family="var(--f-m)" font-size="14" fill="{C_PAPER}" font-weight="700" text-anchor="middle">27 张三端同步表</text>')
    parts.append(f'<rect x="1016" y="306" width="388" height="36" rx="18" fill="{C_GREEN}"/>')
    parts.append(f'<text x="1210" y="329" font-family="var(--f-m)" font-size="13.5" fill="{C_PAPER}" font-weight="700" text-anchor="middle">SyncPipeline 零停机热迁移</text>')
    parts.append("</g>")

    # Layer 4: 3 Database Cylinders
    parts.append(s(4))
    dbs = [
        ("SQLite WAL + FTS5", "默认零配置 · 个人开发机", "单二进制内嵌 · 读写亚毫秒", C_TEAL, 248),
        ("PostgreSQL + pgvector", "多 Agent 高并发集群", "tsvector + 原生 Advisory Lock", C_GREEN, 736),
        ("DuckDB 列存 OLAP", "审计热力图 + Parquet 导出", "受控 threads=2 · 256MB 上限", C_GOLD, 1224),
    ]
    for title, role, spec, col, dx in dbs:
        parts.append(f'<line x1="{dx}" y1="358" x2="{dx}" y2="410" stroke="{col}" stroke-width="3.2" stroke-dasharray="6 4" class="flow-line" marker-end="url(#st_t)"/>')
        parts.append(f'<rect x="{dx - 216}" y="416" width="432" height="202" rx="20" fill="{C_WHITE}" stroke="{col}" stroke-width="2.2" filter="url(#st_sh)"/>')
        parts.append(f'<rect x="{dx - 216}" y="416" width="432" height="10" rx="5" fill="{col}"/>')
        parts.append(icon_badge("database", dx - 150, 488, 30, col))
        parts.append(f'<text x="{dx - 102}" y="476" font-family="var(--f-r)" font-size="21" fill="{C_INK}" font-weight="700">{title}</text>')
        parts.append(f'<text x="{dx - 102}" y="508" font-family="var(--f-s)" font-size="14" fill="{col}" font-weight="700">{role}</text>')
        parts.append(f'<rect x="{dx - 188}" y="544" width="376" height="46" rx="12" fill="{C_PAPER}"/>')
        parts.append(f'<text x="{dx}" y="572" font-family="var(--f-m)" font-size="13" fill="{C_INK2}" text-anchor="middle">{spec}</text>')
    # Hot migration arrows between DBs
    parts.append(f'<line x1="464" y1="518" x2="518" y2="518" stroke="{C_GREEN}" stroke-width="3" marker-end="url(#st_g)"/>')
    parts.append(f'<line x1="952" y1="518" x2="1006" y2="518" stroke="{C_GOLD}" stroke-width="3" marker-end="url(#st_o)"/>')
    parts.append("</g>")

    parts.append("</svg>")
    return "".join(parts)


def vis_parity_console(animated: bool = True) -> str:
    """43/43 Black-Screen (MCP/CLI/Hooks) <-> Parity Matrix <-> White-Screen Web Cockpit + 7-Act Theater."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("pc")]

    # Left: Black-Screen Brain Interface (Light-slate terminal frame, readable not harsh black)
    parts.append(s(1))
    parts.append(f'<rect x="16" y="20" width="534" height="516" rx="20" fill="#233A44" stroke="{C_TEAL}" stroke-width="2"/>')
    parts.append(f'<circle cx="46" cy="48" r="6" fill="#E36D5D"/><circle cx="68" cy="48" r="6" fill="#E6B95E"/><circle cx="90" cy="48" r="6" fill="#63BA8B"/>')
    parts.append(f'<text x="120" y="53" font-family="var(--f-m)" font-size="12.5" fill="#A6CFD5" font-weight="700">BLACK SCREEN · 大脑极速接口</text>')
    # Terminal visual pills
    term_items = [
        ("43 原生 MCP 工具", "JSON-RPC 2.0 stdio · body_*", 112, "#357B92"),
        ("30 顶层 CLI 命令", "$ agent-body brief / links / perf-awr", 222, "#2E7B64"),
        ("7 大确定性 Hook", "SessionStart · PreToolUse · PostToolUse", 332, "#B86B43"),
        ("3 套内置 SKILL", "guide / developer / doc-writer", 442, "#9B7B32"),
    ]
    for tt, ts, ty, bg in term_items:
        parts.append(f'<rect x="44" y="{ty - 24}" width="478" height="86" rx="14" fill="#1B2E36" stroke="{bg}" stroke-width="1.6"/>')
        parts.append(f'<rect x="44" y="{ty - 24}" width="8" height="86" rx="4" fill="{bg}"/>')
        parts.append(f'<text x="72" y="{ty + 10}" font-family="var(--f-r)" font-size="19" fill="{C_PAPER}" font-weight="700">{tt}</text>')
        parts.append(f'<text x="72" y="{ty + 40}" font-family="var(--f-m)" font-size="13" fill="#B5D4D8">{ts}</text>')
    parts.append("</g>")

    # Center: 43/43 Parity Lock Core
    parts.append(s(2))
    parts.append(f'<line x1="550" y1="278" x2="626" y2="278" stroke="{C_TEAL}" stroke-width="4" stroke-dasharray="7 5" class="flow-line" marker-end="url(#pc_t)"/>')
    parts.append(f'<line x1="846" y1="278" x2="914" y2="278" stroke="{C_GREEN}" stroke-width="4" stroke-dasharray="7 5" class="flow-line" marker-end="url(#pc_g)"/>')
    parts.append(f'<rect x="630" y="138" width="212" height="280" rx="22" fill="{C_TEAL_D}" filter="url(#pc_sh)"/>')
    parts.append(icon("check", 736, 198, 44, C_GREEN_M))
    parts.append(f'<text x="736" y="264" font-family="var(--f-m)" font-size="36" fill="{C_PAPER}" font-weight="700" text-anchor="middle">43/43</text>')
    parts.append(f'<text x="736" y="298" font-family="var(--f-r)" font-size="19" fill="{C_PAPER}" font-weight="700" text-anchor="middle">1:1:1 同源对齐</text>')
    parts.append(f'<text x="736" y="334" font-family="var(--f-m)" font-size="11.5" fill="#A6CFD5" text-anchor="middle">/api/v1/parity/matrix</text>')
    parts.append(f'<rect x="664" y="356" width="144" height="36" rx="18" fill="{C_GREEN}"/>')
    parts.append(f'<text x="736" y="379" font-family="var(--f-m)" font-size="13" fill="{C_PAPER}" font-weight="700" text-anchor="middle">0 漂移强门禁</text>')
    parts.append("</g>")

    # Right: White-Screen Web Cockpit Wireframe Illustration
    parts.append(s(3))
    parts.append(f'<rect x="922" y="20" width="534" height="516" rx="20" fill="{C_WHITE}" stroke="{C_TEAL}" stroke-width="2" filter="url(#pc_sh)"/>')
    parts.append(f'<rect x="922" y="20" width="534" height="52" rx="18" fill="{C_TEAL_L}"/>')
    parts.append(f'<text x="952" y="52" font-family="var(--f-m)" font-size="12.5" fill="{C_TEAL_D}" font-weight="700">WHITE SCREEN · Web 指挥舱 (:8765)</text>')
    # Mini cockpit layout wireframe inside
    # 9 workbenches visual grid (3x3 mini tiles)
    benches = [
        ("拓扑图", C_TEAL), ("事实雷达", C_TEAL_2), ("协作锁", C_CLAY),
        ("规约栈", C_GOLD), ("双域审计", C_TEAL_D), ("AWR 大屏", C_GREEN),
        ("Bench", C_TEAL), ("生态 SPI", C_GREEN), ("7 幕剧场", C_CLAY),
    ]
    for i, (bn, col) in enumerate(benches):
        bx = 950 + (i % 3) * 164
        by = 96 + (i // 3) * 92
        parts.append(f'<rect x="{bx}" y="{by}" width="148" height="74" rx="12" fill="{C_PAPER}" stroke="{col}" stroke-width="1.5"/>')
        parts.append(f'<rect x="{bx}" y="{by}" width="148" height="6" rx="3" fill="{col}"/>')
        parts.append(f'<text x="{bx + 74}" y="{by + 48}" font-family="var(--f-s)" font-size="14.5" fill="{C_INK}" font-weight="700" text-anchor="middle">{bn}</text>')
    # Bottom 7-Act Roadshow Theater Highlight
    parts.append(f'<rect x="950" y="384" width="478" height="128" rx="16" fill="{C_CLAY_L}" stroke="{C_CLAY}" stroke-width="1.8"/>')
    parts.append(icon_badge("rocket", 1004, 448, 26, C_CLAY, C_WHITE))
    parts.append(f'<text x="1046" y="432" font-family="var(--f-r)" font-size="20" fill="{C_INK}" font-weight="700">7 幕真引擎路演剧场</text>')
    parts.append(f'<text x="1046" y="462" font-family="var(--f-s)" font-size="13.5" fill="{C_INK2}">左右分屏对比：盲人翻车 vs 本体化解</text>')
    parts.append(f'<text x="1046" y="490" font-family="var(--f-m)" font-size="12.5" fill="{C_CLAY}" font-weight="700">含第 7 幕「换脑不失忆」· 演完一键复原</text>')
    parts.append("</g>")

    parts.append(f'<rect x="16" y="566" width="1440" height="58" rx="14" fill="{C_TEAL_D}"/>')
    parts.append(f'<text x="48" y="601" font-family="var(--f-r)" font-size="19" fill="{C_PAPER}" font-weight="700">网页每点一次，旁边实时回显等价 CLI / MCP 命令——彻底告别黑白屏割裂。</text>')
    parts.append(f'<text x="1426" y="601" font-family="var(--f-m)" font-size="12.5" fill="#A6CFD5" text-anchor="end">单二进制内嵌 · 昼夜双主题</text>')
    parts.append("</svg>")
    return "".join(parts)


def vis_quality_gates(test_cnt: int = 396, mod_cnt: int = 27, tool_cnt: int = 43, animated: bool = True) -> str:
    """7-Stage Deterministic Quality Gate Security Conveyor + Runtime Budget Gauges."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("qg")]

    # Step 1: 6+1 Quality Gate Arches in Top Conveyor
    parts.append(s(1))
    parts.append(f'<rect x="16" y="16" width="1440" height="316" rx="22" fill="{C_WHITE}" stroke="{C_TEAL}" stroke-width="2"/>')
    parts.append(f'<text x="44" y="50" font-family="var(--f-m)" font-size="12.5" fill="{C_TEAL}" font-weight="700" letter-spacing="1.8">DETERMINISTIC QUALITY GATES · 每次提交必过确定性流水线门禁</text>')
    gates = [
        ("01", "evolution", "0 漂移", C_TEAL_D),
        ("02", "arch-check", "0 内核污染", C_TEAL),
        ("03", "lint-panic", "0 unwrap", C_CLAY),
        ("04", "hygiene", "0 >1000L", C_GOLD),
        ("05", "clippy -D", "0 warning", C_TEAL_2),
        ("06", "cargo test", f"{test_cnt} 全绿", C_GREEN),
        ("07", "Playwright", "0 溢出重叠", C_GREEN),
    ]
    for i, (no, gn, gs, col) in enumerate(gates):
        gx = 46 + i * 202
        parts.append(f'<rect x="{gx}" y="86" width="176" height="214" rx="16" fill="{C_PAPER}" stroke="{col}" stroke-width="2"/>')
        parts.append(f'<rect x="{gx}" y="86" width="176" height="40" rx="14" fill="{col}"/>')
        parts.append(f'<text x="{gx + 88}" y="112" font-family="var(--f-m)" font-size="15" fill="{C_PAPER}" font-weight="700" text-anchor="middle">GATE {no}</text>')
        parts.append(icon_badge("check", gx + 88, 174, 24, col, C_WHITE))
        parts.append(f'<text x="{gx + 88}" y="232" font-family="var(--f-m)" font-size="14.5" fill="{C_INK}" font-weight="700" text-anchor="middle">{gn}</text>')
        parts.append(f'<rect x="{gx + 22}" y="250" width="132" height="32" rx="16" fill="{C_WHITE}" stroke="{col}" stroke-width="1.3"/>')
        parts.append(f'<text x="{gx + 88}" y="271" font-family="var(--f-m)" font-size="12.5" fill="{col}" font-weight="700" text-anchor="middle">{gs}</text>')
        if i < len(gates) - 1:
            parts.append(f'<line x1="{gx + 176}" y1="192" x2="{gx + 200}" y2="192" stroke="{col}" stroke-width="2.6" marker-end="url(#qg_g)"/>')
    parts.append("</g>")

    # Step 2: Bottom 3 Runtime Engineering Bills (Big Visual Numbers)
    parts.append(s(2))
    bills = [
        ("6.7 MB", "政务探针极致体积", "0 libc++ · 0 DuckDB · 旗舰 CLI <15MB", "rocket", C_CLAY, 16),
        ("100%", "位置无关 &amp; 跨平台", "单二进制任意目录即跑 · Linux/Mac/Win", "cube", C_TEAL, 504),
        (f"{mod_cnt} / {tool_cnt}", "零重扫演进体系", "EVOLUTION_GUIDE + .codegraph 2 轮定位", "microscope", C_GREEN, 992),
    ]
    for bv, bt, bs, ic, col, bx in bills:
        parts.append(f'<rect x="{bx}" y="356" width="464" height="264" rx="20" fill="{C_WHITE}" stroke="{col}" stroke-width="2" filter="url(#qg_sh)"/>')
        parts.append(f'<rect x="{bx}" y="356" width="464" height="10" rx="5" fill="{col}"/>')
        parts.append(icon_badge(ic, bx + 66, 434, 30, col))
        parts.append(f'<text x="{bx + 118}" y="448" font-family="var(--f-m)" font-size="44" fill="{col}" font-weight="700">{bv}</text>')
        parts.append(f'<text x="{bx + 36}" y="514" font-family="var(--f-r)" font-size="22" fill="{C_INK}" font-weight="700">{bt}</text>')
        parts.append(f'<rect x="{bx + 32}" y="540" width="400" height="52" rx="12" fill="{C_PAPER}"/>')
        parts.append(f'<text x="{bx + 232}" y="571" font-family="var(--f-s)" font-size="13.5" fill="{C_INK2}" font-weight="600" text-anchor="middle">{bs}</text>')
    parts.append("</g>")

    parts.append("</svg>")
    return "".join(parts)


def vis_three_tier_pyramid(
    tiers: list[tuple[str, str, str, str]],
    footer: str = "",
    animated: bool = True,
) -> str:
    """Visual 3-Tier Pyramid Diagram (e.g., Naming Contract / Evolution Layers / 3-Track Bench)."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">']

    # Draw 3 stacked trapezoid / widening bars centered at x=520 + right callout cards
    widths = [440, 620, 800]
    ys = [44, 214, 384]
    h = 148
    icons_def = ["rocket", "plug", "shield"]
    for i, (tag, title, detail, col) in enumerate(tiers[:3]):
        parts.append(s(i + 1))
        w = widths[i]
        y = ys[i]
        cx = 440
        x0 = cx - w / 2
        # Tapered trapezoid for visual pyramid feel
        top_w = w - 68
        pts = f"{cx - top_w/2:.1f},{y} {cx + top_w/2:.1f},{y} {cx + w/2:.1f},{y + h} {cx - w/2:.1f},{y + h}"
        parts.append(f'<polygon points="{pts}" fill="{col}"/>')
        parts.append(f'<text x="{cx}" y="{y + 58}" font-family="var(--f-m)" font-size="13" fill="{C_PAPER}" opacity="0.85" font-weight="700" text-anchor="middle">{esc(tag)}</text>')
        parts.append(f'<text x="{cx}" y="{y + 102}" font-family="var(--f-r)" font-size="25" fill="{C_PAPER}" font-weight="700" text-anchor="middle">{esc(title)}</text>')

        # Connecting horizontal line to right visual badge card
        rx = 890
        parts.append(f'<line x1="{cx + w/2 - 14:.1f}" y1="{y + h/2:.1f}" x2="{rx}" y2="{y + h/2:.1f}" stroke="{col}" stroke-width="2.4" stroke-dasharray="6 4"/>')
        parts.append(f'<rect x="{rx}" y="{y + 14}" width="564" height="120" rx="16" fill="{C_WHITE}" stroke="{col}" stroke-width="2"/>')
        parts.append(icon_badge(icons_def[i], rx + 54, y + 74, 26, col))
        parts.append(f'<text x="{rx + 98}" y="{y + 62}" font-family="var(--f-r)" font-size="20" fill="{C_INK}" font-weight="700">{esc(title)}</text>')
        parts.append(f'<text x="{rx + 98}" y="{y + 96}" font-family="var(--f-s)" font-size="14.5" fill="{C_INK2}">{esc(detail)}</text>')
        parts.append("</g>")

    if footer:
        parts.append(f'<rect x="16" y="566" width="1440" height="58" rx="14" fill="{C_TEAL_D}"/>')
        parts.append(f'<text x="48" y="601" font-family="var(--f-r)" font-size="19" fill="{C_PAPER}" font-weight="700">{esc(footer)}</text>')
    parts.append("</svg>")
    return "".join(parts)


def vis_command_console(
    title: str,
    commands: list[tuple[str, str, str]],
    visual_badges: list[tuple[str, str, str, str]],
    footer: str = "",
) -> str:
    """Interactive-looking Terminal Console (left) + Visual Result Dashboard Cards (right)."""
    parts = ['<svg class="fig" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("cc")]

    # Left clean slate terminal (high contrast readable, not harsh black)
    parts.append(f'<rect x="16" y="16" width="840" height="534" rx="20" fill="#21353E" stroke="{C_TEAL}" stroke-width="2"/>')
    parts.append(f'<rect x="16" y="16" width="840" height="48" rx="18" fill="#2B4450"/>')
    parts.append(f'<circle cx="46" cy="40" r="6" fill="#E36D5D"/><circle cx="68" cy="40" r="6" fill="#E6B95E"/><circle cx="90" cy="40" r="6" fill="#63BA8B"/>')
    parts.append(f'<text x="118" y="45" font-family="var(--f-m)" font-size="13" fill="#B8DBE0" font-weight="700">{esc(title)}</text>')

    n = max(1, len(commands))
    step = min(84, 456 // n)
    for i, (cmd, desc, tag) in enumerate(commands[:6]):
        cy = 96 + i * step
        parts.append(f'<rect x="40" y="{cy - 18}" width="792" height="{step - 14}" rx="10" fill="#182830"/>')
        parts.append(f'<text x="58" y="{cy + 8}" font-family="var(--f-m)" font-size="14" fill="#7DE0B8" font-weight="700">$ {esc(cmd[:54])}</text>')
        parts.append(f'<text x="58" y="{cy + 34}" font-family="var(--f-s)" font-size="12.5" fill="#B5CDD2">{esc(desc[:48])}</text>')
        parts.append(f'<rect x="698" y="{cy - 6}" width="120" height="28" rx="14" fill="#2D5F70"/>')
        parts.append(f'<text x="758" y="{cy + 13}" font-family="var(--f-m)" font-size="11.5" fill="{C_PAPER}" font-weight="700" text-anchor="middle">{esc(tag[:12])}</text>')

    # Right visual live-output telemetry cards
    parts.append(f'<text x="888" y="42" font-family="var(--f-m)" font-size="12.5" fill="{C_TEAL}" font-weight="700" letter-spacing="1.6">LIVE ENGINE OUTPUT · 实时回显</text>')
    nb = max(1, len(visual_badges))
    bh = (494 - 16 * (nb - 1)) // nb
    for i, (val, lbl, sub, col) in enumerate(visual_badges[:4]):
        by = 56 + i * (bh + 16)
        parts.append(f'<rect x="888" y="{by}" width="568" height="{bh}" rx="16" fill="{C_WHITE}" stroke="{col}" stroke-width="2" filter="url(#cc_sh)"/>')
        parts.append(f'<rect x="888" y="{by}" width="8" height="{bh}" rx="4" fill="{col}"/>')
        parts.append(f'<text x="920" y="{by + int(bh * 0.56)}" font-family="var(--f-m)" font-size="30" fill="{col}" font-weight="700">{esc(val)}</text>')
        parts.append(f'<text x="1116" y="{by + int(bh * 0.42)}" font-family="var(--f-r)" font-size="19" fill="{C_INK}" font-weight="700">{esc(lbl)}</text>')
        parts.append(f'<text x="1116" y="{by + int(bh * 0.74)}" font-family="var(--f-s)" font-size="13.5" fill="{C_INK2}">{esc(sub[:28])}</text>')

    if footer:
        parts.append(f'<rect x="16" y="570" width="1440" height="54" rx="14" fill="{C_TEAL_D}"/>')
        parts.append(f'<text x="48" y="603" font-family="var(--f-r)" font-size="18.5" fill="{C_PAPER}" font-weight="700">{esc(footer)}</text>')
    parts.append("</svg>")
    return "".join(parts)


def vis_roadmap_journey(
    stops: list[tuple[str, str, str, str, str]],
    footer: str = "",
    animated: bool = True,
) -> str:
    """Visual Winding Journey Map (e.g., 6-Talk Curriculum or Evolution Roadmap)."""
    cls = "fig anim" if animated else "fig"
    s = lambda k: f'<g data-step="{k}">' if animated else "<g>"
    parts = [f'<svg class="{cls}" viewBox="0 0 1472 640" width="1472" height="640" xmlns="http://www.w3.org/2000/svg">', _arrow_defs("rj")]

    # Connecting S-curve path for 6 stops (3 top, 3 bottom) or linear for <=4 stops
    n = len(stops)
    if n >= 5:
        coords = [(250, 148), (736, 148), (1222, 148), (1222, 408), (736, 408), (250, 408)][:n]
        parts.append(
            f'<path d="M 250,148 L 1222,148 C 1380,148 1380,408 1222,408 L 250,408" '
            f'fill="none" stroke="{C_TEAL_M}" stroke-width="5" stroke-dasharray="10 7" class="flow-line"/>'
        )
    else:
        gap = 1280 // max(1, n - 1) if n > 1 else 0
        coords = [(96 + i * gap, 276) for i in range(n)]
        parts.append(f'<line x1="96" y1="276" x2="1376" y2="276" stroke="{C_TEAL_M}" stroke-width="5" stroke-dasharray="10 7" class="flow-line"/>')

    for i, ((no, title, metric, ic, col), (cx, cy)) in enumerate(zip(stops, coords)):
        parts.append(s(min(i + 1, 6)))
        w = 404 if n >= 5 else (310 if n == 4 else 410)
        h = 178 if n >= 5 else 248
        rx, ry = cx - w / 2, cy - h / 2
        parts.append(f'<rect x="{rx:.1f}" y="{ry:.1f}" width="{w}" height="{h}" rx="18" fill="{C_WHITE}" stroke="{col}" stroke-width="2" filter="url(#rj_sh)"/>')
        parts.append(f'<rect x="{rx:.1f}" y="{ry:.1f}" width="{w}" height="8" rx="4" fill="{col}"/>')
        parts.append(icon_badge(ic, rx + 50, ry + 62, 24, col))
        parts.append(f'<text x="{rx + 92:.1f}" y="{ry + 44:.1f}" font-family="var(--f-m)" font-size="12" fill="{col}" font-weight="700">{esc(no)}</text>')
        parts.append(f'<text x="{rx + 92:.1f}" y="{ry + 78:.1f}" font-family="var(--f-r)" font-size="20" fill="{C_INK}" font-weight="700">{esc(title[:16])}</text>')
        parts.append(f'<rect x="{rx + 24:.1f}" y="{ry + h - 54:.1f}" width="{w - 48}" height="34" rx="17" fill="{C_PAPER}"/>')
        parts.append(f'<text x="{cx:.1f}" y="{ry + h - 32:.1f}" font-family="var(--f-m)" font-size="13" fill="{C_INK2}" font-weight="600" text-anchor="middle">{esc(metric[:28])}</text>')
        parts.append("</g>")

    if footer:
        parts.append(f'<rect x="16" y="566" width="1440" height="58" rx="14" fill="{C_TEAL_D}"/>')
        parts.append(f'<text x="48" y="601" font-family="var(--f-r)" font-size="19" fill="{C_PAPER}" font-weight="700">{esc(footer)}</text>')
    parts.append("</svg>")
    return "".join(parts)
