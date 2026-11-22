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
        let _ = conn.execute_batch("SET threads = 2; SET memory_limit = '256MB';");
        let provider = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        provider.init()?;
        Ok(provider)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .context("打开 DuckDB 内存数据库失败")?;
        let _ = conn.execute_batch("SET threads = 2; SET memory_limit = '256MB';");
        let provider = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        provider.init()?;
        Ok(provider)
    }

    pub fn conn(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }

    pub fn begin_transaction(&self) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute_batch("BEGIN TRANSACTION;")
            .context("开启 DuckDB 事务失败")?;
        Ok(())
    }

    pub fn commit_transaction(&self) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute_batch("COMMIT;")
            .context("提交 DuckDB 事务失败")?;
        Ok(())
    }

    pub fn rollback_transaction(&self) -> Result<()> {
        let conn = self.lock_conn()?;
        let _ = conn.execute_batch("ROLLBACK;");
        Ok(())
    }

    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|e| anyhow::anyhow!("DuckDB 连接锁异常 (mutex poisoned): {}", e))
    }
}

impl StorageProvider for DuckDbStorageProvider {
    fn init(&self) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute_batch(DUCKDB_SCHEMA_SQL)
            .context("执行 DuckDB Schema 批处理失败")?;
        let _ = conn.execute("ALTER TABLE assets ADD COLUMN embedding VARCHAR DEFAULT '[]'", []);
        let _ = conn.execute("ALTER TABLE fact_nodes ADD COLUMN embedding VARCHAR DEFAULT '[]'", []);
        let _ = conn.execute("ALTER TABLE fact_edges ADD COLUMN evidence_tier VARCHAR DEFAULT 'L1_EXPLICIT'", []);
        let _ = conn.execute("ALTER TABLE fact_edges ADD COLUMN weight FLOAT DEFAULT 1.0", []);
        let _ = conn.execute("ALTER TABLE fact_edges ADD COLUMN direction_kind VARCHAR DEFAULT 'forward_dependency'", []);

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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
        let query_pattern = format!("%{}%", query);
        let mut stmt = conn.prepare(
            "SELECT id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status
             FROM assets
             WHERE status = 'active' AND (path ILIKE ?1 OR description ILIKE ?1 OR usage ILIKE ?1 OR tags ILIKE ?1)
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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
        let rows = conn.execute(
            "UPDATE memories SET status = ?1, updated_at = strftime(now(), '%Y-%m-%dT%H:%M:%SZ') WHERE id = ?2",
            params![status, memory_id],
        )?;
        Ok(rows)
    }

