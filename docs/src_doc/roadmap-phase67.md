# Agent Body Roadmap — Phase 67: 全仓生产文件 `< 1000L` 模块拆分与 `let _ = <= 15` 吞错治理

> **文档状态**：✅ **已实施** | **基线版本**：`v1.1.0` (`ff16846`)  
> **定位**：彻底消灭 `>= 1000L` 生产准 God File，将 `scripts/code_hygiene.py` 复杂度红线从 `1500L` 收紧至 `1000L`、静默 `let _ =` 预算从 `50` 收紧至 `15`，并修复 `prod_slice` 误命中原始字符串 `#[cfg(test)]` 的漏检漏洞。

---

## 1. 模块化拆分清单（全仓 `>1000L` 生产文件清零）

| 原单文件 (原生产行数) | 拆分后子模块结构 | 说明 |
|---|---|---|
| `src/api/mcp_tools.rs` (`1451L`) | [`src/api/mcp_tools/{mod,core_handlers,infra_handlers,body_handlers}.rs`](../../src/api/mcp_tools/mod.rs) | 按核心/基建/本体三域分流 43 个 MCP 工具处理臂 |
| `src/benchmark/workloads/mod.rs` (`1427L`) & `advanced.rs` (`1431L`) | [`mod.rs`](../../src/benchmark/workloads/mod.rs) + `{core,infra,governance}_workloads.rs` + `advanced.rs` | `W1~W4` / `W5~W8` / `W9~W13` / `W14~W18` 各 <730L |
| `src/scanner/multimodal_ext.rs` (`1314L`) | [`src/scanner/multimodal_ext/{mod,format_probes,kv_filter}.rs`](../../src/scanner/multimodal_ext/mod.rs) | 拆出 HTML/Parquet/CSV/JSONL/音频探针与 `KvPredicate` |
| [`src/config/mod.rs`](../../src/config/mod.rs) (`1282L`) | `src/config/{mod,types}.rs` | 独立所有配置结构体、`RuntimeMode`/`McpToolProfile` 与默认值 |
| [`src/scanner/metadata.rs`](../../src/scanner/metadata.rs) (`1274L`) | `src/scanner/{metadata,redactor}.rs` | 独立 `SecretRedactor`、`PromptInjectionScanner` 与 `is_sensitive_file` |
| [`src/fact/knowledge_graph.rs`](../../src/fact/knowledge_graph.rs) (`1183L`) | `src/fact/{knowledge_graph,knowledge_terms}.rs` | 独立 `HeadingChunker`、`AliasCanonicalizer` 与中英文分词/锚点函数 |
| [`src/cli_args.rs`](../../src/cli_args.rs) (`1111L`) | `src/cli_args.rs` + [`src/cli_subcommands.rs`](../../src/cli_subcommands.rs) | 保留 `Commands` 于 `cli_args.rs`（兼容清单提取），拆出 `Mesh`..`Memory` 枚举 |
| [`src/body/perf_awr/awr_engine.rs`](../../src/body/perf_awr/awr_engine.rs) (`1101L`) | `src/body/perf_awr/{awr_engine,addm_tuner}.rs` | 独立 AWR 指标结构体与 `evaluate_addm_rules` / `run_l1_auto_tune` |
| [`src/preflight/mod.rs`](../../src/preflight/mod.rs) (`1066L`) | `src/preflight/{mod,checks}.rs` | 拆出外部工具、存储后端、磁盘水位与平台系统信息检查 |

---

## 2. 静默吞错 `let _ =` 治理与质量门禁升级

1. 修复 [`scripts/code_hygiene.py`](../../scripts/code_hygiene.py) `prod_slice()` 忽略 `r#*"..."#*` 原始字符串内嵌 `#[cfg(test)]` 字面量。
2. 将 `MAX_PROD_LOC` 从 `1500` 收紧至 **`1000`**，`MAX_SILENT_LET_UNDERSCORE` 从 `50` 收紧至 **`15`**，并将 `src/{body,scanner,hooks,web,links,fact,audit,config}/` 全部纳入核心零静默前缀列表。
3. 清理生产路径中的静默 `let _ =`（转为显式 `if let Err(e)` 告警日志、错误 `?` 上抛或干净变量绑定）。
