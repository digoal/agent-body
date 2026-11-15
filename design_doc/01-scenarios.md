# 01 — 使用场景集（Scenarios）

> Agent Host Steward 产品设计 · 交付件 1/7
> 版本: v1.0 · 更新: 2026-11-15

---

## 一、用户画像

在定义场景之前，先锁定"谁在用"。大管家的目标用户不是所有人，而是 **在本地长期驻留使用 AI Agent 的专业工作者**。

### 画像 A：独立开发者 / 技术博主（"德哥"原型）

- 本机常驻运行 Agent（如 Antigravity、Cursor、Aider 等）
- 有多个项目并行推进，项目间有依赖
- 产出大量文章、脚本、笔记、素材
- 有自己的目录组织规范和文件命名约定
- 运行着 PostgreSQL、DuckDB、Ollama 等多种服务
- 设置了多个定时任务（cron），产出日报/周报/扫描结果
- **核心痛点**：Agent 换了一个会话就"失忆"，不知道我机器上有什么、项目进展到哪了、哪个脚本能做什么

### 画像 B：AI 应用开发团队的工程师

- 在开发机上跑多个 Agent 框架（LangChain、CrewAI、AutoGen 等）
- 需要让不同框架共享环境信息和历史上下文
- 经常切换分支、切换环境，需要 Agent 跟上环境变化
- **核心痛点**：每个框架各自为政，记忆不互通，环境信息靠手动注入 prompt

### 画像 C：运维 / DBA / SRE

- 本机或跳板机上驻留 Agent 做自动化运维
- 管理多台机器的数据库实例、监控系统
- Agent 需要知道"哪台机器上跑着什么版本的什么数据库、配置是什么"
- **核心痛点**：每次让 Agent 帮忙排障，都要先花 10 分钟"教"它当前环境是什么样的

### 画像 D：研究者 / 内容创作者

- 用 Agent 做文献调研、资料整理、写作辅助
- 本机积累了大量论文 PDF、笔记、数据集
- 需要 Agent 能从过去的研究中找到相关素材
- **核心痛点**：Agent 只能搜当前上下文，不能搜我本机几百个 PDF 和几千条笔记

---

## 二、场景矩阵

我按 **触发时机 × 信息维度** 来组织场景，确保覆盖大管家五层能力：

| | 记忆 | 资产 | 环境 | 变更 | 关联 |
|---|---|---|---|---|---|
| **冷启动** | S-01 | S-02 | S-03 | — | S-04 |
| **日常工作** | S-05 | S-06 | S-07 | S-08 | S-09 |
| **跨会话续接** | S-10 | S-11 | — | S-12 | S-13 |
| **异常/排障** | S-14 | — | S-15 | S-16 | S-17 |
| **定时/自动** | S-18 | S-19 | S-20 | S-21 | S-22 |
| **多 Agent 协作** | S-23 | S-24 | S-25 | — | S-26 |

---

## 三、详细场景

### S-01：冷启动 — 初始记忆导入

**画像**: A（德哥原型）
**触发**: 第一次安装大管家

**故事**:
> 德哥装好了 Agent Host Steward。启动后，大管家问：
>
> "检测到你有 Antigravity 的会话历史（3284 条），要不要导入为初始记忆？"
>
> 德哥选了"导入"。大管家花了 2 分钟扫描历史会话，提取了：
> - 142 个关键决策点（"选了 DuckDB 而不是 ClickHouse 做分析引擎，因为嵌入式零依赖"）
> - 67 个任务摘要（"2026-10-20：写了一篇 PG 17 新特性文章"）
> - 23 条用户偏好（"默认用中文回复"、"北京时区"、"脚本放 /root/new/shared_proj/"）
> - 15 条错题录（"SVG 文件要先落盘再引用，不要内联"）
>
> "初始记忆建好了。随时可以查看和修正。"

**价值**: 不是空白开始，而是站在过去所有协作经验上。

