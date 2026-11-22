## 未来演进与待探讨 ISSUE (Roadmap & Open Issues)

1. **分布式/多机 Agent Steward 跨主机网状协调 (Gossip / Raft / HTTP Mesh)** `✅ Phase 18 已落地轻量 HTTP Peer Mesh`：
   - 当宿主环境扩展为多台机器构成的集群时，将协作锁与事实图谱提升为跨节点分布式网状协调。

2. **智能死锁检测与等待唤醒机制 (Wait-Queue / Push Notification)** `✅ Phase 17 已落地 WFG + Wait-Queue`：
   - 为排他锁提供可订阅的等待队列，当持锁 Agent 释放资源时，主动通知排队中的 Agent，避免轮询开销。

3. **跨平台原生适配深化 (macOS / Windows)** `✅ Phase 18 已落地 PlatformProbe 抽象`：
   - 当前以 Linux（/proc、inotify、Systemd）为首选核心运行环境，未来进一步拓展对非 Linux 平台的底层环境感知。
  
4. **管理范围的扩大** `✅ Phase 16 已落地 Ecosystem & Context 防爆路由`：
   - 还得帮 Agent 管 MCP, SKILL, 可以使用的外部服务等。因为 MCP、SKILL 多了之后啊，Context 容易被打爆，导致大模型注意力下降，影响模型生成质量。 管外部服务的目的则是 Agent 宿主机本机环境的延展，就像销售人员有一本外部联系人名单一样，随时可以按需来 call。
  
5. **Web UI 显示内容优化与交互重构** `✅ Phase 19 已落地 Web UI 2.0 五大工作台与真实拓扑`：
   - Web UI 显示的内容有点简陋，也不美观。需要重新设计一下。

6. **黑屏功能和白屏功能双向同步** `✅ Phase 21 已落地 39 工具 100% Parity 同步（Phase 28 已扩展至 40/40）`：
   - 黑屏功能和白屏功能要同步，白屏主要体现在WEB端，黑屏体现在 MCP,SKILL,CLI 侧（已升格为开发规约 #16）。

7. **WEB 侧路演与对外分享演示剧场** `✅ Phase 22 已落地「🎬 路演演示剧场」与第 39 工具`：
   - 要在WEB侧额外设计更多演示功能，路演和对外分享时使用，从产品经理和市场营销推广的角度来挑选哪些场景需要被拿来演示（已升格为开发规约 #17）。

8. **元数据定义哪些 Key 与本地 Ollama 小模型自动标注** `✅ Phase 23 已落地三层正交元数据体系与 Ollama 异步双轨标注`：
   - 定义 **L1 通用语义核心 (`purpose`/`domain`/`capabilities`/`entry_point`/`usage_hint`) + L2 类型特化 Facet + L0 管线溯源 (`content_hash`/`annotation_status`/`annotation_model`)** 三层元数据 Key 体系；
   - 采用 **L1 Rust 微秒级确定性语法提取 + L2 本地 Ollama 小模型（`qwen3.5:0.8b`）脱敏异步精炼** 双轨流水线，配合 **Micro (~25 tok) / Standard (~85 tok) / Full (~180 tok)** 三级渐进式 Token 披露，彻底告别 `{"watcher_sync":true}` 空洞元数据且零云端 Token 消耗。

9. **「比较优势」立意源起与 PostgreSQL 调研痛点补完** `✅ Phase 24 已落地 Idea 起源篇与双 HTML 演说融合`：
   把为什么要做 agent-steward 项目的以下这些意境和萌芽态的想法，整理后补充到 Idea 文档和路演、演讲 HTML 里。
```
我看到硅谷 101采访一位硅谷创业者，这位创业者居然在办公区（负一楼）将近一周都沉浸在 coding 中，原因是这个园区的物业把创业者的琐事都办了，创业者足不出户，沉浸在主业 coding 中。
企业的高管基本上都会配置独立或共享的秘书，把琐事办掉，高管专一的干本职工作，发挥最大能效。
家庭也一样，很多家庭家庭主妇把老公的一切都包了，老公专心工作赚钱。
这些都是很好的互相搭配，每个人都把主要精力放在最本职、最专长的事情上，有个经济学术语叫比较优势。
Agent 呢，我用久了 Agent 后发现，Agent 缺个秘书或者说大管家。为什么呢？比如我经常用 Agent 调研 PostgreSQL 的代码、功能、或应用场景等，每次给 Agent 提交任务，它都要重新拉取代码、文档，然后编译，安装，验证等。其实本机已经有了环境，或以前已经下载好了代码或文档，素材都是现成的，但 Agent 不知道，如果能给 Agent 配个小秘书或大管家，先问小秘书，这事不就解决了吗？也不会浪费时间、浪费空间折腾。Agent 的效率高了，也省 token 了。
```

