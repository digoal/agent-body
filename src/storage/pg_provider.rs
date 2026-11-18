use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};
use postgres::{Client, NoTls};
use crate::storage::models::*;
use crate::storage::spi::StorageProvider;

pub const PG_SCHEMA_SQL: &str = include_str!("pg_schema.sql");

#[derive(Clone)]
pub struct PgStorageProvider {
    client: Arc<Mutex<Client>>,
}

impl PgStorageProvider {
    /// 连接到 PostgreSQL 数据库并自动执行 Schema 初始化
    pub fn connect(url: &str) -> Result<Self> {
        let url_owned = url.to_string();
        let client = std::thread::spawn(move || {
            Client::connect(&url_owned, NoTls)
        })
        .join()
        .map_err(|_| anyhow::anyhow!("连接 PostgreSQL 线程异常退出"))?
        .with_context(|| format!("连接 PostgreSQL 数据库失败: {}", url))?;

        let provider = Self {
            client: Arc::new(Mutex::new(client)),
        };
        provider.init()?;
        Ok(provider)
    }

    /// 基于已建立的 Client 构造
    pub fn from_client(client: Client) -> Result<Self> {
        let provider = Self {
            client: Arc::new(Mutex::new(client)),
        };
        provider.init()?;
        Ok(provider)
    }
}

impl StorageProvider for PgStorageProvider {
    fn init(&self) -> Result<()> {
        let mut client = self.client.lock().unwrap();
        client.batch_execute(PG_SCHEMA_SQL)
            .context("执行内置 PostgreSQL Schema 批处理失败")?;

        // 预置放置规范规则
        let preset_rules = [
            ("rule-1", "shared_tools", "script", "/root/new/shared_proj/", "{name}/", "长期沉淀的可复用脚本与程序库", "建立独立子目录，配备 README.md 与入口脚本，严禁硬编码敏感信息", 10),
            ("rule-2", "work_projects", "project", "/root/new/work/", "{name}/", "常规立项的工作工程与开发目录", "每个工程独立版本控制，优先采用设计文档闭环验证", 20),
            ("rule-3", "ai_skills", "skill", "/root/new/skills/", "{name}/", "沉淀的通用高频高质量 AI 技能", "必须包含标准 SKILL.md，保障被高频调用的稳定性", 30),
            ("rule-4", "open_source_src", "src", "/root/new/src/", "{name}/", "浅克隆的开源项目源码库", "克隆后进入根目录优先使用 codegraph init 生成代码地图", 40),
            ("rule-5", "notes_and_lists", "note", "/root/new/note/", "{name}.md", "重要笔记、知识点与权威网站清单", "好记性不如烂笔头，用于跨周期翻阅沉淀", 50),
            ("rule-6", "cron_jobs", "other", "/root/new/cron/", "{name}/", "定时任务的 prompt 与输出产物", "多 cron 必须先查撞点表错峰，粒度至少 30 分钟", 60),
            ("rule-7", "temp_files", "tmp", "/root/new/tmp/", "{name}", "可随时删除的临时文件与调试日志", "明确生命周期，定期巡检清理", 70),
            ("rule-8", "open_source_shared", "other", "/root/new/shared_git/", "{name}/", "沉淀的可对外开源精品项目", "高质量开源工程，结构规整文档齐全", 80),
        ];

        for r in preset_rules {
            let _ = client.execute(
                "INSERT INTO placement_rules (id, rule_name, pattern_type, target_base_dir, naming_pattern, description, guideline, priority)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                 ON CONFLICT (id) DO NOTHING",
                &[&r.0, &r.1, &r.2, &r.3, &r.4, &r.5, &r.6, &r.7],
            );
        }

        // 预置扫描策略
        let preset_policies = [
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

        for p in preset_policies {
            let _ = client.execute(
                "INSERT INTO scan_policies (id, path_pattern, tier, max_depth, enable_fts, enable_vec, enable_graph, notes)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                 ON CONFLICT (id) DO NOTHING",
                &[&p.0, &p.1, &p.2, &p.3, &p.4, &p.5, &p.6, &p.7],
            );
        }

        Ok(())
    }

    fn provider_name(&self) -> &'static str {
        "postgres"
    }

    fn get_placement_rules(&self) -> Result<Vec<PlacementRule>> {
        let mut client = self.client.lock().unwrap();
        let rows = client.query(
            "SELECT id, rule_name, pattern_type, target_base_dir, naming_pattern, description, guideline, priority 
             FROM placement_rules ORDER BY priority ASC",
            &[],
        )?;

        let mut rules = Vec::new();
        for row in rows {
            rules.push(PlacementRule {
                id: row.get(0),
                rule_name: row.get(1),
                pattern_type: row.get(2),
                target_base_dir: row.get(3),
                naming_pattern: row.get(4),
                description: row.get(5),
                guideline: row.get(6),
                priority: row.get(7),
            });
        }
        Ok(rules)
    }

