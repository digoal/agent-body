use std::path::Path;
use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};
use duckdb::{params, Connection};
use crate::storage::models::*;
use crate::storage::spi::StorageProvider;

pub const DUCKDB_SCHEMA_SQL: &str = include_str!("duckdb_schema.sql");

#[derive(Clone)]
pub struct DuckDbStorageProvider {
    conn: Arc<Mutex<Connection>>,
}

impl DuckDbStorageProvider {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("创建 DuckDB 数据库目录失败: {:?}", parent))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("打开 DuckDB 数据库文件失败: {:?}", path))?;
        let provider = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        provider.init()?;
        Ok(provider)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .context("打开 DuckDB 内存数据库失败")?;
        let provider = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        provider.init()?;
        Ok(provider)
    }

    pub fn conn(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }
}

impl StorageProvider for DuckDbStorageProvider {
    fn init(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(DUCKDB_SCHEMA_SQL)
            .context("执行 DuckDB Schema 批处理失败")?;

        // 预置放置规则
        let presets = [
            ("rule-1", "shared_tools", "script", "/root/new/shared_proj/", "{name}/", "长期沉淀的可复用脚本与程序库", "建立独立子目录，配备 README.md 与入口脚本，严禁硬编码敏感信息", 10),
            ("rule-2", "work_projects", "project", "/root/new/work/", "{name}/", "常规立项的工作工程与开发目录", "每个工程独立版本控制，优先采用设计文档闭环验证", 20),
            ("rule-3", "ai_skills", "skill", "/root/new/skills/", "{name}/", "沉淀的通用高频高质量 AI 技能", "必须包含标准 SKILL.md，保障被高频调用的稳定性", 30),
            ("rule-4", "open_source_src", "src", "/root/new/src/", "{name}/", "浅克隆的开源项目源码库", "克隆后进入根目录优先使用 codegraph init 生成代码地图", 40),
            ("rule-5", "notes_and_lists", "note", "/root/new/note/", "{name}.md", "重要笔记、知识点与权威网站清单", "好记性不如烂笔头，用于跨周期翻阅沉淀", 50),
            ("rule-6", "cron_jobs", "other", "/root/new/cron/", "{name}/", "定时任务的 prompt 与输出产物", "多 cron 必须先查撞点表错峰，粒度至少 30 分钟", 60),
            ("rule-7", "temp_files", "tmp", "/root/new/tmp/", "{name}", "可随时删除的临时文件与调试日志", "明确生命周期，定期巡检清理", 70),
            ("rule-8", "open_source_shared", "other", "/root/new/shared_git/", "{name}/", "沉淀的可对外开源精品项目", "高质量开源工程，结构规整文档齐全", 80),
        ];

        for (id, name, ptype, base, pat, desc, guide, prio) in presets {
            let _ = conn.execute(
                "INSERT INTO placement_rules (id, rule_name, pattern_type, target_base_dir, naming_pattern, description, guideline, priority, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, strftime(now(), '%Y-%m-%dT%H:%M:%SZ'), strftime(now(), '%Y-%m-%dT%H:%M:%SZ'))
                 ON CONFLICT(rule_name) DO UPDATE SET
                    pattern_type = excluded.pattern_type,
                    target_base_dir = excluded.target_base_dir,
                    naming_pattern = excluded.naming_pattern,
                    description = excluded.description,
                    guideline = excluded.guideline,
                    priority = excluded.priority,
                    updated_at = strftime(now(), '%Y-%m-%dT%H:%M:%SZ')",
                params![id, name, ptype, base, pat, desc, guide, prio],
            );
        }

        // 预置扫描策略
        let scan_policies = [
            ("policy-1", "/root/new/shared_proj/**", "deep", -1, true, true, true, "白名单: 核心工具库，深度递归到底"),
            ("policy-2", "/root/new/work/**", "deep", -1, true, true, true, "白名单: 核心工作区，深度递归到底"),
            ("policy-3", "/root/new/skills/**", "deep", -1, true, true, true, "白名单: 技能库，全量递归"),
            ("policy-4", "**/node_modules/**", "shallow", 0, false, false, false, "黑名单: 仅记录当前目录实体本身标量，严禁递归下钻"),
            ("policy-5", "**/.cache/**", "shallow", 0, false, false, false, "黑名单: 缓存目录，仅记录当前目录标量，不下钻"),
            ("policy-6", "**/.git/**", "shallow", 0, false, false, false, "黑名单: Git 内部目录，仅记录当前目录标量，不下钻"),
            ("policy-7", "/root/.ssh/**", "shallow", 0, false, false, false, "黑名单: 私密目录，仅记录目录存在，严禁提取内容"),
            ("policy-8", "/proc/**", "shallow", 0, false, false, false, "黑名单: 系统虚拟文件系统，仅记录根目录，严禁递归"),
            ("policy-9", "/root/new/src/**", "restricted", 2, true, false, false, "受限区: 开源项目库，仅索引项目入口与 README"),
            ("policy-10", "/root/new/tmp/**", "restricted", 1, false, false, false, "受限区: 临时目录，仅记录浅层文件"),
        ];

        for (id, pat, tier, depth, fts, vec, graph, notes) in scan_policies {
            let _ = conn.execute(
                "INSERT INTO scan_policies (id, path_pattern, tier, max_depth, enable_fts, enable_vec, enable_graph, notes, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, strftime(now(), '%Y-%m-%dT%H:%M:%SZ'))
                 ON CONFLICT(path_pattern) DO UPDATE SET
                    tier = excluded.tier,
                    max_depth = excluded.max_depth,
                    enable_fts = excluded.enable_fts,
                    enable_vec = excluded.enable_vec,
                    enable_graph = excluded.enable_graph,
                    notes = excluded.notes",
                params![id, pat, tier, depth, fts, vec, graph, notes],
            );
        }

        Ok(())
    }