10. **路演与演讲 HTML「地层剖面 / 工程蓝图」重构与全功能自检** `✅ Phase 24 已落地 16 页路演版与 22 页演讲版双 HTML + Playwright 自检全绿`：
    路演文档不够美观，也没有配演讲稿，可以改进一下：
````
生成单个 html 格式的演说文件(文件名不要出现中文字符)，请都放到一个文件中包括图片你可以用 base64 嵌入。产生的 html 最重要的是内容的正确性、并且是图文并茂的。一图胜千言，图的目的是帮助受众理解内容(原理图、架构图、使用场景图、最佳实践图等等)。图你可以现场绘制或使用原文中的，图要求美观且易于理解，图中表达的任何模块或文字必须在视觉上无任何遮挡、出界、交叉等问题。整个 slide 还可增加一定的动画演示（得有输入、播放等配套按钮），但只在对受众理解内容有帮助时增加。html 中还可以添加一下快捷键，必含按键“目录 G、本页讲稿 N、全文讲稿 T、自动播放、全屏 F”，其它按键按需提供，全屏后按键也要在页面底部显示。讲稿内容要注重逻辑推演和实证，口语化，逻辑要连绵不绝。讲稿不要弹窗打开，而是采用页内开窗，按一下显示，再按一下关闭，循环，在页面右侧显示，显示讲稿时原页面内容矢量缩放到左侧展示，按 T 显示全部讲稿时自动定位到当前 page，右侧显示讲稿时翻页时也进行联动自动滚动右侧演讲稿到当前页面。
html 内容16比9的比例，采用适合阅读的视觉风格。
```
【视觉风格：地层剖面 / 工程蓝图】
浅色主题，深字浅底。
色系：底 #F7F6F2 暖纸白、次级底 #F1EFE9；墨 #1B2B33、次级字 #46585F、
弱化字 #89938F、描边 #D9D5CB。主色 深靛青 #245A6B（浅 #E5EFF0），
强调 陶橙 #BE653B（浅 #F7E9DE），收益 青绿 #2F7A66，提示 金 #AE8A3E。
地层七阶（深→浅）#1E4B5A #2E6373 #47808D #74A5AD #A4C4C6 #CFE0DC #EBF1EE，
越深＝越底层。语义：靛青＝结构/机制，陶橙＝矛盾/代价/承接，青绿＝正确动作，金＝过渡。
字体：标题宋体 Songti SC / Noto Serif CJK SC，正文 PingFang SC / Noto Sans CJK SC，
数字页码 SF Mono；中文标点用「」『』——。
版式：16:9 固定舞台 1600×900，等比缩放 + letterbox，禁止响应式重排；左边距 64px，
眉标 13px/字距3.4px，标题 44px 宋体，导语 19px，1px 分隔线，图区 1400×430，
底部「承接 · NEXT」20px 宋体 + 3px 陶橙左边框，页脚等宽页码。
图形语法（硬规则）：所有示意图现场绘制为内联 SVG，不用外链位图；规整网格／分层横带；
连线只允许水平或竖直直角，禁止斜线穿过文字；文字与连线分离、框内留白 ≥14px、
按框宽自动缩字号，保证零出界／零遮挡／零交叉；一页一图一命题。
禁止：紫粉渐变、emoji、圆角卡片堆叠、滥用投影、装饰性图形、编造统计数字。
交付：单个 HTML 零外部依赖；
```  
注意，交付之前请自检 html 的各项功能是否都正常：内置动画是否正常、按键是否正常、显示讲稿后画面是否完整、缩放后的画面显示是否正常且完整、全文讲稿页面定位是否正确等。
````
  
