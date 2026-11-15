use std::path::Path;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use walkdir::WalkDir;
use crate::storage::Storage;
use crate::storage::models::{AssetRecord, ScanPolicy};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShallowBlockInfo {
    pub path: String,
    pub tier: String,
    pub reason: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ScanReport {
    pub scanned_dirs: usize,
    pub deep_indexed_files: usize,
    pub shallow_blocked_dirs: Vec<ShallowBlockInfo>,
    pub total_assets_recorded: usize,
}

pub struct Scanner {
    storage: Storage,
    policies: Vec<ScanPolicy>,
}

impl Scanner {
    pub fn new(storage: Storage) -> Result<Self> {
        let policies = storage.get_scan_policies()?;
        Ok(Self { storage, policies })
    }

    /// 执行分级颗粒度扫描
    pub fn scan_path(&self, root_dir: &Path) -> Result<ScanReport> {
        let mut report = ScanReport::default();
        let mut it = WalkDir::new(root_dir).into_iter();

        while let Some(entry_res) = it.next() {
            let entry = match entry_res {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();
            let is_dir = entry.file_type().is_dir();
            let path_str = path.to_string_lossy().to_string();

            if is_dir {
                report.scanned_dirs += 1;
                let tier = self.determine_tier(path);

                if tier == "shallow" {
                    // ★ 黑名单规则：止于当前目录层级，不再深入递归！
                    it.skip_current_dir();

                    let metadata = entry.metadata().ok();
                    let size_bytes = metadata.as_ref().map(|m| m.len() as i64);

                    let record = AssetRecord {
                        id: uuid::Uuid::new_v4().to_string(),
                        path: path_str.clone(),
                        asset_type: "directory".to_string(),
                        owner: "system".to_string(),
                        security_level: "confidential".to_string(),
                        scan_tier: "shallow".to_string(),
                        description: Some("黑名单目录: 止于当前目录层级，未向下递归解析".to_string()),
                        desc_kv: serde_json::json!({
                            "policy": "shallow_tier",
                            "blocked_descendants": true,
                            "reason": "黑名单阻断区: 避免无差别扫描打爆存储并防敏感泄露"
                        }),
                        usage: None,
                        size_bytes,
                        file_count: None,
                        tags: vec!["shallow_blacklist".to_string()],
                        status: "active".to_string(),
                    };

                    self.storage.upsert_asset(&record)?;
                    report.total_assets_recorded += 1;
                    report.shallow_blocked_dirs.push(ShallowBlockInfo {
                        path: path_str,
                        tier: "shallow".to_string(),
                        reason: "黑名单策略命中: 仅建档当前目录标量，阻断递归".to_string(),
                    });
                    continue;
                }
            } else if entry.file_type().is_file() {
                // 文件处理 (处于 deep 或 restricted 模式下)
                let tier = self.determine_tier(path);
                let (asset_type, tags) = infer_file_attributes(path);
                let metadata = entry.metadata().ok();
                let size_bytes = metadata.as_ref().map(|m| m.len() as i64);

                let record = AssetRecord {
                    id: uuid::Uuid::new_v4().to_string(),
                    path: path_str,
                    asset_type,
                    owner: "user:digoal".to_string(),
                    security_level: "internal".to_string(),
                    scan_tier: tier.clone(),
                    description: Some(format!("资产文件 ({})", entry.file_name().to_string_lossy())),
                    desc_kv: serde_json::json!({
                        "filename": entry.file_name().to_string_lossy(),
                        "tier": tier
                    }),
                    usage: None,
                    size_bytes,
                    file_count: None,
                    tags,
                    status: "active".to_string(),
                };

                self.storage.upsert_asset(&record)?;
                report.deep_indexed_files += 1;
                report.total_assets_recorded += 1;
            }
        }

        Ok(report)
    }

    /// 判定给定路径所属的颗粒度等级
    pub fn determine_tier(&self, path: &Path) -> String {
        let path_str = path.to_string_lossy();

        // 优先检查黑名单 (Shallow)
        for p in &self.policies {
            if p.tier == "shallow" && matches_pattern(&path_str, &p.path_pattern) {
                return "shallow".to_string();
            }
        }

        // 检查受限区 (Restricted)
        for p in &self.policies {
            if p.tier == "restricted" && matches_pattern(&path_str, &p.path_pattern) {
                return "restricted".to_string();
            }
        }

        // 检查白名单 (Deep)
        for p in &self.policies {
            if p.tier == "deep" && matches_pattern(&path_str, &p.path_pattern) {
                return "deep".to_string();
            }
        }

        "deep".to_string()
    }
}

fn matches_pattern(path: &str, pattern: &str) -> bool {
    // 简易通配符匹配: 支持 ** 通配及关键字匹配
    if pattern.contains("node_modules") && path.contains("node_modules") {
        return true;
    }
    if pattern.contains(".cache") && path.contains(".cache") {
        return true;
    }
    if pattern.contains(".git") && path.contains(".git") {
        return true;
    }
    if pattern.contains(".ssh") && path.contains(".ssh") {
        return true;
    }
    if pattern.contains("/proc") && path.starts_with("/proc") {
        return true;
    }

    let clean_pattern = pattern.trim_end_matches("/**").trim_end_matches('*');
    if !clean_pattern.is_empty() && path.starts_with(clean_pattern) {
        return true;
    }

    false
}

fn infer_file_attributes(path: &Path) -> (String, Vec<String>) {
    let mut tags = Vec::new();
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

    let asset_type = match ext {
        "rs" => {
            tags.push("rust".to_string());
            "script".to_string()
        }
        "py" => {
            tags.push("python".to_string());
            "script".to_string()
        }
        "sh" | "bash" => {
            tags.push("shell".to_string());
            "script".to_string()
        }
        "md" | "txt" => {
            tags.push("document".to_string());
            "document".to_string()
        }
        "toml" | "json" | "yaml" | "yml" | "sql" => {
            tags.push("config".to_string());
            "config".to_string()
        }
        _ => "other".to_string(),
    };

    (asset_type, tags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs::{create_dir_all, File};
    use std::io::Write;

    #[test]
    fn test_granularity_scanning_blacklist_shallowness() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let scanner = Scanner::new(storage.clone()).expect("new scanner");

        let tmp = tempdir().expect("tempdir");
        let root = tmp.path();

        // 1. 构建白名单深度文件: shared_proj/my_tool/main.py
        let tool_dir = root.join("shared_proj").join("my_tool");
        create_dir_all(&tool_dir).expect("create tool_dir");
        let mut f1 = File::create(tool_dir.join("main.py")).expect("create main.py");
        writeln!(f1, "print('hello')").expect("write f1");

        // 2. 构建黑名单目录与深层文件: work/proj/node_modules/heavy_dep/index.js
        let node_modules = root.join("work").join("proj").join("node_modules");
        let deep_nested = node_modules.join("heavy_dep");
        create_dir_all(&deep_nested).expect("create deep_nested");
        let mut f2 = File::create(deep_nested.join("index.js")).expect("create index.js");
        writeln!(f2, "module.exports = {{}}").expect("write f2");

        // 执行扫描
        let report = scanner.scan_path(root).expect("scan_path");

        // 验证：
        // 1. node_modules 被判定为 shallow 并阻断
        assert!(!report.shallow_blocked_dirs.is_empty(), "node_modules 必须被作为黑名单记录");
        assert!(report.shallow_blocked_dirs.iter().any(|d| d.path.contains("node_modules")));

        // 2. 数据库中应该存在 node_modules 作为 directory 资产
        let nm_asset = storage.get_asset_by_path(&node_modules.to_string_lossy())
            .expect("query node_modules asset")
            .expect("node_modules 必须作为一个目录对象记录在 assets 表中");
        assert_eq!(nm_asset.asset_type, "directory");
        assert_eq!(nm_asset.scan_tier, "shallow");

        // 3. ★ 核心断言：node_modules 下面的 heavy_dep/index.js 绝不能被作为独立资产入库！
        let nested_js_path = deep_nested.join("index.js").to_string_lossy().to_string();
        let js_asset = storage.get_asset_by_path(&nested_js_path)
            .expect("query nested js");
        assert!(js_asset.is_none(), "★ 黑名单目录下的子文件绝对不能被索引入库！");
    }
}