    fn insert_placement_rule(&self, rule: &PlacementRule) -> Result<()> {
        let mut client = self.client.lock().unwrap();
        client.execute(
            "INSERT INTO placement_rules (id, rule_name, pattern_type, target_base_dir, naming_pattern, description, guideline, priority)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (rule_name) DO UPDATE SET
                pattern_type = EXCLUDED.pattern_type,
                target_base_dir = EXCLUDED.target_base_dir,
                naming_pattern = EXCLUDED.naming_pattern,
                description = EXCLUDED.description,
                guideline = EXCLUDED.guideline,
                priority = EXCLUDED.priority,
                updated_at = NOW()",
            &[
                &rule.id,
                &rule.rule_name,
                &rule.pattern_type,
                &rule.target_base_dir,
                &rule.naming_pattern,
                &rule.description,
                &rule.guideline,
                &rule.priority,
            ],
        )?;
        Ok(())
    }

    fn get_scan_policies(&self) -> Result<Vec<ScanPolicy>> {
        let mut client = self.client.lock().unwrap();
        let rows = client.query(
            "SELECT id, path_pattern, tier, max_depth, enable_fts, enable_vec, enable_graph, notes 
             FROM scan_policies",
            &[],
        )?;

        let mut policies = Vec::new();
        for row in rows {
            policies.push(ScanPolicy {
                id: row.get(0),
                path_pattern: row.get(1),
                tier: row.get(2),
                max_depth: row.get(3),
                enable_fts: row.get(4),
                enable_vec: row.get(5),
                enable_graph: row.get(6),
                notes: row.get(7),
            });
        }
        Ok(policies)
    }

    fn upsert_asset(&self, asset: &AssetRecord) -> Result<()> {
        let mut client = self.client.lock().unwrap();
        client.execute(
            "INSERT INTO assets (id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, NOW())
             ON CONFLICT (path) DO UPDATE SET
                type = EXCLUDED.type,
                owner = EXCLUDED.owner,
                security_level = EXCLUDED.security_level,
                scan_tier = EXCLUDED.scan_tier,
                description = EXCLUDED.description,
                desc_kv = EXCLUDED.desc_kv,
                usage = EXCLUDED.usage,
                size_bytes = EXCLUDED.size_bytes,
                file_count = EXCLUDED.file_count,
                tags = EXCLUDED.tags,
                status = EXCLUDED.status,
                updated_at = NOW()",
            &[
                &asset.id,
                &asset.path,
                &asset.asset_type,
                &asset.owner,
                &asset.security_level,
                &asset.scan_tier,
                &asset.description,
                &asset.desc_kv,
                &asset.usage,
                &asset.size_bytes,
                &asset.file_count,
                &serde_json::to_value(&asset.tags)?,
                &asset.status,
            ],
        )?;
        Ok(())
    }

    fn get_asset_by_path(&self, path: &str) -> Result<Option<AssetRecord>> {
        let mut client = self.client.lock().unwrap();
        let rows = client.query(
            "SELECT id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status
             FROM assets WHERE path = $1",
            &[&path],
        )?;

        if let Some(row) = rows.first() {
            let tags_val: serde_json::Value = row.get(11);
            let tags: Vec<String> = serde_json::from_value(tags_val).unwrap_or_default();

            Ok(Some(AssetRecord {
                id: row.get(0),
                path: row.get(1),
                asset_type: row.get(2),
                owner: row.get(3),
                security_level: row.get(4),
                scan_tier: row.get(5),
                description: row.get(6),
                desc_kv: row.get(7),
                usage: row.get(8),
                size_bytes: row.get(9),
                file_count: row.get(10),
                tags,
                status: row.get(12),
            }))
        } else {
            Ok(None)
        }
    }

