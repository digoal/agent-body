# 04 — Agent 接入与 MCP 协议指南（MCP Integration）

> **章节目标**: 指导 AI Agent（如 Claude Desktop、Cursor、Antigravity 等）通过标准 Model Context Protocol (MCP) 原生挂载并调用大管家。

---

## 1. MCP 协议工作模式

Agent Host Steward 原生内置了轻量级、无依赖的 JSON-RPC 2.0 stdio 服务端。

```mermaid
sequenceDiagram
    participant Agent as "AI Agent (Claude / Cursor / AGY)"
    participant Steward as "steward mcp (stdio 子进程)"
    participant SQLite as "steward.db"

    Agent->>Steward: JSON-RPC "initialize"
    Steward-->>Agent: capabilities (tools: true), serverInfo
    Agent->>Steward: JSON-RPC "tools/list"
    Steward-->>Agent: 返回 14 个原生工具列表与 JSON Schema
    Agent->>Steward: JSON-RPC "tools/call" (steward_brief)
    Steward->>SQLite: 聚合查询环境与记忆
    Steward-->>Agent: 返回结果内容文本
```

- **通信方式**: Stdio（标准输入/输出），按换行符（`\n`）分隔 JSON-RPC 报文；
- **启动命令**: `agent-steward mcp`；
- **零外部端口依赖**: 不占用本地 TCP 端口，物理级进程隔离，随 Agent 会话启停。

---

## 2. 客户端配置指南

### 2.1 Claude Desktop 配置
在 Claude Desktop 配置文件中（Linux: `~/.config/Claude/claude_desktop_config.json` 或 macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`）添加：

```json
{
  "mcpServers": {
    "agent-steward": {
      "command": "/usr/local/bin/agent-steward",
      "args": ["mcp"]
    }
  }
}
```

### 2.2 Cursor 配置
在 Cursor Settings -> Features -> MCP Servers 中新增：
- **Name**: `agent-steward`
- **Type**: `command`
- **Command**: `/root/new/work/agent-steward/target/release/agent-steward mcp`

---

## 3. 14 个原生 MCP 工具标准字典

大管家向 Agent 暴露 14 个原子级认知与管控工具：

| MCP 工具名 | 核心功能 | 核心参数说明 |
|---|---|---|
| `steward_brief` | 一键获取宿主硬件、活跃任务、资产与记忆简报 | 无必填参数 |
| `steward_placement_suggest` | 索取新文件/工程的标准存放路径与命名建议 | `type`, `name`, `intent`, `security` |
| `steward_placement_rules` | 查看生效的 8 大目录放置规范 | 无 |
| `steward_asset_search` | 全文与语义检索已知纳管资产 | `query`, `limit` |
| `steward_asset_register` | 手动注册一个新资产对象 | `path`, `type`, `owner`, `desc` |
| `steward_asset_scan` | 触发指定目录白黑名单分级扫描 | `path` |
| `steward_env_processes` | 检索当前运行的普通任务进程 (过滤底噪) | 无 |
| `steward_env_show` | 查看系统硬件规格 (CPU/内存/磁盘) | 无 |
| `steward_port_check` | 端口防撞检测与空闲可用端口推荐 | `port`, `suggest_count` |
| `steward_memory_add` | 写入一条跨会话长期记忆 | `type`, `content`, `summary` |
| `steward_memory_search` | 语义+全文混合 RRF 检索记忆 | `query`, `limit`, `hybrid` |
| `steward_links_query` | 查询指定实体的依赖关联关系 | `type`, `id` |
| `steward_links_visualize` | 导出当前实体拓扑关系为 Mermaid 架构图 | `limit` |
| `steward_links_impact` | 变更爆炸半径推演 (BFS 逆向多跳分析) | `target`, `max_depth` |
| `steward_changes_since` | 查看自指定时间点以来的宿主环境变更历史 | `since`, `limit` |
| `steward_session_close` | 会话完工结算与垃圾回收 | `session_id`, `clean_temp` |

---

## 4. MCP Stdio 通信协议报文示例

### (1) 初始化握手 (`initialize`)
**请求**:
```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"antigravity","version":"1.0"}}}
```
**响应**:
```json
{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"agent-steward","version":"0.2.0"}}}
```

### (2) 工具调用 (`tools/call`) — 放置建议
**请求**:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "steward_placement_suggest",
    "arguments": {
      "type": "script",
      "name": "sec_fetcher",
      "intent": "抓取上市公司财报"
    }
  }
}
```
**响应**:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "╔═══════════════════════════════════════════════════════╗\n║             Steward 对象放置与路径建议                ║\n╚═══════════════════════════════════════════════════════╝\n推荐目标路径:   \"/root/new/shared_proj/sec_fetcher/\"\n命中规范规则:   shared_tools\n..."
      }
    ]
  }
}
```
