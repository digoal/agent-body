use anyhow::Result;
use crate::storage::Storage;
use crate::storage::models::{AssetRecord, ChangeRecord};
use crate::scanner::Scanner;
use crate::watcher::{FileChangeEvent, FileChangeKind};

pub struct ChangeHandler {
    storage: Storage,
    scanner: Scanner,
    noise_filter: crate::scanner::NoiseFilter,
    event_bus: Option<crate::hooks::EventBusHandle>,
    default_owner: String,
}

impl ChangeHandler {
    pub fn new(storage: Storage) -> Result<Self> {
        let cfg = crate::config::Config::load_effective_or_default();
        Self::with_options(
            storage,
            None,
            cfg.assets.ignore_patterns.clone(),
            cfg.effective_default_owner(),
        )
    }

    pub fn with_event_bus(storage: Storage, event_bus: crate::hooks::EventBusHandle) -> Result<Self> {
        let cfg = crate::config::Config::load_effective_or_default();
        Self::with_options(
            storage,
            Some(event_bus),
            cfg.assets.ignore_patterns.clone(),
            cfg.effective_default_owner(),
        )
    }

    pub fn with_options(
        storage: Storage,
        event_bus: Option<crate::hooks::EventBusHandle>,
        ignore_patterns: Vec<String>,
        default_owner: String,
    ) -> Result<Self> {
        let scanner = Scanner::with_options(storage.clone(), ignore_patterns.clone(), default_owner.clone())?;
        Ok(Self {
            storage,
            scanner,
            noise_filter: crate::scanner::NoiseFilter::with_ignore_patterns(ignore_patterns),
            event_bus,
            default_owner,
        })
    }

