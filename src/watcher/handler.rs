use anyhow::Result;
use crate::storage::Storage;
use crate::storage::models::{AssetRecord, ChangeRecord};
use crate::scanner::Scanner;
use crate::watcher::{FileChangeEvent, FileChangeKind};

pub struct ChangeHandler {
    storage: Storage,
    scanner: Scanner,
    event_bus: Option<crate::hooks::EventBusHandle>,
}

impl ChangeHandler {
    pub fn new(storage: Storage) -> Result<Self> {
        let scanner = Scanner::new(storage.clone())?;
        Ok(Self { storage, scanner, event_bus: None })
    }

    pub fn with_event_bus(storage: Storage, event_bus: crate::hooks::EventBusHandle) -> Result<Self> {
        let scanner = Scanner::new(storage.clone())?;
        Ok(Self { storage, scanner, event_bus: Some(event_bus) })
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
                        let metadata = event.path.metadata().ok();
                        let size_bytes = metadata.map(|m| m.len() as i64);
                        let ext = event.path.extension().and_then(|s| s.to_str()).unwrap_or("");
                        let asset_type = match ext {
                            "rs" | "py" | "sh" | "js" | "ts" => "script",
                            "md" | "txt" => "document",
                            "toml" | "json" | "yaml" | "sql" => "config",
                            _ => "other",
                        };

                        let record = AssetRecord {
                            id: uuid::Uuid::new_v4().to_string(),
                            path: path_str.clone(),
                            asset_type: asset_type.to_string(),
                            owner: "user:digoal".to_string(),
                            security_level: "internal".to_string(),
                            scan_tier: tier.clone(),
                            description: Some(format!("文件变更动态同步 ({})", event.path.file_name().unwrap_or_default().to_string_lossy())),
                            desc_kv: serde_json::json!({"watcher_sync": true}),
                            usage: None,
                            size_bytes,
                            file_count: None,
                            tags: vec![ext.to_string()],
                            status: "active".to_string(),
                        };
                        self.storage.upsert_asset(&record)?;
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
                        let _ = bus.emit(steward_ev);
                    }
                    processed += 1;
                }

                FileChangeKind::Deleted => {
                    if let Ok(Some(mut asset)) = self.storage.get_asset_by_path(&path_str) {
                        asset.status = "missing".to_string();
                        self.storage.upsert_asset(&asset)?;
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
                        let _ = bus.emit(crate::hooks::StewardEvent::FileDeleted {
                            path: path_str,
                            source: "watcher".to_string(),
                            timestamp: chrono::Local::now().to_rfc3339(),
                        });
                    }
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
}