11. **WEB UI 美化与白天/夜间双色调实时切换** `✅ Phase 25 已落地「☀️ 白天蓝图 / 🌙 夜间深空」双主题系统与面板美化`：
    - 彻底清除硬编码暗色值，建立完整的 `[data-theme="light"]`（暖纸工程蓝图 `#F7F6F2` + 深靛青 `#245A6B` + 陶橙 `#BE653B`，与路演/演讲 HTML 品牌同源）与 `[data-theme="dark"]`（深空指挥舱）两套语义 CSS Token 体系；
    - 顶栏提供一键切换按钮（支持 `T` 快捷键与 `localStorage` 记忆），切换瞬间同步重绘 Canvas 拓扑图配色，并将端口防撞、Pre-Hook、放置推导、DuckDB 热力图与趋势、宿主 GC 等面板升级为「可视化卡片 + 可折叠原始 JSON」。
  
12. **WEB UI「路演演示剧场」三层故事化直观重构** `✅ Phase 25 已落地三层渐进式故事舞台与 6 大专属可视化组件`：
    - 告别主舞台大段裸露 JSON 调试框：顶部增设经济学「比较优势」立意导览与 6 幕场景摘要卡片；
    - 每幕由真引擎返回 **💡 一句话生活比喻 (`analogy`) + 🎯 核心用意 (`intent_summary`) + 3 张高冲击 ROI 指标卡 (`hero_metrics`)**，左屏呈现盲人 Agent 三步翻车故事线，右屏渲染 **6 套专属可视化组件**（黑白名单阻断漏斗+武器库墙、Pre-Hook 1ms 红绿灯、3 跳蝴蝶效应因果链、双 Agent 死锁环路图、Token 99.9% 对比尺、1 秒简报仪表+安全自洁盾），原始 JSON 收纳至底部默认折叠的技术专家核验抽屉。  

13. **实证 Benchmark 对照评测体系与生产效能存证账本（确保使用效果有据可查）** `✅ Phase 27~28 已落地双轨实证引擎、PR-01~08 生产账本与 40/40 黑白屏同步`:
    - 构建「实验室 A/B 可复现对照基准（8 大标准宿主工作负载：Without Steward 裸奔物理探针基线 vs With Steward 真引擎实测 + 5 大内核微基准 SLA）」与「生产运行态每次调用效能存证账本（Token 节省、轮次缩减、端口/锁/断链/乱放冲突拦截、黑名单噪音阻断实时带出处入账）」双轨闭环；
    - 通过第 40 个原生 MCP 工具 `steward_benchmark`、CLI `agent-steward bench` 与 Web UI「📊 实证基准与账本」工作台实现 40/40 黑白屏双向同步与 SHA-256 存证签名，让每一项收益均可量化、可复现、可审计（已升格为开发规约 #18）。

14. **实际使用反馈闭环与痛点持续治理 (`user_feedback.md` & Feedback-to-Memory Loop)** `✅ Phase 30 已落地`:
    - 建立 [user_feedback.md](user_feedback.md) 作为主理人与 Agent 实际使用体验的一手反馈总账（覆盖 `PRO-XX` 优点、`CON-XX` 痛点、`BENCH-XX` 评测失真与 `REQ-XX` 新需求），打通「使用反馈 → 根因修复 → 记忆沉淀 → 回归门禁」双闭环；
    - Phase 30 已攻克现网实测暴露的 `steward_brief` 474ms 冷启动瓶颈、`LinksEngine` 76.2% 假阳性断链、`effectiveness_ledger` 全渠道记账漏失、`memories` 长期记忆饥饿与 91GB `target/` 满盘预警问题。

15. **Benchmark 标准持续迭代机制 (`Benchmark Spec v1.0 → v2.0+ Six-Dimension Matrix`)** `✅ Phase 31 已落地`:
    - 引入 `spec_version` 基准协议版本化，从算术总耗时比演进为**几何平均加速比（Geometric Mean Speedup）+ 分组加速比**，新增 **D6 认知质量与准确率维度 (`Recall@K / Precision / 覆盖率`)**；
    - 完成 `W2/W3/W4/W7` 物理等价基线校准，并上线 `W9`（向量+属性混合检索召回）、`W10`（模板抗假阳性回归）与 `DQ1~DQ3`（现网宿主机数据健康度）基准负载（详见 [roadmap-phase30.md](src_doc/roadmap-phase30.md)）。