    /// 处理一组防抖后的文件变更事件
    pub fn process_events(&self, events: &[FileChangeEvent]) -> Result<usize> {
        let mut processed = 0;

        for event in events {
            let path_str = event.path.to_string_lossy().to_string();

            match event.kind {
                FileChangeKind::Created | FileChangeKind::Modified => {
                    if !event.path.exists() {
                        continue;
                    }

                    let is_dir = event.path.is_dir();
                    let tier = self.scanner.determine_tier(&event.path);

                    if is_dir {
                        if tier == "shallow" {
                            // 黑名单目录：仅建档该目录本身标量，不深入
                            let record = AssetRecord {
                                id: uuid::Uuid::new_v4().to_string(),
                                path: path_str.clone(),
                                asset_type: "directory".to_string(),
                                owner: "system".to_string(),
                                security_level: "confidential".to_string(),
                                scan_tier: "shallow".to_string(),
                                description: Some("黑名单目录 (动态感知)".to_string()),
                                desc_kv: serde_json::json!({"policy": "shallow_watcher_blocked"}),
                                usage: None,
                                size_bytes: None,
                                file_count: None,
                                tags: vec!["shallow_blacklist".to_string()],
                                status: "active".to_string(),
                            };
                            self.storage.upsert_asset(&record)?;
                        }
                    } else if event.path.is_file() {
                        if tier == "shallow" {
                            // 黑名单子文件：直接跳过，既不入资产库也不记变更噪音
                            continue;
                        }
                        let metadata = event.path.metadata().ok();
                        let size_bytes = metadata.as_ref().map(|m| m.len() as i64);
                        let mtime_secs = metadata
                            .as_ref()
                            .and_then(|m| m.modified().ok())
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs());
                        let ext = event.path.extension().and_then(|s| s.to_str()).unwrap_or("");
                        let asset_type = match ext {
                            "rs" | "py" | "sh" | "js" | "ts" | "go" => "script",
                            "md" | "txt" => "document",
                            "toml" | "json" | "yaml" | "yml" | "sql" | "service" | "conf" => "config",
                            _ => "other",
                        };

                        let (should_record, imp_score) =
                            self.noise_filter.should_record(&event.path, asset_type, size_bytes, mtime_secs);
                        if !should_record {
                            // 噪声文件（.log / .swx / .pyc 等）跳过入库与变更审计
                            continue;
                        }

                        let extracted = crate::scanner::metadata::HeuristicMetadataExtractor::extract(
                            &event.path,
                            asset_type,
                            &tier,
                            Some(&imp_score),
                            size_bytes,
                        );
                        let mut tags = if ext.is_empty() { vec![] } else { vec![ext.to_string()] };
                        for t in extracted.extra_tags {
                            if !tags.contains(&t) {
                                tags.push(t);
                            }
                        }

                        let record = AssetRecord {
                            id: uuid::Uuid::new_v4().to_string(),
                            path: path_str.clone(),
                            asset_type: asset_type.to_string(),
                            owner: self.default_owner.clone(),
                            security_level: extracted.security_level,
                            scan_tier: tier.clone(),
                            description: Some(extracted.description),
                            desc_kv: extracted.desc_kv,
                            usage: extracted.usage,
                            size_bytes,
                            file_count: None,
                            tags,
                            status: "active".to_string(),
                        };
                        let existing = self.storage.get_asset_by_path(&path_str).ok().flatten();
                        let merged = crate::scanner::metadata::merge_with_existing_asset(record, existing.as_ref());
                        self.storage.upsert_asset(&merged)?;

                        if tier == "deep" {
                            let links_engine = crate::links::LinksEngine::new(std::sync::Arc::new(self.storage.clone()));
                            if let Err(e) = links_engine.refresh_links_for_asset(&event.path) {
                                eprintln!("[Watcher][WARN] 刷新资产链接失败 ({}): {}", path_str, e);
                            }
                        }
                    }

                    // 写入变更审计日志
                    let change_type = match event.kind {
                        FileChangeKind::Created => "file_created",
                        _ => "file_modified",
                    };
                    self.storage.record_change(&ChangeRecord {
                        id: None,
                        change_type: change_type.to_string(),
                        target: path_str.clone(),
                        description: Some("inotify 实时监听捕获并同步".to_string()),
                        details: serde_json::json!({"tier": tier}),
                        source: "watcher".to_string(),
                        created_at: None,
                    })?;

                    if let Some(ref bus) = self.event_bus {
                        let now = chrono::Local::now().to_rfc3339();
                        let steward_ev = match event.kind {
                            FileChangeKind::Created => crate::hooks::StewardEvent::FileCreated {
                                path: path_str,
                                source: "watcher".to_string(),
                                timestamp: now,
                            },
                            _ => crate::hooks::StewardEvent::FileModified {
                                path: path_str,
                                source: "watcher".to_string(),
                                timestamp: now,
                            },
                        };
                        let _delivered = bus.emit(steward_ev);
                    }
                    crate::audit::AuditLogger::record_best_effort(
                        &self.storage,
                        crate::audit::AuditEventInput::out_of_agent(
                            "watcher_inotify",
                            "watcher",
                            "fs_watch_sync",
                            change_type,
                        )
                        .with_target("file", event.path.to_string_lossy().to_string())
                        .with_outcome("success")
                        .with_delta(serde_json::json!({"tier": tier})),
                    );
                    processed += 1;
                }

                FileChangeKind::Deleted => {
                    if let Ok(Some(mut asset)) = self.storage.get_asset_by_path(&path_str) {
                        asset.status = "missing".to_string();
                        self.storage.upsert_asset(&asset)?;
                    }
                    if let Err(e) = self.storage.delete_auto_links_for_source(&path_str) {
                        eprintln!("[Watcher][WARN] 清理已删文件源边失败 ({}): {}", path_str, e);
                    }
                    if let Err(e) = self.storage.delete_links_for_target(&path_str) {
                        eprintln!("[Watcher][WARN] 清理已删文件目标边失败 ({}): {}", path_str, e);
                    }
                    if let Err(e) = self.storage.delete_fact_node(&format!("file:{}", path_str)) {
                        eprintln!("[Watcher][WARN] 清理已删文件事实节点失败 ({}): {}", path_str, e);
                    }

                    let marker = format!("产出文件 `{}` 摘要:", path_str);
                    if let Ok(mems) = self.storage.list_all_active_memories(500) {
                        for m in mems {
                            if m.content.contains(&marker) {
                                if let Err(e) = self.storage.update_memory_status(&m.id, "archived") {
                                    eprintln!("[Watcher][WARN] 归档已删文件关联记忆失败 ({}): {}", m.id, e);
                                }
                            }
                        }
                    }

                    self.storage.record_change(&ChangeRecord {
                        id: None,
                        change_type: "file_deleted".to_string(),
                        target: path_str.clone(),
                        description: Some("inotify 实时监听捕获删除".to_string()),
                        details: serde_json::json!({}),
                        source: "watcher".to_string(),
                        created_at: None,
                    })?;

                    if let Some(ref bus) = self.event_bus {
                        let _delivered = bus.emit(crate::hooks::StewardEvent::FileDeleted {
                            path: path_str.clone(),
                            source: "watcher".to_string(),
                            timestamp: chrono::Local::now().to_rfc3339(),
                        });
                    }
                    crate::audit::AuditLogger::record_best_effort(
                        &self.storage,
                        crate::audit::AuditEventInput::out_of_agent(
                            "watcher_inotify",
                            "watcher",
                            "fs_watch_delete",
                            "file_deleted",
                        )
                        .with_target("file", path_str)
                        .with_outcome("success")
                        .with_delta(serde_json::json!({"cascaded": true})),
                    );
                    processed += 1;
                }
            }
        }

