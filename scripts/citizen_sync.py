#!/usr/bin/env python3
"""Agent Body — 16 类一等公民增量变更路由器与动态交叉一致性审计器 (citizen_sync.py)。

设计目的（把战略/代码变更精确、低成本地传导至全仓毛细血管，杜绝高成本盲目全仓扫描）:
  1. --registry : 打印四圈层 16 类一等公民全景注册表
  2. --impact   : O(1) 基于 git diff / 指定文件，路由出受影响毛细血管清单、改动分级(L1/L2/L3)与最小验证命令
  3. --audit    : 7 项全动态跨端交叉审计 (CC-1 ~ CC-7)，纳入 `make quality-gate` 自动拦截漏网之鱼
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from evolution_manifest import ROOT, extract_manifest  # noqa: E402


# ---------------------------------------------------------------------------
# 一、四圈层 16 类一等公民全景注册表 (C01 ~ C16)
# ---------------------------------------------------------------------------

CITIZEN_REGISTRY: list[dict] = [
    {
        "id": "C01",
        "ring": "Ring 1 · 内核与契约源头",
        "name": "源码内联注释、Rustdoc 与错误消息",
        "paths": ["src/lib.rs", "src/benchmark/ledger.rs", "src/feedback/probes.rs", "src/config/types.rs"],
        "verify": "python3 scripts/citizen_sync.py --audit",
    },
    {
        "id": "C02",
        "ring": "Ring 1 · 内核与契约源头",
        "name": "四类多层次测试套件（单测 / 集成 / 契约 / E2E）",
        "paths": ["tests/", "contrib/mem_pg_tx/tests/regress.rs", "scripts/verify_webui_e2e.py"],
        "verify": "RUST_TEST_THREADS=1 make test",
    },
    {
        "id": "C03",
        "ring": "Ring 1 · 内核与契约源头",
        "name": "CLI 命令树、子命令参数与 `--help` 文本",
        "paths": ["src/main.rs", "src/cli_args.rs", "src/cli_subcommands.rs", "src/cli_handlers/"],
        "verify": "make evolution-check && cargo test --test cli_e2e",
    },
    {
        "id": "C04",
        "ring": "Ring 1 · 内核与契约源头",
        "name": "MCP 工具 Schema、Web REST API、Parity 矩阵与 UI",
        "paths": [
            "src/api/mcp_schemas.rs",
            "src/api/mcp_schemas_ops.rs",
            "src/api/mcp_tools/",
            "src/web/server.rs",
            "src/web/parity_api/",
            "src/web/assets/",
        ],
        "verify": "cargo test --test mcp_all_tools_e2e && cargo test --test web_full_e2e && make verify-webui",
    },
    {
        "id": "C05",
        "ring": "Ring 1 · 内核与契约源头",
        "name": "配置模板、环境变量契约与容器部署清单",
        "paths": ["src/config/types.rs", "config.example.toml", "Dockerfile", "docker-compose.yml", "Makefile"],
        "verify": "python3 scripts/citizen_sync.py --audit",
    },
    {
        "id": "C06",
        "ring": "Ring 1 · 内核与契约源头",
        "name": "三后端存储 Schema、无损自愈迁移与热迁移管线",
        "paths": [
            "src/storage/schema.sql",
            "src/storage/pg_schema.sql",
            "src/storage/duckdb_schema.sql",
            "src/storage/sqlite_provider/schema_reconcile.rs",
            "contrib/mem_pg_tx/src/schema.rs",
        ],
        "verify": "cargo test --test schema_migration_regression && cargo test --test storage_spi_contract",
    },
    {
        "id": "C07",
        "ring": "Ring 2 · 自动化验证与门禁",
        "name": "三轨实证 Benchmark、DQ 门禁与 PR 效能存证账本",
        "paths": ["src/benchmark/mod.rs", "src/benchmark/dq.rs", "src/benchmark/provenance.rs", "src/benchmark/ledger.rs"],
        "verify": "cargo test --lib benchmark::tests && cargo test --test benchmarks",
    },
    {
        "id": "C08",
        "ring": "Ring 2 · 自动化验证与门禁",
        "name": "六重确定性质量门禁与双平台 GitHub Actions CI/CD",
        "paths": [
            "scripts/evolution_manifest.py",
            "scripts/citizen_sync.py",
            "scripts/arch_boundary_check.py",
            "scripts/code_hygiene.py",
            ".github/workflows/ci.yml",
            ".github/workflows/release.yml",
        ],
        "verify": "make quality-gate",
    },
    {
        "id": "C09",
        "ring": "Ring 2 · 自动化验证与门禁",
        "name": "自进化活地图 (`EVOLUTION_GUIDE.md`) 与 `.codegraph/` 符号索引",
        "paths": ["EVOLUTION_GUIDE.md"],
        "verify": "make evolution-check && make citizen-audit",
    },
    {
        "id": "C10",
        "ring": "Ring 3 · 工程知识与操作手册",
        "name": "架构规格书 (`docs/design_doc/`) 与产品边界准则",
        "paths": [
            "docs/design_doc/05-architecture.md",
            "docs/design_doc/06-api-spec.md",
            "docs/design_doc/07-data-model.md",
            "docs/design_doc/10-benchmark-spec.md",
            "docs/design_doc/22-pg-tx-memory-spi-a-to-b-spec.md",
            "docs/src_doc/product-boundaries-and-governance.md",
        ],
        "verify": "python3 scripts/citizen_sync.py --audit",
    },
    {
        "id": "C11",
        "ring": "Ring 3 · 工程知识与操作手册",
        "name": "需求演进账 (`ISSUE.md`)、体验总账 (`user_feedback.md`) 与活跃路线图",
        "paths": ["ISSUE.md", "user_feedback.md", "docs/src_doc/roadmap-phase76-78.md", "docs/src_doc/progress.md"],
        "verify": "make evolution-check",
    },
    {
        "id": "C12",
        "ring": "Ring 3 · 工程知识与操作手册",
        "name": "开发者指南、生产部署手册 (`INSTALL.md`) 与三视角架构指南",
        "paths": [
            "INSTALL.md",
            "docs/src_doc/development.md",
            "docs/arch/01-for-architects.md",
            "docs/arch/02-for-developers.md",
            "docs/arch/03-for-users.md",
        ],
        "verify": "python3 scripts/citizen_sync.py --audit",
    },
    {
        "id": "C13",
        "ring": "Ring 3 · 工程知识与操作手册",
        "name": "10 卷官方使用手册 (`docs/manual/`) 与入口索引",
        "paths": [
            "docs/README.md",
            "docs/src_doc/product-manual.md",
            "docs/manual/README.md",
            "docs/manual/03-cli-reference.md",
            "docs/manual/04-mcp-integration.md",
            "docs/manual/06-advanced-features.md",
            "docs/manual/08-web-dashboard.md",
            "docs/manual/10-benchmark-and-roi.md",
        ],
        "verify": "make evolution-check && make citizen-audit",
    },
    {
        "id": "C14",
        "ring": "Ring 4 · 对外阵地与智能体认知",
        "name": "项目总门面 (`README.md`) 与版本发版说明 (`docs/releases/`)",
        "paths": ["README.md", "docs/releases/README.md", "docs/releases/v1.1.0.md"],
        "verify": "make evolution-check && make citizen-audit",
    },
    {
        "id": "C15",
        "ring": "Ring 4 · 对外阵地与智能体认知",
        "name": "Agent 内置技能包 (`skills/agent-body-*/SKILL.md`)",
        "paths": [
            "skills/agent-body-developer/SKILL.md",
            "skills/agent-body-doc-writer/SKILL.md",
            "skills/agent-body-guide/SKILL.md",
        ],
        "verify": "make evolution-check && make citizen-audit",
    },
    {
        "id": "C16",
        "ring": "Ring 4 · 对外阵地与智能体认知",
        "name": "Decks-as-Code 演说体系、对外技术长文与矢量架构 SVG",
        "paths": [
            "scripts/deck_engine.py",
            "scripts/deck_visuals.py",
            "scripts/build_decks_part1.py",
            "scripts/build_decks_part2.py",
            "scripts/build_flagship_roadshow.py",
            "scripts/build_flagship_techdeck.py",
            "docs/markdown/why-agent-needs-agent-body.md",
            "docs/markdown/svg/architecture.svg",
            "presentation/index.html",
        ],
        "verify": "make verify-decks && make citizen-audit",
    },
]


# ---------------------------------------------------------------------------
# 二、O(1) 增量变更传导路由规则 (Change -> Exact Capillaries + Verify Cmds)
# ---------------------------------------------------------------------------

IMPACT_RULES: list[dict] = [
    {
        "name": "配置字段与环境变量变更",
        "pattern": re.compile(r"^src/config/"),
        "tier": "L2",
        "targets": [
            ("config.example.toml", "C05 · 同步新配置字段默认值与注释示例"),
            ("src/preflight/", "C05 · 若新增外部服务 URL，补充 preflight 预检探针"),
            ("INSTALL.md", "C12 · 标记新依赖为 REQUIRED 或 OPTIONAL 及降级表现"),
            ("docs/manual/06-advanced-features.md", "C13 · 更新进阶配置章节"),
        ],
        "verify": ["python3 scripts/citizen_sync.py --audit", "cargo test --lib config"],
    },
    {
        "name": "存储 Schema / 表 / 列 / SPI 契约变更",
        "pattern": re.compile(r"^src/storage/|^contrib/mem_pg_tx/src/schema\.rs$"),
        "tier": "L3",
        "targets": [
            ("src/storage/schema.sql", "C06 · SQLite DDL 与 pg_schema.sql / duckdb_schema.sql 保持同步"),
            ("src/storage/pg_schema.sql", "C06 · PostgreSQL DDL 同步"),
            ("src/storage/duckdb_schema.sql", "C06 · DuckDB DDL 及 sync_pipeline Appender 列序同步"),
            ("src/storage/sqlite_provider/schema_reconcile.rs", "C06 · 老库无损补列/改名与 FTS5 重建自愈"),
            ("tests/schema_migration_regression.rs", "C02 · 补充老库无损升级与三端 Schema 一致回归"),
            ("tests/storage_spi_contract.rs", "C02 · SQLite/DuckDB 等价 SPI 契约对齐"),
            ("docs/design_doc/07-data-model.md", "C10 · 更新生产表与列说明"),
            ("EVOLUTION_GUIDE.md", "C09 · 校验 <!-- MANIFEST: --> tables/spi_methods"),
        ],
        "verify": [
            "cargo test --test schema_migration_regression",
            "cargo test --test storage_spi_contract",
            "make verify-webui",
        ],
    },
    {
        "name": "MCP 工具 / CLI 命令 / Web API 三端对齐变更",
        "pattern": re.compile(r"^src/(?:api|cli_args|cli_subcommands|cli_handlers|web)/"),
        "tier": "L2",
        "targets": [
            ("src/api/mcp_schemas.rs", "C04 · Schema & 参数说明同步"),
            ("src/cli_args.rs", "C03 · CLI 参数与 --help 文案同步"),
            ("src/cli_subcommands.rs", "C03 · CLI 子命令 1:1 映射同步"),
            ("src/web/parity_api/ops_endpoints.rs", "C04 · Parity 路由映射同步"),
            ("src/web/server.rs", "C04 · REST 路由与写鉴权 check_auth 同步"),
            ("src/web/assets/", "C04 · 前端控制台交互与镜像命令更新"),
            ("docs/manual/03-cli-reference.md", "C13 · CLI 字典与实操示例更新"),
            ("docs/manual/04-mcp-integration.md", "C13 · MCP 工具参数表同步"),
            ("docs/manual/08-web-dashboard.md", "C13 · Web REST API 与工作台说明同步"),
            ("docs/design_doc/06-api-spec.md", "C10 · 顶层接口契约指针校验"),
        ],
        "verify": [
            "cargo test --test mcp_all_tools_e2e",
            "cargo test --test web_full_e2e",
            "cargo test --test cli_e2e",
            "make verify-webui",
        ],
    },
    {
        "name": "Benchmark / DQ / 效能账本 PR-XX 变更",
        "pattern": re.compile(r"^src/benchmark/"),
        "tier": "L3",
        "targets": [
            ("src/benchmark/mod.rs", "C07 · BENCHMARK_SPEC_VERSION 与断言文案同步"),
            ("src/benchmark/ledger.rs", "C01 · 更新 PR-01~PR-XX Rustdoc 与渠道记账映射"),
            ("src/cli_args.rs", "C03 · CLI bench help 中的 W1~WN / PR-01~N 同步"),
            ("EVOLUTION_GUIDE.md", "C09 · 刷新 workloads / dq / pr_rules / verified_tests 锚点"),
            ("README.md", "C14 · 刷新实证指标表与规约 #18 Benchmark 版本"),
            ("docs/design_doc/10-benchmark-spec.md", "C10 · 新增版本增量规格章节"),
            ("docs/manual/10-benchmark-and-roi.md", "C13 · 更新用户侧 Benchmark 解读"),
            ("scripts/build_decks_part1.py", "C16 · 触发演说重生成（Deck 已接入动态 SSoT）"),
            ("docs/markdown/why-agent-needs-agent-body.md", "C16 · 更新对外长文 Benchmark 战报"),
            ("docs/markdown/svg/architecture.svg", "C16 · 更新架构图 Layer 4 Benchmark 标注"),
        ],
        "verify": [
            "cargo test --lib benchmark::tests",
            "python3 scripts/citizen_sync.py --audit",
            "make verify-decks",
        ],
    },
    {
        "name": "微内核 / 周边 Contrib / 精简发行版变更",
        "pattern": re.compile(r"^(?:kernel|contrib|distributions)/"),
        "tier": "L3",
        "targets": [
            ("scripts/arch_boundary_check.py", "C08 · 校验 kernel/ 零业务词与单向依赖"),
            ("docs/design_doc/20-plugin-microkernel-architecture-spec.md", "C10 · 更新五件套与发行版规格"),
            ("docs/design_doc/22-pg-tx-memory-spi-a-to-b-spec.md", "C10 · 若涉及 mem_pg_tx 更新方案 A→B 状态"),
            ("docs/arch/01-for-architects.md", "C12 · 刷新微内核与周边扩展盘点"),
            ("docs/arch/02-for-developers.md", "C12 · 刷新模块坐标与测试数"),
            ("docs/markdown/svg/architecture.svg", "C16 · 刷新 Layer 3 contrib/ 扩展数量与清单"),
        ],
        "verify": [
            "make arch-check",
            "RUST_TEST_THREADS=1 make test",
        ],
    },
    {
        "name": "集成测试增删变更",
        "pattern": re.compile(r"^tests/.*\.rs$"),
        "tier": "L2",
        "targets": [
            ("EVOLUTION_GUIDE.md", "C09 · 在 4.3 集成测试矩阵登记套件文件名并更新 verified_tests"),
            ("docs/src_doc/development.md", "C12 · 在 tests/ 目录树登记并更新测试总数"),
            ("README.md", "C14 · 更新 Badge、测试套数、总数及规约 #13"),
            ("skills/agent-body-developer/SKILL.md", "C15 · 更新步骤 4 与自检 Checklist 测试总数"),
            ("skills/agent-body-doc-writer/SKILL.md", "C15 · 更新文档基准测试总数"),
            ("skills/agent-body-guide/SKILL.md", "C15 · 更新一键编译测试数量"),
        ],
        "verify": [
            "RUST_TEST_THREADS=1 cargo test --workspace",
            "make quality-gate",
        ],
    },
    {
        "name": "记忆 / PG 事务海马体 / 本体连续性胶囊变更",
        "pattern": re.compile(r"^src/(?:memory|body/continuity|body/envelope|feedback)/"),
        "tier": "L2",
        "targets": [
            ("src/feedback/probes.rs", "C01 · 更新 P1~P5 探针治愈建议与 PR 编号文案"),
            ("docs/src_doc/roadmap-phase76-78.md", "C11 · 更新阶段检查点与实机验收数据"),
            ("docs/src_doc/progress.md", "C11 · 刷新进度总表"),
            ("docs/src_doc/product-boundaries-and-governance.md", "C10 · 核对脑体分离与递弱代偿边界"),
            ("docs/manual/06-advanced-features.md", "C13 · 更新事务记忆/胶囊进阶实操"),
        ],
        "verify": [
            "cargo test --test fast_weight_m3_integration",
            "cargo test --lib body::continuity",
        ],
    },
    {
        "name": "演说生成脚本与视觉组件变更",
        "pattern": re.compile(r"^scripts/(?:build_.*|deck_.*)\.py$"),
        "tier": "L2",
        "targets": [
            ("presentation/index.html", "C16 · 重新执行 4 套 build_*.py 重生成全部 9 份 HTML"),
        ],
        "verify": [
            "python3 scripts/build_decks_part1.py && python3 scripts/build_decks_part2.py && python3 scripts/build_flagship_roadshow.py && python3 scripts/build_flagship_techdeck.py",
            "make verify-decks",
        ],
    },
    {
        "name": "版本号或里程碑发版收口变更",
        "pattern": re.compile(r"^Cargo\.toml$|^docs/src_doc/roadmap-phase|^ISSUE\.md$"),
        "tier": "L3",
        "targets": [
            ("docs/releases/README.md", "C14 · 更新发版汇总表当前版本行"),
            ("docs/releases/v1.1.0.md", "C14 · 补充当前版本增量交付章节"),
            ("docs/src_doc/progress.md", "C11 · 更新交付台账状态"),
            ("README.md", "C14 · 刷新演进路线表与规约"),
            ("docs/src_doc/product-manual.md", "C13 · 同步指向当前 Benchmark 版本"),
        ],
        "verify": ["make quality-gate"],
    },
]


def _detect_changed_files(base_ref: str | None, explicit_files: list[str]) -> list[str]:
    if explicit_files:
        return [f.strip().lstrip("./") for f in explicit_files if f.strip()]

    files: set[str] = set()
    if base_ref:
        out = subprocess.check_output(
            ["git", "diff", "--name-only", base_ref, "HEAD"],
            cwd=ROOT,
            text=True,
            stderr=subprocess.DEVNULL,
        )
        files.update(line.strip() for line in out.splitlines() if line.strip())

    status_out = subprocess.check_output(
        ["git", "status", "--porcelain"],
        cwd=ROOT,
        text=True,
    )
    for raw in status_out.splitlines():
        if len(raw) >= 4:
            path = raw[3:].strip()
            if " -> " in path:
                path = path.split(" -> ", 1)[1].strip()
            if path:
                files.add(path)
    return sorted(files)


def compute_impact(changed_files: list[str]) -> dict:
    changed_set = set(changed_files)
    matched_rules: list[dict] = []
    target_map: dict[str, dict] = {}
    verify_cmds: list[str] = []
    highest_tier = "L1"
    tier_rank = {"L1": 1, "L2": 2, "L3": 3}

    for f in changed_files:
        for rule in IMPACT_RULES:
            if rule["pattern"].search(f):
                if rule["name"] not in [r["name"] for r in matched_rules]:
                    matched_rules.append({"name": rule["name"], "tier": rule["tier"], "trigger": f})
                if tier_rank[rule["tier"]] > tier_rank[highest_tier]:
                    highest_tier = rule["tier"]
                for tgt, reason in rule["targets"]:
                    already_touched = tgt in changed_set or (
                        tgt.endswith("/") and any(cf.startswith(tgt) for cf in changed_set)
                    )
                    entry = target_map.setdefault(
                        tgt,
                        {"target": tgt, "already_touched": already_touched, "reasons": []},
                    )
                    if reason not in entry["reasons"]:
                        entry["reasons"].append(reason)
                for cmd in rule["verify"]:
                    if cmd not in verify_cmds:
                        verify_cmds.append(cmd)

    if not verify_cmds:
        verify_cmds = ["make evolution-check", "make citizen-audit"]

    return {
        "changed_files": changed_files,
        "tier": highest_tier,
        "matched_rules": matched_rules,
        "impacted_targets": list(target_map.values()),
        "recommended_verify": verify_cmds,
    }


# ---------------------------------------------------------------------------
# 三、7 项动态跨端交叉一致性审计 (CC-1 ~ CC-7 · 零硬编码旧值)
# ---------------------------------------------------------------------------

def _ver_tuple(ver_str: str) -> tuple[int, ...]:
    return tuple(int(p) for p in re.findall(r"\d+", ver_str))


def run_cross_citizen_audit() -> dict:
    manifest = extract_manifest()
    c = manifest["counts"]
    app_ver = manifest["app_version"]
    bench_ver = manifest["bench_spec_version"]
    bench_ver_t = _ver_tuple(bench_ver)
    wl_cnt = c["workloads"]
    dq_cnt = c["dq"]
    pr_cnt = c["pr_rules"]
    contrib_cnt = c["contrib_exts"]
    verified_tests = c["verified_tests"]
    actual_test_suites = sorted(p.name for p in (ROOT / "tests").glob("*.rs"))
    suite_cnt = len(actual_test_suites)

    checks: dict[str, list[str]] = {
        "CC-1_capillary_dynamic_parity": [],
        "CC-2_config_struct_vs_example_toml": [],
        "CC-3_tests_matrix_vs_docs": [],
        "CC-4_rustdoc_and_cli_comments": [],
        "CC-5_deck_generators_dynamic_ssot": [],
        "CC-6_release_article_boundary_sync": [],
        "CC-7_citizen_registry_paths": [],
    }

    # --- CC-1: 全活跃毛细血管动态版本/计数检查 ---
    cc1_files = sorted(
        set(
            [
                ROOT / "README.md",
                ROOT / "EVOLUTION_GUIDE.md",
                ROOT / "INSTALL.md",
                ROOT / "docs/README.md",
                ROOT / "docs/src_doc/development.md",
                ROOT / "docs/src_doc/progress.md",
                ROOT / "docs/src_doc/product-manual.md",
                ROOT / "docs/src_doc/product-boundaries-and-governance.md",
                ROOT / "docs/markdown/why-agent-needs-agent-body.md",
                ROOT / "docs/markdown/svg/architecture.svg",
            ]
            + list((ROOT / "docs/arch").glob("*.md"))
            + list((ROOT / "docs/manual").glob("*.md"))
            + list((ROOT / "skills").rglob("*.md"))
            + list((ROOT / "scripts").glob("build_*.py"))
            + list((ROOT / "scripts").glob("deck_*.py"))
            + list((ROOT / "presentation").glob("*.html"))
        )
    )
    hist_tokens = (
        "→", "Phase ", "历史", "演进", "早期", "归档", "基线", "v0.", "v1.0",
        "run-", "前 9", "前 10", "W20`", "W21`", "v3.8`", "v3.9`", "v4.0`",
        "**Phase", "**v1.1.0 品质硬化**", "装配 ", "纯微内核，",
    )

    re_bench = re.compile(r"(?:Bench(?:mark)?(?:\s+Spec)?\s*)v(\d+\.\d+)\b")
    re_wl = re.compile(r"\bW1~W(\d+)\b")
    re_dq = re.compile(r"\bDQ1~DQ(\d+)\b")
    re_pr = re.compile(r"\bPR-01~(?:PR-)?(\d+)\b")
    re_contrib = re.compile(r"\b([1-9]\d*)\s*(?:个|套)?\s*(?:`?contrib/?`?\s*(?:扩展|周边|标准五件套)|周边\s*\(`?contrib/?`?\)|五件套扩展)")
    re_tests = re.compile(r"\b(387|396|414|428)\s*(?:项|个|测|tests|Passed|测试|workspace\s*tests)")
    re_suites = re.compile(r"\b(1[678])\s*(?:套|个)\s*(?:自动化)?(?:集成|测试)")

    def _is_hist(txt: str, start: int, end: int) -> bool:
        ctx = txt[max(0, start - 48) : min(len(txt), end + 48)]
        if any(t in ctx for t in hist_tokens):
            return True
        line_start = txt.rfind("\n", 0, start) + 1
        line_end = txt.find("\n", end)
        line = txt[line_start : (len(txt) if line_end == -1 else line_end)]
        return line.lstrip().startswith("|") and any(t in line for t in hist_tokens)

    for p in cc1_files:
        if not p.exists():
            continue
        rel = p.relative_to(ROOT).as_posix()
        txt = p.read_text(encoding="utf-8", errors="ignore")
        is_html = rel.endswith(".html")

        for m in re_bench.finditer(txt):
            if not is_html and _is_hist(txt, m.start(), m.end()):
                continue
            if _ver_tuple(m.group(1)) < bench_ver_t:
                ln = txt[: m.start()].count("\n") + 1
                checks["CC-1_capillary_dynamic_parity"].append(
                    f"{rel}:{ln} 发现过期 Benchmark 版本 v{m.group(1)} (源码为 v{bench_ver})"
                )

        for m in re_wl.finditer(txt):
            if not is_html and _is_hist(txt, m.start(), m.end()):
                continue
            if int(m.group(1)) < wl_cnt:
                ln = txt[: m.start()].count("\n") + 1
                checks["CC-1_capillary_dynamic_parity"].append(
                    f"{rel}:{ln} 发现过期负载范围 W1~W{m.group(1)} (源码为 W1~W{wl_cnt})"
                )

        for m in re_dq.finditer(txt):
            if not is_html and _is_hist(txt, m.start(), m.end()):
                continue
            if int(m.group(1)) < dq_cnt:
                ln = txt[: m.start()].count("\n") + 1
                checks["CC-1_capillary_dynamic_parity"].append(
                    f"{rel}:{ln} 发现过期 DQ 范围 DQ1~DQ{m.group(1)} (源码为 DQ1~DQ{dq_cnt})"
                )

        for m in re_pr.finditer(txt):
            if not is_html and _is_hist(txt, m.start(), m.end()):
                continue
            if int(m.group(1)) < pr_cnt:
                ln = txt[: m.start()].count("\n") + 1
                checks["CC-1_capillary_dynamic_parity"].append(
                    f"{rel}:{ln} 发现过期 PR 范围 PR-01~{m.group(1)} (源码为 PR-01~{pr_cnt})"
                )

        for m in re_contrib.finditer(txt):
            if not is_html and _is_hist(txt, m.start(), m.end()):
                continue
            if int(m.group(1)) < contrib_cnt:
                ln = txt[: m.start()].count("\n") + 1
                checks["CC-1_capillary_dynamic_parity"].append(
                    f"{rel}:{ln} 发现过期 contrib 数量 {m.group(1)} (源码为 {contrib_cnt})"
                )

        for m in re_tests.finditer(txt):
            if not is_html and _is_hist(txt, m.start(), m.end()):
                continue
            if int(m.group(1)) != verified_tests:
                ln = txt[: m.start()].count("\n") + 1
                checks["CC-1_capillary_dynamic_parity"].append(
                    f"{rel}:{ln} 发现过期测试数量 {m.group(1)} (Manifest 为 {verified_tests})"
                )

        for m in re_suites.finditer(txt):
            if not is_html and _is_hist(txt, m.start(), m.end()):
                continue
            if int(m.group(1)) < suite_cnt:
                ln = txt[: m.start()].count("\n") + 1
                checks["CC-1_capillary_dynamic_parity"].append(
                    f"{rel}:{ln} 发现过期集成套件数 {m.group(1)} (tests/*.rs 实测 {suite_cnt})"
                )

    # --- CC-2: src/config/types.rs 结构体字段 <-> config.example.toml 双向校验 ---
    types_p = ROOT / "src/config/types.rs"
    cfg_p = ROOT / "config.example.toml"
    if types_p.exists() and cfg_p.exists():
        types_txt = types_p.read_text(encoding="utf-8")
        cfg_txt = cfg_p.read_text(encoding="utf-8")
        for sname, body in re.findall(r"pub struct (\w+)\s*\{([^}]+)\}", types_txt):
            if sname == "Config":
                continue
            for field in re.findall(r"pub\s+([a-z0-9_]+)\s*:", body):
                has_key = re.search(rf"\b{re.escape(field)}\s*=", cfg_txt)
                has_subtable = re.search(rf"\[[a-z0-9_]+\.{re.escape(field)}\]", cfg_txt)
                if not (has_key or has_subtable):
                    checks["CC-2_config_struct_vs_example_toml"].append(
                        f"config.example.toml 缺少配置字段 {sname}.{field}"
                    )

    # --- CC-3: tests/*.rs 与 EVOLUTION_GUIDE.md & development.md 登记校验 ---
    guide_txt = (ROOT / "EVOLUTION_GUIDE.md").read_text(encoding="utf-8") if (ROOT / "EVOLUTION_GUIDE.md").exists() else ""
    dev_txt = (ROOT / "docs/src_doc/development.md").read_text(encoding="utf-8") if (ROOT / "docs/src_doc/development.md").exists() else ""
    for ts in actual_test_suites:
        if ts not in guide_txt:
            checks["CC-3_tests_matrix_vs_docs"].append(f"EVOLUTION_GUIDE.md 4.3 集成矩阵未登记 tests/{ts}")
        if ts not in dev_txt:
            checks["CC-3_tests_matrix_vs_docs"].append(f"docs/src_doc/development.md tests/ 目录树未登记 {ts}")

    # --- CC-4: Rustdoc 门面与源码关键注释鲜活度 ---
    lib_txt = (ROOT / "src/lib.rs").read_text(encoding="utf-8") if (ROOT / "src/lib.rs").exists() else ""
    if "//!" not in lib_txt:
        checks["CC-4_rustdoc_and_cli_comments"].append("src/lib.rs 缺少 crate 级 //! 文档注释")
    for src_rel in (
        "src/lib.rs",
        "src/main.rs",
        "src/cli_args.rs",
        "src/cli_subcommands.rs",
        "src/benchmark/ledger.rs",
        "src/feedback/probes.rs",
        "src/config/types.rs",
    ):
        sp = ROOT / src_rel
        if not sp.exists():
            continue
        stxt = sp.read_text(encoding="utf-8")
        for m in re_pr.finditer(stxt):
            if int(m.group(1)) < pr_cnt:
                ln = stxt[: m.start()].count("\n") + 1
                line = stxt.splitlines()[ln - 1]
                if "Phase " not in line and "assert" not in line:
                    checks["CC-4_rustdoc_and_cli_comments"].append(
                        f"{src_rel}:{ln} 源码注释/字面量残留 {m.group(0)} (当前 PR-01~{pr_cnt})"
                    )
        for m in re.finditer(r"8756\b", stxt):
            ln = stxt[: m.start()].count("\n") + 1
            line = stxt.splitlines()[ln - 1]
            if "test" not in src_rel and "#[cfg(test)]" not in stxt[: m.start()]:
                checks["CC-4_rustdoc_and_cli_comments"].append(
                    f"{src_rel}:{ln} 源码注释残留旧端口 8756: {line.strip()[:80]}"
                )

    # --- CC-5: 演说生成器与视觉模块禁止硬编码旧版本/测试数 ---
    for py_p in sorted((ROOT / "scripts").glob("deck_*.py")) + sorted((ROOT / "scripts").glob("build_*.py")):
        rel = py_p.relative_to(ROOT).as_posix()
        ptxt = py_p.read_text(encoding="utf-8")
        for bad_pat in (
            re.compile(r"Bench(?:mark)?\s*v(?:3\.9|4\.0)\b"),
            re.compile(r"\b(?:396|414|428)\s*(?:项|测试|测)"),
        ):
            for m in bad_pat.finditer(ptxt):
                ln = ptxt[: m.start()].count("\n") + 1
                checks["CC-5_deck_generators_dynamic_ssot"].append(
                    f"{rel}:{ln} 硬编码过时口径 '{m.group(0)}'，应改为读取 evolution_manifest SSoT"
                )

    # --- CC-6: 发版说明、对外技术长文、SVG 与边界文档同步校验 ---
    rel_note = ROOT / f"docs/releases/v{app_ver}.md"
    if not rel_note.exists():
        checks["CC-6_release_article_boundary_sync"].append(f"缺少当前版本发版说明 docs/releases/v{app_ver}.md")
    else:
        rntxt = rel_note.read_text(encoding="utf-8")
        if f"v{bench_ver}" not in rntxt or str(verified_tests) not in rntxt:
            checks["CC-6_release_article_boundary_sync"].append(
                f"docs/releases/v{app_ver}.md 未同步 Bench v{bench_ver} 与测试数 {verified_tests}"
            )
    rel_readme = ROOT / "docs/releases/README.md"
    if rel_readme.exists():
        rrtxt = rel_readme.read_text(encoding="utf-8")
        if f"v{bench_ver}" not in rrtxt or str(verified_tests) not in rrtxt:
            checks["CC-6_release_article_boundary_sync"].append(
                f"docs/releases/README.md 最新发版行未同步 v{bench_ver} 与 {verified_tests}"
            )
    svg_p = ROOT / "docs/markdown/svg/architecture.svg"
    if svg_p.exists():
        stxt = svg_p.read_text(encoding="utf-8")
        if f"v{bench_ver}" not in stxt or f"周边 {contrib_cnt}" not in stxt:
            checks["CC-6_release_article_boundary_sync"].append(
                f"docs/markdown/svg/architecture.svg 未同步 Bench v{bench_ver} 与 周边 {contrib_cnt}"
            )
    art_p = ROOT / "docs/markdown/why-agent-needs-agent-body.md"
    if art_p.exists():
        atxt = art_p.read_text(encoding="utf-8")
        if f"v{bench_ver}" not in atxt or str(verified_tests) not in atxt:
            checks["CC-6_release_article_boundary_sync"].append(
                f"docs/markdown/why-agent-needs-agent-body.md 未同步 Bench v{bench_ver} 与 {verified_tests}"
            )

    # --- CC-7: 16 类一等公民注册路径存在性校验 ---
    for item in CITIZEN_REGISTRY:
        for p_rel in item["paths"]:
            if not (ROOT / p_rel).exists():
                checks["CC-7_citizen_registry_paths"].append(
                    f"{item['id']} 注册路径不存在: {p_rel}"
                )

    total_errors = sum(len(v) for v in checks.values())
    return {
        "manifest_snapshot": {
            "app_version": app_ver,
            "bench_spec_version": bench_ver,
            "workloads": wl_cnt,
            "dq": dq_cnt,
            "pr_rules": pr_cnt,
            "contrib_exts": contrib_cnt,
            "test_suites": suite_cnt,
            "verified_tests": verified_tests,
        },
        "checks": checks,
        "total_errors": total_errors,
        "passed": total_errors == 0,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Agent Body 16 类一等公民增量路由与交叉一致性审计")
    parser.add_argument("--registry", action="store_true", help="输出 16 类一等公民全景图谱")
    parser.add_argument("--impact", action="store_true", help="O(1) 计算当前改动影响的毛细血管与最小验证命令")
    parser.add_argument("--audit", action="store_true", help="运行 7 项动态跨端交叉一致性审计 (CC-1~CC-7)")
    parser.add_argument("--base", help="git diff 基准引用 (默认同时含工作区改动)")
    parser.add_argument("--files", nargs="*", default=[], help="显式指定变更文件路径")
    parser.add_argument("--json", action="store_true", help="输出机器可读 JSON")
    args = parser.parse_args()

    if not (args.registry or args.impact or args.audit):
        args.audit = True

    if args.registry:
        if args.json:
            print(json.dumps(CITIZEN_REGISTRY, ensure_ascii=False, indent=2))
        else:
            print("📋 Agent Body 16 类一等公民四圈层全景:")
            for item in CITIZEN_REGISTRY:
                print(f"  [{item['id']}] {item['ring']} | {item['name']}")
        return 0

    if args.impact:
        changed = _detect_changed_files(args.base, args.files)
        report = compute_impact(changed)
        if args.json:
            print(json.dumps(report, ensure_ascii=False, indent=2))
        else:
            print(f"🧭 [CitizenImpact] 变更文件 {len(changed)} 个 | 传导等级: {report['tier']}")
            if not changed:
                print("  工作区无未提交改动；可用 `--base HEAD~1` 或 `--files <path>` 推演影响面。")
            for mr in report["matched_rules"]:
                print(f"  - 命中规则 [{mr['tier']}]: {mr['name']} (触发: {mr['trigger']})")
            print("🎯 受影响的一等公民毛细血管:")
            for item in report["impacted_targets"]:
                mark = "✅ 已同期改动" if item["already_touched"] else "⚠️  需核对补齐"
                print(f"  - {mark} {item['target']} -> {'; '.join(item['reasons'])}")
            print("🧪 最小化精准验证命令:")
            for cmd in report["recommended_verify"]:
                print(f"  $ {cmd}")
        return 0

    audit_rep = run_cross_citizen_audit()
    if args.json:
        print(json.dumps(audit_rep, ensure_ascii=False, indent=2))
    else:
        s = audit_rep["manifest_snapshot"]
        print(
            f"🔎 [CitizenAudit] 16 类一等公民 7 项交叉审计 "
            f"(v{s['app_version']} · Bench v{s['bench_spec_version']} · W:{s['workloads']} DQ:{s['dq']} PR:{s['pr_rules']} · {s['test_suites']} suites / {s['verified_tests']} tests)"
        )
        for cname, errs in audit_rep["checks"].items():
            status = "✅ PASS" if not errs else f"❌ FAIL ({len(errs)})"
            print(f"  - {cname}: {status}")
            for err in errs:
                print(f"      · {err}")
    return 0 if audit_rep["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