---

### S-02：冷启动 — 资产发现

**画像**: A
**触发**: 首次启动后自动触发

**故事**:
> 大管家开始扫描关键目录：
>
> ```
> 发现 12 个项目目录 (/root/new/work/*)
> 发现 34 个脚本/工具 (/root/new/shared_proj/*)
> 发现 8 个克隆的开源项目 (/root/new/src/*)
> 发现 156 篇文章/笔记 (/root/new/tmp/markdown/*, /root/new/note/*)
> 发现 3 个 SKILL 目录 (/root/new/skills/*)
> ```
>
> 对每个资产自动生成摘要标签：
> - `/root/new/shared_proj/earnings_scan/` → 标签: [美股, 财报扫描, SEC EDGAR, Python]
> - `/root/new/src/duckdb/` → 标签: [DuckDB, 源码, C++, 已 codegraph init]
>
> "资产索引完成。共索引 213 个资产，你可以随时查询。"

**价值**: Agent 终于知道"我有什么了"。

---

### S-03：冷启动 — 环境快照

**画像**: B（AI 工程师）
**触发**: 首次启动后自动触发

**故事**:
> 大管家自动采集环境信息：
>
> ```yaml
> OS: Ubuntu 22.04 LTS (kernel 5.15.0)
> CPU: 16 cores (AMD EPYC)
> RAM: 64GB (已用 28GB)
> Disk: 500GB NVMe (剩余 312GB)
>
> 已安装服务:
>   PostgreSQL: 17.2 (running, port 5432, data: /var/lib/postgresql/17/main)
>   DuckDB: 1.2.0 (CLI only)
>   Ollama: 0.7.1 (running, port 11434, models: qwen2.5, nomic-embed-text)
>   Redis: 7.2.4 (running, port 6379)
>   Node.js: 22.11.0
>   Python: 3.12.1 (venv: /root/.venvs/default)
>
> 网络:
>   代理: SOCKS5 on :1080
>   外网连通性: 正常 (延迟 43ms to 1.1.1.1)
>
> 定时任务: 14 个 Hermes cron (12 active, 2 paused)
> ```
>
> "环境快照已建立，后续变更会自动跟踪。"

**价值**: Agent 不用每次都跑 `uname -a` / `pg_config` / `systemctl list-units` 来搞清楚环境。

---

### S-04：冷启动 — 智能关联建立

**画像**: A
**触发**: 前三步完成后自动触发

**故事**:
> 大管家开始建立资产间的关联：
>
> ```
> 关联发现:
> ● /root/new/shared_proj/earnings_scan/ 的 Python 脚本依赖 PostgreSQL (连接 port 5432)
> ● /root/new/src/duckdb/ 的 codegraph 数据引用了 /root/new/work/ 下的分析笔记
> ● Hermes cron "daily-finance" 输出到 /root/new/cron/daily-finance/
> ● Hermes cron "db-digest" 读取 PostgreSQL 实例的 pg_stat_* 视图
> ● 笔记 /root/new/note/权威网站.md 被 3 个 SKILL 引用
> ```
>
> 这些关联被存为图结构。Agent 以后问"我的财报扫描脚本还能跑吗"，大管家可以沿着关联链检查：脚本在→Python 环境 OK→PG 服务在跑→端口可达→"能跑"。

**价值**: 信息不再是割裂的四个数据库，而是一张有机关联网。

---

### S-05：日常 — 记忆辅助决策

**画像**: A
**触发**: 工作中 Agent 需要历史决策信息

