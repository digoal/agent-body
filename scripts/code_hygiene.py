#!/usr/bin/env python3
"""Agent Body 确定性代码质量与卫生门禁 (make lint-hygiene, 对应规约 #13 & #28)。

检查项:
  1. 位置无关性红线: 生产代码 (#[cfg(test)] 之前) 严禁硬编码 env!("CARGO_MANIFEST_DIR")，且非 Benchmark 生产代码严禁新增 /root/new/ 与 user:digoal 硬编码
  2. 活跃部署与配置红线: Dockerfile / docker-compose.yml / config.example.toml 严禁残留旧数据目录路径
  3. 关键自治路径错误可观测性: 统计生产代码 let _ = 静默吞错数量 (<= 15) 且核心自治/SQLite/本体路径必须为 0
  4. 模块化复杂度红线: 严格禁止任何生产 Rust 文件超过 1000 行 (>1000L 清零)
  5. WaitEventId 全埋点红线: 所有 WaitEventId 枚举变体必须在生产路径中具备 WaitEventGuard::enter / WaitEventRegistry::record_us 埋点
  6. 全仓统一命名红线: 源码、测试、脚本、Skills、文档与演说文件严禁残留旧项目命名（统一为 agent-body / Agent Body / Agent 本体）
"""

from __future__ import annotations

import argparse
import glob
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# 随着 Phase 49 -> Phase 56 -> Phase 67 治理逐步收紧的静默吞错预算上限（142 -> 130 -> 50 -> 15）
MAX_SILENT_LET_UNDERSCORE = 15
MAX_PROD_LOC = 1000
CORE_ZERO_SILENT_PREFIXES = (
    "src/daemon/",
    "src/session/",
    "src/watcher/",
    "src/coordination/",
    "src/storage/sqlite_provider/",
    "src/hooks/auto_sync.rs",
    "src/links/audit.rs",
    "src/body/",
    "src/scanner/",
    "src/hooks/",
    "src/web/",
    "src/links/",
    "src/fact/",
    "src/audit/",
    "src/config/",
)


def prod_slice(path: Path) -> str:
    text = path.read_text(encoding="utf-8")
    # 先屏蔽 Rust 原始字符串 r#*"..."#* 内的假 #[cfg(test)] 文本（要求 r 前非标识符字符），避免误截断或吞掉后续代码
    masked = list(text)
    for m in re.finditer(r'(?<![A-Za-z0-9_])r(#*)"[\s\S]*?"\1', text):
        for idx in range(m.start(), m.end()):
            if masked[idx] != "\n":
                masked[idx] = " "
    masked_text = "".join(masked)
    m = re.search(r"(?m)^#\[cfg\(test\)\]\s*\n\s*(?:pub\s+)?mod\s+[a-zA-Z0-9_]+\b", masked_text)
    return text[: m.start()] if m else text