16. **宿主大管家安全纵深防御、全链路脱敏与抗投毒体系演进 (`design_doc/11-security-spec.md` & Phase 32~33)** `✅ Phase 32~33 已落地`:
    - 补齐安全架构规格（[11-security-spec.md](design_doc/11-security-spec.md)）与顶层安全规约 #19，围绕代码审计暴露的 `SEC-GAP-01~05` 完成全链路 `SecretRedactor`、Hook/DuckDB 防注入、Web 全量写鉴权、`PromptInjectionScanner` 抗投毒、`0600/0700` 权限收敛与 `W11`/`DQ4` 安全回归基准落地（详见 [roadmap-phase32.md](src_doc/roadmap-phase32.md)）。
  
17. **零重扫项目演进指南与架构防漂移门禁 (`EVOLUTION_GUIDE.md` + 自建 CodeGraph + 清单自检)** `✅ Phase 34 已落地`:
    - 建立方便后续项目演进的零重扫导航体系，避免新会话每次重复扫描 25 个模块（3.8 万行源码）和 48 份历史 `src_doc/` 文档；
    - Phase 34 已落地四位一体防重扫体系：根目录 O(1) 演进活地图 [EVOLUTION_GUIDE.md](EVOLUTION_GUIDE.md)（8 大演进场景变更触点矩阵 + `Tier-0/1/2` 文档分级禁扫令）、本仓 `.codegraph/` 符号地图、确定性架构检查脚本 `scripts/evolution_manifest.py`（`make evolution-check`）及 `SKILL` 零重扫开工协议（详见 [roadmap-phase34-35.md](src_doc/roadmap-phase34-35.md)）。
  
18. **热门开源竞品横向对标与 Benchmark `v3.0` 轨道 C (`Horizontal Competitor Benchmark`)** `✅ Phase 35 已落地`:
    - Benchmark 不仅和裸奔自己比，也与外部主流产品同场实测比对；
    - Phase 35 已构建四大赛道竞品对标体系：**C1 长期记忆**（Mem0 / Letta / PowerContext / Shodh-Memory）、**C2 上下文数据库与路由**（OpenViking / 全量静态注入）、**C3 Agent 文件与协同治理**（AgentFS / OpenEnv）、**C4 宿主探针与代码依赖**（`rg`+`psutil`+`ss`+`git` / CodeGraph），在 Benchmark `v3.0` 上线 `COMP-C1~C4` 同机同数据集真跑负载与 12 维源码能力矩阵（严格区分 `LIVE_BENCHMARKED` 与 `ARCH_VERIFIED`，详见 [12-competitor-benchmark-spec.md](design_doc/12-competitor-benchmark-spec.md)）。

19. **Benchmark `v3.1`「既省 Token × 上下文质量不降反升」帕累托双门禁演进 (`Token-Quality Pareto Dual-Gate`)** `✅ Phase 36 已落地`:
    - 关于 Benchmark 的核心修正准则：不能只体现省 Token，**既要省 Token，同时上下文质量不能下降甚至应该有显著提升**；
    - Phase 36 已消除 `W1~W8` 质量评分缺口（`BENCH-06`），落地确定性四维上下文质量模型 $Q = 40\% R_{\text{fact}} + 30\% P_{\text{signal}} + 20\% A_{\text{action}} + 10\% S_{\text{safety}}$ 与**有效信息密度倍数（`Facts / 1k Tokens`）**，为 `W1~W11` 与 `COMP-C1~C4` 全量实装真值事实断言、`Baseline Q vs Steward Q (+ΔQ)` 成对比较及防截断帕累托 SLA 硬门禁（详见 [roadmap-phase36.md](src_doc/roadmap-phase36.md)）。