    fn record_change(&self, change: &ChangeRecord) -> Result<()> {
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
        conn.execute("DELETE FROM links WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn delete_links_for_target(&self, target_id: &str) -> Result<usize> {
        let conn = self.lock_conn()?;
        let count = conn.execute("DELETE FROM links WHERE target_id = ?1", params![target_id])?;
        Ok(count)
    }

    fn delete_auto_links_for_source(&self, source_id: &str) -> Result<usize> {
        let conn = self.lock_conn()?;
        let count = conn.execute(
            "DELETE FROM links WHERE source_id = ?1 AND auto_discovered = TRUE",
            params![source_id],
        )?;
        Ok(count)
    }

    fn update_link_target(&self, source_id: &str, old_target: &str, new_target: &str) -> Result<usize> {
        let conn = self.lock_conn()?;
        let count = conn.execute(
            "UPDATE links SET target_id = ?1 WHERE source_id = ?2 AND target_id = ?3",
            params![new_target, source_id, old_target],
        )?;
        Ok(count)
    }

    fn query_links_for_node(&self, node_type: &str, node_id: &str) -> Result<Vec<LinkRecord>> {
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
        let props_str = serde_json::to_string(&edge.properties)?;

        let _ = conn.execute(
            "DELETE FROM fact_edges WHERE source_id = ?1 AND target_id = ?2 AND relation = ?3",
            params![edge.source_id, edge.target_id, edge.relation],
        );
        conn.execute(
            "INSERT INTO fact_edges (source_id, target_id, relation, properties, confidence, evidence_tier, weight, direction_kind, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, strftime(now(), '%Y-%m-%dT%H:%M:%SZ'))",
            params![
                edge.source_id,
                edge.target_id,
                edge.relation,
                props_str,
                edge.confidence as f64,
                edge.evidence_tier,
                edge.weight as f64,
                edge.direction_kind,
            ],
        )?;
        Ok(())
    }

    fn get_fact_node(&self, id: &str) -> Result<Option<FactNodeRecord>> {
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
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

        let conn = self.lock_conn()?;
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
                    "SELECT id, source_id, target_id, relation, properties, confidence, evidence_tier, weight, direction_kind, updated_at
                     FROM fact_edges
                     WHERE source_id = ?1 OR target_id = ?1"
                )?;

                let edges = stmt.query_map(params![node_id], |row| {
                    let props_str: String = row.get(4)?;
                    let props = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));
                    let conf_f: f64 = row.get(5)?;
                    let tier: Option<String> = row.get(6)?;
                    let weight_f: Option<f64> = row.get(7)?;
                    let dir: Option<String> = row.get(8)?;
                    Ok(FactEdgeRecord {
                        id: row.get(0)?,
                        source_id: row.get(1)?,
                        target_id: row.get(2)?,
                        relation: row.get(3)?,
                        properties: props,
                        confidence: conf_f as f32,
                        evidence_tier: tier.unwrap_or_else(|| "L1_EXPLICIT".to_string()),
                        weight: weight_f.unwrap_or(conf_f) as f32,
                        direction_kind: dir.unwrap_or_else(|| "forward_dependency".to_string()),
                        updated_at: row.get(9)?,
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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, source_id, target_id, relation, properties, confidence, evidence_tier, weight, direction_kind, updated_at
             FROM fact_edges
             ORDER BY updated_at DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let props_str: String = row.get(4)?;
            let properties: serde_json::Value = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));
            let conf_f: f64 = row.get(5)?;
            let tier: Option<String> = row.get(6)?;
            let weight_f: Option<f64> = row.get(7)?;
            let dir: Option<String> = row.get(8)?;

            Ok(FactEdgeRecord {
                id: row.get(0)?,
                source_id: row.get(1)?,
                target_id: row.get(2)?,
                relation: row.get(3)?,
                properties,
                confidence: conf_f as f32,
                evidence_tier: tier.unwrap_or_else(|| "L1_EXPLICIT".to_string()),
                weight: weight_f.unwrap_or(conf_f) as f32,
                direction_kind: dir.unwrap_or_else(|| "forward_dependency".to_string()),
                updated_at: row.get(9)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn delete_fact_node(&self, id: &str) -> Result<usize> {
        let conn = self.lock_conn()?;
        conn.execute(
            "DELETE FROM fact_edges WHERE source_id = ?1 OR target_id = ?1",
            params![id],
        )?;
        let affected = conn.execute("DELETE FROM fact_nodes WHERE id = ?1", params![id])?;
        Ok(affected)
    }

    fn delete_fact_edges_by_tier(&self, tier: &str) -> Result<usize> {
        let conn = self.lock_conn()?;
        let affected = conn.execute(
            "DELETE FROM fact_edges WHERE evidence_tier = ?1",
            params![tier],
        )?;
        Ok(affected)
    }

    fn update_asset_embedding(&self, path: &str, embedding: &[f32]) -> Result<()> {
        let conn = self.lock_conn()?;
        let emb_str = serde_json::to_string(embedding)?;
        conn.execute(
            "UPDATE assets SET embedding = ?1, updated_at = strftime(now(), '%Y-%m-%dT%H:%M:%SZ') WHERE path = ?2",
            params![emb_str, path],
        )?;
        Ok(())
    }

    fn get_asset_embedding(&self, path: &str) -> Result<Vec<f32>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare("SELECT COALESCE(embedding, '[]') FROM assets WHERE path = ?1")?;
        let mut rows = stmt.query(params![path])?;
        if let Some(row) = rows.next()? {
            let s: String = row.get(0)?;
            Ok(serde_json::from_str(&s).unwrap_or_default())
        } else {
            Ok(Vec::new())
        }
    }

    fn update_fact_node_embedding(&self, id: &str, embedding: &[f32]) -> Result<()> {
        let conn = self.lock_conn()?;
        let emb_str = serde_json::to_string(embedding)?;
        conn.execute(
            "UPDATE fact_nodes SET embedding = ?1, updated_at = strftime(now(), '%Y-%m-%dT%H:%M:%SZ') WHERE id = ?2",
            params![emb_str, id],
        )?;
        Ok(())
    }

    fn get_fact_node_embedding(&self, id: &str) -> Result<Vec<f32>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare("SELECT COALESCE(embedding, '[]') FROM fact_nodes WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let s: String = row.get(0)?;
            Ok(serde_json::from_str(&s).unwrap_or_default())
        } else {
            Ok(Vec::new())
        }
    }

    // ─── Multi-Agent Coordination Locks (多 Agent 协作锁) ────────────
    fn insert_coordination_lock(&self, lock: &CoordinationLock) -> Result<bool> {
        let conn = self.lock_conn()?;

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
        let conn = self.lock_conn()?;
        let affected = conn.execute("DELETE FROM coordination_locks WHERE lock_id = ?1", params![lock_id])?;
        Ok(affected > 0)
    }

    fn get_coordination_lock(&self, resource_type: &str, resource_id: &str) -> Result<Option<CoordinationLock>> {
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
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
        let conn = self.lock_conn()?;
        let count = conn.execute(
            "DELETE FROM coordination_locks WHERE expires_at <= strftime(now(), '%Y-%m-%dT%H:%M:%SZ')",
            [],
        )?;
        Ok(count)
    }

    fn upsert_mcp_catalog(&self, rec: &McpCatalogRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let tools_str = serde_json::to_string(&rec.tools_json)?;
        conn.execute(
            "INSERT OR REPLACE INTO mcp_catalog (id, server_name, client_source, transport, command_or_url, tools_json, tool_count, est_total_tokens, status, last_scanned_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, strftime(now(), '%Y-%m-%dT%H:%M:%SZ'))",
            params![
                rec.id, rec.server_name, rec.client_source, rec.transport,
                rec.command_or_url, tools_str, rec.tool_count as i32,
                rec.est_total_tokens as i32, rec.status
            ],
        )?;
        Ok(())
    }

    fn list_mcp_catalog(&self) -> Result<Vec<McpCatalogRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, server_name, client_source, transport, command_or_url, tools_json, tool_count, est_total_tokens, status, last_scanned_at
             FROM mcp_catalog ORDER BY server_name ASC"
        )?;
        let rows = stmt.query_map([], Self::map_mcp_row)?;
        let mut res = Vec::new();
        for r in rows { res.push(r?); }
        Ok(res)
    }

    fn search_mcp_catalog(&self, query: &str, limit: usize) -> Result<Vec<McpCatalogRecord>> {
        let conn = self.lock_conn()?;
        let kw = format!("%{}%", query.to_lowercase());
        let mut stmt = conn.prepare(
            "SELECT id, server_name, client_source, transport, command_or_url, tools_json, tool_count, est_total_tokens, status, last_scanned_at
             FROM mcp_catalog
             WHERE lower(server_name) LIKE ?1 OR lower(tools_json) LIKE ?1
             ORDER BY tool_count DESC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![kw, limit as i64], Self::map_mcp_row)?;
        let mut res = Vec::new();
        for r in rows { res.push(r?); }
        Ok(res)
    }

    fn upsert_skill_catalog(&self, rec: &SkillCatalogRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let triggers_str = serde_json::to_string(&rec.triggers)?;
        conn.execute(
            "INSERT OR REPLACE INTO skills_catalog (id, skill_name, root_source, skill_path, description, triggers_json, est_tokens, has_scripts, has_references, status, last_scanned_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, strftime(now(), '%Y-%m-%dT%H:%M:%SZ'))",
            params![
                rec.id, rec.skill_name, rec.root_source, rec.skill_path,
                rec.description, triggers_str, rec.est_tokens as i32,
                rec.has_scripts, rec.has_references, rec.status
            ],
        )?;
        Ok(())
    }

    fn list_skills_catalog(&self) -> Result<Vec<SkillCatalogRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, skill_name, root_source, skill_path, description, triggers_json, est_tokens, has_scripts, has_references, status, last_scanned_at
             FROM skills_catalog ORDER BY skill_name ASC"
        )?;
        let rows = stmt.query_map([], Self::map_skill_row)?;
        let mut res = Vec::new();
        for r in rows { res.push(r?); }
        Ok(res)
    }

    fn search_skills_catalog(&self, query: &str, limit: usize) -> Result<Vec<SkillCatalogRecord>> {
        let conn = self.lock_conn()?;
        let kw = format!("%{}%", query.to_lowercase());
        let mut stmt = conn.prepare(
            "SELECT id, skill_name, root_source, skill_path, description, triggers_json, est_tokens, has_scripts, has_references, status, last_scanned_at
             FROM skills_catalog
             WHERE lower(skill_name) LIKE ?1 OR lower(description) LIKE ?1 OR lower(triggers_json) LIKE ?1
             ORDER BY skill_name ASC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![kw, limit as i64], Self::map_skill_row)?;
        let mut res = Vec::new();
        for r in rows { res.push(r?); }
        Ok(res)
    }

    fn upsert_external_service(&self, rec: &ExternalServiceRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO external_services (id, service_name, category, endpoint, protocol, capabilities, auth_type, auth_env_var, health_endpoint, usage_example, status, last_checked_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, COALESCE(?12, strftime(now(), '%Y-%m-%dT%H:%M:%SZ')))",
            params![
                rec.id, rec.service_name, rec.category, rec.endpoint, rec.protocol,
                rec.capabilities, rec.auth_type, rec.auth_env_var, rec.health_endpoint,
                rec.usage_example, rec.status, rec.last_checked_at
            ],
        )?;
        Ok(())
    }

    fn list_external_services(&self, category: Option<&str>) -> Result<Vec<ExternalServiceRecord>> {
        let conn = self.lock_conn()?;
        let mut res = Vec::new();
        match category {
            Some(cat) => {
                let mut stmt = conn.prepare(
                    "SELECT id, service_name, category, endpoint, protocol, capabilities, auth_type, auth_env_var, health_endpoint, usage_example, status, last_checked_at
                     FROM external_services WHERE category = ?1 ORDER BY service_name ASC"
                )?;
                let rows = stmt.query_map(params![cat], Self::map_ext_svc_row)?;
                for r in rows { res.push(r?); }
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, service_name, category, endpoint, protocol, capabilities, auth_type, auth_env_var, health_endpoint, usage_example, status, last_checked_at
                     FROM external_services ORDER BY service_name ASC"
                )?;
                let rows = stmt.query_map([], Self::map_ext_svc_row)?;
                for r in rows { res.push(r?); }
            }
        }
        Ok(res)
    }

    fn search_external_services(&self, query: &str, limit: usize) -> Result<Vec<ExternalServiceRecord>> {
        let conn = self.lock_conn()?;
        let kw = format!("%{}%", query.to_lowercase());
        let mut stmt = conn.prepare(
            "SELECT id, service_name, category, endpoint, protocol, capabilities, auth_type, auth_env_var, health_endpoint, usage_example, status, last_checked_at
             FROM external_services
             WHERE lower(service_name) LIKE ?1 OR lower(category) LIKE ?1 OR lower(capabilities) LIKE ?1
             ORDER BY service_name ASC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![kw, limit as i64], Self::map_ext_svc_row)?;
        let mut res = Vec::new();
        for r in rows { res.push(r?); }
        Ok(res)
    }

    fn enqueue_lock_wait(&self, entry: &LockWaitEntry) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO lock_wait_queue (wait_id, resource_type, resource_id, scope, waiting_agent, priority, enqueued_at, timeout_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                entry.wait_id, entry.resource_type, entry.resource_id, entry.scope,
                entry.waiting_agent, entry.priority, entry.enqueued_at, entry.timeout_at, entry.status
            ],
        )?;
        Ok(())
    }

    fn list_lock_wait_queue(&self, resource_type: Option<&str>, resource_id: Option<&str>) -> Result<Vec<LockWaitEntry>> {
        let conn = self.lock_conn()?;
        let mut res = Vec::new();
        match (resource_type, resource_id) {
            (Some(rt), Some(rid)) => {
                let mut stmt = conn.prepare(
                    "SELECT wait_id, resource_type, resource_id, scope, waiting_agent, priority, enqueued_at, timeout_at, status
                     FROM lock_wait_queue
                     WHERE resource_type = ?1 AND resource_id = ?2 AND status IN ('waiting','notified')
                       AND timeout_at > strftime(now(), '%Y-%m-%dT%H:%M:%SZ')
                     ORDER BY priority ASC, enqueued_at ASC"
                )?;
                let rows = stmt.query_map(params![rt, rid], Self::map_wait_row)?;
                for r in rows { res.push(r?); }
            }
            (Some(rt), None) => {
                let mut stmt = conn.prepare(
                    "SELECT wait_id, resource_type, resource_id, scope, waiting_agent, priority, enqueued_at, timeout_at, status
                     FROM lock_wait_queue
                     WHERE resource_type = ?1 AND status IN ('waiting','notified')
                       AND timeout_at > strftime(now(), '%Y-%m-%dT%H:%M:%SZ')
                     ORDER BY priority ASC, enqueued_at ASC"
                )?;
                let rows = stmt.query_map(params![rt], Self::map_wait_row)?;
                for r in rows { res.push(r?); }
            }
            _ => {
                let mut stmt = conn.prepare(
                    "SELECT wait_id, resource_type, resource_id, scope, waiting_agent, priority, enqueued_at, timeout_at, status
                     FROM lock_wait_queue
                     WHERE status IN ('waiting','notified')
                       AND timeout_at > strftime(now(), '%Y-%m-%dT%H:%M:%SZ')
                     ORDER BY priority ASC, enqueued_at ASC"
                )?;
                let rows = stmt.query_map([], Self::map_wait_row)?;
                for r in rows { res.push(r?); }
            }
        }
        Ok(res)
    }

    fn pop_next_lock_waiter(&self, resource_type: &str, resource_id: &str) -> Result<Option<LockWaitEntry>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT wait_id, resource_type, resource_id, scope, waiting_agent, priority, enqueued_at, timeout_at, status
             FROM lock_wait_queue
             WHERE resource_type = ?1 AND resource_id = ?2 AND status = 'waiting'
               AND timeout_at > strftime(now(), '%Y-%m-%dT%H:%M:%SZ')
             ORDER BY priority ASC, enqueued_at ASC LIMIT 1"
        )?;
        let mut rows = stmt.query_map(params![resource_type, resource_id], Self::map_wait_row)?;
        if let Some(first) = rows.next() {
            let mut item = first?;
            drop(rows);
            drop(stmt);
            conn.execute(
                "UPDATE lock_wait_queue SET status = 'notified' WHERE wait_id = ?1",
                params![item.wait_id],
            )?;
            item.status = "notified".to_string();
            Ok(Some(item))
        } else {
            Ok(None)
        }
    }

    fn update_lock_wait_status(&self, wait_id: &str, status: &str) -> Result<bool> {
        let conn = self.lock_conn()?;
        let affected = conn.execute(
            "UPDATE lock_wait_queue SET status = ?1 WHERE wait_id = ?2",
            params![status, wait_id],
        )?;
        Ok(affected > 0)
    }

    fn gc_expired_lock_waits(&self) -> Result<usize> {
        let conn = self.lock_conn()?;
        let affected = conn.execute(
            "UPDATE lock_wait_queue SET status = 'expired'
             WHERE status IN ('waiting','notified') AND timeout_at <= strftime(now(), '%Y-%m-%dT%H:%M:%SZ')",
            [],
        )?;
        Ok(affected)
    }

    fn insert_benchmark_run(&self, run: &BenchmarkRunRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let host_fp = serde_json::to_string(&run.host_fingerprint)?;
        let res_json = serde_json::to_string(&run.results_json)?;
        conn.execute(
            "INSERT OR REPLACE INTO benchmark_runs (
                run_id, suite_name, git_commit, app_version, host_fingerprint,
                workload_count, total_baseline_tokens, total_steward_tokens,
                overall_token_savings_pct, overall_speedup_factor,
                total_hazards_intercepted, total_noise_blocked, sla_passed,
                results_json, report_sha256, duration_ms, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                run.run_id,
                run.suite_name,
                run.git_commit,
                run.app_version,
                host_fp,
                run.workload_count as i32,
                run.total_baseline_tokens,
                run.total_steward_tokens,
                run.overall_token_savings_pct,
                run.overall_speedup_factor,
                run.total_hazards_intercepted,
                run.total_noise_blocked,
                run.sla_passed,
                res_json,
                run.report_sha256,
                run.duration_ms,
                run.created_at,
            ],
        )?;
        Ok(())
    }

    fn get_benchmark_run(&self, run_id: &str) -> Result<Option<BenchmarkRunRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT run_id, suite_name, git_commit, app_version, host_fingerprint,
                    workload_count, total_baseline_tokens, total_steward_tokens,
                    overall_token_savings_pct, overall_speedup_factor,
                    total_hazards_intercepted, total_noise_blocked, sla_passed,
                    results_json, report_sha256, duration_ms, created_at
             FROM benchmark_runs WHERE run_id = ?1",
        )?;
        let mut rows = stmt.query(params![run_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::map_benchmark_run_row(row)?))
        } else {
            Ok(None)
        }
    }

    fn list_benchmark_runs(&self, limit: usize) -> Result<Vec<BenchmarkRunRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT run_id, suite_name, git_commit, app_version, host_fingerprint,
                    workload_count, total_baseline_tokens, total_steward_tokens,
                    overall_token_savings_pct, overall_speedup_factor,
                    total_hazards_intercepted, total_noise_blocked, sla_passed,
                    results_json, report_sha256, duration_ms, created_at
             FROM benchmark_runs ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], Self::map_benchmark_run_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn insert_effectiveness_event(&self, event: &EffectivenessLedgerRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO effectiveness_ledger (
                event_id, session_id, source_channel, tool_or_action, provenance_rule,
                baseline_tokens, steward_tokens, tokens_saved,
                baseline_rounds, steward_rounds, rounds_saved,
                hazard_intercepted, noise_blocked, elapsed_us, evidence_summary, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                event.event_id,
                event.session_id,
                event.source_channel,
                event.tool_or_action,
                event.provenance_rule,
                event.baseline_tokens,
                event.steward_tokens,
                event.tokens_saved,
                event.baseline_rounds,
                event.steward_rounds,
                event.rounds_saved,
                event.hazard_intercepted,
                event.noise_blocked,
                event.elapsed_us,
                event.evidence_summary,
                event.created_at,
            ],
        )?;
        Ok(())
    }

    fn query_effectiveness_ledger(
        &self,
        window: &str,
        tool_filter: Option<&str>,
        limit: usize,
    ) -> Result<EffectivenessSummary> {
        let conn = self.lock_conn()?;
        let cutoff = crate::storage::window_to_cutoff_iso(window).unwrap_or_default();
        let tool = tool_filter.unwrap_or("").to_string();

        let (total_events, total_base, total_stew, total_saved, total_rounds, total_haz, total_noise, avg_us): (
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            f64,
        ) = conn.query_row(
            "SELECT COUNT(*),
                    CAST(COALESCE(SUM(baseline_tokens), 0) AS BIGINT),
                    CAST(COALESCE(SUM(steward_tokens), 0) AS BIGINT),
                    CAST(COALESCE(SUM(tokens_saved), 0) AS BIGINT),
                    CAST(COALESCE(SUM(rounds_saved), 0) AS BIGINT),
                    CAST(COALESCE(SUM(hazard_intercepted), 0) AS BIGINT),
                    CAST(COALESCE(SUM(noise_blocked), 0) AS BIGINT),
                    CAST(COALESCE(AVG(elapsed_us), 0.0) AS DOUBLE)
             FROM effectiveness_ledger
             WHERE (?1 = '' OR created_at >= ?1)
               AND (?2 = '' OR tool_or_action = ?2)",
            params![cutoff, tool],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                ))
            },
        )?;

        let mut rule_stmt = conn.prepare(
            "SELECT provenance_rule,
                    COUNT(*),
                    CAST(COALESCE(SUM(tokens_saved), 0) AS BIGINT),
                    CAST(COALESCE(SUM(rounds_saved), 0) AS BIGINT),
                    CAST(COALESCE(SUM(hazard_intercepted), 0) AS BIGINT),
                    CAST(COALESCE(SUM(noise_blocked), 0) AS BIGINT)
             FROM effectiveness_ledger
             WHERE (?1 = '' OR created_at >= ?1)
               AND (?2 = '' OR tool_or_action = ?2)
             GROUP BY provenance_rule
             ORDER BY SUM(tokens_saved) DESC, COUNT(*) DESC",
        )?;
        let by_rule = rule_stmt
            .query_map(params![cutoff, tool], |r| {
                Ok(RuleAggregate {
                    provenance_rule: r.get(0)?,
                    event_count: r.get::<_, i64>(1)? as usize,
                    tokens_saved: r.get(2)?,
                    rounds_saved: r.get(3)?,
                    hazards_intercepted: r.get(4)?,
                    noise_blocked: r.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut ev_stmt = conn.prepare(
            "SELECT event_id, session_id, source_channel, tool_or_action, provenance_rule,
                    baseline_tokens, steward_tokens, tokens_saved,
                    baseline_rounds, steward_rounds, rounds_saved,
                    hazard_intercepted, noise_blocked, elapsed_us, evidence_summary, created_at
             FROM effectiveness_ledger
             WHERE (?1 = '' OR created_at >= ?1)
               AND (?2 = '' OR tool_or_action = ?2)
             ORDER BY created_at DESC LIMIT ?3",
        )?;
        let recent_events = ev_stmt
            .query_map(params![cutoff, tool, limit as i64], |r| {
                Ok(EffectivenessLedgerRecord {
                    event_id: r.get(0)?,
                    session_id: r.get(1)?,
                    source_channel: r.get(2)?,
                    tool_or_action: r.get(3)?,
                    provenance_rule: r.get(4)?,
                    baseline_tokens: r.get(5)?,
                    steward_tokens: r.get(6)?,
                    tokens_saved: r.get(7)?,
                    baseline_rounds: r.get(8)?,
                    steward_rounds: r.get(9)?,
                    rounds_saved: r.get(10)?,
                    hazard_intercepted: r.get(11)?,
                    noise_blocked: r.get(12)?,
                    elapsed_us: r.get(13)?,
                    evidence_summary: r.get(14)?,
                    created_at: r.get(15)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let token_savings_pct = if total_base > 0 {
            (total_saved as f64 / total_base as f64) * 100.0
        } else {
            0.0
        };

        Ok(EffectivenessSummary {
            window: window.to_string(),
            total_events: total_events as usize,
            total_baseline_tokens: total_base,
            total_steward_tokens: total_stew,
            total_tokens_saved: total_saved,
            token_savings_pct,
            total_rounds_saved: total_rounds,
            total_hazards_intercepted: total_haz,
            total_noise_blocked: total_noise,
            avg_elapsed_us: avg_us,
            by_rule,
            recent_events,
        })
    }

    fn delete_bench_sandbox_records(&self) -> Result<usize> {
        let conn = self.lock_conn()?;
        let mut removed = 0usize;
        removed += conn.execute(
            "DELETE FROM coordination_locks WHERE holder_agent LIKE 'bench-%' OR resource_id LIKE '%bench:%'",
            [],
        )?;
        removed += conn.execute(
            "DELETE FROM lock_wait_queue WHERE waiting_agent LIKE 'bench-%' OR resource_id LIKE '%bench:%'",
            [],
        )?;
        removed += conn.execute(
            "DELETE FROM effectiveness_ledger WHERE session_id LIKE 'bench:%' OR tool_or_action LIKE 'bench:%'",
            [],
        )?;
        removed += conn.execute(
            "DELETE FROM benchmark_runs WHERE run_id LIKE 'bench:%' OR suite_name LIKE 'bench:%'",
            [],
        )?;
        Ok(removed)
    }

    fn insert_mesh_dialogue(&self, rec: &MeshDialogueRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let _ = conn.execute("DELETE FROM mesh_dialogues WHERE dialogue_id = ?1", params![rec.dialogue_id]);
        conn.execute(
            "INSERT INTO mesh_dialogues
             (dialogue_id, thread_id, from_node, from_agent, to_node, to_agent, intent, content, redaction_hits, injection_flagged, verified_signature, status, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![
                rec.dialogue_id,
                rec.thread_id,
                rec.from_node,
                rec.from_agent,
                rec.to_node,
                rec.to_agent,
                rec.intent,
                rec.content,
                rec.redaction_hits as i32,
                if rec.injection_flagged { 1i32 } else { 0i32 },
                if rec.verified_signature { 1i32 } else { 0i32 },
                rec.status,
                rec.created_at,
            ],
        )?;
        Ok(())
    }

    fn list_mesh_dialogues(&self, limit: usize) -> Result<Vec<MeshDialogueRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT dialogue_id, thread_id, from_node, from_agent, to_node, to_agent, intent, content, redaction_hits, injection_flagged, verified_signature, status, created_at
             FROM mesh_dialogues ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], |r| {
                Ok(MeshDialogueRecord {
                    dialogue_id: r.get(0)?,
                    thread_id: r.get(1)?,
                    from_node: r.get(2)?,
                    from_agent: r.get(3)?,
                    to_node: r.get(4)?,
                    to_agent: r.get(5)?,
                    intent: r.get(6)?,
                    content: r.get(7)?,
                    redaction_hits: r.get::<_, i32>(8)? as usize,
                    injection_flagged: r.get::<_, i32>(9)? != 0,
                    verified_signature: r.get::<_, i32>(10)? != 0,
                    status: r.get(11)?,
                    created_at: r.get(12)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn update_mesh_dialogue_status(&self, dialogue_id: &str, status: &str) -> Result<bool> {
        let conn = self.lock_conn()?;
        let n = conn.execute(
            "UPDATE mesh_dialogues SET status = ?1 WHERE dialogue_id = ?2",
            params![status, dialogue_id],
        )?;
        Ok(n > 0)
    }

    fn insert_mesh_transfer(&self, rec: &MeshTransferRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let _ = conn.execute("DELETE FROM mesh_transfers WHERE transfer_id = ?1", params![rec.transfer_id]);
        conn.execute(
            "INSERT INTO mesh_transfers
             (transfer_id, from_node, from_agent, to_node, file_name, size_bytes, sha256, inbox_path, secret_warnings, verified_signature, status, accepted_path, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![
                rec.transfer_id,
                rec.from_node,
                rec.from_agent,
                rec.to_node,
                rec.file_name,
                rec.size_bytes as i64,
                rec.sha256,
                rec.inbox_path,
                rec.secret_warnings as i32,
                if rec.verified_signature { 1i32 } else { 0i32 },
                rec.status,
                rec.accepted_path,
                rec.created_at,
                rec.updated_at,
            ],
        )?;
        Ok(())
    }

    fn list_mesh_transfers(&self, limit: usize) -> Result<Vec<MeshTransferRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT transfer_id, from_node, from_agent, to_node, file_name, size_bytes, sha256, inbox_path, secret_warnings, verified_signature, status, accepted_path, created_at, updated_at
             FROM mesh_transfers ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], |r| {
                Ok(MeshTransferRecord {
                    transfer_id: r.get(0)?,
                    from_node: r.get(1)?,
                    from_agent: r.get(2)?,
                    to_node: r.get(3)?,
                    file_name: r.get(4)?,
                    size_bytes: r.get::<_, i64>(5)? as u64,
                    sha256: r.get(6)?,
                    inbox_path: r.get(7)?,
                    secret_warnings: r.get::<_, i32>(8)? as usize,
                    verified_signature: r.get::<_, i32>(9)? != 0,
                    status: r.get(10)?,
                    accepted_path: r.get(11)?,
                    created_at: r.get(12)?,
                    updated_at: r.get(13)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn update_mesh_transfer_status(
        &self,
        transfer_id: &str,
        status: &str,
        accepted_path: Option<&str>,
    ) -> Result<bool> {
        let conn = self.lock_conn()?;
        let now = chrono::Utc::now().to_rfc3339();
        let n = conn.execute(
            "UPDATE mesh_transfers SET status = ?1, accepted_path = COALESCE(?2, accepted_path), updated_at = ?3 WHERE transfer_id = ?4",
            params![status, accepted_path, now, transfer_id],
        )?;
        Ok(n > 0)
    }

    fn insert_audit_event(&self, rec: &AuditEventRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let delta_str = serde_json::to_string(&rec.before_after_delta)?;
        let _ = conn.execute("DELETE FROM audit_events WHERE event_id = ?1", params![rec.event_id]);
        conn.execute(
            "INSERT INTO audit_events (
                event_id, trace_id, session_id, domain, actor_kind, actor_id,
                source_channel, category, action, target_type, target_id,
                concurrency_mode, outcome, before_after_delta, elapsed_us,
                evidence_sha256, created_at
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
            params![
                rec.event_id,
                rec.trace_id,
                rec.session_id,
                rec.domain,
                rec.actor_kind,
                rec.actor_id,
                rec.source_channel,
                rec.category,
                rec.action,
                rec.target_type,
                rec.target_id,
                rec.concurrency_mode,
                rec.outcome,
                delta_str,
                rec.elapsed_us,
                rec.evidence_sha256,
                rec.created_at,
            ],
        )?;
        Ok(())
    }

    fn query_audit_events(&self, filter: &AuditQueryFilter) -> Result<Vec<AuditEventRecord>> {
        let conn = self.lock_conn()?;
        let domain = filter.domain.as_deref().unwrap_or("");
        let actor_kind = filter.actor_kind.as_deref().unwrap_or("");
        let category = filter.category.as_deref().unwrap_or("");
        let action = filter.action.as_deref().unwrap_or("");
        let outcome = filter.outcome.as_deref().unwrap_or("");
        let trace_id = filter.trace_id.as_deref().unwrap_or("");
        let session_id = filter.session_id.as_deref().unwrap_or("");
        let target_kw = filter.target_contains.as_deref().unwrap_or("");
        let cutoff = filter
            .window
            .as_deref()
            .and_then(crate::storage::window_to_cutoff_iso)
            .unwrap_or_default();
        let lim = if filter.limit == 0 { 50 } else { filter.limit.min(5000) } as i64;

        let mut stmt = conn.prepare(
            "SELECT event_id, trace_id, session_id, domain, actor_kind, actor_id,
                    source_channel, category, action, target_type, target_id,
                    concurrency_mode, outcome, before_after_delta, elapsed_us,
                    evidence_sha256, created_at
             FROM audit_events
             WHERE (?1 = '' OR domain = ?1)
               AND (?2 = '' OR actor_kind = ?2)
               AND (?3 = '' OR category = ?3)
               AND (?4 = '' OR action = ?4)
               AND (?5 = '' OR outcome = ?5)
               AND (?6 = '' OR trace_id = ?6)
               AND (?7 = '' OR session_id = ?7)
               AND (?8 = '' OR target_id LIKE '%' || ?8 || '%')
               AND (?9 = '' OR created_at >= ?9)
             ORDER BY created_at DESC LIMIT ?10",
        )?;
        let rows = stmt
            .query_map(
                params![domain, actor_kind, category, action, outcome, trace_id, session_id, target_kw, cutoff, lim],
                |r| {
                    let delta_str: String = r.get(13)?;
                    Ok(AuditEventRecord {
                        event_id: r.get(0)?,
                        trace_id: r.get(1)?,
                        session_id: r.get(2)?,
                        domain: r.get(3)?,
                        actor_kind: r.get(4)?,
                        actor_id: r.get(5)?,
                        source_channel: r.get(6)?,
                        category: r.get(7)?,
                        action: r.get(8)?,
                        target_type: r.get(9)?,
                        target_id: r.get(10)?,
                        concurrency_mode: r.get(11)?,
                        outcome: r.get(12)?,
                        before_after_delta: serde_json::from_str(&delta_str)
                            .unwrap_or_else(|_| serde_json::json!({})),
                        elapsed_us: r.get(14)?,
                        evidence_sha256: r.get(15)?,
                        created_at: r.get(16)?,
                    })
                },
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn audit_domain_summary(&self, window: &str) -> Result<AuditDomainSummary> {
        let conn = self.lock_conn()?;
        let cutoff = crate::storage::window_to_cutoff_iso(window).unwrap_or_default();
        let mut stmt = conn.prepare(
            "SELECT domain, actor_kind, outcome,
                    COUNT(*),
                    CAST(COALESCE(AVG(elapsed_us), 0.0) AS DOUBLE)
             FROM audit_events
             WHERE (?1 = '' OR created_at >= ?1)
             GROUP BY domain, actor_kind, outcome
             ORDER BY COUNT(*) DESC",
        )?;
        let breakdown = stmt
            .query_map(params![cutoff], |r| {
                Ok(AuditDomainBreakdown {
                    domain: r.get(0)?,
                    actor_kind: r.get(1)?,
                    outcome: r.get(2)?,
                    event_count: r.get::<_, i64>(3)? as usize,
                    avg_elapsed_us: r.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut summary = AuditDomainSummary {
            window: window.to_string(),
            breakdown,
            ..Default::default()
        };
        for b in &summary.breakdown {
            summary.total_events += b.event_count;
            if b.domain == "IN_AGENT" {
                summary.in_agent_count += b.event_count;
            } else {
                summary.out_of_agent_count += b.event_count;
            }
            if b.outcome == "failed" || b.outcome == "blocked" {
                summary.failed_or_blocked_count += b.event_count;
            }
            if b.outcome == "recovered" {
                summary.recovered_count += b.event_count;
            }
        }
        Ok(summary)
    }

    fn prune_old_audit_events(&self, before_iso: &str) -> Result<usize> {
        let conn = self.lock_conn()?;
        let affected = conn.execute("DELETE FROM audit_events WHERE created_at < ?1", params![before_iso])?;
        Ok(affected)
    }

    fn insert_body_checkpoint(&self, rec: &BodyCheckpointRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let active_files = serde_json::to_string(&rec.active_files)?;
        let active_locks = serde_json::to_string(&rec.active_locks)?;
        let verified_facts = serde_json::to_string(&rec.verified_facts)?;
        let open_blockers = serde_json::to_string(&rec.open_blockers)?;
        let _ = conn.execute(
            "DELETE FROM body_checkpoints WHERE checkpoint_id = ?1",
            params![rec.checkpoint_id],
        );
        conn.execute(
            "INSERT INTO body_checkpoints (
                checkpoint_id, body_id, from_brain, session_id, goal_summary,
                active_files_json, active_locks_json, verified_facts_json, open_blockers_json,
                next_step_hint, token_budget, status, evidence_sha256, created_at,
                resumed_by_brain, resumed_at
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
            params![
                rec.checkpoint_id,
                rec.body_id,
                rec.from_brain,
                rec.session_id,
                rec.goal_summary,
                active_files,
                active_locks,
                verified_facts,
                open_blockers,
                rec.next_step_hint,
                rec.token_budget as i32,
                rec.status,
                rec.evidence_sha256,
                rec.created_at,
                rec.resumed_by_brain,
                rec.resumed_at,
            ],
        )?;
        Ok(())
    }

    fn get_latest_active_body_checkpoint(&self, body_id: &str) -> Result<Option<BodyCheckpointRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT checkpoint_id, body_id, from_brain, session_id, goal_summary,
                    active_files_json, active_locks_json, verified_facts_json, open_blockers_json,
                    next_step_hint, token_budget, status, evidence_sha256, created_at,
                    resumed_by_brain, resumed_at
             FROM body_checkpoints
             WHERE status = 'active' AND (?1 = '' OR body_id = ?1)
             ORDER BY created_at DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![body_id], Self::map_body_checkpoint_row)?;
        if let Some(r) = rows.next() {
            Ok(Some(r?))
        } else {
            Ok(None)
        }
    }

    fn list_body_checkpoints(&self, status_filter: Option<&str>, limit: usize) -> Result<Vec<BodyCheckpointRecord>> {
        let conn = self.lock_conn()?;
        let st = status_filter.unwrap_or("");
        let lim = if limit == 0 { 20 } else { limit.min(1000) } as i64;
        let mut stmt = conn.prepare(
            "SELECT checkpoint_id, body_id, from_brain, session_id, goal_summary,
                    active_files_json, active_locks_json, verified_facts_json, open_blockers_json,
                    next_step_hint, token_budget, status, evidence_sha256, created_at,
                    resumed_by_brain, resumed_at
             FROM body_checkpoints
             WHERE (?1 = '' OR status = ?1)
             ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![st, lim], Self::map_body_checkpoint_row)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    fn mark_body_checkpoint_resumed(&self, checkpoint_id: &str, by_brain: &str, resumed_at: &str) -> Result<bool> {
        let conn = self.lock_conn()?;
        let affected = conn.execute(
            "UPDATE body_checkpoints
             SET status = 'resumed', resumed_by_brain = ?2, resumed_at = ?3
             WHERE checkpoint_id = ?1",
            params![checkpoint_id, by_brain, resumed_at],
        )?;
        Ok(affected > 0)
    }
}

impl DuckDbStorageProvider {
    fn map_body_checkpoint_row(row: &duckdb::Row) -> duckdb::Result<BodyCheckpointRecord> {
        let files_s: String = row.get(5)?;
        let locks_s: String = row.get(6)?;
        let facts_s: String = row.get(7)?;
        let blockers_s: String = row.get(8)?;
        let tb: i32 = row.get(10)?;
        Ok(BodyCheckpointRecord {
            checkpoint_id: row.get(0)?,
            body_id: row.get(1)?,
            from_brain: row.get(2)?,
            session_id: row.get(3)?,
            goal_summary: row.get(4)?,
            active_files: serde_json::from_str(&files_s).unwrap_or_default(),
            active_locks: serde_json::from_str(&locks_s).unwrap_or_default(),
            verified_facts: serde_json::from_str(&facts_s).unwrap_or_default(),
            open_blockers: serde_json::from_str(&blockers_s).unwrap_or_default(),
            next_step_hint: row.get(9)?,
            token_budget: tb.max(0) as usize,
            status: row.get(11)?,
            evidence_sha256: row.get(12)?,
            created_at: row.get(13)?,
            resumed_by_brain: row.get(14)?,
            resumed_at: row.get(15)?,
        })
    }

    fn map_benchmark_run_row(row: &duckdb::Row) -> duckdb::Result<BenchmarkRunRecord> {
        let host_str: String = row.get(4)?;
        let res_str: String = row.get(13)?;
        Ok(BenchmarkRunRecord {
            run_id: row.get(0)?,
            suite_name: row.get(1)?,
            git_commit: row.get(2)?,
            app_version: row.get(3)?,
            host_fingerprint: serde_json::from_str(&host_str).unwrap_or_else(|_| serde_json::json!({})),
            workload_count: row.get::<_, i32>(5)? as usize,
            total_baseline_tokens: row.get(6)?,
            total_steward_tokens: row.get(7)?,
            overall_token_savings_pct: row.get(8)?,
            overall_speedup_factor: row.get(9)?,
            total_hazards_intercepted: row.get(10)?,
            total_noise_blocked: row.get(11)?,
            sla_passed: row.get(12)?,
            results_json: serde_json::from_str(&res_str).unwrap_or_else(|_| serde_json::json!([])),
            report_sha256: row.get(14)?,
            duration_ms: row.get(15)?,
            created_at: row.get(16)?,
        })
    }
    fn map_wait_row(row: &duckdb::Row) -> duckdb::Result<LockWaitEntry> {
        Ok(LockWaitEntry {
            wait_id: row.get(0)?,
            resource_type: row.get(1)?,
            resource_id: row.get(2)?,
            scope: row.get(3)?,
            waiting_agent: row.get(4)?,
            priority: row.get(5)?,
            enqueued_at: row.get(6)?,
            timeout_at: row.get(7)?,
            status: row.get(8)?,
        })
    }
    fn map_mcp_row(row: &duckdb::Row) -> duckdb::Result<McpCatalogRecord> {
        let tools_str: String = row.get(5)?;
        let tools_json = serde_json::from_str(&tools_str).unwrap_or_else(|_| serde_json::json!([]));
        Ok(McpCatalogRecord {
            id: row.get(0)?,
            server_name: row.get(1)?,
            client_source: row.get(2)?,
            transport: row.get(3)?,
            command_or_url: row.get(4)?,
            tools_json,
            tool_count: row.get::<_, i32>(6)? as usize,
            est_total_tokens: row.get::<_, i32>(7)? as usize,
            status: row.get(8)?,
            last_scanned_at: row.get(9)?,
        })
    }

    fn map_skill_row(row: &duckdb::Row) -> duckdb::Result<SkillCatalogRecord> {
        let triggers_str: String = row.get(5)?;
        let triggers: Vec<String> = serde_json::from_str(&triggers_str).unwrap_or_default();
        Ok(SkillCatalogRecord {
            id: row.get(0)?,
            skill_name: row.get(1)?,
            root_source: row.get(2)?,
            skill_path: row.get(3)?,
            description: row.get(4)?,
            triggers,
            est_tokens: row.get::<_, i32>(6)? as usize,
            has_scripts: row.get(7)?,
            has_references: row.get(8)?,
            status: row.get(9)?,
            last_scanned_at: row.get(10)?,
        })
    }

    fn map_ext_svc_row(row: &duckdb::Row) -> duckdb::Result<ExternalServiceRecord> {
        Ok(ExternalServiceRecord {
            id: row.get(0)?,
            service_name: row.get(1)?,
            category: row.get(2)?,
            endpoint: row.get(3)?,
            protocol: row.get(4)?,
            capabilities: row.get(5)?,
            auth_type: row.get(6)?,
            auth_env_var: row.get(7)?,
            health_endpoint: row.get(8)?,
            usage_example: row.get(9)?,
            status: row.get(10)?,
            last_checked_at: row.get(11)?,
        })
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
        let edge = FactEdgeRecord::new_explicit(
            node1.id.clone(),
            node2.id.clone(),
            "depends_on",
            serde_json::json!({"scope": "analytics"}),
            0.95,
        );

        provider.upsert_fact_node(&node1).expect("upsert node1");
        provider.upsert_fact_node(&node2).expect("upsert node2");
        provider.upsert_fact_edge(&edge).expect("upsert edge");

        let graph = provider.traverse_fact_graph(&node1.id, 2).expect("traverse graph");
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].relation, "depends_on");
    }

    #[test]
    fn test_duckdb_audit_events_crud_and_summary() {
        let provider = DuckDbStorageProvider::open_in_memory().expect("open duckdb in memory");
        let rec1 = AuditEventRecord {
            event_id: "aud-duck-1".to_string(),
            trace_id: "tr-duck-1".to_string(),
            session_id: "sess-1".to_string(),
            domain: "IN_AGENT".to_string(),
            actor_kind: "mcp_client".to_string(),
            actor_id: "agent".to_string(),
            source_channel: "mcp".to_string(),
            category: "tool_call".to_string(),
            action: "steward_port_check".to_string(),
            target_type: "port".to_string(),
            target_id: "5432".to_string(),
            concurrency_mode: Some("OPTIMISTIC".to_string()),
            outcome: "success".to_string(),
            before_after_delta: serde_json::json!({"port": 5432}),
            elapsed_us: 120,
            evidence_sha256: "a".repeat(64),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let rec2 = AuditEventRecord {
            event_id: "aud-duck-2".to_string(),
            trace_id: "tr-duck-2".to_string(),
            session_id: "system".to_string(),
            domain: "OUT_OF_AGENT".to_string(),
            actor_kind: "governance_engine".to_string(),
            actor_id: "steward".to_string(),
            source_channel: "gc".to_string(),
            category: "host_gc".to_string(),
            action: "cleanup_tmp".to_string(),
            target_type: "dir".to_string(),
            target_id: "/root/new/tmp".to_string(),
            concurrency_mode: None,
            outcome: "dry_run".to_string(),
            before_after_delta: serde_json::json!({"candidates": 2}),
            elapsed_us: 400,
            evidence_sha256: "b".repeat(64),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        provider.insert_audit_event(&rec1).expect("insert 1");
        provider.insert_audit_event(&rec2).expect("insert 2");

        let rows = provider
            .query_audit_events(&AuditQueryFilter {
                domain: Some("OUT_OF_AGENT".to_string()),
                limit: 10,
                ..Default::default()
            })
            .expect("query");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].event_id, "aud-duck-2");

        let sum = provider.audit_domain_summary("7d").expect("summary");
        assert_eq!(sum.total_events, 2);
        assert_eq!(sum.in_agent_count, 1);
        assert_eq!(sum.out_of_agent_count, 1);
    }
}
