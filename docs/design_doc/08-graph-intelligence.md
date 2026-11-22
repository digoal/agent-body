# Agent Host Steward — 设计文档 08: 图智能与环境拓扑演进 (Graph Intelligence)

> **版本**: v3.0  
> **更新时间**: 2026-11-15  
> **状态**: 规范草案（设计先行）  
> **关联文档**: `docs/design_doc/05-architecture.md`, `docs/design_doc/06-api-spec.md`, `docs/design_doc/07-data-model.md`

---

## 1. 架构动机与 CodeGraph 吸收反思

### 1.1 现状与痛点
在 Agent Host Steward 早期版本中，图关系（`links`）主要用于记录抽象的实体关联（如 `/root/new/work/demo.py` 依赖 `/root/new/shared_proj/util.py`）。但在实际大规模 Agent 常驻运行时，暴露了以下核心短板：
1. **出处黑盒（Lack of Provenance）**：Agent 被告知“A 依赖 B”，但不知道在 A 的**哪一行**、通过**什么代码**调用的 B，导致 Agent 仍需手动全量 grep 源码，增加了上下文 token 消耗。
2. **粗暴正则伪依赖与假阴性**：基于纯字符串正则查找 `/root/new/`，漏掉了相对导入、跨包 import、Shell `source` 等标准语法。
3. **断链与坏死幽灵依赖（Dangling Links）**：宿主机资产被移动或删除后，依赖关系依旧残留在数据库中；或者脚本硬编码依赖了未启动的端口，缺乏主动健康审计机制。
4. **孤岛割裂（宏观微观脱节）**：宿主大管家停留在“文件/服务”级别，而开源项目内部已经存在 `codegraph` 建立的“函数/类/AST”微观图。大管家无法下钻，导致爆炸半径评估无法细化到符号级。

### 1.2 从 CodeGraph 吸收的四大核心设计
1. **边元数据强化（Edge Metadata & Provenance）**：吸收 `edges.line`、`edges.col`、`edges.provenance` 设计，将依赖出处固化到行号与代码片段。
2. **精准语法特征提取（Syntax-Aware Import Parsing）**：吸收多语言模块依赖抽取模式，针对 Python、Shell、Node.js 提供精准 import 发现。
3. **断链体检与悬空管理（Dangling / Unresolved Reference Auditor）**：吸收 `unresolved_refs` 设计，提供主动健康体检与故障预警。
4. **宏微双层联邦拓扑（Federated Graph Bridge）**：Steward 作为宿主宏观图调度中心，联动项目局部 `.codegraph/` 微观图，实现“文件级 -> 函数级”穿透式下钻。

---

## 2. 数据模型变更 (Schema v3)

### 2.1 `links` 表结构扩展
对既有 `links` 表进行字段追加：

```sql
-- 增加行号、代码切片与出处标记
ALTER TABLE links ADD COLUMN line_number INTEGER DEFAULT NULL;
ALTER TABLE links ADD COLUMN snippet TEXT DEFAULT NULL;
ALTER TABLE links ADD COLUMN provenance TEXT DEFAULT 'static_scan';
```

- `line_number`: 触发引用的行号（从 1 开始）；若为系统隐式依赖或动态关联则为 NULL。
- `snippet`: 触发引用的代码行截取（截取长度不超过 160 字符，并做空白去重）。
- `provenance`: 发现方式，枚举：
  - `'ast_import'`: 通过语言 import/require/source 语法分析提取。
  - `'path_regex'`: 绝对路径扫描匹配。
  - `'port_detect'`: 网络与端口连接串侦测。
  - `'manual'`: 人工或 Agent 主动注册。
  - `'codegraph'`: 从局部代码图提取沉淀。

### 2.2 自动兼容与迁移策略
- 升级 `config` 表中的 `schema_version` 为 `'3'`。
- `Storage::init` 自动检测并执行增量 `ALTER TABLE`，保障既有 `~/.steward/steward.db` 平滑无缝升级。

---

## 3. 核心功能设计

### 3.1 语法级多语言依赖抽取器 (Syntax-Aware Link Discovery)
升级 `LinksEngine::discover_links_from_script`，按行流式扫描：

