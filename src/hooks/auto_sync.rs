use std::fs;
use std::path::Path;
use anyhow::Result;
use crate::storage::Storage;
use crate::storage::models::{AssetRecord, MemoryRecord, FactNodeRecord};
use crate::scanner::importance::{ImportanceScorer, RuleBasedImportanceScorer};

/// 自动回写同步报告
#[derive(Debug, Clone, Default)]
pub struct AutoSyncResult {
    pub asset_registered: bool,
    pub memory_created: Option<String>,
    pub topology_updated: bool,
    pub reason: String,
}

/// Post-Hook 全自动回写引擎
pub struct AutoSyncEngine {
    storage: Storage,
    scorer: Box<dyn ImportanceScorer>,
    blacklist_patterns: Vec<String>,
}

impl AutoSyncEngine {
    pub fn new(storage: Storage) -> Self {
        let blacklist_patterns = vec![
            ".git".to_string(),
            "node_modules".to_string(),
            "__pycache__".to_string(),
            ".steward".to_string(),
            "target".to_string(),
            ".venv".to_string(),
            ".cache".to_string(),
        ];
        Self {
            storage,
            scorer: Box::new(RuleBasedImportanceScorer::default_scorer()),
            blacklist_patterns,
        }
    }

    /// 评估路径是否在黑名单中 (规约 #6 白黑名单深度法则)
    pub fn is_blacklisted(&self, path: &Path) -> bool {
        for comp in path.components() {
            let comp_str = comp.as_os_str().to_string_lossy();
            for pattern in &self.blacklist_patterns {
                if pattern.trim_matches('/').trim_matches('*') == comp_str {
                    return true;
                }
            }
        }
        false
    }

    /// 执行完整的产出文件后置自动同步 (资产注册 + 门禁记忆沉淀 + 拓扑关联)
    pub fn sync_file_output(&self, path_str: &str, tool_name: &str) -> Result<AutoSyncResult> {
        let path = Path::new(path_str);

        // 1. 黑名单目录严格过滤（无论文件当前是否已在磁盘生成，黑名单目录一律最优先拦截）
        if self.is_blacklisted(path) {
            return Ok(AutoSyncResult {
                asset_registered: false,
                memory_created: None,
                topology_updated: false,
                reason: "命中黑名单目录，依据白黑名单深度法则严禁入库".to_string(),
            });
        }

        if !path.exists() {
            return Ok(AutoSyncResult {
                asset_registered: false,
                memory_created: None,
                topology_updated: false,
                reason: "目标文件在宿主机上不存在，跳过回写".to_string(),
            });
        }

        let mut res = AutoSyncResult::default();

        // 2. 自动资产注册 (Asset Catalog Ingestion)
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        let asset_type = match ext {
            "rs" | "py" | "sh" | "js" | "ts" => "script",
            "md" | "txt" => "document",
            "toml" | "json" | "yaml" | "sql" => "config",
            _ => "other",
        };

        let metadata = path.metadata().ok();
        let size_bytes = metadata.map(|m| m.len() as i64);

        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
        let asset_rec = AssetRecord {
            id: uuid::Uuid::new_v4().to_string(),
            path: path_str.to_string(),
            asset_type: asset_type.to_string(),
            owner: "user:digoal".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some(format!("Agent 通过 `{}` 产出的新资产 ({})", tool_name, file_name)),
            desc_kv: serde_json::json!({"created_by_tool": tool_name}),
            usage: None,
            size_bytes,
            file_count: None,
            tags: vec![ext.to_string(), "agent_output".to_string()],
            status: "active".to_string(),
        };

        self.storage.upsert_asset(&asset_rec)?;
        res.asset_registered = true;

        // 3. 自动记忆沉淀门禁 (规约 #4 摄入门禁 & 规约 #2 禁止使用外部 LLM)
        if ext == "md" || ext == "toml" || ext == "json" || path_str.contains("SKILL.md") {
            let score = self.scorer.score_asset(path, asset_type, size_bytes, None);

            // 只要不是被明确判定为噪音垃圾 (is_noise = false)，对于显式产出的知识文档或配置，提取关键首部沉淀为长期记忆
            if !score.is_noise {
                if let Ok(content) = fs::read_to_string(path) {
                    let mut summary_lines = Vec::new();
                    for line in content.lines().take(5) {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            summary_lines.push(trimmed);
                        }
                    }
                    let summary_str = summary_lines.join(" ");

                    let mem_id = format!("mem-hook-{}", uuid::Uuid::new_v4());
                    let mem_rec = MemoryRecord {
                        id: mem_id.clone(),
                        memory_type: "note".to_string(),
                        content: format!("产出文件 `{}` 摘要:\n{}", path_str, summary_str),
                        summary: Some(format!("文件产出沉淀: {}", file_name)),
                        source_agent: Some(format!("hook:{}", tool_name)),
                        source_session: None,
                        tags: vec!["auto_sync".to_string(), ext.to_string()],
                        weight: 0.7,
                        pinned: false,
                        status: "active".to_string(),
                        embedding: vec![],
                    };

                    match self.storage.insert_memory(&mem_rec) {
                        Ok(_) => {
                            res.memory_created = Some(mem_id);
                        }
                        Err(e) => {
                            eprintln!("AutoSyncEngine failed to insert memory: {}", e);
                        }
                    }
                }
            }
        }