    fn search_assets_fts(&self, query: &str, limit: usize) -> Result<Vec<AssetRecord>> {
        let mut client = self.client.lock().unwrap();
        let rows = client.query(
            "SELECT id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status
             FROM assets
             WHERE fts_doc @@ plainto_tsquery('simple', $1)
                OR path ILIKE '%' || $1 || '%'
                OR description ILIKE '%' || $1 || '%'
                OR usage ILIKE '%' || $1 || '%'
             ORDER BY 
                CASE WHEN fts_doc @@ plainto_tsquery('simple', $1) 
                     THEN ts_rank(fts_doc, plainto_tsquery('simple', $1)) + 1.0 
                     ELSE 0.5 
                END DESC,
                updated_at DESC
             LIMIT $2",
            &[&query, &(limit as i64)],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let tags_val: serde_json::Value = row.get(11);
            let tags: Vec<String> = serde_json::from_value(tags_val).unwrap_or_default();

            results.push(AssetRecord {
                id: row.get(0),
                path: row.get(1),
                asset_type: row.get(2),
                owner: row.get(3),
                security_level: row.get(4),
                scan_tier: row.get(5),
                description: row.get(6),
                desc_kv: row.get(7),
                usage: row.get(8),
                size_bytes: row.get(9),
                file_count: row.get(10),
                tags,
                status: row.get(12),
            });
        }
        Ok(results)
    }

    fn list_assets(&self, limit: usize) -> Result<Vec<AssetRecord>> {
        let mut client = self.client.lock().unwrap();
        let rows = client.query(
            "SELECT id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status
             FROM assets
             ORDER BY updated_at DESC
             LIMIT $1",
            &[&(limit as i64)],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let tags_val: serde_json::Value = row.get(11);
            let tags: Vec<String> = serde_json::from_value(tags_val).unwrap_or_default();

            results.push(AssetRecord {
                id: row.get(0),
                path: row.get(1),
                asset_type: row.get(2),
                owner: row.get(3),
                security_level: row.get(4),
                scan_tier: row.get(5),
                description: row.get(6),
                desc_kv: row.get(7),
                usage: row.get(8),
                size_bytes: row.get(9),
                file_count: row.get(10),
                tags,
                status: row.get(12),
            });
        }
        Ok(results)
    }