**故事**:
> 德哥：
> "我想给 DuckDB 源码分析写第三篇文章，前两篇写了什么来着？"
>
> Agent 调大管家的记忆 API：
>
> ```json
> GET /memory/search
> {
>   "query": "DuckDB 源码分析文章",
>   "type": "task",
>   "limit": 5
> }
> ```
>
> 大管家返回：
> - 第一篇 (2026-09-15): "DuckDB 存储引擎解析"，发到了公众号，阅读量 3200
> - 第二篇 (2026-10-08): "DuckDB 向量化执行引擎"，配了 4 张 SVG 架构图
> - 当时规划的第三篇主题是"DuckDB 优化器"，但因为插入了财经文章任务被搁置
> - 上次决定的写作风格是"源码 + 实操验证 + SVG 图解"
>
> Agent 不用问德哥就知道该接着写什么、用什么风格。

---

### S-06：日常 — 资产检索

**画像**: D（研究者）
**触发**: 需要找到本机的某个素材

**故事**:
> 用户：
> "我之前下载过一篇关于 RAG 评估方法的论文，找不到了。"
>
> Agent 调大管家：
>
> ```json
> GET /assets/search
> {
>   "query": "RAG 评估方法 论文",
>   "type": "file",
>   "file_type": "pdf"
> }
> ```
>
> 大管家返回：
> - `/root/new/src/papers/ragas-evaluation-framework.pdf` (2026-08-12 下载, 22 页)
> - `/root/new/note/rag-eval-notes.md` (2026-08-14, 你读完后写的笔记)
> - 关联记忆: "2026-08-14 讨论过 RAGAS vs DeepEval，最终选了 RAGAS 因为指标更全"

**价值**: 找资料从"翻遍文件系统"变成"语义搜索 + 关联推荐"。

---

### S-07：日常 — 环境感知辅助

**画像**: C（DBA/运维）
**触发**: Agent 需要知道当前环境状态

**故事**:
> 用户：
> "帮我检查一下 PG 的 shared_buffers 够不够"
>
> Agent 不需要先问"你的 PG 装在哪"、"配置文件在哪"。它直接调大管家：
>
> ```json
> GET /env/service/postgresql
> ```
>
> 返回：
> ```json
> {
>   "version": "17.2",
>   "status": "running",
>   "port": 5432,
>   "data_dir": "/var/lib/postgresql/17/main",
>   "config_file": "/etc/postgresql/17/main/postgresql.conf",
>   "key_params": {
>     "shared_buffers": "4GB",
>     "work_mem": "256MB",
>     "effective_cache_size": "32GB"
>   },
>   "host_ram": "64GB",
>   "current_shared_buffers_usage": "3.2GB (80%)"
> }
> ```
>
> Agent 综合判断："shared_buffers 是 4GB，主机 64GB 内存，占比 6.25%。建议提到 16GB（25%），但注意 effective_cache_size 已经设了 32GB，需要确认 OS 有足够的 page cache 空间。"
>
> **全程零次手动注入环境信息。**

---

### S-08：日常 — 变更感知

**画像**: A
**触发**: 环境发生了变化

**故事**:
> 德哥在另一个终端里跑了 `pip install duckdb --upgrade`。
>
> 大管家检测到变更，主动记录：
> ```
> [2026-11-15 10:30:22] CHANGE DETECTED
> Type: package_upgrade
> Package: duckdb (Python)
> From: 1.1.3 → To: 1.2.0
> Env: /root/.venvs/default
> Affected assets:
>   - /root/new/shared_proj/duckdb_bench/ (imports duckdb)
>   - /root/new/work/agent-steward/ 的数据模型文档提到了 DuckDB
> ```
>
> 下次 Agent 启动新会话时，大管家主动推送：
> "注意：上次会话后，duckdb Python 包升级到了 1.2.0，你 shared_proj 里的 duckdb_bench 脚本可能需要测试兼容性。"

**价值**: 环境变了，Agent 知道；不会拿着旧信息做决策。

---

### S-09：日常 — 关联推理

**画像**: A
**触发**: 用户的问题需要跨维度信息