20. **生产运行态深度对齐、拓扑零误报硬化与 Benchmark `v3.2` 等价校准 (`Production Polish & Topology Hygiene`)** `✅ Phase 37 已落地`:
    - **CLI `brief` 与 `BriefAssembler` 统一 & 进程命令去噪**：消除 `SessionStart` 钩子仅输出旧版浅表进程列表的分裂问题（规约 #16），并规范化/折叠超长多行内联脚本进程（`node -e` / `python3 -c`），同时重置 Unix `SIGPIPE` 彻底消灭管道截断 Panic（规约 #12）；
    - **`LinksEngine` 测试/注释/文档示例免疫**：排除 `.rs` 测试夹具/`#[cfg(test)]`/`placeholder` 与 `.md` 示例代码路径导致的假断链自污染，使现网 `links audit` 恢复 `100/100` 健康分；
    - **Systemd 守护工具链探测 & Benchmark `v3.2`**：让 `PreflightChecker` 自动识别 NVM/Cargo 路径下的 `codegraph`；对齐 `DQ3` 物理健康度口径并校准 `COMP-C4` 等价采集基线（详见 [roadmap-phase37.md](src_doc/roadmap-phase37.md)）。
  
21. **全生命周期信息新鲜度治理与现实对账体系 (`Information Freshness & Reality Reconciliation`)** `✅ Phase 38 已落地`:
    - 彻底治理七大陈旧漏斗（`STALE-01~07`）：文件修改后写穿重置旧向量 (`embedding = '[]'`, `embedding_status = "stale"`) 与旧 L2 标注 (`needs_re_enrich`)；文件删除后 FTS 强制 `status = 'active'` 零僵尸召回并级联清理 `fact_nodes` / `links` / `auto_sync` 记忆；`AutoSyncEngine` 对同路径摘要记忆幂等去重归档；`PropertyGraphOrganizer` 与 `FreshnessReconciler` 自动剪除已关闭的幽灵端口节点、核销 `scripts/backfill_embeddings.py` 回填态 (`"ready"`) 并核验 `.codegraph/codegraph.db` 滞后度；现网 `DQ5` 抽检 `598` 项达成 **`100.0%` ✅**（详见 [roadmap-phase38-39.md](src_doc/roadmap-phase38-39.md)）。
  
22. **内生对比探针驱动的闭环自进化引擎 (`Probe-Driven Closed-Loop Self-Evolution`)** `✅ Phase 39 已落地`:
    - 在 Agent 执行回路的 5 个关键切面（`P1 开工 Brief` / `P2 事前 Pre-Hook` / `P3 检索效用` / `P4 事后漂移` / `P5 周期回归与图谱`）部署微秒级对比探针，以物理真值、SLA 基线与历史 Benchmark 批次为参照系量化输出数字差值（Before/After/Delta），并集成去重幂等的 `FeedbackEngine::add_item_deduped` 自动驱动 `user_feedback.md`、自愈对账与 Benchmark `v3.3` (`W12` + `DQ5`) 闭环自进化；首轮部署即由 `P3` 探针真实捕获 `stale_vec=3` 缺口并自愈至 **`5/5 ✅`**，现网 `v3.3` 全量评测 `D1 -99.65%`、`D6 37.6 → 99.8 (+62.1)` 全面通过（详见 [13-freshness-and-self-evolution-spec.md](design_doc/13-freshness-and-self-evolution-spec.md)）。
  
23. **图谱增强：多维证据分层、有向加权与隐式语义关系挖掘** `✅ Phase 40 已落地（对应规约 #5, #15, #23）`:
    > 原始探讨：图谱增强，也就是说事实、服务等之间存在的关系，关系的强弱，关系的方向等。这个似乎除了明面上的联系，实则还存在看似断连确存在语义上的联系，例如两个项目，一个服务和一个项目，两个服务之间都可能存在关系。
    - **现网实证动机**：实测 `/root/.steward/steward.db` 中 `fact_nodes` 已达 `355` 节点（`354` 个拥有向量），但 `fact_edges` 仅 `49` 条边，`project` 节点 `214/215` 孤立（**99.5%**），`links` 表 `5,116` 条文件引用未上卷，服务单元与运行进程同体分裂；
    - **演进方案**：已落地 **`L1_EXPLICIT`（物理硬依赖, `0.85~1.00`） / `L2_STRUCTURAL`（文件引用上卷 + 服务桥接 + 工具依赖, `0.55~0.84`） / `L3_SEMANTIC`（向量余弦 $\ge 0.78$ + 标签混合发现, `0.35~0.65`, `Top-K=3` 剪枝）** 三层有向加权关系引擎与跳数衰减爆炸半径推演（详见 [14-graph-intelligence-and-mesh-protocol-spec.md](design_doc/14-graph-intelligence-and-mesh-protocol-spec.md) 与 [roadmap-phase40-41.md](src_doc/roadmap-phase40-41.md)）。