    fn replace_active_processes(&self, procs: &[ActiveProcessRecord]) -> Result<()> {
        let mut client = self.client.lock().unwrap();
        let mut tx = client.transaction()?;

        tx.execute("DELETE FROM active_processes", &[])?;

        for p in procs {
            tx.execute(
                "INSERT INTO active_processes (id, pid, name, command, owner, associated_path, listening_ports, status, cpu_pct, mem_bytes, gpu_allocated_mb, started_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
                &[
                    &p.id,
                    &(p.pid as i32),
                    &p.name,
                    &p.command,
                    &p.owner,
                    &p.associated_path,
                    &serde_json::to_value(&p.listening_ports)?,
                    &p.status,
                    &p.cpu_pct,
                    &(p.mem_bytes as i64),
                    &(p.gpu_allocated_mb as i32),
                    &p.started_at,
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    fn list_active_processes(&self) -> Result<Vec<ActiveProcessRecord>> {
        let mut client = self.client.lock().unwrap();
        let rows = client.query(
            "SELECT id, pid, name, command, owner, associated_path, listening_ports, status, cpu_pct, mem_bytes, gpu_allocated_mb, started_at
             FROM active_processes ORDER BY cpu_pct DESC",
            &[],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let pid_i32: i32 = row.get(1);
            let ports_val: serde_json::Value = row.get(6);
            let listening_ports: Vec<u16> = serde_json::from_value(ports_val).unwrap_or_default();
            let mem_bytes_i64: i64 = row.get(9);
            let gpu_mb: i32 = row.get(10);

            results.push(ActiveProcessRecord {
                id: row.get(0),
                pid: pid_i32 as u32,
                name: row.get(2),
                command: row.get(3),
                owner: row.get(4),
                associated_path: row.get(5),
                listening_ports,
                status: row.get(7),
                cpu_pct: row.get(8),
                mem_bytes: mem_bytes_i64 as u64,
                gpu_allocated_mb: gpu_mb as u32,
                started_at: row.get(11),
            });
        }
        Ok(results)
    }

    fn insert_memory(&self, mem: &MemoryRecord) -> Result<()> {
        let mut client = self.client.lock().unwrap();
        client.execute(
            "INSERT INTO memories (id, type, content, summary, source_agent, source_session, tags, weight, pinned, status, embedding)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
             ON CONFLICT (id) DO UPDATE SET
                content = EXCLUDED.content,
                summary = EXCLUDED.summary,
                weight = EXCLUDED.weight,
                status = EXCLUDED.status,
                updated_at = NOW()",
            &[
                &mem.id,
                &mem.memory_type,
                &mem.content,
                &mem.summary,
                &mem.source_agent,
                &mem.source_session,
                &serde_json::to_value(&mem.tags)?,
                &mem.weight,
                &mem.pinned,
                &mem.status,
                &serde_json::to_string(&mem.embedding)?,
            ],
        )?;
        Ok(())
    }

    fn search_memories_fts(&self, query: &str, limit: usize) -> Result<Vec<MemoryRecord>> {
        let mut client = self.client.lock().unwrap();
        let rows = client.query(
            "SELECT id, type, content, summary, source_agent, source_session, tags, weight, pinned, status, embedding
             FROM memories
             WHERE (fts_doc @@ plainto_tsquery('simple', $1)
                OR content ILIKE '%' || $1 || '%'
                OR summary ILIKE '%' || $1 || '%')
               AND status = 'active'
             ORDER BY 
                CASE WHEN fts_doc @@ plainto_tsquery('simple', $1)
                     THEN ts_rank(fts_doc, plainto_tsquery('simple', $1)) + 1.0
                     ELSE 0.5
                END DESC,
                weight DESC
             LIMIT $2",
            &[&query, &(limit as i64)],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let tags_val: serde_json::Value = row.get(6);
            let tags: Vec<String> = serde_json::from_value(tags_val).unwrap_or_default();
            let emb_str: String = row.get(10);
            let embedding: Vec<f32> = serde_json::from_str(&emb_str).unwrap_or_default();

            results.push(MemoryRecord {
                id: row.get(0),
                memory_type: row.get(1),
                content: row.get(2),
                summary: row.get(3),
                source_agent: row.get(4),
                source_session: row.get(5),
                tags,
                weight: row.get(7),
                pinned: row.get(8),
                status: row.get(9),
                embedding,
            });
        }
        Ok(results)
    }

    fn list_all_active_memories(&self, limit: usize) -> Result<Vec<MemoryRecord>> {
        let mut client = self.client.lock().unwrap();
        let rows = client.query(
            "SELECT id, type, content, summary, source_agent, source_session, tags, weight, pinned, status, embedding
             FROM memories
             WHERE status = 'active'
             ORDER BY updated_at DESC
             LIMIT $1",
            &[&(limit as i64)],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let tags_val: serde_json::Value = row.get(6);
            let tags: Vec<String> = serde_json::from_value(tags_val).unwrap_or_default();
            let emb_str: String = row.get(10);
            let embedding: Vec<f32> = serde_json::from_str(&emb_str).unwrap_or_default();

            results.push(MemoryRecord {
                id: row.get(0),
                memory_type: row.get(1),
                content: row.get(2),
                summary: row.get(3),
                source_agent: row.get(4),
                source_session: row.get(5),
                tags,
                weight: row.get(7),
                pinned: row.get(8),
                status: row.get(9),
                embedding,
            });
        }
        Ok(results)
    }

    fn update_memory_status(&self, memory_id: &str, status: &str) -> Result<usize> {
        let mut client = self.client.lock().unwrap();
        let count = client.execute(
            "UPDATE memories SET status = $2, updated_at = NOW() WHERE id = $1",
            &[&memory_id, &status],
        )?;
        Ok(count as usize)
    }

    fn record_change(&self, change: &ChangeRecord) -> Result<()> {
        let mut client = self.client.lock().unwrap();
        client.execute(
            "INSERT INTO changes (type, target, description, details, source)
             VALUES ($1, $2, $3, $4, $5)",
            &[
                &change.change_type,
                &change.target,
                &change.description,
                &change.details,
                &change.source,
            ],
        )?;
        Ok(())
    }

    fn list_changes(&self, limit: usize) -> Result<Vec<ChangeRecord>> {
        let mut client = self.client.lock().unwrap();
        let rows = client.query(
            "SELECT id, type, target, description, details, source, created_at
             FROM changes ORDER BY created_at DESC LIMIT $1",
            &[&(limit as i64)],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let id_i64: i64 = row.get(0);
            let created: Option<chrono::DateTime<chrono::Utc>> = row.get(6);
            results.push(ChangeRecord {
                id: Some(id_i64),
                change_type: row.get(1),
                target: row.get(2),
                description: row.get(3),
                details: row.get(4),
                source: row.get(5),
                created_at: created.map(|c| c.to_rfc3339()),
            });
        }
        Ok(results)
    }

    fn add_link(&self, link: &LinkRecord) -> Result<()> {
        let mut client = self.client.lock().unwrap();
        client.execute(
            "INSERT INTO links (source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             ON CONFLICT (source_type, source_id, target_type, target_id, relation) DO UPDATE SET
                confidence = EXCLUDED.confidence,
                line_number = EXCLUDED.line_number,
                snippet = EXCLUDED.snippet,
                provenance = EXCLUDED.provenance",
            &[
                &link.source_type,
                &link.source_id,
                &link.target_type,
                &link.target_id,
                &link.relation,
                &link.confidence,
                &link.auto_discovered,
                &link.line_number,
                &link.snippet,
                &link.provenance.as_deref().unwrap_or("static_scan"),
            ],
        )?;
        Ok(())
    }

    fn delete_link(&self, id: i64) -> Result<()> {
        let mut client = self.client.lock().unwrap();
        client.execute("DELETE FROM links WHERE id = $1", &[&id])?;
        Ok(())
    }

    fn delete_links_for_target(&self, target_id: &str) -> Result<usize> {
        let mut client = self.client.lock().unwrap();
        let count = client.execute("DELETE FROM links WHERE target_id = $1", &[&target_id])?;
        Ok(count as usize)
    }

    fn update_link_target(&self, source_id: &str, old_target: &str, new_target: &str) -> Result<usize> {
        let mut client = self.client.lock().unwrap();
        let count = client.execute(
            "UPDATE links SET target_id = $3 WHERE source_id = $1 AND target_id = $2",
            &[&source_id, &old_target, &new_target],
        )?;
        Ok(count as usize)
    }

    fn query_links_for_node(&self, node_type: &str, node_id: &str) -> Result<Vec<LinkRecord>> {
        let mut client = self.client.lock().unwrap();
        let rows = client.query(
            "SELECT id, source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance, created_at
             FROM links
             WHERE (source_type = $1 AND source_id = $2) OR (target_type = $1 AND target_id = $2)
             ORDER BY created_at DESC",
            &[&node_type, &node_id],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let id_i64: i64 = row.get(0);
            let created: Option<chrono::DateTime<chrono::Utc>> = row.get(11);
            results.push(LinkRecord {
                id: Some(id_i64),
                source_type: row.get(1),
                source_id: row.get(2),
                target_type: row.get(3),
                target_id: row.get(4),
                relation: row.get(5),
                confidence: row.get(6),
                auto_discovered: row.get(7),
                line_number: row.get(8),
                snippet: row.get(9),
                provenance: row.get(10),
                created_at: created.map(|c| c.to_rfc3339()),
            });
        }
        Ok(results)
    }

    fn list_all_links(&self, limit: usize) -> Result<Vec<LinkRecord>> {
        let mut client = self.client.lock().unwrap();
        let rows = client.query(
            "SELECT id, source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance, created_at
             FROM links
             ORDER BY created_at DESC
             LIMIT $1",
            &[&(limit as i64)],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let id_i64: i64 = row.get(0);
            let created: Option<chrono::DateTime<chrono::Utc>> = row.get(11);
            results.push(LinkRecord {
                id: Some(id_i64),
                source_type: row.get(1),
                source_id: row.get(2),
                target_type: row.get(3),
                target_id: row.get(4),
                relation: row.get(5),
                confidence: row.get(6),
                auto_discovered: row.get(7),
                line_number: row.get(8),
                snippet: row.get(9),
                provenance: row.get(10),
                created_at: created.map(|c| c.to_rfc3339()),
            });
        }
        Ok(results)
    }

    fn create_scan_job(&self, job: &ScanJobRecord) -> Result<()> {
        let mut client = self.client.lock().unwrap();
        client.execute(
            "INSERT INTO scan_jobs (id, root_path, status, last_path, scanned_dirs, scanned_files, total_assets, shallow_blocked, error_message, started_at, paused_at, completed_at, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW()::text, NOW()::text)",
            &[
                &job.id,
                &job.root_path,
                &job.status,
                &job.last_path,
                &job.scanned_dirs,
                &job.scanned_files,
                &job.total_assets,
                &job.shallow_blocked,
                &job.error_message,
                &job.started_at,
                &job.paused_at,
                &job.completed_at,
            ],
        )?;
        Ok(())
    }

    fn update_scan_job(&self, job: &ScanJobRecord) -> Result<()> {
        let mut client = self.client.lock().unwrap();
        client.execute(
            "UPDATE scan_jobs SET
                status = $2,
                last_path = $3,
                scanned_dirs = $4,
                scanned_files = $5,
                total_assets = $6,
                shallow_blocked = $7,
                error_message = $8,
                started_at = $9,
                paused_at = $10,
                completed_at = $11,
                updated_at = NOW()::text
             WHERE id = $1",
            &[
                &job.id,
                &job.status,
                &job.last_path,
                &job.scanned_dirs,
                &job.scanned_files,
                &job.total_assets,
                &job.shallow_blocked,
                &job.error_message,
                &job.started_at,
                &job.paused_at,
                &job.completed_at,
            ],
        )?;
        Ok(())
    }

    fn get_scan_job(&self, id: &str) -> Result<Option<ScanJobRecord>> {
        let mut client = self.client.lock().unwrap();
        let rows = client.query(
            "SELECT id, root_path, status, last_path, scanned_dirs, scanned_files, total_assets, shallow_blocked, error_message, started_at, paused_at, completed_at, created_at, updated_at
             FROM scan_jobs WHERE id = $1",
            &[&id],
        )?;

        if let Some(row) = rows.first() {
            Ok(Some(ScanJobRecord {
                id: row.get(0),
                root_path: row.get(1),
                status: row.get(2),
                last_path: row.get(3),
                scanned_dirs: row.get(4),
                scanned_files: row.get(5),
                total_assets: row.get(6),
                shallow_blocked: row.get(7),
                error_message: row.get(8),
                started_at: row.get(9),
                paused_at: row.get(10),
                completed_at: row.get(11),
                created_at: row.get(12),
                updated_at: row.get(13),
            }))
        } else {
            Ok(None)
        }
    }

    fn list_scan_jobs(&self, limit: usize) -> Result<Vec<ScanJobRecord>> {
        let mut client = self.client.lock().unwrap();
        let rows = client.query(
            "SELECT id, root_path, status, last_path, scanned_dirs, scanned_files, total_assets, shallow_blocked, error_message, started_at, paused_at, completed_at, created_at, updated_at
             FROM scan_jobs ORDER BY created_at DESC LIMIT $1",
            &[&(limit as i64)],
        )?;

        let mut results = Vec::new();
        for row in rows {
            results.push(ScanJobRecord {
                id: row.get(0),
                root_path: row.get(1),
                status: row.get(2),
                last_path: row.get(3),
                scanned_dirs: row.get(4),
                scanned_files: row.get(5),
                total_assets: row.get(6),
                shallow_blocked: row.get(7),
                error_message: row.get(8),
                started_at: row.get(9),
                paused_at: row.get(10),
                completed_at: row.get(11),
                created_at: row.get(12),
                updated_at: row.get(13),
            });
        }
        Ok(results)
    }

    fn get_active_scan_job(&self) -> Result<Option<ScanJobRecord>> {
        let mut client = self.client.lock().unwrap();
        let rows = client.query(
            "SELECT id, root_path, status, last_path, scanned_dirs, scanned_files, total_assets, shallow_blocked, error_message, started_at, paused_at, completed_at, created_at, updated_at
             FROM scan_jobs WHERE status IN ('pending', 'running', 'paused') ORDER BY updated_at DESC LIMIT 1",
            &[],
        )?;

        if let Some(row) = rows.first() {
            Ok(Some(ScanJobRecord {
                id: row.get(0),
                root_path: row.get(1),
                status: row.get(2),
                last_path: row.get(3),
                scanned_dirs: row.get(4),
                scanned_files: row.get(5),
                total_assets: row.get(6),
                shallow_blocked: row.get(7),
                error_message: row.get(8),
                started_at: row.get(9),
                paused_at: row.get(10),
                completed_at: row.get(11),
                created_at: row.get(12),
                updated_at: row.get(13),
            }))
        } else {
            Ok(None)
        }
    }

    fn upsert_fact_node(&self, node: &FactNodeRecord) -> Result<()> {
        let mut client = self.client.lock().unwrap();
        client.execute(
            "INSERT INTO fact_nodes (id, node_type, name, label, properties, updated_at)
             VALUES ($1, $2, $3, $4, $5, NOW())
             ON CONFLICT (id) DO UPDATE SET
                node_type = EXCLUDED.node_type,
                name = EXCLUDED.name,
                label = EXCLUDED.label,
                properties = EXCLUDED.properties,
                updated_at = NOW()",
            &[&node.id, &node.node_type, &node.name, &node.label, &node.properties],
        )?;
        Ok(())
    }

    fn upsert_fact_edge(&self, edge: &FactEdgeRecord) -> Result<()> {
        let mut client = self.client.lock().unwrap();
        client.execute(
            "INSERT INTO fact_edges (source_id, target_id, relation, properties, confidence, updated_at)
             VALUES ($1, $2, $3, $4, $5, NOW())
             ON CONFLICT (source_id, target_id, relation) DO UPDATE SET
                properties = EXCLUDED.properties,
                confidence = EXCLUDED.confidence,
                updated_at = NOW()",
            &[&edge.source_id, &edge.target_id, &edge.relation, &edge.properties, &edge.confidence],
        )?;
        Ok(())
    }

    fn get_fact_node(&self, id: &str) -> Result<Option<FactNodeRecord>> {
        let mut client = self.client.lock().unwrap();
        let rows = client.query(
            "SELECT id, node_type, name, label, properties, updated_at FROM fact_nodes WHERE id = $1",
            &[&id],
        )?;

        if let Some(row) = rows.first() {
            let updated: Option<chrono::DateTime<chrono::Utc>> = row.get(5);
            Ok(Some(FactNodeRecord {
                id: row.get(0),
                node_type: row.get(1),
                name: row.get(2),
                label: row.get(3),
                properties: row.get(4),
                updated_at: updated.map(|u| u.to_rfc3339()),
            }))
        } else {
            Ok(None)
        }
    }

    fn find_fact_nodes_by_name(&self, name: &str) -> Result<Vec<FactNodeRecord>> {
        let mut client = self.client.lock().unwrap();
        let pattern = format!("%{}%", name);
        let rows = client.query(
            "SELECT id, node_type, name, label, properties, updated_at FROM fact_nodes 
             WHERE name ILIKE $1 OR label ILIKE $1 LIMIT 50",
            &[&pattern],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let updated: Option<chrono::DateTime<chrono::Utc>> = row.get(5);
            results.push(FactNodeRecord {
                id: row.get(0),
                node_type: row.get(1),
                name: row.get(2),
                label: row.get(3),
                properties: row.get(4),
                updated_at: updated.map(|u| u.to_rfc3339()),
            });
        }
        Ok(results)
    }

    fn traverse_fact_graph(&self, root_id: &str, max_hops: usize) -> Result<SubGraph> {
        let root = match self.get_fact_node(root_id)? {
            Some(r) => r,
            None => return Ok(SubGraph::default()),
        };

        let mut client = self.client.lock().unwrap();
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
                let rows = client.query(
                    "SELECT id, source_id, target_id, relation, properties, confidence, updated_at
                     FROM fact_edges
                     WHERE source_id = $1 OR target_id = $1",
                    &[&node_id],
                )?;

                for row in rows {
                    let id_i64: i64 = row.get(0);
                    let src: String = row.get(1);
                    let tgt: String = row.get(2);
                    let rel: String = row.get(3);
                    let props: serde_json::Value = row.get(4);
                    let conf: f32 = row.get(5);
                    let updated: Option<chrono::DateTime<chrono::Utc>> = row.get(6);

                    let other_id = if &src == node_id {
                        tgt.clone()
                    } else {
                        src.clone()
                    };

                    if !visited_node_ids.contains(&other_id) {
                        visited_node_ids.insert(other_id.clone());
                        next_frontier.push(other_id);
                    }

                    if !collected_edges.iter().any(|existing: &FactEdgeRecord| {
                        existing.source_id == src && existing.target_id == tgt && existing.relation == rel
                    }) {
                        collected_edges.push(FactEdgeRecord {
                            id: Some(id_i64),
                            source_id: src,
                            target_id: tgt,
                            relation: rel,
                            properties: props,
                            confidence: conf,
                            updated_at: updated.map(|u| u.to_rfc3339()),
                        });
                    }
                }
            }
            current_frontier = next_frontier;
        }

        let mut collected_nodes = Vec::new();
        for nid in &visited_node_ids {
            let rows = client.query(
                "SELECT id, node_type, name, label, properties, updated_at FROM fact_nodes WHERE id = $1",
                &[&nid],
            )?;
            if let Some(row) = rows.first() {
                let updated: Option<chrono::DateTime<chrono::Utc>> = row.get(5);
                collected_nodes.push(FactNodeRecord {
                    id: row.get(0),
                    node_type: row.get(1),
                    name: row.get(2),
                    label: row.get(3),
                    properties: row.get(4),
                    updated_at: updated.map(|u| u.to_rfc3339()),
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
        let mut client = self.client.lock().unwrap();
        let rows = client.query(
            "SELECT id, node_type, name, label, properties, updated_at FROM fact_nodes ORDER BY id LIMIT $1",
            &[&(limit as i64)],
        )?;
        let mut nodes = Vec::new();
        for row in rows {
            let updated: Option<chrono::DateTime<chrono::Utc>> = row.get(5);
            nodes.push(FactNodeRecord {
                id: row.get(0),
                node_type: row.get(1),
                name: row.get(2),
                label: row.get(3),
                properties: row.get(4),
                updated_at: updated.map(|u| u.to_rfc3339()),
            });
        }
        Ok(nodes)
    }

    fn list_all_fact_edges(&self, limit: usize) -> Result<Vec<FactEdgeRecord>> {
        let mut client = self.client.lock().unwrap();
        let rows = client.query(
            "SELECT id, source_id, target_id, relation, properties, confidence, updated_at FROM fact_edges ORDER BY id LIMIT $1",
            &[&(limit as i64)],
        )?;
        let mut edges = Vec::new();
        for row in rows {
            let id_i64: i64 = row.get(0);
            let updated: Option<chrono::DateTime<chrono::Utc>> = row.get(6);
            edges.push(FactEdgeRecord {
                id: Some(id_i64),
                source_id: row.get(1),
                target_id: row.get(2),
                relation: row.get(3),
                properties: row.get(4),
                confidence: row.get(5),
                updated_at: updated.map(|u| u.to_rfc3339()),
            });
        }
        Ok(edges)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PG_TEST_URL: &str = "postgresql://postgres@127.0.0.1:5432/agent_steward_test";

    fn get_test_provider() -> Option<PgStorageProvider> {
        PgStorageProvider::connect(PG_TEST_URL).ok()
    }

    #[test]
    fn test_pg_provider_schema_init_and_presets() {
        let provider = match get_test_provider() {
            Some(p) => p,
            None => {
                eprintln!("跳过本地 PG 测试 (本地 PG 未就绪)");
                return;
            }
        };

        assert_eq!(provider.provider_name(), "postgres");

        let rules = provider.get_placement_rules().expect("get placement rules");
        assert_eq!(rules.len(), 8, "PG 应加载 8 条默认放置规则");

        let policies = provider.get_scan_policies().expect("get scan policies");
        assert_eq!(policies.len(), 10, "PG 应加载 10 条默认扫描策略");
    }

    #[test]
    fn test_pg_provider_assets_crud_and_tsvector() {
        let provider = match get_test_provider() {
            Some(p) => p,
            None => return,
        };

        let uid = uuid::Uuid::new_v4();
        let test_path = format!("/root/new/shared_proj/pg_analyzer_{}", uid);
        let asset = AssetRecord {
            id: format!("pg-asset-{}", uid),
            path: test_path.clone(),
            asset_type: "script".to_string(),
            owner: "user:digoal".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some("PostgreSQL 性能诊断与内核分析脚本".to_string()),
            desc_kv: serde_json::json!({"engine": "postgres", "author": "digoal"}),
            usage: Some("python run_pg.py".to_string()),
            size_bytes: Some(4096),
            file_count: Some(2),
            tags: vec!["postgres".to_string(), "analyzer".to_string()],
            status: "active".to_string(),
        };

        provider.upsert_asset(&asset).expect("upsert asset");

        let fetched = provider.get_asset_by_path(&test_path)
            .expect("get asset")
            .expect("must exist");
        assert_eq!(fetched.owner, "user:digoal");

        // 测试 tsvector 全文检索
        let search_res = provider.search_assets_fts("内核分析", 10).expect("fts search");
        assert!(!search_res.is_empty(), "PG tsvector 检索应该成功命中目标资产");
        assert!(search_res.iter().any(|r| r.id == asset.id));
    }

    #[test]
    fn test_pg_provider_property_graph() {
        let provider = match get_test_provider() {
            Some(p) => p,
            None => return,
        };

        let node = FactNodeRecord {
            id: "service:pg_test_daemon".to_string(),
            node_type: "service".to_string(),
            name: "pg_test_daemon".to_string(),
            label: "PG测试守护进程".to_string(),
            properties: serde_json::json!({"port": 5432}),
            updated_at: None,
        };
        provider.upsert_fact_node(&node).expect("upsert fact node");

        let fetched_node = provider.get_fact_node("service:pg_test_daemon")
            .expect("get node")
            .expect("must exist");
        assert_eq!(fetched_node.name, "pg_test_daemon");

        let port_node = FactNodeRecord {
            id: "port:5432".to_string(),
            node_type: "port".to_string(),
            name: "5432".to_string(),
            label: "PostgreSQL端口".to_string(),
            properties: serde_json::json!({}),
            updated_at: None,
        };
        provider.upsert_fact_node(&port_node).expect("upsert port node");

        let edge = FactEdgeRecord {
            id: None,
            source_id: "service:pg_test_daemon".to_string(),
            target_id: "port:5432".to_string(),
            relation: "listens_on".to_string(),
            properties: serde_json::json!({}),
            confidence: 1.0,
            updated_at: None,
        };
        provider.upsert_fact_edge(&edge).expect("upsert edge");

        let sub = provider.traverse_fact_graph("service:pg_test_daemon", 1).expect("traverse graph");
        assert_eq!(sub.nodes.len(), 2);
        assert_eq!(sub.edges.len(), 1);
    }
}