**故事**:
> 德哥：
> "我上周写的那个财报扫描脚本，现在还能跑吗？"
>
> 这个问题涉及四个维度：
> 1. **记忆**: 上周写了什么脚本？→ `earnings_scan/scan_sec.py`
> 2. **资产**: 脚本在不在？→ 在，`/root/new/shared_proj/earnings_scan/scan_sec.py`
> 3. **环境**: 依赖满足吗？→ Python 3.12 ✓, requests ✓, psycopg2 ✓, PG running ✓
> 4. **变更**: 有没有什么变了？→ 上周到现在 SEC 的 API 返回格式变了（变更日志里有一条 cron 报错）
>
> 大管家综合回答：
> "脚本在，依赖都满足。但注意：11 月 12 日的 cron 执行日志显示 SEC EDGAR 的 `company-search` 接口返回格式变了，`scan_sec.py` 的第 87 行解析会报 KeyError。建议先修复再跑。"

**价值**: 这就是关联的力量。单独问记忆、资产、环境都得不到这个答案。

---

### S-10：跨会话 — 任务续接

**画像**: B（AI 工程师）
**触发**: 新会话需要接上旧任务

**故事**:
> 工程师昨天让 Agent 帮忙做 API 性能测试，做到一半下班了。今天打开新会话：
>
> "继续昨天的性能测试。"
>
> Agent 调大管家：
> ```json
> GET /memory/tasks/latest
> {
>   "query": "性能测试",
>   "status": "in_progress"
> }
> ```
>
> 返回：
> ```json
> {
>   "task_id": "task-2026-11-14-perf-test",
>   "title": "API 性能测试 - /api/v2/search 端点",
>   "started": "2026-11-14T14:30:00+08:00",
>   "progress": "60%",
>   "completed_steps": [
>     "✅ 编写了 k6 测试脚本 → /root/new/work/api-perf/load_test.js",
>     "✅ 跑了 10 并发基准测试 → p99: 450ms",
>     "✅ 跑了 50 并发压测 → p99: 1200ms，发现瓶颈在 DB 查询"
>   ],
>   "next_steps": [
>     "⬜ 分析慢查询 SQL，尝试加索引",
>     "⬜ 100 并发压测",
>     "⬜ 生成报告"
>   ],
>   "related_files": [
>     "/root/new/work/api-perf/load_test.js",
>     "/root/new/work/api-perf/baseline_result.json",
>     "/root/new/work/api-perf/50_concurrent_result.json"
>   ]
> }
> ```
>
> Agent 直接接上：
> "好的，昨天做到 50 并发，发现瓶颈在 DB 查询。我先看看慢查询 SQL……"

**价值**: 零成本跨会话任务续接。不用用户重述，不用 Agent 猜。

---

### S-11：跨会话 — 资产变更追踪

**画像**: A
**触发**: 不同会话中资产发生了变化

**故事**:
> 德哥上午在会话 A 里克隆了一个新项目：
> `git clone https://github.com/example/new-vector-db.git /root/new/src/new-vector-db`
>
> 大管家自动注册这个新资产，打标签、跑 codegraph init。
>
> 下午在会话 B 里：
> "那个新克隆的向量数据库项目，架构图跑出来了吗？"
>
> 大管家回答：
> "已注册。codegraph init 在 14:22 完成，生成了 2,341 个符号索引。项目结构：Rust 核心引擎 + Python binding + HTTP server。"

**价值**: 会话 A 做的事，会话 B 自动知道。

---

### S-12：跨会话 — 变更摘要

**画像**: A
**触发**: 新会话开始时

**故事**:
> 德哥早上打开新会话。Agent 主动推送大管家的变更摘要：
>
> "距上次会话（昨晚 22:15）以来发生了以下变化：
> 1. Hermes cron `daily-finance` 凌晨 6:00 正常执行，输出了今日财经简报
> 2. Hermes cron `db-digest` 凌晨 3:00 执行失败 — 原因：PG 连接超时（看起来 PG 在 2:58 做了一次自动 vacuum，占满了连接）
> 3. 系统自动更新了 3 个包：openssl 3.1.4→3.1.5, curl 8.5.0→8.6.0, git 2.43.0→2.43.1
> 4. 磁盘使用从 188GB 增加到 194GB（+6GB，主要是 cron 输出和日志）
>
> 需要关注的：db-digest cron 失败，要不要我看看？"