24. **Mesh 协同协议：基于签名信封、防重放认证的跨机对话、沙盒文件投递与图谱联邦** `✅ Phase 41 已落地（对应规约 #2, #16, #19, #24）`:
    > 原始探讨：mesh, 除了能通信，是否还要设计通信协议，基于通信的功能点，例如传送文件，交换对话，对话间的身份认证安全措施等。
    - **演进方案**：遵循递弱代偿原则（不造 Raft/Rsync/聊天软件 IM），已落地 **`MeshEnvelope v1.0`** 线协议（`HMAC-SHA256` 规范签名 + `+-300s` 时间窗与 `NonceCache` 防重放 + `Peer Scopes` 最小权限），在此之上交付 **跨机图谱联邦探查 (`mesh/graph`)**、**双向脱敏抗投毒结构化对话 (`mesh/dialogue`)** 与 **限额 `<=10MB` + SHA-256 校验 + `~/.steward/mesh_inbox/` 隔离沙盒投递 (`mesh/transfer`)**，同步补齐 `W13`/`W14`/`DQ6` Benchmark `v3.4`（详见 [14-graph-intelligence-and-mesh-protocol-spec.md](design_doc/14-graph-intelligence-and-mesh-protocol-spec.md) 与 [roadmap-phase40-41.md](src_doc/roadmap-phase40-41.md)）。

25. **Agent 并发控制：乐观直通 + 悲观门禁自适应控制 (AOPCC)** `✅ Phase 42 已落地（对应规约 #8, #18, #22, #25）`:
   > 原始探讨：我从数据库悲观锁和乐观锁想到一个问题，如果你没次都从agent-steward获取（执行回路中强制）这不就悲观锁么，但这一定好吗？有时乐观锁可能更合适，例如我要做个postgresql验证，我可以假设本机5432已经默认有一个可用的实例，且无需密码，直接就连了。不需要从agent-steward先获取一次事实信息啊。你觉得呢？这个想法对接下来的agent-steward产品优化有什么作用么？

   - **核心收敛判断**：在 Agent 执行回路中强制事前查 `steward` 本质是**悲观并发控制（PCC）**——即便 `steward` 响应仅 `~1 ms`，也会迫使 Agent 多走一整轮 LLM 对话（重发 `5k~30k` 上下文 + `1.5s~4s` 推理延迟，即**整轮对话税 $C_{\text{turn}}$**）。面向本机 PostgreSQL `:5432` 等高置信、可逆、低回滚代价的只读/试探动作，应采用**乐观直通（`OPTIMISTIC`）**；而涉及破坏性写操作、端口绑定冲突、多 Agent 写锁与生产路径变更时，仍强制**悲观门禁（`PESSIMISTIC_LOCK`）**；介于两者之间的依赖写入采用**轻量条件校验（`CAUTIOUS_CAS`）**。
   - **已落地三层引擎**：(1) `OptimisticLeaseEngine` 在 `SessionStart (steward_brief)` 签发 `<=120 tok` 的 **MVCC 短时乐观直通契约 (`Optimistic Leases`)**；(2) `ConcurrencyPolicyClassifier` 精确区分客户端连接（如 `psql -p 5432`）与服务端端口绑定（如 `uvicorn --port`）；(3) `OptimisticFailureDiagnoser` 在 `PostToolUse(success=false)` 与 `steward_bridge.py` 中捕获 `ECONNREFUSED`、`password authentication failed`、`command not found`、`EADDRINUSE` 并在毫秒级内反查宿主机注入替代处方且自动降级失效租约，同步交付 Benchmark `v3.5`（`W15` + `PR-11`，详见 [15-adaptive-optimistic-pessimistic-concurrency-spec.md](design_doc/15-adaptive-optimistic-pessimistic-concurrency-spec.md) 与 [roadmap-phase42.md](src_doc/roadmap-phase42.md)）。