    fn provider_name(&self) -> &'static str {
        "duckdb"
    }

    fn get_placement_rules(&self) -> Result<Vec<PlacementRule>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, rule_name, pattern_type, target_base_dir, naming_pattern, description, guideline, priority 
             FROM placement_rules ORDER BY priority ASC"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(PlacementRule {
                id: row.get(0)?,
                rule_name: row.get(1)?,
                pattern_type: row.get(2)?,
                target_base_dir: row.get(3)?,
                naming_pattern: row.get(4)?,
                description: row.get(5)?,
                guideline: row.get(6)?,
                priority: row.get(7)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn insert_placement_rule(&self, rule: &PlacementRule) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO placement_rules (id, rule_name, pattern_type, target_base_dir, naming_pattern, description, guideline, priority, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, strftime(now(), '%Y-%m-%dT%H:%M:%SZ'), strftime(now(), '%Y-%m-%dT%H:%M:%SZ'))
             ON CONFLICT(rule_name) DO UPDATE SET
                pattern_type = excluded.pattern_type,
                target_base_dir = excluded.target_base_dir,
                naming_pattern = excluded.naming_pattern,
                description = excluded.description,
                guideline = excluded.guideline,
                priority = excluded.priority,
                updated_at = strftime(now(), '%Y-%m-%dT%H:%M:%SZ')",
            params![
                rule.id,
                rule.rule_name,
                rule.pattern_type,
                rule.target_base_dir,
                rule.naming_pattern,
                rule.description,
                rule.guideline,
                rule.priority,
            ],
        )?;
        Ok(())
    }

    fn get_scan_policies(&self) -> Result<Vec<ScanPolicy>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, path_pattern, tier, max_depth, enable_fts, enable_vec, enable_graph, notes 
             FROM scan_policies"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ScanPolicy {
                id: row.get(0)?,
                path_pattern: row.get(1)?,
                tier: row.get(2)?,
                max_depth: row.get(3)?,
                enable_fts: row.get::<_, bool>(4)?,
                enable_vec: row.get::<_, bool>(5)?,
                enable_graph: row.get::<_, bool>(6)?,
                notes: row.get(7)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn upsert_asset(&self, asset: &AssetRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let desc_kv_str = serde_json::to_string(&asset.desc_kv)?;
        let tags_str = serde_json::to_string(&asset.tags)?;

        conn.execute(
            "INSERT INTO assets (id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, strftime(now(), '%Y-%m-%dT%H:%M:%SZ'))
             ON CONFLICT(path) DO UPDATE SET
                type = excluded.type,
                owner = excluded.owner,
                security_level = excluded.security_level,
                scan_tier = excluded.scan_tier,
                description = excluded.description,
                desc_kv = excluded.desc_kv,
                usage = excluded.usage,
                size_bytes = excluded.size_bytes,
                file_count = excluded.file_count,
                tags = excluded.tags,
                status = excluded.status,
                updated_at = strftime(now(), '%Y-%m-%dT%H:%M:%SZ')",
            params![
                asset.id,
                asset.path,
                asset.asset_type,
                asset.owner,
                asset.security_level,
                asset.scan_tier,
                asset.description,
                desc_kv_str,
                asset.usage,
                asset.size_bytes,
                asset.file_count,
                tags_str,
                asset.status,
            ],
        )?;
        Ok(())
    }

    fn get_asset_by_path(&self, path: &str) -> Result<Option<AssetRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status
             FROM assets WHERE path = ?1"
        )?;

        let mut rows = stmt.query(params![path])?;
        if let Some(row) = rows.next()? {
            let desc_kv_str: String = row.get(7)?;
            let tags_str: String = row.get(11)?;
            let desc_kv: serde_json::Value = serde_json::from_str(&desc_kv_str).unwrap_or(serde_json::json!({}));
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();

            Ok(Some(AssetRecord {
                id: row.get(0)?,
                path: row.get(1)?,
                asset_type: row.get(2)?,
                owner: row.get(3)?,
                security_level: row.get(4)?,
                scan_tier: row.get(5)?,
                description: row.get(6)?,
                desc_kv,
                usage: row.get(8)?,
                size_bytes: row.get(9)?,
                file_count: row.get(10)?,
                tags,
                status: row.get(12)?,
            }))
        } else {
            Ok(None)
        }
    }

    fn search_assets_fts(&self, query: &str, limit: usize) -> Result<Vec<AssetRecord>> {
        let conn = self.conn.lock().unwrap();
        let query_pattern = format!("%{}%", query);
        let mut stmt = conn.prepare(
            "SELECT id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status
             FROM assets
             WHERE path ILIKE ?1 OR description ILIKE ?1 OR usage ILIKE ?1 OR tags ILIKE ?1
             ORDER BY updated_at DESC
             LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![query_pattern, limit as i64], |row| {
            let desc_kv_str: String = row.get(7)?;
            let tags_str: String = row.get(11)?;
            let desc_kv: serde_json::Value = serde_json::from_str(&desc_kv_str).unwrap_or(serde_json::json!({}));
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();

            Ok(AssetRecord {
                id: row.get(0)?,
                path: row.get(1)?,
                asset_type: row.get(2)?,
                owner: row.get(3)?,
                security_level: row.get(4)?,
                scan_tier: row.get(5)?,
                description: row.get(6)?,
                desc_kv,
                usage: row.get(8)?,
                size_bytes: row.get(9)?,
                file_count: row.get(10)?,
                tags,
                status: row.get(12)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn list_assets(&self, limit: usize) -> Result<Vec<AssetRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status
             FROM assets
             ORDER BY updated_at DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let desc_kv_str: String = row.get(7)?;
            let tags_str: String = row.get(11)?;
            let desc_kv: serde_json::Value = serde_json::from_str(&desc_kv_str).unwrap_or(serde_json::json!({}));
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();

            Ok(AssetRecord {
                id: row.get(0)?,
                path: row.get(1)?,
                asset_type: row.get(2)?,
                owner: row.get(3)?,
                security_level: row.get(4)?,
                scan_tier: row.get(5)?,
                description: row.get(6)?,
                desc_kv,
                usage: row.get(8)?,
                size_bytes: row.get(9)?,
                file_count: row.get(10)?,
                tags,
                status: row.get(12)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn replace_active_processes(&self, procs: &[ActiveProcessRecord]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM active_processes", [])?;
        for p in procs {
            let ports_str = serde_json::to_string(&p.listening_ports)?;
            conn.execute(
                "INSERT INTO active_processes (id, pid, name, command, owner, associated_path, listening_ports, status, cpu_pct, mem_bytes, gpu_allocated_mb, started_at, last_seen_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, strftime(now(), '%Y-%m-%dT%H:%M:%SZ'))",
                params![
                    p.id,
                    p.pid as i32,
                    p.name,
                    p.command,
                    p.owner,
                    p.associated_path,
                    ports_str,
                    p.status,
                    p.cpu_pct as f64,
                    p.mem_bytes as i64,
                    p.gpu_allocated_mb as i32,
                    p.started_at,
                ],
            )?;
        }
        Ok(())
    }

    fn list_active_processes(&self) -> Result<Vec<ActiveProcessRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, pid, name, command, owner, associated_path, listening_ports, status, cpu_pct, mem_bytes, gpu_allocated_mb, started_at, last_seen_at
             FROM active_processes ORDER BY pid ASC"
        )?;

        let rows = stmt.query_map([], |row| {
            let ports_str: String = row.get(6)?;
            let ports: Vec<u16> = serde_json::from_str(&ports_str).unwrap_or_default();
            let pid_i: i32 = row.get(1)?;
            let cpu_f: f64 = row.get(8)?;
            let mem_i: i64 = row.get(9)?;
            let gpu_i: i32 = row.get(10)?;

            Ok(ActiveProcessRecord {
                id: row.get(0)?,
                pid: pid_i as u32,
                name: row.get(2)?,
                command: row.get(3)?,
                owner: row.get(4)?,
                associated_path: row.get(5)?,
                listening_ports: ports,
                status: row.get(7)?,
                cpu_pct: cpu_f as f32,
                mem_bytes: mem_i as u64,
                gpu_allocated_mb: gpu_i as u32,
                started_at: row.get(11)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn insert_memory(&self, mem: &MemoryRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let tags_str = serde_json::to_string(&mem.tags)?;
        let embedding_str = serde_json::to_string(&mem.embedding)?;

        conn.execute(
            "INSERT INTO memories (id, type, content, summary, source_agent, source_session, tags, weight, pinned, status, embedding, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, strftime(now(), '%Y-%m-%dT%H:%M:%SZ'), strftime(now(), '%Y-%m-%dT%H:%M:%SZ'))
             ON CONFLICT(id) DO UPDATE SET
                type = excluded.type,
                content = excluded.content,
                summary = excluded.summary,
                tags = excluded.tags,
                weight = excluded.weight,
                pinned = excluded.pinned,
                status = excluded.status,
                embedding = excluded.embedding,
                updated_at = strftime(now(), '%Y-%m-%dT%H:%M:%SZ')",
            params![
                mem.id,
                mem.memory_type,
                mem.content,
                mem.summary,
                mem.source_agent,
                mem.source_session,
                tags_str,
                mem.weight as f64,
                mem.pinned,
                mem.status,
                embedding_str,
            ],
        )?;
        Ok(())
    }

    fn search_memories_fts(&self, query: &str, limit: usize) -> Result<Vec<MemoryRecord>> {
        let conn = self.conn.lock().unwrap();
        let query_pattern = format!("%{}%", query);
        let mut stmt = conn.prepare(
            "SELECT id, type, content, summary, source_agent, source_session, tags, weight, pinned, status, embedding
             FROM memories
             WHERE (content ILIKE ?1 OR summary ILIKE ?1 OR tags ILIKE ?1) AND status = 'active'
             ORDER BY updated_at DESC
             LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![query_pattern, limit as i64], |row| {
            let tags_str: String = row.get(6)?;
            let emb_str: String = row.get(10)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            let embedding: Vec<f32> = serde_json::from_str(&emb_str).unwrap_or_default();
            let weight_f: f64 = row.get(7)?;

            Ok(MemoryRecord {
                id: row.get(0)?,
                memory_type: row.get(1)?,
                content: row.get(2)?,
                summary: row.get(3)?,
                source_agent: row.get(4)?,
                source_session: row.get(5)?,
                tags,
                weight: weight_f as f32,
                pinned: row.get(8)?,
                status: row.get(9)?,
                embedding,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn list_all_active_memories(&self, limit: usize) -> Result<Vec<MemoryRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, type, content, summary, source_agent, source_session, tags, weight, pinned, status, embedding
             FROM memories
             WHERE status = 'active'
             ORDER BY updated_at DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let tags_str: String = row.get(6)?;
            let emb_str: String = row.get(10)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            let embedding: Vec<f32> = serde_json::from_str(&emb_str).unwrap_or_default();
            let weight_f: f64 = row.get(7)?;

            Ok(MemoryRecord {
                id: row.get(0)?,
                memory_type: row.get(1)?,
                content: row.get(2)?,
                summary: row.get(3)?,
                source_agent: row.get(4)?,
                source_session: row.get(5)?,
                tags,
                weight: weight_f as f32,
                pinned: row.get(8)?,
                status: row.get(9)?,
                embedding,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn update_memory_status(&self, memory_id: &str, status: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE memories SET status = ?1, updated_at = strftime(now(), '%Y-%m-%dT%H:%M:%SZ') WHERE id = ?2",
            params![status, memory_id],
        )?;
        Ok(rows)
    }

    fn record_change(&self, change: &ChangeRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let details_str = serde_json::to_string(&change.details)?;
        conn.execute(
            "INSERT INTO changes (type, target, description, details, source, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, strftime(now(), '%Y-%m-%dT%H:%M:%SZ'))",
            params![
                change.change_type,
                change.target,
                change.description,
                details_str,
                change.source,
            ],
        )?;
        Ok(())
    }

    fn list_changes(&self, limit: usize) -> Result<Vec<ChangeRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, type, target, description, details, source, created_at
             FROM changes
             ORDER BY created_at DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let details_str: String = row.get(4)?;
            let details: serde_json::Value = serde_json::from_str(&details_str).unwrap_or(serde_json::json!({}));

            Ok(ChangeRecord {
                id: row.get(0)?,
                change_type: row.get(1)?,
                target: row.get(2)?,
                description: row.get(3)?,
                details,
                source: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn add_link(&self, link: &LinkRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO links (source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, strftime(now(), '%Y-%m-%dT%H:%M:%SZ'))",
            params![
                link.source_type,
                link.source_id,
                link.target_type,
                link.target_id,
                link.relation,
                link.confidence as f64,
                link.auto_discovered,
                link.line_number,
                link.snippet,
                link.provenance,
            ],
        )?;
        Ok(())
    }

    fn delete_link(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM links WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn delete_links_for_target(&self, target_id: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute("DELETE FROM links WHERE target_id = ?1", params![target_id])?;
        Ok(count)
    }

    fn update_link_target(&self, source_id: &str, old_target: &str, new_target: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute(
            "UPDATE links SET target_id = ?1 WHERE source_id = ?2 AND target_id = ?3",
            params![new_target, source_id, old_target],
        )?;
        Ok(count)
    }

    fn query_links_for_node(&self, node_type: &str, node_id: &str) -> Result<Vec<LinkRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance, created_at
             FROM links
             WHERE (source_type = ?1 AND source_id = ?2) OR (target_type = ?1 AND target_id = ?2)"
        )?;

        let rows = stmt.query_map(params![node_type, node_id], |row| {
            let conf_f: f64 = row.get(6)?;
            Ok(LinkRecord {
                id: row.get(0)?,
                source_type: row.get(1)?,
                source_id: row.get(2)?,
                target_type: row.get(3)?,
                target_id: row.get(4)?,
                relation: row.get(5)?,
                confidence: conf_f as f32,
                auto_discovered: row.get(7)?,
                line_number: row.get(8)?,
                snippet: row.get(9)?,
                provenance: row.get(10)?,
                created_at: row.get(11)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn list_all_links(&self, limit: usize) -> Result<Vec<LinkRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance, created_at
             FROM links
             ORDER BY id DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let conf_f: f64 = row.get(6)?;
            Ok(LinkRecord {
                id: row.get(0)?,
                source_type: row.get(1)?,
                source_id: row.get(2)?,
                target_type: row.get(3)?,
                target_id: row.get(4)?,
                relation: row.get(5)?,
                confidence: conf_f as f32,
                auto_discovered: row.get(7)?,
                line_number: row.get(8)?,
                snippet: row.get(9)?,
                provenance: row.get(10)?,
                created_at: row.get(11)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn create_scan_job(&self, job: &ScanJobRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO scan_jobs (id, root_path, status, last_path, scanned_dirs, scanned_files, total_assets, shallow_blocked, error_message, started_at, paused_at, completed_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                job.id,
                job.root_path,
                job.status,
                job.last_path,
                job.scanned_dirs,
                job.scanned_files,
                job.total_assets,
                job.shallow_blocked,
                job.error_message,
                job.started_at,
                job.paused_at,
                job.completed_at,
                job.created_at,
                job.updated_at,
            ],
        )?;
        Ok(())
    }

    fn update_scan_job(&self, job: &ScanJobRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE scan_jobs SET
                status = ?1,
                last_path = ?2,
                scanned_dirs = ?3,
                scanned_files = ?4,
                total_assets = ?5,
                shallow_blocked = ?6,
                error_message = ?7,
                started_at = ?8,
                paused_at = ?9,
                completed_at = ?10,
                updated_at = strftime(now(), '%Y-%m-%dT%H:%M:%SZ')
             WHERE id = ?11",
            params![
                job.status,
                job.last_path,
                job.scanned_dirs,
                job.scanned_files,
                job.total_assets,
                job.shallow_blocked,
                job.error_message,
                job.started_at,
                job.paused_at,
                job.completed_at,
                job.id,
            ],
        )?;
        Ok(())
    }

    fn get_scan_job(&self, id: &str) -> Result<Option<ScanJobRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, root_path, status, last_path, scanned_dirs, scanned_files, total_assets, shallow_blocked, error_message, started_at, paused_at, completed_at, created_at, updated_at
             FROM scan_jobs WHERE id = ?1"
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(ScanJobRecord {
                id: row.get(0)?,
                root_path: row.get(1)?,
                status: row.get(2)?,
                last_path: row.get(3)?,
                scanned_dirs: row.get(4)?,
                scanned_files: row.get(5)?,
                total_assets: row.get(6)?,
                shallow_blocked: row.get(7)?,
                error_message: row.get(8)?,
                started_at: row.get(9)?,
                paused_at: row.get(10)?,
                completed_at: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            }))
        } else {
            Ok(None)
        }
    }

    fn list_scan_jobs(&self, limit: usize) -> Result<Vec<ScanJobRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, root_path, status, last_path, scanned_dirs, scanned_files, total_assets, shallow_blocked, error_message, started_at, paused_at, completed_at, created_at, updated_at
             FROM scan_jobs ORDER BY created_at DESC LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(ScanJobRecord {
                id: row.get(0)?,
                root_path: row.get(1)?,
                status: row.get(2)?,
                last_path: row.get(3)?,
                scanned_dirs: row.get(4)?,
                scanned_files: row.get(5)?,
                total_assets: row.get(6)?,
                shallow_blocked: row.get(7)?,
                error_message: row.get(8)?,
                started_at: row.get(9)?,
                paused_at: row.get(10)?,
                completed_at: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn get_active_scan_job(&self) -> Result<Option<ScanJobRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, root_path, status, last_path, scanned_dirs, scanned_files, total_assets, shallow_blocked, error_message, started_at, paused_at, completed_at, created_at, updated_at
             FROM scan_jobs WHERE status IN ('running', 'paused') ORDER BY created_at DESC LIMIT 1"
        )?;

        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            Ok(Some(ScanJobRecord {
                id: row.get(0)?,
                root_path: row.get(1)?,
                status: row.get(2)?,
                last_path: row.get(3)?,
                scanned_dirs: row.get(4)?,
                scanned_files: row.get(5)?,
                total_assets: row.get(6)?,
                shallow_blocked: row.get(7)?,
                error_message: row.get(8)?,
                started_at: row.get(9)?,
                paused_at: row.get(10)?,
                completed_at: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            }))
        } else {
            Ok(None)
        }
    }

    fn upsert_fact_node(&self, node: &FactNodeRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let props_str = serde_json::to_string(&node.properties)?;

        conn.execute(
            "INSERT INTO fact_nodes (id, node_type, name, label, properties, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, strftime(now(), '%Y-%m-%dT%H:%M:%SZ'))
             ON CONFLICT(id) DO UPDATE SET
                node_type = excluded.node_type,
                name = excluded.name,
                label = excluded.label,
                properties = excluded.properties,
                updated_at = strftime(now(), '%Y-%m-%dT%H:%M:%SZ')",
            params![
                node.id,
                node.node_type,
                node.name,
                node.label,
                props_str,
            ],
        )?;
        Ok(())
    }

    fn upsert_fact_edge(&self, edge: &FactEdgeRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let props_str = serde_json::to_string(&edge.properties)?;

        conn.execute(
            "INSERT INTO fact_edges (source_id, target_id, relation, properties, confidence, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, strftime(now(), '%Y-%m-%dT%H:%M:%SZ'))",
            params![
                edge.source_id,
                edge.target_id,
                edge.relation,
                props_str,
                edge.confidence as f64,
            ],
        )?;
        Ok(())
    }

    fn get_fact_node(&self, id: &str) -> Result<Option<FactNodeRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, node_type, name, label, properties, updated_at
             FROM fact_nodes WHERE id = ?1"
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let props_str: String = row.get(4)?;
            let properties: serde_json::Value = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));

            Ok(Some(FactNodeRecord {
                id: row.get(0)?,
                node_type: row.get(1)?,
                name: row.get(2)?,
                label: row.get(3)?,
                properties,
                updated_at: row.get(5)?,
            }))
        } else {
            Ok(None)
        }
    }

    fn find_fact_nodes_by_name(&self, name: &str) -> Result<Vec<FactNodeRecord>> {
        let conn = self.conn.lock().unwrap();
        let name_pattern = format!("%{}%", name);
        let mut stmt = conn.prepare(
            "SELECT id, node_type, name, label, properties, updated_at
             FROM fact_nodes WHERE name ILIKE ?1 OR label ILIKE ?1"
        )?;

        let rows = stmt.query_map(params![name_pattern], |row| {
            let props_str: String = row.get(4)?;
            let properties: serde_json::Value = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));

            Ok(FactNodeRecord {
                id: row.get(0)?,
                node_type: row.get(1)?,
                name: row.get(2)?,
                label: row.get(3)?,
                properties,
                updated_at: row.get(5)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn traverse_fact_graph(&self, root_id: &str, max_hops: usize) -> Result<SubGraph> {
        let root = match self.get_fact_node(root_id)? {
            Some(r) => r,
            None => return Ok(SubGraph::default()),
        };

        let conn = self.conn.lock().unwrap();
        let mut visited_node_ids = std::collections::HashSet::new();
        visited_node_ids.insert(root_id.to_string());

        let mut current_frontier = vec![root_id.to_string()];
        let mut collected_edges = Vec::new();

        for _ in 0..max_hops {
            if current_frontier.is_empty() {
                break;
            }
            let mut next_frontier = Vec::new();

            for node_id in &current_frontier {
                let mut stmt = conn.prepare(
                    "SELECT id, source_id, target_id, relation, properties, confidence, updated_at
                     FROM fact_edges
                     WHERE source_id = ?1 OR target_id = ?1"
                )?;

                let edges = stmt.query_map(params![node_id], |row| {
                    let props_str: String = row.get(4)?;
                    let props = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));
                    let conf_f: f64 = row.get(5)?;
                    Ok(FactEdgeRecord {
                        id: row.get(0)?,
                        source_id: row.get(1)?,
                        target_id: row.get(2)?,
                        relation: row.get(3)?,
                        properties: props,
                        confidence: conf_f as f32,
                        updated_at: row.get(6)?,
                    })
                })?.collect::<std::result::Result<Vec<_>, _>>()?;

                for e in edges {
                    let other_id = if &e.source_id == node_id {
                        &e.target_id
                    } else {
                        &e.source_id
                    };

                    if !visited_node_ids.contains(other_id) {
                        visited_node_ids.insert(other_id.clone());
                        next_frontier.push(other_id.clone());
                    }
                    if !collected_edges.iter().any(|existing: &FactEdgeRecord| {
                        existing.source_id == e.source_id && existing.target_id == e.target_id && existing.relation == e.relation
                    }) {
                        collected_edges.push(e);
                    }
                }
            }
            current_frontier = next_frontier;
        }

        let mut collected_nodes = Vec::new();
        for nid in &visited_node_ids {
            let mut stmt = conn.prepare(
                "SELECT id, node_type, name, label, properties, updated_at FROM fact_nodes WHERE id = ?1"
            )?;
            let mut rows = stmt.query(params![nid])?;
            if let Some(row) = rows.next()? {
                let props_str: String = row.get(4)?;
                let properties: serde_json::Value = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));
                collected_nodes.push(FactNodeRecord {
                    id: row.get(0)?,
                    node_type: row.get(1)?,
                    name: row.get(2)?,
                    label: row.get(3)?,
                    properties,
                    updated_at: row.get(5)?,
                });
            }
        }

        Ok(SubGraph {
            root_node: Some(root),
            nodes: collected_nodes,
            edges: collected_edges,
        })
    }

    fn list_all_fact_nodes(&self, limit: usize) -> Result<Vec<FactNodeRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, node_type, name, label, properties, updated_at
             FROM fact_nodes
             ORDER BY updated_at DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let props_str: String = row.get(4)?;
            let properties: serde_json::Value = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));

            Ok(FactNodeRecord {
                id: row.get(0)?,
                node_type: row.get(1)?,
                name: row.get(2)?,
                label: row.get(3)?,
                properties,
                updated_at: row.get(5)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn list_all_fact_edges(&self, limit: usize) -> Result<Vec<FactEdgeRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, source_id, target_id, relation, properties, confidence, updated_at
             FROM fact_edges
             ORDER BY updated_at DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let props_str: String = row.get(4)?;
            let properties: serde_json::Value = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));
            let conf_f: f64 = row.get(5)?;

            Ok(FactEdgeRecord {
                id: row.get(0)?,
                source_id: row.get(1)?,
                target_id: row.get(2)?,
                relation: row.get(3)?,
                properties,
                confidence: conf_f as f32,
                updated_at: row.get(6)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    // ─── Multi-Agent Coordination Locks (多 Agent 协作锁) ────────────
    fn insert_coordination_lock(&self, lock: &CoordinationLock) -> Result<bool> {
        let conn = self.conn.lock().unwrap();

        // 1. 清理该资源已过期的锁
        conn.execute(
            "DELETE FROM coordination_locks WHERE resource_type = ?1 AND resource_id = ?2 AND expires_at <= strftime(now(), '%Y-%m-%dT%H:%M:%SZ')",
            params![lock.resource_type, lock.resource_id],
        )?;

        // 2. 检查有效冲突
        let mut stmt = conn.prepare(
            "SELECT lock_id, scope, holder_agent, expires_at FROM coordination_locks 
             WHERE resource_type = ?1 AND resource_id = ?2 AND expires_at > strftime(now(), '%Y-%m-%dT%H:%M:%SZ')"
        )?;
        let mut rows = stmt.query(params![lock.resource_type, lock.resource_id])?;

        while let Some(row) = rows.next()? {
            let existing_scope: String = row.get(1)?;
            let existing_holder: String = row.get(2)?;

            if existing_holder == lock.holder_agent {
                continue;
            }

            if lock.scope == "exclusive" || existing_scope == "exclusive" {
                return Ok(false);
            }
        }

        // 3. 插入新锁
        let metadata_str = lock.metadata.as_ref()
            .map(|m| serde_json::to_string(m).unwrap_or_else(|_| "{}".to_string()))
            .unwrap_or_else(|| "{}".to_string());

        let pid_i64 = lock.holder_pid.map(|p| p as i64);

        conn.execute(
            "INSERT INTO coordination_locks (lock_id, resource_type, resource_id, scope, holder_agent, holder_pid, acquired_at, expires_at, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(lock_id) DO UPDATE SET
                expires_at = excluded.expires_at,
                metadata = excluded.metadata,
                scope = excluded.scope,
                holder_pid = excluded.holder_pid",
            params![
                lock.lock_id,
                lock.resource_type,
                lock.resource_id,
                lock.scope,
                lock.holder_agent,
                pid_i64,
                lock.acquired_at,
                lock.expires_at,
                metadata_str,
            ],
        )?;
        Ok(true)
    }

    fn delete_coordination_lock(&self, lock_id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute("DELETE FROM coordination_locks WHERE lock_id = ?1", params![lock_id])?;
        Ok(affected > 0)
    }

    fn get_coordination_lock(&self, resource_type: &str, resource_id: &str) -> Result<Option<CoordinationLock>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT lock_id, resource_type, resource_id, scope, holder_agent, holder_pid, acquired_at, expires_at, metadata
             FROM coordination_locks
             WHERE resource_type = ?1 AND resource_id = ?2 AND expires_at > strftime(now(), '%Y-%m-%dT%H:%M:%SZ')
             ORDER BY acquired_at DESC LIMIT 1"
        )?;
        let mut rows = stmt.query(params![resource_type, resource_id])?;
        if let Some(row) = rows.next()? {
            let meta_str: String = row.get(8)?;
            let metadata = serde_json::from_str(&meta_str).ok();
            let pid_i64: Option<i64> = row.get(5)?;
            Ok(Some(CoordinationLock {
                lock_id: row.get(0)?,
                resource_type: row.get(1)?,
                resource_id: row.get(2)?,
                scope: row.get(3)?,
                holder_agent: row.get(4)?,
                holder_pid: pid_i64.map(|p| p as u32),
                acquired_at: row.get(6)?,
                expires_at: row.get(7)?,
                metadata,
            }))
        } else {
            Ok(None)
        }
    }

    fn list_coordination_locks(&self, resource_type: Option<&str>) -> Result<Vec<CoordinationLock>> {
        let conn = self.conn.lock().unwrap();
        let sql = match resource_type {
            Some(_) => "SELECT lock_id, resource_type, resource_id, scope, holder_agent, holder_pid, acquired_at, expires_at, metadata
                        FROM coordination_locks
                        WHERE resource_type = ?1 AND expires_at > strftime(now(), '%Y-%m-%dT%H:%M:%SZ')
                        ORDER BY acquired_at DESC",
            None => "SELECT lock_id, resource_type, resource_id, scope, holder_agent, holder_pid, acquired_at, expires_at, metadata
                     FROM coordination_locks
                     WHERE expires_at > strftime(now(), '%Y-%m-%dT%H:%M:%SZ')
                     ORDER BY acquired_at DESC",
        };
        let mut stmt = conn.prepare(sql)?;
        let mut rows = match resource_type {
            Some(rt) => stmt.query(params![rt])?,
            None => stmt.query([])?,
        };

        let mut list = Vec::new();
        while let Some(row) = rows.next()? {
            let meta_str: String = row.get(8)?;
            let metadata = serde_json::from_str(&meta_str).ok();
            let pid_i64: Option<i64> = row.get(5)?;
            list.push(CoordinationLock {
                lock_id: row.get(0)?,
                resource_type: row.get(1)?,
                resource_id: row.get(2)?,
                scope: row.get(3)?,
                holder_agent: row.get(4)?,
                holder_pid: pid_i64.map(|p| p as u32),
                acquired_at: row.get(6)?,
                expires_at: row.get(7)?,
                metadata,
            });
        }
        Ok(list)
    }

    fn gc_expired_coordination_locks(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute(
            "DELETE FROM coordination_locks WHERE expires_at <= strftime(now(), '%Y-%m-%dT%H:%M:%SZ')",
            [],
        )?;
        Ok(count)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_duckdb_provider_init_and_presets() {
        let provider = DuckDbStorageProvider::open_in_memory().expect("open duckdb in memory");
        let rules = provider.get_placement_rules().expect("get rules");
        assert!(rules.len() >= 8, "应至少有 8 条预置规则");

        let policies = provider.get_scan_policies().expect("get policies");
        assert!(policies.len() >= 10, "应至少有 10 条扫描策略");
    }

    #[test]
    fn test_duckdb_provider_assets_crud_and_fts() {
        let provider = DuckDbStorageProvider::open_in_memory().expect("open duckdb in memory");
        let asset = AssetRecord {
            id: "asset-duck-1".to_string(),
            path: "/root/new/work/duckdb-test".to_string(),
            asset_type: "project".to_string(),
            owner: "user:digoal".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some("DuckDB 高性能分析引擎测试工程".to_string()),
            desc_kv: serde_json::json!({"engine": "duckdb"}),
            usage: Some("用于测试 DuckDB Provider SPI 实现".to_string()),
            size_bytes: Some(1024),
            file_count: Some(5),
            tags: vec!["duckdb".to_string(), "olap".to_string()],
            status: "active".to_string(),
        };

        provider.upsert_asset(&asset).expect("upsert asset");
        let fetched = provider.get_asset_by_path(&asset.path).expect("get asset").expect("must exist");
        assert_eq!(fetched.id, asset.id);
        assert_eq!(fetched.tags, vec!["duckdb", "olap"]);

        // 全文/模糊检索
        let search_res = provider.search_assets_fts("性能分析", 10).expect("fts search");
        assert_eq!(search_res.len(), 1);
        assert_eq!(search_res[0].id, asset.id);
    }

    #[test]
    fn test_duckdb_provider_property_graph() {
        let provider = DuckDbStorageProvider::open_in_memory().expect("open duckdb in memory");
        let node1 = FactNodeRecord {
            id: "node:agent-steward".to_string(),
            node_type: "project".to_string(),
            name: "agent-steward".to_string(),
            label: "宿主环境大管家".to_string(),
            properties: serde_json::json!({"lang": "rust"}),
            updated_at: Some("2026-11-18T12:00:00Z".to_string()),
        };
        let node2 = FactNodeRecord {
            id: "node:duckdb".to_string(),
            node_type: "tool".to_string(),
            name: "duckdb".to_string(),
            label: "DuckDB CLI".to_string(),
            properties: serde_json::json!({"type": "olap"}),
            updated_at: Some("2026-11-18T12:00:00Z".to_string()),
        };
        let edge = FactEdgeRecord {
            id: Some(1),
            source_id: node1.id.clone(),
            target_id: node2.id.clone(),
            relation: "depends_on".to_string(),
            properties: serde_json::json!({"scope": "analytics"}),
            confidence: 0.95,
            updated_at: Some("2026-11-18T12:00:00Z".to_string()),
        };

        provider.upsert_fact_node(&node1).expect("upsert node1");
        provider.upsert_fact_node(&node2).expect("upsert node2");
        provider.upsert_fact_edge(&edge).expect("upsert edge");

        let graph = provider.traverse_fact_graph(&node1.id, 2).expect("traverse graph");
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].relation, "depends_on");
    }
}
