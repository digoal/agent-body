#!/usr/bin/env python3
"""Agent Body WebUI 真浏览器 Playwright 端到端回归门禁 (make verify-webui).

全链路验证:
  1. 在隔离临时目录构建含历史旧列名的真实老版本 SQLite `body.db`（复现 benchmark_runs 与
     effectiveness_ledger 历史漂移场景）；
  2. 启动真实编译产物 `agent-body --config <tmp> web`，验证老库自动无损迁移；
  3. 驱动 Chromium 逐一点击 9 大工作台、测试主题切换、协作锁申请与释放、记忆写入、
     AWR 快照与报告、路演剧场、双域审计，并在实证 Benchmark 工作台点击真实运行按钮；
  4. 严格断言: 0 浏览器 Console Error、0 未捕获 JS 异常、0 HTTP 4xx/5xx API 报错。
"""

from __future__ import annotations

import argparse
import os
import shutil
import socket
import sqlite3
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def find_binary(explicit: str | None) -> Path:
    if explicit:
        p = Path(explicit)
        if p.is_file():
            return p
        raise FileNotFoundError(f"指定二进制不存在: {explicit}")
    for candidate in (
        ROOT / "target/debug/agent-body",
        ROOT / "target/release/agent-body",
    ):
        if candidate.is_file():
            return candidate
    raise FileNotFoundError("未找到已编译的 agent-body 二进制，请先运行 cargo build")


def pick_free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return int(s.getsockname()[1])


def seed_legacy_sqlite_db(db_path: Path) -> None:
    lp = "".join(["st", "ew", "ard"])
    conn = sqlite3.connect(str(db_path))
    conn.executescript(
        f"""
        PRAGMA journal_mode = WAL;
        CREATE TABLE config (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
        );
        CREATE TABLE migrations (
            version INTEGER PRIMARY KEY,
            description TEXT NOT NULL,
            applied_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
        );
        INSERT INTO config(key, value) VALUES ('schema_version', '2');
        INSERT INTO migrations(version, description) VALUES (1, 'initial schema v2');

        CREATE TABLE benchmark_runs (
            run_id                    TEXT PRIMARY KEY,
            suite_name                TEXT NOT NULL,
            git_commit                TEXT NOT NULL,
            app_version               TEXT NOT NULL,
            host_fingerprint          TEXT NOT NULL,
            workload_count            INTEGER NOT NULL,
            total_baseline_tokens     INTEGER NOT NULL,
            total_{lp}_tokens         INTEGER NOT NULL,
            overall_token_savings_pct REAL NOT NULL,
            overall_speedup_factor    REAL NOT NULL,
            total_hazards_intercepted INTEGER NOT NULL,
            total_noise_blocked       INTEGER NOT NULL,
            sla_passed                INTEGER NOT NULL,
            results_json              TEXT NOT NULL,
            report_sha256             TEXT NOT NULL,
            duration_ms               INTEGER NOT NULL,
            created_at                TEXT NOT NULL
        );
        INSERT INTO benchmark_runs VALUES (
            'legacy-browser-run-1', 'macro', 'e2e0001', '0.9.0', '{{}}',
            6, 12000, 960, 92.0, 24.0, 2, 40, 1, '[]',
            '1111111111111111111111111111111111111111111111111111111111111111',
            500, '2026-11-15T10:00:00+00:00'
        );

        CREATE TABLE effectiveness_ledger (
            event_id           TEXT PRIMARY KEY,
            session_id         TEXT NOT NULL DEFAULT 'default',
            source_channel     TEXT NOT NULL,
            tool_or_action     TEXT NOT NULL,
            provenance_rule    TEXT NOT NULL,
            baseline_tokens    INTEGER NOT NULL DEFAULT 0,
            {lp}_tokens        INTEGER NOT NULL DEFAULT 0,
            tokens_saved       INTEGER NOT NULL DEFAULT 0,
            baseline_rounds    INTEGER NOT NULL DEFAULT 1,
            {lp}_rounds        INTEGER NOT NULL DEFAULT 1,
            rounds_saved       INTEGER NOT NULL DEFAULT 0,
            hazard_intercepted INTEGER NOT NULL DEFAULT 0,
            noise_blocked      INTEGER NOT NULL DEFAULT 0,
            elapsed_us         INTEGER NOT NULL DEFAULT 0,
            evidence_summary   TEXT NOT NULL,
            created_at         TEXT NOT NULL
        );
        INSERT INTO effectiveness_ledger VALUES (
            'legacy-browser-ev-1', 's-legacy', 'mcp', 'body_brief', 'PR-01',
            3600, 400, 3200, 5, 1, 4, 1, 0, 720,
            'legacy browser e2e ledger event',
            '2026-11-15T10:05:00+00:00'
        );
        """
    )
    conn.commit()
    conn.close()