        Ok(processed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_change_handler_sync() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let handler = ChangeHandler::new(storage.clone()).expect("new handler");

        let tmp = tempdir().expect("tempdir");
        let file_path = tmp.path().join("watcher_sync.py");
        let mut f = File::create(&file_path).expect("create file");
        writeln!(f, "print('sync')").expect("write");

        let event = FileChangeEvent {
            path: file_path.clone(),
            kind: FileChangeKind::Created,
        };

        let processed = handler.process_events(&[event]).expect("process_events");
        assert_eq!(processed, 1);

        // 验证 assets 表同步
        let asset = storage.get_asset_by_path(&file_path.to_string_lossy())
            .expect("get_asset")
            .expect("should exist in assets");
        assert_eq!(asset.asset_type, "script");

        // 验证 changes 表同步
        let changes = storage.list_changes(10).expect("list_changes");
        assert!(!changes.is_empty());
        assert_eq!(changes[0].change_type, "file_created");
    }

    #[test]
    fn test_change_handler_blocks_noise_and_shallow_files() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let handler = ChangeHandler::new(storage.clone()).expect("new handler");

        let tmp = tempdir().expect("tempdir");
        let log_path = tmp.path().join("daemon.log");
        File::create(&log_path).unwrap().write_all(b"noise log").unwrap();

        let git_dir = tmp.path().join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();
        let git_cfg = git_dir.join("config");
        File::create(&git_cfg).unwrap().write_all(b"[core]").unwrap();

        let processed = handler
            .process_events(&[
                FileChangeEvent { path: log_path.clone(), kind: FileChangeKind::Modified },
                FileChangeEvent { path: git_cfg.clone(), kind: FileChangeKind::Modified },
            ])
            .expect("process_events");

        assert_eq!(processed, 0, "噪声日志与 .git 内部文件必须被拦截");
        assert!(storage.get_asset_by_path(&log_path.to_string_lossy()).unwrap().is_none());
        assert!(storage.get_asset_by_path(&git_cfg.to_string_lossy()).unwrap().is_none());
        assert!(storage.list_changes(10).unwrap().is_empty());
    }

    #[test]
    fn test_change_handler_deleted_cascades_fts_fact_node_and_memories() {
        use crate::storage::models::{FactNodeRecord, MemoryRecord};

        let storage = Storage::open_in_memory().expect("open_in_memory");
        let handler = ChangeHandler::new(storage.clone()).expect("new handler");

        let tmp = tempdir().expect("tempdir");
        let file_path = tmp.path().join("ephemeral_tool.py");
        File::create(&file_path).unwrap().write_all(b"print('hi')").unwrap();
        let path_str = file_path.to_string_lossy().to_string();

        handler
            .process_events(&[FileChangeEvent {
                path: file_path.clone(),
                kind: FileChangeKind::Created,
            }])
            .unwrap();
        storage
            .upsert_fact_node(&FactNodeRecord {
                id: format!("file:{}", path_str),
                node_type: "file".to_string(),
                name: "ephemeral_tool.py".to_string(),
                label: "ephemeral_tool".to_string(),
                properties: serde_json::json!({}),
                updated_at: None,
            })
            .unwrap();
        storage
            .insert_memory(&MemoryRecord {
                id: "mem-del-1".to_string(),
                memory_type: "note".to_string(),
                content: format!("产出文件 `{}` 摘要:\nprint('hi')", path_str),
                summary: None,
                source_agent: None,
                source_session: None,
                tags: vec![],
                weight: 0.7,
                pinned: false,
                status: "active".to_string(),
                embedding: vec![],
            })
            .unwrap();

        assert!(!storage.search_assets_fts("ephemeral_tool", 10).unwrap().is_empty());

        std::fs::remove_file(&file_path).unwrap();
        handler
            .process_events(&[FileChangeEvent {
                path: file_path,
                kind: FileChangeKind::Deleted,
            }])
            .unwrap();

        assert!(
            storage.search_assets_fts("ephemeral_tool", 10).unwrap().is_empty(),
            "已删除文件不应再被 FTS 召回"
        );
        assert!(storage.get_fact_node(&format!("file:{}", path_str)).unwrap().is_none());
        assert!(storage.list_all_active_memories(10).unwrap().is_empty());
    }
}