        // 4. 宏观属性图拓扑刷新
        let node_id = format!("file:{}", path_str);
        let fact_node = FactNodeRecord {
            id: node_id.clone(),
            node_type: "file".to_string(),
            name: file_name.to_string(),
            label: format!("{}:{}", asset_type, file_name),
            properties: serde_json::json!({
                "path": path_str,
                "asset_type": asset_type,
                "source": "hook_auto_sync"
            }),
            updated_at: Some(chrono::Local::now().to_rfc3339()),
        };
        let _ = self.storage.upsert_fact_node(&fact_node);
        res.topology_updated = true;
        res.reason = "产出文件自动完成资产入库、记忆沉淀与属性图关联".to_string();

        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_sync_file_output_full_flow() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let engine = AutoSyncEngine::new(storage.clone());

        // 创建带 /work/ 目录且 .md 后缀的临时文件，确保通过重要性门禁
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let work_dir = temp_dir.path().join("work");
        fs::create_dir_all(&work_dir).expect("create dir");
        let file_path = work_dir.join("architecture.md");
        fs::write(&file_path, "# Architecture 架构重构决策记录\n核心设计原则落地于 Phase 14。\n第二行重要信息").expect("write file");
        let path = file_path.to_str().unwrap();

        let sync_res = engine.sync_file_output(path, "write_file").expect("sync");
        assert!(sync_res.asset_registered);
        assert!(sync_res.topology_updated);
        assert!(sync_res.memory_created.is_some());

        // 验证资产已注册
        let asset = storage.get_asset_by_path(path).unwrap().expect("asset");
        assert_eq!(asset.asset_type, "document");

        // 验证记忆已沉淀
        let mems = storage.list_all_active_memories(5).unwrap();
        assert!(!mems.is_empty());
        assert!(mems[0].content.contains("架构重构决策记录"));

        // 验证 FTS 检索
        let fts_mems = storage.search_memories_fts("Architecture", 5).unwrap();
        assert!(!fts_mems.is_empty());
    }

    #[test]
    fn test_auto_sync_blocks_blacklist_path() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let engine = AutoSyncEngine::new(storage);

        let sync_res = engine.sync_file_output("/root/new/work/frontend/node_modules/lib/test.js", "write_file").expect("sync");
        assert!(!sync_res.asset_registered);
        assert!(sync_res.reason.contains("黑名单"));
    }
}
