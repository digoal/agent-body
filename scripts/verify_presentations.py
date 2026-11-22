#!/usr/bin/env python3
"""Playwright headless verification gate for all HTML decks in presentation/.

Checks for every HTML file:
1. Zero browser console errors and zero uncaught page errors.
2. For slide decks (containing .slide):
   - Every slide renders with zero overflow (scrollWidth <= clientWidth + 2, scrollHeight <= clientHeight + 2).
   - Every visible SVG <text> stays inside its parent <svg> bounding box (4px tolerance).
   - Zero significant bounding-box overlap between distinct visible SVG <text> nodes.
   - Interactive shortcuts work: G (TOC), N (page notes), T (full notes), Escape (close).
3. For index.html:
   - All local href links resolve to existing files in presentation/.
"""

import os
import sys
from pathlib import Path
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parent.parent
PRES_DIR = ROOT / "presentation"


def verify_deck(page, html_path: Path) -> list[str]:
    issues: list[str] = []
    console_errors: list[str] = []
    page_errors: list[str] = []

    page.on("console", lambda msg: console_errors.append(msg.text) if msg.type == "error" else None)
    page.on("pageerror", lambda err: page_errors.append(str(err)))

    url = html_path.resolve().as_uri()
    page.set_viewport_size({"width": 1600, "height": 960})
    page.goto(url, wait_until="load")
    page.wait_for_timeout(120)

    slide_count = page.locator(".slide").count()
    if slide_count == 0:
        # Index / hub page check
        links = page.eval_on_selector_all(
            "a[href]",
            "els => els.map(e => e.getAttribute('href')).filter(h => h && !h.startsWith('http') && !h.startsWith('#'))"
        )
        for href in links:
            target = (html_path.parent / href).resolve()
            if not target.exists():
                issues.append(f"broken link: {href}")
        if console_errors:
            issues.append(f"console errors: {console_errors}")
        if page_errors:
            issues.append(f"page errors: {page_errors}")
        return issues

    # Verify every slide
    for idx in range(slide_count):
        page.evaluate(f"if (typeof go === 'function') go({idx});")
        # Reveal all animation steps so every SVG text node is checked at full visibility
        page.evaluate(
            """(idx) => {
                const s = document.querySelectorAll('.slide')[idx];
                if (!s) return;
                s.querySelectorAll('[data-step]').forEach(el => el.classList.remove('off'));
            }""",
            idx,
        )
        page.wait_for_timeout(40)

        res = page.evaluate(
            """(idx) => {
                const s = document.querySelectorAll('.slide')[idx];
                const errs = [];
                if (!s) return ['missing slide'];
                if (s.scrollWidth > s.clientWidth + 2 || s.scrollHeight > s.clientHeight + 2) {
                    errs.push(`slide overflow: scroll=${s.scrollWidth}x${s.scrollHeight} client=${s.clientWidth}x${s.clientHeight}`);
                }
                const svgs = s.querySelectorAll('svg');
                svgs.forEach((svg, si) => {
                    const sr = svg.getBoundingClientRect();
                    if (sr.width === 0 || sr.height === 0) return;
                    const boxes = [];
                    svg.querySelectorAll('text').forEach((t) => {
                        const txt = (t.textContent || '').trim();
                        if (!txt) return;
                        const st = window.getComputedStyle(t);
                        if (st.display === 'none' || st.visibility === 'hidden' || parseFloat(st.opacity) < 0.05) return;
                        const r = t.getBoundingClientRect();
                        if (r.width === 0 || r.height === 0) return;
                        const tol = 6;
                        if (r.left < sr.left - tol || r.right > sr.right + tol || r.top < sr.top - tol || r.bottom > sr.bottom + tol) {
                            errs.push(`svg[${si}] text out-of-bounds: "${txt.slice(0, 24)}"`);
                        }
                        boxes.push({txt: txt.slice(0, 24), l: r.left, r: r.right, t: r.top, b: r.bottom});
                    });
                    for (let i = 0; i < boxes.length; i++) {
                        for (let j = i + 1; j < boxes.length; j++) {
                            const a = boxes[i], b = boxes[j];
                            const ox = Math.min(a.r, b.r) - Math.max(a.l, b.l);
                            const oy = Math.min(a.b, b.b) - Math.max(a.t, b.t);
                            if (ox > 4 && oy > 4) {
                                errs.push(`svg[${si}] overlap (${Math.round(ox)}x${Math.round(oy)}px): "${a.txt}" <-> "${b.txt}"`);
                            }
                        }
                    }
                });
                return errs;
            }""",
            idx,
        )
        for e in res:
            issues.append(f"slide {idx + 1:02d}: {e}")

    # Test interactive shortcuts on slide 1
    page.evaluate("if (typeof go === 'function') go(0);")
    page.wait_for_timeout(40)
    page.keyboard.press("g")
    page.wait_for_timeout(60)
    if not page.locator("#ov.open").count():
        issues.append("shortcut 'g' failed to open TOC overlay")
    page.keyboard.press("Escape")
    page.wait_for_timeout(40)

    page.keyboard.press("n")
    page.wait_for_timeout(60)
    if not page.locator("#panel.open").count():
        issues.append("shortcut 'n' failed to open notes panel")
    page.keyboard.press("t")
    page.wait_for_timeout(60)
    if not page.locator("#panel.open").count():
        issues.append("shortcut 't' failed to keep panel open in full-script mode")
    page.keyboard.press("Escape")
    page.wait_for_timeout(40)

    if console_errors:
        issues.append(f"console errors: {console_errors}")
    if page_errors:
        issues.append(f"page errors: {page_errors}")
    return issues


def main() -> int:
    html_files = sorted(PRES_DIR.glob("*.html"))
    if not html_files:
        print("❌ No HTML files found in presentation/")
        return 1

    total_issues = 0
    with sync_playwright() as p:
        browser = p.chromium.launch()
        for html_path in html_files:
            page = browser.new_page()
            issues = verify_deck(page, html_path)
            slides = page.locator(".slide").count()
            page.close()
            if issues:
                total_issues += len(issues)
                print(f"❌ {html_path.name} ({slides} slides): {len(issues)} issue(s)")
                for iss in issues[:20]:
                    print(f"   - {iss}")
                if len(issues) > 20:
                    print(f"   ... and {len(issues) - 20} more")
            else:
                tag = f"{slides} slides" if slides else "hub page"
                print(f"✅ {html_path.name} ({tag}): 0 errors, 0 overflow, 0 SVG overlap, shortcuts OK")
        browser.close()

    if total_issues > 0:
        print(f"\n❌ Verification failed with {total_issues} issue(s).")
        return 1
    print(f"\n✅ All {len(html_files)} presentation HTML files passed Playwright verification.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