**价值**: 每天开工就有 briefing，不会遗漏昨晚发生的事。

---

### S-13：跨会话 — 关联上下文恢复

**画像**: A
**触发**: 上一次会话的决策影响了当前操作

**故事**:
> 上周德哥在某个会话里决定："以后 SVG 图统一存到 `svg/` 子目录，不再内联到 markdown 里。"
>
> 这周新会话里写文章，Agent 生成了一张 SVG 图。
>
> 大管家拦截检查（或 Agent 主动查询偏好）：
> "根据 11 月 8 日的决策（context: conv-xxxx），SVG 应存到 `svg/` 子目录再引用，不要内联。"
>
> Agent 自动遵循这个偏好。

**价值**: 偏好和约定不随会话消亡。

---

### S-14：排障 — 历史经验召回

**画像**: C（DBA）
**触发**: 遇到类似之前处理过的问题

**故事**:
> 用户：
> "PG 的 WAL 堆积到了 200GB，怎么回事？"
>
> Agent 调大管家搜索历史经验：
> ```json
> GET /memory/search
> {
>   "query": "WAL 堆积 PostgreSQL",
>   "type": "experience"
> }
> ```
>
> 返回两条：
> 1. "2026-07-20：WAL 堆积 150GB，原因是 replication slot 的 consumer 挂了。解决：drop 掉挂掉的 slot，WAL 立即回收。"
> 2. "2026-09-03：WAL 堆积 80GB，原因是 archive_command 失败但没告警。解决：修复归档路径，WAL 逐步回收。"
>
> Agent 综合历史经验 + 当前环境（大管家知道这台机器有 3 个 replication slot）：
> "先检查 replication slot 状态，上次是 slot consumer 挂了导致的……"

**价值**: 不只是"记得做过"，而是"记得怎么做的、为什么那样做"。

---

### S-15：排障 — 环境对比诊断

**画像**: C
**触发**: "之前好好的，突然不行了"

**故事**:
> 用户：
> "昨天跑得好好的脚本，今天报 ImportError 了。"
>
> Agent 调大管家的变更日志：
> ```json
> GET /changes/since
> {
>   "since": "2026-11-14T00:00:00+08:00",
>   "scope": "python_packages"
> }
> ```
>
> 返回：
> ```
> [2026-11-14 23:45] pip install --upgrade numpy (1.26.2 → 2.1.0)
> [2026-11-14 23:45] numpy 2.1.0 dropped numpy.distutils (deprecated since 1.23)
> ```
>
> Agent：
> "找到了。昨晚 numpy 从 1.26 升到了 2.1，numpy 2.x 删除了 numpy.distutils 模块。你的脚本如果用了这个模块就会报 ImportError。要回退到 1.26 还是改代码？"

**价值**: "什么变了"是排障的第一问题。变更日志直接回答。

---

### S-16：排障 — 变更影响分析

**画像**: B
**触发**: 准备做一个变更，需要评估影响

**故事**:
> 工程师：
> "我想把 Redis 从 7.2 升到 7.4，会影响什么？"
>
> Agent 调大管家：
> ```json
> GET /links/dependents
> {
>   "asset_type": "service",
>   "name": "redis",
>   "version": "7.2"
> }
> ```
>
> 返回依赖 Redis 的所有资产：
> - `/root/new/work/session-cache/` 项目 — 用了 Redis Stream，7.4 兼容
> - `/root/new/shared_proj/rate-limiter/` 脚本 — 用了 Redis Lua scripting，7.4 兼容
> - Hermes cron 的 session 存储 — 直接依赖 Redis，需要确认 key 格式没变
>
> "升级 Redis 7.4 应该安全，但建议先确认 Hermes session 存储的兼容性。要不要我先在测试环境试一下？"