1. **Python 模块与路径识别**:
   - `import <module>` 或 `from <module> import <symbol>`
   - 映射本地目录：如果 `<module>` 对应 `/root/new/shared_proj/<module>` 或本地同名目录，自动建立 `references` 关系。
   - `sys.path.append("<path>")` 或绝对路径引用，提取行号与代码切片。
2. **Shell / Bash 脚本依赖识别**:
   - `source <path>` 或 `. <path>`
   - `bash <path>` 或 `python3 <path>`
   - 提取被执行或引入的外部脚本，记录精准行号。
3. **服务端口与连接串识别**:
   - 匹配常见端口或连接串（`5432` / `postgresql://`, `6379` / `redis://`, `11434` / `ollama`, `8080`, `9090`）。
   - 关联对应服务实体 `service:<port>`，记录行号。

### 3.2 宿主断链与幽灵依赖体检 (`steward links audit`)
提供健康体检引擎 `LinksEngine::audit_links`：
- **检查项 1: 缺失文件资产 (Missing File Asset)**：
  - 检查所有 `target_type == 'asset'` 的对象，若本地物理路径不存在，标记为 `MISSING_FILE`。
- **检查项 2: 离线服务端口 (Offline Service Port)**：
  - 检查所有 `target_type == 'service'` 的对象（如 `postgresql:5432`），读取 `/proc/net/tcp` 检查端口是否处于 `TCP_LISTEN`。若未在监听，标记为 `OFFLINE_SERVICE`。
- **检查项 3: 悬空记忆引用 (Dangling Memory)**：
  - 检查引用已标记为 `archived` 或删除的记忆。
- **输出报告**:
  - 返回健康分（Health Score 0~100）、断链明细列表与修复建议（如清理废弃链接或重启服务）。

### 3.3 爆炸半径出处增强 (`ImpactAnalyzer`)
- 在输出的 `direct_dependents` 和 `indirect_dependents` 中携带 `line_number`、`snippet` 与 `provenance`。
- Agent 在执行改动前，直接获得精确到行的高亮调用链上下文。

### 3.4 CodeGraph 联邦桥接器 (`CodeGraphBridge`)
提供针对特定目录的微观代码图能力钻取：
- **探测逻辑**:
  - 给定路径 `target_path`，逐级向上查找是否存在 `.codegraph/codegraph.db`。
- **查询功能**:
  1. `detect(path) -> Option<PathBuf>`: 检测代码图数据库是否存在。
  2. `query_symbol_callers(project_root, symbol) -> Result<Vec<CallerSymbol>>`:
     - 直连项目内 SQLite `.codegraph/codegraph.db`，执行关联查询：
       ```sql
       SELECT s.name, s.qualified_name, s.file_path, e.line, e.kind
       FROM edges e
       JOIN nodes t ON e.target = t.id
       JOIN nodes s ON e.source = s.id
       WHERE t.name = ? OR t.qualified_name = ?
       LIMIT 50;
       ```
  3. 当用户通过 CLI 或 MCP 询问“查看该项目的函数调用方”时，无需额外调用复杂的外部进程，直接在毫秒级输出微观调用关系！

---

## 4. MCP 工具与 CLI 接口规范

### 4.1 新增 MCP 工具

#### `steward_links_audit`
- **说明**: 扫描宿主机全局依赖拓扑，检测悬空断链（文件被删、服务离线、无效端口等），输出系统健康报告。
- **参数**:
  - `fix_dangling` (boolean, 可选, 默认 false): 是否自动清除已确认死绝的无效关联边。
- **返回**: 包含健康评分、异常依赖列表及修复建议的 JSON。

#### `steward_codegraph_bridge`
- **说明**: 跨层联动 CodeGraph。检测目标工程是否建有代码图，并查询特定符号的调用方或微观代码依赖。
- **参数**:
  - `path` (string, 必填): 项目工程路径或文件路径。
  - `symbol` (string, 可选): 待查询的函数、类或变量名。若省略则返回该工程的微观图统计信息。
- **返回**: 符号调用链、文件位置及行号列表。

### 4.2 CLI 命令对齐

```bash
# 1. 扫描并审计全局依赖拓扑
agent-steward links audit [--fix]

# 2. 联动 CodeGraph 查看特定文件或符号的调用图
agent-steward links codegraph /root/new/src/hk2 [symbol_name]
```