def run_checks() -> tuple[list[str], list[str]]:
    errors: list[str] = []
    notes: list[str] = []

    rs_files = sorted(Path(p) for p in glob.glob(str(ROOT / "src/**/*.rs"), recursive=True))

    # 1. 生产代码禁止 env!("CARGO_MANIFEST_DIR") 与非基准硬编码 /root/new/ & user:digoal
    manifest_dir_hits: list[str] = []
    hardcoded_host_hits: list[str] = []
    core_silent_hits: list[str] = []
    silent_let_total = 0
    oversized_files: list[tuple[int, str]] = []
    all_prod_texts: list[tuple[str, str]] = []

    for p in rs_files:
        rel = p.relative_to(ROOT).as_posix()
        prod = prod_slice(p)
        all_prod_texts.append((rel, prod))
        is_benchmark = rel.startswith("src/benchmark/")
        for lineno, line in enumerate(prod.splitlines(), 1):
            stripped = line.strip()
            if "CARGO_MANIFEST_DIR" in line:
                manifest_dir_hits.append(f"{rel}:{lineno}: {stripped}")
            if not is_benchmark and not stripped.startswith("//"):
                if "/root/new" in line or "user:digoal" in line:
                    hardcoded_host_hits.append(f"{rel}:{lineno}: {stripped}")
        file_silent = len(re.findall(r"\blet\s+_\s*=", prod))
        silent_let_total += file_silent
        if file_silent > 0 and rel.startswith(CORE_ZERO_SILENT_PREFIXES):
            core_silent_hits.append(f"{rel} ({file_silent}处)")
        loc = len(prod.splitlines())
        if loc > MAX_PROD_LOC:
            oversized_files.append((loc, rel))

    if manifest_dir_hits:
        errors.append(
            "生产代码禁止硬编码 env!(\"CARGO_MANIFEST_DIR\")（违反规约 #28 位置无关性）:\n    "
            + "\n    ".join(manifest_dir_hits)
        )
    if hardcoded_host_hits:
        errors.append(
            "非 Benchmark 生产代码禁止新增 `/root/new` 或 `user:digoal` 硬编码:\n    "
            + "\n    ".join(hardcoded_host_hits)
        )
    if core_silent_hits:
        errors.append(
            "核心自治与 SQLite 主路径禁止 `let _ =` 静默吞错: " + ", ".join(core_silent_hits)
        )

    # 2. 活跃部署与配置禁止残留旧数据目录路径
    legacy_dir = "/." + "st" + "ew" + "ard"
    for rel in ("Dockerfile", "docker-compose.yml", "config.example.toml"):
        p = ROOT / rel
        if p.exists() and legacy_dir in p.read_text(encoding="utf-8"):
            errors.append(f"{rel} 存在旧路径残留 '{legacy_dir}'")

    # 3. 静默吞错上限检查
    if silent_let_total > MAX_SILENT_LET_UNDERSCORE:
        errors.append(
            f"生产代码 `let _ =` 静默吞错数 {silent_let_total} 超过预算上限 {MAX_SILENT_LET_UNDERSCORE}"
        )
    notes.append(f"静默 `let _ =` 计数: {silent_let_total} / {MAX_SILENT_LET_UNDERSCORE}")

    # 4. 模块化复杂度红线 (>1000L 清零)
    oversized_files.sort(reverse=True)
    if oversized_files:
        top_str = ", ".join(f"{rel}({loc}L)" for loc, rel in oversized_files[:6])
        errors.append(f"存在 >{MAX_PROD_LOC}L 生产 God File ({len(oversized_files)} 个): {top_str}")
        notes.append(f">{MAX_PROD_LOC}L 生产文件 ({len(oversized_files)} 个): {top_str}")
    else:
        notes.append(f">{MAX_PROD_LOC}L 生产文件: 0 个")

    # 5. WaitEventId 全枚举埋点覆盖率检查
    we_file = ROOT / "src/body/perf_awr/wait_events.rs"
    if we_file.exists():
        we_src = we_file.read_text(encoding="utf-8")
        enum_m = re.search(r"pub enum WaitEventId\s*\{(.*?)\}", we_src, flags=re.S)
        if enum_m:
            variants = re.findall(r"^\s*([A-Z][A-Za-z0-9]+)\s*(?:=\s*\d+)?,", enum_m.group(1), flags=re.M)
            non_we_prod = "\n".join(prod for rel, prod in all_prod_texts if rel != "src/body/perf_awr/wait_events.rs")
            missing_we = [
                v for v in variants
                if not re.search(rf"WaitEvent(?:Guard::enter|Registry::record_us)\([^)]*WaitEventId::{v}\b", non_we_prod)
            ]
            if missing_we:
                errors.append(f"存在未埋点 WaitEventId 变体: {', '.join(missing_we)}")
            notes.append(f"WaitEventId 埋点覆盖: {len(variants) - len(missing_we)} / {len(variants)}")

    # 6. 全仓统一命名检查（零旧称、零缩写与零机械重复残留）
    legacy_re = re.compile(
        r"(?:"
        + "st" + "ew"
        + "|" + "but" + "ler"
        + "|" + "secre" + "tary"
        + "|" + "大" + "管" + "家"
        + "|" + "管" + "家"
        + "|" + "秘" + "书"
        + r"|Agent\s*本体\s*/\s*Agent\s*本体|Agent\s+Agent"
        + r")",
        re.IGNORECASE,
    )
    skip_dirs = {".git", "target", ".codegraph", "node_modules", "__pycache__"}
    legacy_hits: list[str] = []
    for sub in ("src", "tests", "scripts", "skills", "docs", "presentation"):
        base = ROOT / sub
        if not base.exists():
            continue
        for p in sorted(base.rglob("*")):
            if not p.is_file() or any(part in skip_dirs for part in p.parts):
                continue
            rel = p.relative_to(ROOT).as_posix()
            if ("st" + "ew") in p.name.lower():
                legacy_hits.append(f"{rel} (文件名)")
                continue
            try:
                txt = p.read_text(encoding="utf-8")
            except Exception:
                continue
            m = legacy_re.search(txt)
            if m:
                ln = txt[: m.start()].count("\n") + 1
                legacy_hits.append(f"{rel}:{ln} ({m.group(0)})")
    for p in sorted(ROOT.iterdir()):
        if p.is_file() and p.suffix in (".md", ".toml"):
            try:
                txt = p.read_text(encoding="utf-8")
            except Exception:
                continue
            m = legacy_re.search(txt)
            if m:
                ln = txt[: m.start()].count("\n") + 1
                legacy_hits.append(f"{p.name}:{ln} ({m.group(0)})")
    if legacy_hits:
        sample = ", ".join(legacy_hits[:8])
        errors.append(f"存在 {len(legacy_hits)} 处旧命名残留（应统一为 Agent 本体 / agent-body）: {sample}")
    notes.append(f"旧命名残留: {len(legacy_hits)} 处")

    return errors, notes


def main() -> int:
    parser = argparse.ArgumentParser(description="Agent Body 代码质量与卫生门禁")
    parser.add_argument("--check", action="store_true", help="执行门禁校验，违规返回非零退出码")
    args = parser.parse_args()

    errors, notes = run_checks()
    for n in notes:
        print(f"🔍 [Hygiene] {n}")

    if errors:
        print("❌ 代码质量卫生门禁未通过:")
        for e in errors:
            print(f"  - {e}")
        return 1 if args.check else 0

    print("✅ 代码质量与位置无关性卫生门禁通过 (0 CARGO_MANIFEST_DIR / 0 legacy deploy path)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