**价值**: 变更前的影响评估，靠的是关联图谱。

---

### S-17：排障 — 跨维度根因分析

**画像**: C
**触发**: 复杂问题需要关联多个线索

**故事**:
> 用户：
> "今天的 daily-finance cron 输出质量很差，数据都是昨天的。"
>
> Agent 调大管家做关联分析：
>
> 1. **记忆**: daily-finance cron 的正常行为是什么？→ 每天 6:00 跑，调用 web_search 获取最新新闻
> 2. **变更**: 今天的执行日志有什么异常？→ 6:02 有 3 个 web_search 调用超时
> 3. **环境**: 网络状态？→ 代理 (SOCKS5:1080) 今天 5:50-6:10 之间不可达
> 4. **关联**: 代理不可达是因为什么？→ 变更日志显示 5:48 系统自动重启了 v2ray 服务（OOM killed）
>
> 根因链：
> **v2ray OOM → 代理中断 20 分钟 → web_search 超时 → cron 用了缓存数据 → 输出质量差**
>
> "根因是 v2ray 在 5:48 被 OOM killed，代理中断导致新闻抓取失败。建议：1) 给 v2ray 加内存限制；2) cron 加代理健康检查前置。"

**价值**: 单一维度找不到根因，关联分析才能串起来。

---

### S-18：自动 — 记忆自动维护

**画像**: A
**触发**: 定时任务

**故事**:
> 大管家每天凌晨自动执行记忆维护：
>
> ```
> [Memory Maintenance 2026-11-15 03:00]
>
> 衰减:
>   - 将 45 条超过 90 天未引用的普通记忆权重降低
>   - 3 条记忆权重降至 0，移入归档（可搜但不主动召回）
>
> 强化:
>   - "SVG 先落盘再引用" 这条规则被引用了 12 次，权重 → MAX
>   - "脚本放 /root/new/shared_proj/" 这条偏好被引用了 28 次，权重 → MAX
>
> 合并:
>   - 检测到 3 组重复/近似记忆，合并为 1 条
>
> 冲突检测:
>   - 记忆 #1024 说"DuckDB 不支持窗口函数"，但当前版本 (1.2.0) 已支持 → 标记为过时
>
> 摘要: 维护完成，活跃记忆 1,247 条，归档 89 条，标记过时 1 条。
> ```

**价值**: 记忆不是只增不减的垃圾堆，有自我清洁能力。

---

### S-19：自动 — 资产健康巡检

**画像**: A
**触发**: 每周定时

**故事**:
> ```
> [Asset Health Check 2026-11-15 04:00 Weekly]
>
> 孤儿文件:
>   - /root/new/tmp/test_output_20261003.json (42天未访问, 3.2GB) → 建议清理
>   - /root/new/tmp/debug_*.log (共 15 个, 合计 890MB) → 建议清理
>
> 陈旧项目:
>   - /root/new/src/old-rag-experiment/ 超过 180 天未动 → 建议归档或删除
>
> 磁盘预警:
>   - 按当前增长速率 (约 800MB/天)，磁盘将在 145 天后用完
>   - 可立即回收空间: 4.1GB (上述孤儿文件 + 临时文件)
>
> Git 仓库状态:
>   - /root/new/src/duckdb/: 落后上游 47 commits
>   - /root/new/shared_git/blog/: 有 3 个未推送的 commit
> ```

**价值**: 被动的文件系统变成有人维护的资产库。

---

### S-20：自动 — 环境漂移检测

**画像**: B
**触发**: 定时对比环境快照

