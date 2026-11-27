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

    mcp_schemas_path = ROOT / "src/api/mcp_schemas.rs"
    mcp_schemas_ops_path = ROOT / "src/api/mcp_schemas_ops.rs"
    if mcp_schemas_path.exists():
        schemas_txt = mcp_schemas_path.read_text(encoding="utf-8")
        if mcp_schemas_ops_path.exists():
            schemas_txt += "\n" + mcp_schemas_ops_path.read_text(encoding="utf-8")
        mcp_tools = re.findall(r'"name"\s*:\s*"(body_[a-zA-Z0-9_]+)"', schemas_txt)
    else:
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

    wl_dir = ROOT / "src/benchmark/workloads"
    if wl_dir.is_dir():
        workloads_rs = "\n".join(p.read_text(encoding="utf-8") for p in sorted(wl_dir.glob("*.rs")))
    else:
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
    comp_tracks_path = ROOT / "src/benchmark/competitor_tracks.rs"
    competitors: list[str] = []
    if comp_path.exists():
        comp_rs = comp_path.read_text(encoding="utf-8")
        if comp_tracks_path.exists():
            comp_rs += "\n" + comp_tracks_path.read_text(encoding="utf-8")
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

    cargo_toml = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    ver_match = re.search(r'^version\s*=\s*"([^"]+)"', cargo_toml, flags=re.MULTILINE)
    app_version = ver_match.group(1) if ver_match else "unknown"

    cli_args_path = ROOT / "src/cli_args.rs"
    cli_src_text = (
        cli_args_path.read_text(encoding="utf-8")
        if cli_args_path.exists()
        else (ROOT / "src/main.rs").read_text(encoding="utf-8")
    )
    cmd_block = cli_src_text.split("enum Commands {")[1].split("\n}")[0]
    cli_commands = re.findall(r"^\s{4}([A-Z][A-Za-z0-9_]*)\s*(?:\{|,)", cmd_block, flags=re.MULTILINE)

    kernel_crates = sorted(
        p.name for p in (ROOT / "kernel").iterdir() if p.is_dir() and (p / "Cargo.toml").exists()
    ) if (ROOT / "kernel").is_dir() else []
    contrib_exts = sorted(
        p.name for p in (ROOT / "contrib").iterdir() if p.is_dir() and (p / "Cargo.toml").exists()
    ) if (ROOT / "contrib").is_dir() else []
    distributions = sorted(
        p.name for p in (ROOT / "distributions").iterdir() if p.is_dir() and (p / "Cargo.toml").exists()
    ) if (ROOT / "distributions").is_dir() else []

    design_specs = sorted(
        (ROOT / "docs/design_doc").glob("[0-9][0-9]-*.md"),
        key=lambda sp: int(re.match(r"^(\d+)-", sp.name).group(1)),
    ) if (ROOT / "docs/design_doc").is_dir() else []
    latest_design_spec = design_specs[-1].name if design_specs else ""

    verified_test_count = 396
    guide_p = ROOT / "EVOLUTION_GUIDE.md"
    if guide_p.exists():
        gm = re.search(r"verified_tests=(\d+)", guide_p.read_text(encoding="utf-8"))
        if gm:
            verified_test_count = int(gm.group(1))

    return {
        "app_version": app_version,
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
            "kernel_crates": len(kernel_crates),
            "contrib_exts": len(contrib_exts),
            "distributions": len(distributions),
            "verified_tests": verified_test_count,
        },
        "bench_spec_version": bench_spec_version,
        "latest_design_spec": latest_design_spec,
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
        "kernel_crates": kernel_crates,
        "contrib_exts": contrib_exts,
        "distributions": distributions,
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
            for key in (
                "modules",
                "mcp_tools",
                "spi_methods",
                "tables",
                "workloads",
                "micro",
                "dq",
                "pr_rules",
                "kernel_crates",
                "contrib_exts",
                "distributions",
            ):
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
        ROOT / "docs/manual/06-advanced-features.md",
        ROOT / "docs/manual/08-web-dashboard.md",
        ROOT / "docs/manual/10-benchmark-and-roi.md",
        ROOT / "docs/design_doc/05-architecture.md",
        ROOT / "docs/design_doc/06-api-spec.md",
        ROOT / "INSTALL.md",
        ROOT / "docs/src_doc/development.md",
        ROOT / "docs/markdown/why-agent-needs-agent-body.md",
    ] + sorted((ROOT / "presentation").glob("*.html")) + sorted((ROOT / "scripts").glob("build_*.py"))
    stale_checks = [
        (
            re.compile(r"(?<!第\s)(?<!第)\b(39|40|41|42)\s*(?:个)?\s*(?:原生)?\s*(?:MCP\s*)?(?:工具|Tools?)"),
            f"MCP 工具数 (源码实测 {mcp_cnt})",
        ),
        (
            re.compile(r"\b(55|61|65|74|78|82|87)\s*(?:个)?\s*(?:核心)?(?:语义)?(?:统一)?\s*(?:SPI|方法|接口)"),
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

    # 校验 docs/README.md 与 EVOLUTION_GUIDE.md 均指向最新活跃路线图
    roadmap_files = sorted(
        (ROOT / "docs/src_doc").glob("roadmap-phase*.md"),
        key=lambda p: max([int(n) for n in re.findall(r"\d+", p.stem)] or [0]),
    )
    if roadmap_files and guide_path.exists():
        latest_roadmap = roadmap_files[-1].name
        guide_text = guide_path.read_text(encoding="utf-8")
        if latest_roadmap not in guide_text:
            errors.append(
                f"EVOLUTION_GUIDE.md 活跃路线图漂移: 最新路线图为 {latest_roadmap}，但未在 Tier-0 入口登记"
            )
        docs_readme = ROOT / "docs/README.md"
        if docs_readme.exists() and latest_roadmap not in docs_readme.read_text(encoding="utf-8"):
            errors.append(
                f"docs/README.md 活跃路线图漂移: 最新路线图为 {latest_roadmap}，但未更新指向"
            )

    # 校验核心活跃文档、演说与生成脚本同步最新 Benchmark 版本、W1~W19/DQ1~DQ10 计数与规格书登记
    bench_ver = manifest["bench_spec_version"]
    dq_cnt = counts["dq"]
    bench_docs = active_docs + [
        ROOT / "INSTALL.md",
        ROOT / "docs/design_doc/10-benchmark-spec.md",
    ]
    bench_stale = [
        (re.compile(r"Benchmark\s*v3\.8|Bench\s*v3\.8"), f"过时 Benchmark 版本 (当前 v{bench_ver})"),
        (re.compile(r"\bW1~W18\b"), f"过时负载范围 (当前 W1~W{counts['workloads']})"),
        (re.compile(r"\bDQ1~DQ9\b"), f"过时 DQ 范围 (当前 DQ1~DQ{dq_cnt})"),
    ]
    for bdoc in bench_docs:
        if not bdoc.exists():
            continue
        btxt = bdoc.read_text(encoding="utf-8")
        for pat, desc in bench_stale:
            for m in pat.finditer(btxt):
                ctx = btxt[max(0, m.start() - 40) : min(len(btxt), m.end() + 40)]
                if any(tok in ctx for tok in ("→", "Phase 5", "历史", "演进", "早期", "v3.8`")):
                    continue
                errors.append(f"{bdoc.relative_to(ROOT)} 存在{desc}: '{m.group(0)}'")

    readme_file = ROOT / "README.md"
    if readme_file.exists():
        rtxt = readme_file.read_text(encoding="utf-8")
        if "19-knowledge-network-and-graph-recall-spec.md" not in rtxt:
            errors.append("README.md 缺少最新规格书 19-knowledge-network-and-graph-recall-spec.md 登记")
        if "20-plugin-microkernel-architecture-spec.md" not in rtxt:
            errors.append("README.md 缺少最新规格书 20-plugin-microkernel-architecture-spec.md 登记")
        latest_spec = manifest.get("latest_design_spec", "")
        if latest_spec:
            for spec_target in (
                ROOT / "README.md",
                ROOT / "docs/README.md",
                ROOT / "skills/agent-body-developer/SKILL.md",
            ):
                if spec_target.exists() and latest_spec not in spec_target.read_text(encoding="utf-8"):
                    errors.append(
                        f"[Check G] {spec_target.relative_to(ROOT)} 缺少最新设计规格书 {latest_spec} 登记"
                    )

    # 校验 CLI --help (src/cli_args.rs / src/main.rs) 中不再残留过时的 Benchmark 计数
    cli_help_text = (
        (ROOT / "src/cli_args.rs").read_text(encoding="utf-8")
        if (ROOT / "src/cli_args.rs").exists()
        else (ROOT / "src/main.rs").read_text(encoding="utf-8")
    )
    wl_cnt = counts["workloads"]
    pr_cnt = counts["pr_rules"]
    for m in re.finditer(r"W1~W(\d+)", cli_help_text):
        if int(m.group(1)) != wl_cnt:
            errors.append(
                f"CLI help 漂移: 发现 '{m.group(0)}'，源码实测为 W1~W{wl_cnt}"
            )
    for m in re.finditer(r"PR-01~(\d+)", cli_help_text):
        if int(m.group(1)) != pr_cnt:
            errors.append(
                f"CLI help 漂移: 发现 '{m.group(0)}'，源码实测为 PR-01~{pr_cnt}"
            )

    # 校验活跃部署与示例配置严禁残留旧路径前缀
    legacy_dir = "/." + "st" + "ew" + "ard"
    for deploy_rel in ("Dockerfile", "docker-compose.yml", "config.example.toml"):
        dp = ROOT / deploy_rel
        if dp.exists() and legacy_dir in dp.read_text(encoding="utf-8"):
            errors.append(f"{deploy_rel} 仍残留旧数据目录路径 '{legacy_dir}'，请统一改为 '/.agent-body'")

    # 校验根目录核心文件是否在 README.md「项目结构」中逐一登记并附描述（规约 #20 / ISSUE #34）
    readme_path = ROOT / "README.md"
    if readme_path.exists():
        readme_text = readme_path.read_text(encoding="utf-8")
        struct_match = re.search(r"## 项目结构\s*\n(.*?)(?=\n## |\Z)", readme_text, flags=re.S)
        if not struct_match:
            errors.append("README.md 缺少「## 项目结构」章节")
        else:
            struct_sec = struct_match.group(1)
            described_files = set(
                re.findall(r"^\s*-\s+`([^`/]+)`\s*:\s*\S+", struct_sec, flags=re.MULTILINE)
            )
            root_files = sorted(
                p.name
                for p in ROOT.iterdir()
                if p.is_file() and (not p.name.startswith(".") or p.name in (".gitignore", ".dockerignore"))
            )
            for rf in root_files:
                if rf not in described_files:
                    errors.append(
                        f"README.md 项目结构缺少根目录核心文件 `{rf}` 的逐项职责描述（规约 #20）"
                    )

    # 校验全仓活跃 Markdown 相对链接不断链（排除归档与早期 Phase 日志）
    md_candidates = sorted(
        set(list(ROOT.glob("*.md")) + list((ROOT / "docs").rglob("*.md")) + list((ROOT / "skills").rglob("*.md")))
    )
    for md in md_candidates:
        rel_s = md.relative_to(ROOT).as_posix()
        if (
            "docs/src_doc/archive" in rel_s
            or "docs/src_doc/progress-phase" in rel_s
            or "docs/design_doc/review" in rel_s
            or re.search(r"roadmap-phase([0-9]|[1-4][0-9]|5[01])\b", rel_s)
        ):
            continue
        md_text = md.read_text(encoding="utf-8", errors="ignore")
        md_no_code = re.sub(r"```.*?```", "", md_text, flags=re.S)
        for m in re.finditer(r"\[[^\]]+\]\(([^)]+)\)", md_no_code):
            target = m.group(1).strip()
            if not target or target.startswith(("http://", "https://", "mailto:", "#", "conversation://", "file://")):
                continue
            path_part = target.split("#", 1)[0].split("?", 1)[0].strip()
            if not path_part:
                continue
            resolved = Path(path_part) if path_part.startswith("/") else (md.parent / path_part).resolve()
            if not resolved.exists():
                errors.append(f"{rel_s} 存在断链: ({target})")

    # Check A: Cargo.toml app_version 跨端对齐校验 (Web UI / INSTALL / docs/README / 演说 HTML)
    app_ver = manifest["app_version"]
    idx_html = ROOT / "src/web/assets/index.html"
    if idx_html.exists() and f"v{app_ver}" not in idx_html.read_text(encoding="utf-8"):
        errors.append(f"src/web/assets/index.html 缺少当前版本号 v{app_ver}")
    install_md = ROOT / "INSTALL.md"
    if install_md.exists():
        itxt = install_md.read_text(encoding="utf-8")
        if f"v{app_ver}" not in itxt:
            errors.append(f"INSTALL.md 缺少当前版本号 v{app_ver}")
        if "软件包版本" in itxt and "v0.4.0" in itxt:
            errors.append(f"INSTALL.md 示例输出仍残留旧版本 v0.4.0 (当前 v{app_ver})")
    docs_readme_p = ROOT / "docs/README.md"
    if docs_readme_p.exists() and f"v{app_ver}.md" not in docs_readme_p.read_text(encoding="utf-8"):
        errors.append(f"docs/README.md 缺少最新 Release 说明链接 v{app_ver}.md")
    for html_p in sorted((ROOT / "presentation").glob("*.html")):
        htxt = html_p.read_text(encoding="utf-8")
        if f"v{app_ver}" not in htxt:
            errors.append(f"{html_p.relative_to(ROOT)} 缺少当前版本号 v{app_ver}")
        if "v1.0.0" in htxt:
            errors.append(f"{html_p.relative_to(ROOT)} 仍残留过时版本 v1.0.0 (当前 v{app_ver})")

    # Check B: 核心活跃文档与 SKILL 中的反引号源码/脚本路径存在性校验
    code_ref_re = re.compile(
        r"`((?:src|kernel|contrib|distributions|scripts|skills|presentation|tests)/[A-Za-z0-9_./-]+\.(?:rs|py|md|html|svg|sql|toml|js))`"
    )
    code_ref_docs = (
        [ROOT / "README.md", ROOT / "EVOLUTION_GUIDE.md", ROOT / "INSTALL.md", ROOT / "docs/README.md"]
        + sorted((ROOT / "docs/arch").glob("*.md"))
        + sorted((ROOT / "docs/manual").glob("*.md"))
        + sorted((ROOT / "skills").rglob("*.md"))
    )
    for cd in code_ref_docs:
        if not cd.exists():
            continue
        ctxt = cd.read_text(encoding="utf-8")
        for m in code_ref_re.finditer(ctxt):
            ref = m.group(1)
            if any(ch in ref for ch in ("*", "{", "<")) or ".." in ref:
                continue
            if not (ROOT / ref).exists():
                ln = ctxt[: m.start()].count("\n") + 1
                errors.append(f"{cd.relative_to(ROOT)}:{ln} 引用了不存在的路径 `{ref}`")

    # Check C: parity_api 43 工具与 src/web/server.rs 精确路由双向对齐校验
    parity_ops = ROOT / "src/web/parity_api/ops_endpoints.rs"
    server_rs = ROOT / "src/web/server.rs"
    if parity_ops.exists() and server_rs.exists():
        ptxt = parity_ops.read_text(encoding="utf-8")
        stxt = server_rs.read_text(encoding="utf-8")
        parity_entries = re.findall(r'"(body_[a-z0-9_]+)"\s*=>\s*\("([^"]+)",\s*"([^"]+)",', ptxt)
        if len(parity_entries) != mcp_cnt:
            errors.append(
                f"parity_api 映射数 ({len(parity_entries)}) 与 MCP 工具数 ({mcp_cnt}) 不一致"
            )
        for tool_name, cli_cmd, web_ep in parity_entries:
            if not cli_cmd.startswith("agent-body "):
                errors.append(f"parity_api 工具 `{tool_name}` CLI 前缀异常: `{cli_cmd}`")
            if "*" in web_ep:
                errors.append(f"parity_api 工具 `{tool_name}` 禁止通配路由 `{web_ep}`")
            m_ep = re.search(r"(/api/v1/[a-zA-Z0-9_/-]+)", web_ep)
            if not m_ep:
                errors.append(f"parity_api 工具 `{tool_name}` 缺少 /api/v1/ 路由: `{web_ep}`")
            elif f'"{m_ep.group(1)}"' not in stxt:
                errors.append(
                    f"parity_api 工具 `{tool_name}` 路由 `{m_ep.group(1)}` 未在 src/web/server.rs 注册"
                )

    # Check D/E/F: 一等公民交叉防漂移（contrib 数量 / 六重门禁 / 默认端口 8765 / 静态测试计数）
    contrib_cnt = counts["contrib_exts"]
    stale_contrib_re = re.compile(
        r"(?:contrib\s*(?:9|九)\s*(?:个)?(?:扩展|插件)?|(?:9|九)\s*(?:个|套)?\s*(?:`?contrib`?|周边|标准五件套)\s*(?:扩展|模块|插件)?)",
        re.IGNORECASE,
    )
    stale_five_gate_re = re.compile(r"(?:五重|5\s*重)\s*(?:确定性)?(?:工程|质量)?门禁")
    stale_port_re = re.compile(r"(?:localhost|127\.0\.0\.1|:|--port\s+)8756\b")
    for doc in active_docs:
        if not doc.exists():
            continue
        rel_doc = doc.relative_to(ROOT).as_posix()
        txt_doc = doc.read_text(encoding="utf-8")
        if contrib_cnt >= 10:
            for m in stale_contrib_re.finditer(txt_doc):
                ctx = txt_doc[max(0, m.start() - 32) : min(len(txt_doc), m.end() + 32)]
                if any(tok in ctx for tok in ("Phase 70", "历史", "→", "前 9", "早期")):
                    continue
                errors.append(f"[Check D] {rel_doc} 残留过时 contrib 扩展计数 '{m.group(0)}' (实测 {contrib_cnt})")
        if not rel_doc.startswith("docs/design_doc/0"):
            for m in stale_five_gate_re.finditer(txt_doc):
                ctx = txt_doc[max(0, m.start() - 32) : min(len(txt_doc), m.end() + 32)]
                if any(tok in ctx for tok in ("Phase 4", "Phase 5", "历史", "演进")):
                    continue
                errors.append(f"[Check E] {rel_doc} 残留过时「{m.group(0)}」(当前为六重门禁)")
        for m in stale_port_re.finditer(txt_doc):
            ctx = txt_doc[max(0, m.start() - 32) : min(len(txt_doc), m.end() + 32)]
            if "192.168." in ctx or "mesh" in ctx.lower():
                continue
            errors.append(f"[Check F] {rel_doc} 残留旧默认端口 8756: '{m.group(0)}' (当前 8765)")

    arch_svg = ROOT / "docs/markdown/svg/architecture.svg"
    if arch_svg.exists() and "8756" in arch_svg.read_text(encoding="utf-8"):
        errors.append("[Check F] docs/markdown/svg/architecture.svg 仍残留旧端口 8756")

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
        f"📦 Agent Body v{manifest['app_version']} 架构快照: {c['modules']} 模块 | {c['mcp_tools']} MCP工具 | "
        f"{c['spi_methods']} SPI方法 | {c['tables']} SQLite表 | "
        f"微内核:{c['kernel_crates']} 周边:{c['contrib_exts']} 发行版:{c['distributions']} | "
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
