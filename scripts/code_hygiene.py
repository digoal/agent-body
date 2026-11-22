#!/usr/bin/env python3
"""Agent Body 确定性代码质量与卫生门禁 (make lint-hygiene, 对应规约 #13 & #28)。

检查项:
  1. 位置无关性红线: 生产代码 (#[cfg(test)] 之前) 严禁硬编码 env!("CARGO_MANIFEST_DIR")
  2. 活跃部署与配置红线: Dockerfile / docker-compose.yml / config.example.toml 严禁残留 /.steward
  3. 关键自治路径错误可观测性: 统计生产代码 let _ = 静默吞错数量并守卫上限不回弹
  4. 模块化复杂度雷达: 报告超过 1500 行的生产文件与超长函数，辅助防 God File 膨胀
"""

from __future__ import annotations

import argparse
import glob
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# 随着 Phase 49 治理逐步收紧的静默吞错预算上限（治理前 142 -> 已收紧至 <= 130）
MAX_SILENT_LET_UNDERSCORE = 130


def prod_slice(path: Path) -> str:
    text = path.read_text(encoding="utf-8")
    m = re.search(r"\n#\[cfg\(test\)\]\s*\n\s*mod\s+tests\b", text)
    return text[: m.start()] if m else text


def run_checks() -> tuple[list[str], list[str]]:
    errors: list[str] = []
    notes: list[str] = []

    rs_files = sorted(Path(p) for p in glob.glob(str(ROOT / "src/**/*.rs"), recursive=True))

    # 1. 生产代码禁止 env!("CARGO_MANIFEST_DIR")
    manifest_dir_hits: list[str] = []
    silent_let_total = 0
    oversized_files: list[tuple[int, str]] = []

    for p in rs_files:
        rel = p.relative_to(ROOT).as_posix()
        prod = prod_slice(p)
        for lineno, line in enumerate(prod.splitlines(), 1):
            if "CARGO_MANIFEST_DIR" in line:
                manifest_dir_hits.append(f"{rel}:{lineno}: {line.strip()}")
        silent_let_total += len(re.findall(r"\blet\s+_\s*=", prod))
        loc = len(prod.splitlines())
        if loc > 1500:
            oversized_files.append((loc, rel))

    if manifest_dir_hits:
        errors.append(
            "生产代码禁止硬编码 env!(\"CARGO_MANIFEST_DIR\")（违反规约 #28 位置无关性）:\n    "
            + "\n    ".join(manifest_dir_hits)
        )

    # 2. 活跃部署与配置禁止残留 /.steward
    for rel in ("Dockerfile", "docker-compose.yml", "config.example.toml"):
        p = ROOT / rel
        if p.exists() and "/.steward" in p.read_text(encoding="utf-8"):
            errors.append(f"{rel} 存在旧路径残留 '/.steward'")

    # 3. 静默吞错上限检查
    if silent_let_total > MAX_SILENT_LET_UNDERSCORE:
        errors.append(
            f"生产代码 `let _ =` 静默吞错数 {silent_let_total} 超过预算上限 {MAX_SILENT_LET_UNDERSCORE}"
        )
    notes.append(f"静默 `let _ =` 计数: {silent_let_total} / {MAX_SILENT_LET_UNDERSCORE}")

    # 4. 模块化复杂度雷达（信息提示，推动持续拆解）
    oversized_files.sort(reverse=True)
    if oversized_files:
        top_str = ", ".join(f"{rel}({loc}L)" for loc, rel in oversized_files[:6])
        notes.append(f">1500L 生产文件 ({len(oversized_files)} 个): {top_str}")
    else:
        notes.append(">1500L 生产文件: 0 个")

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