**故事**:
> 大管家每天对比当前环境和上次快照：
>
> ```
> [Environment Drift 2026-11-15 05:00]
>
> 与昨日对比:
>   🟡 PostgreSQL: shared_buffers 从 4GB 变为 2GB
>     → 原因: /etc/postgresql/17/main/postgresql.conf 在 14:22 被修改
>     → 上下文: 昨天会话 conv-xxx 中讨论过调整参数，但记忆显示商定的是 8GB 而不是 2GB
>     → ⚠️ 疑似误操作
>
>   🟢 Python: 新增包 httpx 0.27.0
>   🟢 磁盘: 变化在正常范围
>   🔴 服务: Ollama 未运行 (上次活跃: 2026-11-14 22:30)
>     → 原因: 进程被 OOM killed
>     → 建议: 重启 + 调整内存限制
> ```

**价值**: 环境漂移是沉默的杀手。自动检测比出了事再排查好一百倍。

---

### S-21：自动 — 变更日志归档

**画像**: A
**触发**: 每周自动

**故事**:
> 大管家将本周的变更日志压缩归档：
>
> ```
> [Changelog Archive 2026-11-15 05:30 Weekly]
>
> 本周变更统计:
>   文件变更: 1,247 次 (创建 89, 修改 1,034, 删除 124)
>   包变更: 3 次 (升级 2, 新装 1)
>   服务事件: 5 次 (重启 3, crash 1, config change 1)
>   cron 执行: 98 次 (成功 94, 失败 3, 超时 1)
>
> 归档: /root/new/work/agent-steward/changelogs/week-2026-46.jsonl.gz (2.3MB)
> 原始日志已清理，可通过归档搜索。
> ```

---

### S-22：自动 — 关联图谱更新

**画像**: A
**触发**: 资产或环境变更时

**故事**:
> 德哥新建了一个项目 `/root/new/work/steward-prototype/`，在里面写了一个 Python 脚本引入了 duckdb 和 psycopg2。
>
> 大管家自动分析并更新关联图：
> ```
> 新增关联:
>   steward-prototype → depends_on → duckdb (Python, 1.2.0)
>   steward-prototype → depends_on → PostgreSQL (via psycopg2, port 5432)
>   steward-prototype → created_in → conv-xxx (2026-11-15 的会话)
>   steward-prototype → related_to → agent-steward (同一工作空间，设计文档)
> ```

---

### S-23：多 Agent — 记忆共享

**画像**: B（AI 工程师）
**触发**: 不同 Agent 框架需要共享上下文

**故事**:
> 工程师在 Antigravity 里做了一个技术决策：选用 FastAPI 做 API 层。
>
> 切换到 Cursor 继续写代码。Cursor 的 Agent 调大管家：
> "当前项目的技术栈决策有哪些？"
>
> 大管家返回：
> - API 框架: FastAPI (决策时间: 今天 10:30, 来源: Antigravity 会话)
> - 数据库: PostgreSQL 17 (决策时间: 昨天, 来源: Antigravity 会话)
> - 认证: JWT (决策时间: 昨天, 来源: Antigravity 会话)
>
> **Cursor 的 Agent 不需要重新问用户"你打算用什么框架"。**

**价值**: 大管家是 Agent 之间的"共享大脑"。

---

### S-24：多 Agent — 资产协调

**画像**: A
**触发**: 多个 Agent 操作同一资产

**故事**:
> Hermes cron Agent 正在写 `/root/new/cron/daily-finance/output.md`。
>
> 同时，Antigravity Agent 收到德哥指令："帮我读一下今天的财经简报。"
>
> 大管家检测到冲突：
> "daily-finance cron 正在写这个文件（进度 70%），建议等 2 分钟完成后再读取，或读取昨天的版本。"
>
> Agent："cron 还在写今天的财经简报，大约 2 分钟后完成。要不先看昨天的？"

**价值**: 资产级别的并发协调，避免读到半成品。

---

### S-25：多 Agent — 环境独占/共享

