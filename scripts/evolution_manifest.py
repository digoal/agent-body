#!/usr/bin/env python3
"""Agent Body 确定性架构清单提取与文档防漂移检查器。

用途:
  1. 零重扫打印实时架构事实快照 (<100ms):
     python3 scripts/evolution_manifest.py --json
  2. 校验 EVOLUTION_GUIDE.md / README.md / SKILL.md 与源码真实计数零漂移:
     python3 scripts/evolution_manifest.py --check
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def extract_manifest() -> dict:
    lib_rs = (ROOT / "src/lib.rs").read_text(encoding="utf-8")
    modules = re.findall(r"^pub\s+mod\s+([a-zA-Z0-9_]+);", lib_rs, flags=re.MULTILINE)

    mcp_rs = (ROOT / "src/api/mcp_tools.rs").read_text(encoding="utf-8")
    list_tools_block = mcp_rs.split("pub fn call_tool")[0]
    mcp_tools = re.findall(r'"name"\s*:\s*"(body_[a-zA-Z0-9_]+)"', list_tools_block)

    spi_rs = (ROOT / "src/storage/spi.rs").read_text(encoding="utf-8")
    spi_methods = re.findall(r"^\s*fn\s+([a-zA-Z0-9_]+)\s*\(", spi_rs, flags=re.MULTILINE)

    schema_sql = (ROOT / "src/storage/schema.sql").read_text(encoding="utf-8")
    tables = re.findall(
        r"CREATE\s+(?:VIRTUAL\s+)?TABLE\s+IF\s+NOT\s+EXISTS\s+([a-zA-Z0-9_]+)",
        schema_sql,
        flags=re.IGNORECASE,
    )

    workloads_rs = (ROOT / "src/benchmark/workloads.rs").read_text(encoding="utf-8")
    workloads = sorted(
        set(re.findall(r'id:\s*"(W\d+)"', workloads_rs)),
        key=lambda x: int(x[1:]),
    )

    micro_rs = (ROOT / "src/benchmark/micro.rs").read_text(encoding="utf-8")
    micro = sorted(
        set(re.findall(r'id:\s*"(M\d+)"', micro_rs)),
        key=lambda x: int(x[1:]),
    )

    dq_rs = (ROOT / "src/benchmark/dq.rs").read_text(encoding="utf-8")
    dq = sorted(
        set(re.findall(r'id:\s*"(DQ\d+)"', dq_rs)),
        key=lambda x: int(x[2:]),
    )

    comp_path = ROOT / "src/benchmark/competitors.rs"
    competitors: list[str] = []
    if comp_path.exists():
        comp_rs = comp_path.read_text(encoding="utf-8")
        competitors = sorted(
            set(re.findall(r'id:\s*"(COMP-C\d+)"', comp_rs)),
            key=lambda x: int(x.split("C")[-1]),
        )

    prov_rs = (ROOT / "src/benchmark/provenance.rs").read_text(encoding="utf-8")
    pr_rules = sorted(
        set(re.findall(r'"(PR-\d+)"', prov_rs)),
        key=lambda x: int(x.split("-")[1]),
    )

    bench_mod = (ROOT / "src/benchmark/mod.rs").read_text(encoding="utf-8")
    spec_match = re.search(r'BENCHMARK_SPEC_VERSION\s*:\s*&str\s*=\s*"([^"]+)"', bench_mod)
    bench_spec_version = spec_match.group(1) if spec_match else "unknown"

    main_rs = (ROOT / "src/main.rs").read_text(encoding="utf-8")
    cmd_block = main_rs.split("enum Commands {")[1].split("\n}")[0]
    cli_commands = re.findall(r"^\s{4}([A-Z][A-Za-z0-9_]*)\s*(?:\{|,)", cmd_block, flags=re.MULTILINE)

    return {
        "counts": {
            "modules": len(modules),
            "mcp_tools": len(mcp_tools),
            "cli_commands": len(cli_commands),
            "spi_methods": len(spi_methods),
            "tables": len(tables),
            "workloads": len(workloads),
            "micro": len(micro),
            "dq": len(dq),
            "competitors": len(competitors),
            "pr_rules": len(pr_rules),
        },
        "bench_spec_version": bench_spec_version,
        "modules": modules,
        "mcp_tools": mcp_tools,
        "cli_commands": cli_commands,
        "spi_methods": spi_methods,
        "tables": tables,
        "workloads": workloads,
        "micro": micro,
        "dq": dq,
        "competitors": competitors,
        "pr_rules": pr_rules,
    }


def check_drift(manifest: dict) -> list[str]:
    errors: list[str] = []
    counts = manifest["counts"]

    guide_path = ROOT / "EVOLUTION_GUIDE.md"
    if not guide_path.exists():
        errors.append("缺少根目录 EVOLUTION_GUIDE.md")
    else:
        guide_text = guide_path.read_text(encoding="utf-8")
        tag_match = re.search(r"<!--\s*MANIFEST:([^>]+)-->", guide_text)
        if not tag_match:
            errors.append("EVOLUTION_GUIDE.md 缺少 <!-- MANIFEST: ... --> 机器校验锚点")
        else:
            pairs = dict(re.findall(r"([a-z_]+)=([0-9]+)", tag_match.group(1)))
            for key in ("modules", "mcp_tools", "spi_methods", "tables", "workloads", "micro", "dq", "pr_rules"):
                expected = counts[key]
                actual = int(pairs.get(key, -1))
                if actual != expected:
                    errors.append(
                        f"EVOLUTION_GUIDE.md 漂移: {key} 声明为 {actual}，源码实测为 {expected}"
                    )

    # 校验核心活跃文档不再残留过时计数（MCP 工具数 / SPI 方法数 / 模块数 / CLI 命令数）
    mcp_cnt = counts["mcp_tools"]
    spi_cnt = counts["spi_methods"]
    mod_cnt = counts["modules"]
    cli_cnt = counts["cli_commands"]
    active_docs = [
        ROOT / "README.md",
        ROOT / "EVOLUTION_GUIDE.md",
        ROOT / "skills/agent-body-developer/SKILL.md",
        ROOT / "skills/agent-body-doc-writer/SKILL.md",
        ROOT / "skills/agent-body-guide/SKILL.md",
        ROOT / "docs/arch/01-for-architects.md",
        ROOT / "docs/arch/02-for-developers.md",
        ROOT / "docs/arch/03-for-users.md",
        ROOT / "docs/src_doc/development.md",
        ROOT / "docs/manual/README.md",
        ROOT / "docs/manual/03-cli-reference.md",
        ROOT / "docs/manual/04-mcp-integration.md",
        ROOT / "docs/manual/08-web-dashboard.md",
        ROOT / "docs/manual/10-benchmark-and-roi.md",
        ROOT / "docs/design_doc/06-api-spec.md",
    ]
    stale_checks = [
        (
            re.compile(r"(?<!第\s)(?<!第)\b(39|40|41|42)\s*(?:个)?\s*(?:原生)?\s*(?:MCP\s*)?(?:工具|Tools?)"),
            f"MCP 工具数 (源码实测 {mcp_cnt})",
        ),
        (
            re.compile(r"\b(55|61|65|74|78)\s*(?:个)?\s*(?:核心)?(?:语义)?(?:统一)?\s*(?:SPI|方法|接口)"),
            f"SPI 方法数 (源码实测 {spi_cnt})",
        ),
        (
            re.compile(r"\b(23|24|25|26)\s*(?:个|大)?\s*(?:Rust\s*)?(?:顶层)?(?:核心)?(?:子)?模块"),
            f"顶层模块数 (源码实测 {mod_cnt})",
        ),
        (
            re.compile(r"\b(27|28|29)\s*(?:个)?\s*(?:CLI\s*)?(?:顶层)?(?:子)?(?:命令|Commands)"),
            f"CLI 顶级命令数 (源码实测 {cli_cnt})",
        ),
    ]
    for doc in active_docs:
        if not doc.exists():
            continue
        text = doc.read_text(encoding="utf-8")
        for pat, desc in stale_checks:
            for m in pat.finditer(text):
                # 允许历史变更或早期基线描述
                ctx = text[max(0, m.start() - 24) : min(len(text), m.end() + 24)]
                if any(tok in ctx for tok in ("→", "Phase ", "早期", "历史", "v0.3", "v2.0", "第 ")):
                    continue
                errors.append(
                    f"{doc.relative_to(ROOT)} 存在过期{desc}: '{m.group(0)}'"
                )

    for doc_rel in (
        "README.md",
        "docs/arch/01-for-architects.md",
        "docs/arch/03-for-users.md",
        "skills/agent-body-guide/SKILL.md",
    ):
        p = ROOT / doc_rel
        if p.exists() and "Agent 本体" not in p.read_text(encoding="utf-8"):
            errors.append(f"{doc_rel} 缺少 Tier-1 定位表达「Agent 本体」")

    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description="Agent Body 架构清单与防漂移检查")
    parser.add_argument("--json", action="store_true", help="输出完整 JSON 架构快照")
    parser.add_argument("--check", action="store_true", help="执行文档数字防漂移检查")
    args = parser.parse_args()

    manifest = extract_manifest()
    if args.json:
        print(json.dumps(manifest, ensure_ascii=False, indent=2))
        return 0

    c = manifest["counts"]
    print(
        f"📦 Agent Body 架构快照: {c['modules']} 模块 | {c['mcp_tools']} MCP工具 | "
        f"{c['spi_methods']} SPI方法 | {c['tables']} SQLite表 | "
        f"Bench v{manifest['bench_spec_version']} (W:{c['workloads']}, M:{c['micro']}, DQ:{c['dq']}, COMP:{c['competitors']}, PR:{c['pr_rules']})"
    )

    if args.check:
        errors = check_drift(manifest)
        if errors:
            print("❌ 检测到架构文档漂移:")
            for err in errors:
                print(f"  - {err}")
            return 1
        print("✅ 0 drift: EVOLUTION_GUIDE.md 与核心活跃文档与源码事实完全一致")
    return 0


if __name__ == "__main__":
    sys.exit(main())