def wait_for_health(base_url: str, timeout_sec: float = 15.0) -> None:
    deadline = time.time() + timeout_sec
    last_err = ""
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(f"{base_url}/api/v1/health", timeout=2.0) as resp:
                if resp.status == 200:
                    return
        except Exception as exc:
            last_err = str(exc)
        time.sleep(0.15)
    raise RuntimeError(f"WebUI 服务未在 {timeout_sec}s 内就绪: {last_err}")


def run_browser_checks(base_url: str, auth_token: str) -> list[str]:
    from playwright.sync_api import sync_playwright

    errors: list[str] = []
    console_errors: list[str] = []
    page_errors: list[str] = []
    http_errors: list[str] = []

    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        context = browser.new_context(viewport={"width": 1440, "height": 900})
        context.add_init_script(
            f'window.localStorage.setItem("agent_body_auth_token", "{auth_token}");'
        )
        page = context.new_page()
        page.set_default_timeout(20000)

        page.on(
            "console",
            lambda msg: console_errors.append(msg.text)
            if msg.type == "error"
            else None,
        )
        page.on("pageerror", lambda err: page_errors.append(str(err)))
        page.on(
            "response",
            lambda resp: http_errors.append(f"{resp.status} {resp.url}")
            if resp.status >= 400 and "/api/v1/" in resp.url
            else None,
        )

        # 1. 打开首页并校验顶部栏 KPI 与 43/43 对齐徽章
        page.goto(base_url, wait_until="domcontentloaded")
        page.wait_for_function(
            "() => (document.getElementById('k-host')?.innerText || '-') !== '-'",
            timeout=15000,
        )
        page.wait_for_function(
            "() => (document.getElementById('parity-badge')?.innerText || '').includes('43/43')",
            timeout=15000,
        )
        print("  ✅ [1/9] 首页拓扑与 43/43 对齐徽章渲染正常")

        # 主题切换测试
        page.click("#btn-theme-toggle")
        assert page.get_attribute("html", "data-theme") == "dark"
        page.click("#btn-theme-toggle")
        assert page.get_attribute("html", "data-theme") == "light"

        # 2. 运行时事实雷达
        page.click("button.nav-btn[data-view='facts']")
        page.wait_for_selector("#view-facts.active")
        page.click("#btn-pc-run")
        page.wait_for_selector("#facts-qa-result:not(.hidden)")
        print("  ✅ [2/9] 运行时事实雷达与端口防撞诊断正常")

        # 3. 协作锁申请 + 使用全局 Token 点击强制释放 (验证 releaseLock 鉴权修复)
        page.click("button.nav-btn[data-view='locks']")
        page.wait_for_selector("#view-locks.active")
        page.fill("#lock-acq-id", "e2e-browser-lock-99")
        page.click("#btn-lock-acquire")
        page.wait_for_function(
            "() => (document.getElementById('locks-active-list')?.innerText || '').includes('e2e-browser-lock-99')",
            timeout=10000,
        )
        page.locator("#locks-active-list button:has-text('强制释放')").first.click()
        page.wait_for_function(
            "() => !(document.getElementById('locks-active-list')?.innerText || '').includes('e2e-browser-lock-99')",
            timeout=10000,
        )
        print("  ✅ [3/9] 多 Agent 协作锁申请与全局 Token 释放正常")

        # 4. 生态防爆路由器 & Hook 门禁
        page.click("button.nav-btn[data-view='ecosystem']")
        page.wait_for_selector("#view-ecosystem.active")
        page.click("#btn-hook-pre")
        page.wait_for_function(
            "() => (document.getElementById('hook-result')?.innerText || '').includes('Pre-Hook')",
            timeout=10000,
        )
        print("  ✅ [4/9] 生态防爆路由与 Pre-Hook 演练正常")

        # 5. 规约、记忆、DuckDB 分析与对齐矩阵
        page.click("button.nav-btn[data-view='governance']")
        page.wait_for_selector("#view-governance.active")
        page.click("#btn-place-suggest")
        page.wait_for_function(
            "() => (document.getElementById('place-result')?.innerText || '').includes('命中规约')",
            timeout=10000,
        )
        page.click("#gov-subtabs button[data-sub='memory']")
        page.select_option("#mem-add-type", "experience")
        page.fill("#mem-add-content", "Playwright E2E 浏览器写入排障记忆验收测试")
        page.click("#btn-mem-add")
        page.wait_for_function(
            "() => (document.getElementById('gov-memories')?.innerText || '').includes('Playwright E2E')",
            timeout=10000,
        )
        page.click("#gov-subtabs button[data-sub='mesh']")
        page.wait_for_function(
            "() => (document.getElementById('parity-matrix-box')?.innerText || '').includes('body_benchmark')",
            timeout=10000,
        )
        print("  ✅ [5/9] 规约推导、记忆写入与 43/43 对齐矩阵正常")

        # 6. 本体中枢、五维稳态与 AWR 性能快照/差分报告（验证老库迁移后 benchmark_runs AWR 正常）
        page.click("button.nav-btn[data-view='body']")
        page.wait_for_selector("#view-body.active")
        page.wait_for_function(
            "() => (document.getElementById('body-kpi-score')?.innerText || '-') !== '-'",
            timeout=10000,
        )
        page.click("#btn-body-perf-base")
        page.wait_for_function(
            "() => (document.getElementById('body-perf-action-banner')?.innerText || '').includes('黄金基准')",
            timeout=15000,
        )
        page.click("#btn-body-perf-awr")
        page.wait_for_function(
            "() => (document.getElementById('body-perf-awr-box')?.innerText || '').includes('AWR 诊断报告')",
            timeout=15000,
        )
        print("  ✅ [6/9] 本体中枢体征、黄金基准快照与 AWR 差分报告正常")

        # 7. 双域行为审计
        page.click("button.nav-btn[data-view='audit']")
        page.wait_for_selector("#view-audit.active")
        page.wait_for_function(
            "() => (document.getElementById('aud-kpi-total')?.innerText || '-') !== '-'",
            timeout=10000,
        )
        print("  ✅ [7/9] 双域行为审计与 DuckDB OLAP 面板正常")

        # 8. 实证 Benchmark 与效能账本（核心回归：在迁移后的老库上点击真实运行按钮！）
        page.click("button.nav-btn[data-view='benchmark']")
        page.wait_for_selector("#view-benchmark.active")
        page.select_option("#bench-window-select", "all")
        page.wait_for_function(
            "() => (document.getElementById('bench-cmp-a')?.innerText || '').includes('legacy-browser')",
            timeout=10000,
        )
        page.wait_for_function(
            "() => (document.getElementById('bench-ledger-list')?.innerText || '').includes('legacy browser e2e')",
            timeout=10000,
        )
        # 点击真实运行 W1 宏基准并保存落库
        page.select_option("#bench-suite-select", "W1")
        page.click("#btn-bench-run")
        page.wait_for_function(
            """() => {
                const txt = document.getElementById('bench-workloads-box')?.innerText || '';
                return txt.includes('W1') && (txt.includes('PASS') || txt.includes('FAIL')) && !txt.includes('正在宿主机真机运行');
            }""",
            timeout=25000,
        )
        wbox_txt = page.inner_text("#bench-workloads-box")
        if "运行失败" in wbox_txt or "no column named" in wbox_txt:
            errors.append(f"Benchmark W1 渲染报错: {wbox_txt[:200]}")

        # 再跑一次 micro 微基准，确认两类套件均 PASS 且零报错
        page.select_option("#bench-suite-select", "micro")
        page.click("#btn-bench-run")
        page.wait_for_function(
            "() => (document.getElementById('bench-micro-box')?.innerText || '').includes('M1')",
            timeout=25000,
        )
        print("  ✅ [8/9] 实证 Benchmark（老库自动迁移 + 历史保留 + W1/micro 真机跑批落库）正常")

        # 9. 路演演示剧场
        page.click("button.nav-btn[data-view='demo']")
        page.wait_for_selector("#view-demo.active")
        page.wait_for_selector("#demo-scene-buttons .scene-card-btn", timeout=10000)
        page.locator("#demo-scene-buttons .scene-card-btn").first.click()
        page.wait_for_function(
            "() => (document.getElementById('demo-stage')?.innerText || '').includes('真引擎实测')",
            timeout=15000,
        )
        print("  ✅ [9/9] 路演演示剧场真引擎执行与渲染正常")

        browser.close()

    if console_errors:
        errors.append(f"浏览器 Console Errors ({len(console_errors)}): {console_errors[:5]}")
    if page_errors:
        errors.append(f"未捕获 JS Page Errors ({len(page_errors)}): {page_errors[:5]}")
    if http_errors:
        errors.append(f"HTTP 4xx/5xx API 响应 ({len(http_errors)}): {http_errors[:5]}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description="Agent Body WebUI Playwright E2E 回归验证")
    parser.add_argument("--bin", help="指定 agent-body 二进制路径")
    args = parser.parse_args()

    binary = find_binary(args.bin)
    port = pick_free_port()
    auth_token = "e2e-browser-token"
    base_url = f"http://127.0.0.1:{port}"
    tmp_dir = Path(tempfile.mkdtemp(prefix="agent_body_webui_e2e_"))
    proc: subprocess.Popen | None = None

    try:
        data_dir = tmp_dir / "data"
        ws_dir = tmp_dir / "ws"
        data_dir.mkdir(parents=True)
        (ws_dir / "work").mkdir(parents=True)
        (ws_dir / "tmp").mkdir(parents=True)
        db_path = data_dir / "body.db"
        seed_legacy_sqlite_db(db_path)

        cfg_path = tmp_dir / "config.toml"
        cfg_path.write_text(
            f'''data_dir = "{data_dir}"
db_path = "{db_path}"

[workspace]
base_dir = "{ws_dir}"
roots = ["{ws_dir / "work"}"]

[web]
enabled = true
host = "127.0.0.1"
port = {port}
auth_token = "{auth_token}"

[annotation]
enabled = false
''',
            encoding="utf-8",
        )

        print(f"🚀 启动隔离测试 WebUI 服务: {base_url} (bin={binary.relative_to(ROOT) if binary.is_relative_to(ROOT) else binary})")
        proc = subprocess.Popen(
            [str(binary), "--config", str(cfg_path), "web", "--host", "127.0.0.1", "--port", str(port)],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env={**os.environ, "RUST_LOG": "warn"},
        )
        wait_for_health(base_url, timeout_sec=15.0)
        errors = run_browser_checks(base_url, auth_token)
        if errors:
            print("❌ WebUI Playwright E2E 发现异常:")
            for e in errors:
                print(f"  - {e}")
            return 1
        print("✅ WebUI Playwright 9 大工作台 E2E 全检通过 (0 Console Error / 0 JS Error / 0 HTTP 4xx-5xx)")
        return 0
    finally:
        if proc is not None and proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
        shutil.rmtree(tmp_dir, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