**画像**: B
**触发**: 多个 Agent 需要使用同一资源

**故事**:
> Agent A 正在用 GPU 跑模型推理。
>
> Agent B 想启动另一个推理任务。
>
> 大管家：
> "GPU 0 当前被 Agent A 的推理任务占用（显存使用 22GB/24GB），预计 5 分钟后完成。Agent B 的任务需要 18GB 显存，无法并行。建议排队。"
>
> Agent B 的响应：
> "GPU 正在被另一个任务使用，我帮你排在后面，大约 5 分钟后开始。"

---

### S-26：多 Agent — 关联一致性

**画像**: B
**触发**: 一个 Agent 的操作改变了关联关系

**故事**:
> Agent A 删除了一个 Redis 缓存服务（因为切换到了内存缓存方案）。
>
> 大管家立即更新关联图，并通知所有订阅了 Redis 变更的 Agent：
> "Redis 服务已被停止。以下资产受影响：
> - `/root/new/work/session-cache/` — 硬依赖 Redis，需要修改
> - cron `rate-limit-check` — 依赖 Redis，下次执行会失败"
>
> Agent B（管 cron 的）收到通知，自动暂停 `rate-limit-check` cron 并通知用户。

---

## 四、场景优先级分类

根据用户痛点强度和实现复杂度，将场景分为三个阶段：

### P0 — MVP 必须有（解决最痛的痛点）

| 场景 | 理由 |
|------|------|
| S-02 资产发现 | 解决"Agent 不知道我有什么"的根本痛点 |
| S-03 环境快照 | 解决"每次都要手动告诉 Agent 环境信息"的痛点 |
| S-05 记忆辅助决策 | 跨会话记忆是用户的第一诉求 |
| S-06 资产检索 | "我有个文件找不到了"是高频需求 |
| S-07 环境感知辅助 | 让 Agent 直接知道环境，不再需要手动注入 |
| S-10 任务续接 | 跨会话最高频的场景 |

### P1 — 第二阶段（显著提升体验）

| 场景 | 理由 |
|------|------|
| S-01 记忆导入 | 冷启动体验的关键 |
| S-08 变更感知 | 环境变化感知很有价值但实现较重 |
| S-12 变更摘要 | "早上开工 briefing"很有仪式感和实用性 |
| S-14 历史经验召回 | 对运维/DBA 场景价值很高 |
| S-15 环境对比诊断 | "什么变了"是排障第一问题 |
| S-18 记忆自动维护 | 防止记忆库变成垃圾场 |
| S-19 资产健康巡检 | 主动维护比被动响应好 |

### P2 — 第三阶段（护城河功能）

| 场景 | 理由 |
|------|------|
| S-04 智能关联建立 | 关联是核心价值但也最难做 |
| S-09 关联推理 | 需要关联图谱成熟后才有价值 |
| S-16 变更影响分析 | 需要完整的关联图 |
| S-17 跨维度根因分析 | 最高级能力，需要所有层配合 |
| S-20 环境漂移检测 | 需要长期快照积累 |
| S-23~S-26 多 Agent 系列 | 需要生态成熟 |

---

## 五、场景补充：边界与非场景

以下是大管家 **不做** 的事情，明确边界：

1. **不做 Agent 框架** — 大管家不运行 Agent，它只为 Agent 提供基础设施
2. **不做 LLM 调用** — 大管家本身不调 LLM 做推理（自身可以用轻量 LLM 做摘要/分类，但不处理用户任务）
3. **不做远程机器管理** — 大管家只管本机（多机扩展是未来的事）
4. **不做安全审计** — 大管家可以提供信息，但不做入侵检测、漏洞扫描
5. **不做备份恢复** — 大管家不是备份工具，它只索引和关联
6. **不替代操作系统** — 大管家是 OS 之上的认知层，不替代 OS 的资源管理

---

*下一步: 02-requirements.md — 从场景提炼功能性和非功能性需求*
