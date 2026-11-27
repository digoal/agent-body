#!/usr/bin/env python3
"""Agent Body 微内核与 contrib/ 架构防腐门禁 (第 10 章 5 条规则)。

检查项:
  1. 单向依赖红线: kernel/*/Cargo.toml 与 kernel/**/src/**/*.rs 0 指向 contrib/、distributions/ 或根包 agent-body 的依赖/引用
  2. 内核业务词污染扫描: kernel/**/src/**/*.rs (#[cfg(test)] 之前生产代码) 0 周边业务词污染
  3. contrib/ 五件套与私有表前缀隔离: 若 contrib/ 存在，检查标准五件套及 SQL 表名 ext_<name>__ 前缀
  4. distributions/ 隔离检查: 若 distributions/ 存在，检查 kernel-minimal 0 contrib 依赖
  5. 支持 --check 退出码校验 (违规返回非零退出码)
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

BANNED_KERNEL_BUSINESS_WORDS = (
    "duckdb",
    "ollama",
    "systemd",
    "crontab",
    "jvm",
    "tomcat",
    "nginx",
    "cmdb",
    "siem",
    "kingbase",
    "dameng",
    "placement",
)


def prod_slice(path: Path) -> str:
    text = path.read_text(encoding="utf-8")
    masked = list(text)
    for m in re.finditer(r'(?<![A-Za-z0-9_])r(#*)"[\s\S]*?"\1', text):
        for idx in range(m.start(), m.end()):
            if masked[idx] != "\n":
                masked[idx] = " "
    masked_text = "".join(masked)
    m = re.search(r"(?m)^#\[cfg\(test\)\]\s*\n\s*(?:pub\s+)?mod\s+[a-zA-Z0-9_]+\b", masked_text)
    return text[: m.start()] if m else text


def check_kernel_unidirectional_deps(errors: list[str]) -> int:
    kernel_dir = ROOT / "kernel"
    if not kernel_dir.exists():
        errors.append("缺少 kernel/ 目录")
        return 0

    crate_count = 0
    forbidden_toml_re = re.compile(
        r"""(?:contrib/|distributions/|["']agent-body["'](?:\s*=|\s*,\s*path\s*=))"""
    )
    forbidden_rs_re = re.compile(
        r"""(?:use\s+(?:agent_body(?!_(?:sdk|protocol|kernel)\b)|contrib|distributions)\b|contrib/|distributions/)"""
    )

    for toml_path in sorted(kernel_dir.glob("*/Cargo.toml")):
        crate_count += 1
        rel = toml_path.relative_to(ROOT).as_posix()
        txt = toml_path.read_text(encoding="utf-8")
        for lineno, line in enumerate(txt.splitlines(), 1):
            stripped = line.strip()
            if stripped.startswith("#"):
                continue
            if forbidden_toml_re.search(line):
                errors.append(f"[Rule 1] {rel}:{lineno} 检测到非法跨层依赖: {stripped}")

    for rs_path in sorted(kernel_dir.glob("*/src/**/*.rs")):
        rel = rs_path.relative_to(ROOT).as_posix()
        prod = prod_slice(rs_path)
        for lineno, line in enumerate(prod.splitlines(), 1):
            stripped = line.strip()
            if stripped.startswith("//"):
                continue
            if forbidden_rs_re.search(line):
                errors.append(f"[Rule 1] {rel}:{lineno} 检测到非法跨层引用: {stripped}")

    return crate_count


def check_kernel_business_pollution(errors: list[str]) -> int:
    kernel_dir = ROOT / "kernel"
    if not kernel_dir.exists():
        return 0

    word_re = re.compile(
        r"\b(?:" + "|".join(re.escape(w) for w in BANNED_KERNEL_BUSINESS_WORDS) + r")\b",
        re.IGNORECASE,
    )
    scanned = 0
    for rs_path in sorted(kernel_dir.glob("*/src/**/*.rs")):
        scanned += 1
        rel = rs_path.relative_to(ROOT).as_posix()
        prod = prod_slice(rs_path)
        for lineno, line in enumerate(prod.splitlines(), 1):
            m = word_re.search(line)
            if m:
                errors.append(
                    f"[Rule 2] {rel}:{lineno} 生产代码禁止出现周边业务词 '{m.group(0)}': {line.strip()}"
                )
    return scanned


def check_contrib_packages(errors: list[str]) -> int:
    contrib_dir = ROOT / "contrib"
    if not contrib_dir.exists():
        return 0

    table_target_re = re.compile(
        r"(?:CREATE\s+TABLE(?:\s+IF\s+NOT\s+EXISTS)?|INSERT\s+INTO|UPDATE|DELETE\s+FROM|ALTER\s+TABLE|DROP\s+TABLE(?:\s+IF\s+EXISTS)?)\s+([A-Za-z_][A-Za-z0-9_]*)",
        re.IGNORECASE,
    )

    pkg_count = 0
    for pkg in sorted(p for p in contrib_dir.iterdir() if p.is_dir() and not p.name.startswith(".")):
        pkg_count += 1
        ext_name = pkg.name
        expected_prefix = f"ext_{ext_name}__"

        cargo_toml = pkg / "Cargo.toml"
        control_toml = pkg / f"{ext_name}.control.toml"
        sql_dir = pkg / "sql"
        lib_rs = pkg / "src" / "lib.rs"
        regress_rs = pkg / "tests" / "regress.rs"

        missing = [
            name
            for name, path in (
                ("Cargo.toml", cargo_toml),
                (f"{ext_name}.control.toml", control_toml),
                ("sql/", sql_dir),
                ("src/lib.rs", lib_rs),
                ("tests/regress.rs", regress_rs),
            )
            if not path.exists()
        ]
        if missing:
            errors.append(
                f"[Rule 3] contrib/{ext_name}/ 缺少标准五件套文件: {', '.join(missing)}"
            )

        if lib_rs.exists() and "body_module_magic!" not in lib_rs.read_text(encoding="utf-8"):
            errors.append(f"[Rule 3] contrib/{ext_name}/src/lib.rs 未调用 body_module_magic!()")

        if sql_dir.exists():
            for sql_file in sorted(sql_dir.glob("*.sql")):
                sql_txt = sql_file.read_text(encoding="utf-8")
                for m in table_target_re.finditer(sql_txt):
                    tbl = m.group(1)
                    if not tbl.startswith(expected_prefix):
                        errors.append(
                            f"[Rule 3] {sql_file.relative_to(ROOT).as_posix()} 表名 '{tbl}' 违反前缀隔离 '{expected_prefix}'"
                        )

    return pkg_count


def check_distributions(errors: list[str]) -> int:
    dist_dir = ROOT / "distributions"
    if not dist_dir.exists():
        return 0

    dist_count = sum(1 for p in dist_dir.iterdir() if p.is_dir() and not p.name.startswith("."))
    minimal_dir = dist_dir / "kernel-minimal"
    if minimal_dir.exists():
        for fname in ("Cargo.toml", "distribution.toml"):
            fpath = minimal_dir / fname
            if fpath.exists():
                txt = fpath.read_text(encoding="utf-8")
                if "contrib/" in txt or re.search(r"contrib\s*=", txt):
                    errors.append(
                        f"[Rule 4] distributions/kernel-minimal/{fname} 不允许包含任何 contrib 依赖"
                    )
                if fname == "distribution.toml" and re.search(
                    r'enable\s*=\s*\[\s*"[^"]+', txt
                ):
                    errors.append(
                        f"[Rule 4] distributions/kernel-minimal/{fname} 的 contrib enable 列表必须为空"
                    )
    return dist_count


def run_checks() -> tuple[list[str], list[str]]:
    errors: list[str] = []
    notes: list[str] = []

    crates = check_kernel_unidirectional_deps(errors)
    rs_scanned = check_kernel_business_pollution(errors)
    contrib_pkgs = check_contrib_packages(errors)
    dists = check_distributions(errors)

    notes.append(f"kernel crates 检查: {crates} 个 (单向依赖隔离)")
    notes.append(f"kernel 生产文件业务词扫描: {rs_scanned} 个")
    notes.append(f"contrib/ 扩展包检查: {contrib_pkgs} 个")
    notes.append(f"distributions/ 发行版检查: {dists} 个")
    return errors, notes


def main() -> int:
    parser = argparse.ArgumentParser(description="Agent Body 架构防腐边界门禁")
    parser.add_argument("--check", action="store_true", help="执行门禁校验，违规返回非零退出码")
    args = parser.parse_args()

    errors, notes = run_checks()
    for note in notes:
        print(f"🔍 [ArchBoundary] {note}")

    if errors:
        print("❌ 架构防腐门禁未通过:")
        for err in errors:
            print(f"  - {err}")
        return 1 if args.check else 0

    print("✅ 架构防腐边界门禁全部通过 (kernel 纯净 / 0 跨层依赖 / 0 业务词污染)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
