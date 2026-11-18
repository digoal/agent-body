pub mod matcher;
pub mod state_machine;
pub mod importance;

pub use state_machine::ScanStateMachine;
pub use importance::{ImportanceScorer, NoiseFilter, ImportanceScore};

use std::path::Path;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use walkdir::WalkDir;
use crate::storage::Storage;
use crate::storage::models::AssetRecord;
use matcher::PolicyMatcher;

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
    pub noise_skipped_files: usize,
    pub total_assets_recorded: usize,
}

pub struct Scanner {
    storage: Storage,
    matcher: PolicyMatcher,
    noise_filter: NoiseFilter,
}

impl Scanner {
    pub fn new(storage: Storage) -> Result<Self> {
        let policies = storage.get_scan_policies()?;
        let matcher = PolicyMatcher::from_policies(&policies)?;
        let noise_filter = NoiseFilter::default();
        Ok(Self { storage, matcher, noise_filter })
    }

    pub fn with_noise_filter(storage: Storage, noise_filter: NoiseFilter) -> Result<Self> {
        let policies = storage.get_scan_policies()?;
        let matcher = PolicyMatcher::from_policies(&policies)?;
        Ok(Self { storage, matcher, noise_filter })
    }

    /// 执行高性能分级颗粒度扫描
    pub fn scan_path(&self, root_dir: &Path) -> Result<ScanReport> {
        let mut report = ScanReport::default();
        let mut it = WalkDir::new(root_dir).into_iter();

        let mut batch_records = Vec::new();

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

                    batch_records.push(record);
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
                let mtime_secs = metadata.as_ref()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs());

                // 智能重要性评分与噪声过滤
                let (should_record, imp_score) = self.noise_filter.should_record(path, &asset_type, size_bytes, mtime_secs);
                if !should_record {
                    report.noise_skipped_files += 1;
                    continue;
                }

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
                        "tier": tier,
                        "importance_score": imp_score.score,
                        "importance_reason": imp_score.reason
                    }),
                    usage: None,
                    size_bytes,
                    file_count: None,
                    tags,
                    status: "active".to_string(),
                };

                batch_records.push(record);
                report.deep_indexed_files += 1;
                report.total_assets_recorded += 1;
            }

            // 批处理提交优化：每 200 条批量持久化一次，极大提升大目录扫描性能
            if batch_records.len() >= 200 {
                self.flush_batch(&batch_records)?;
                batch_records.clear();
            }
        }

        // 提交剩余批次
        if !batch_records.is_empty() {
            self.flush_batch(&batch_records)?;
        }

        Ok(report)
    }

    fn flush_batch(&self, records: &[AssetRecord]) -> Result<()> {
        for r in records {
            self.storage.upsert_asset(r)?;
        }
        Ok(())
    }

    /// 判定给定路径所属的颗粒度等级
    pub fn determine_tier(&self, path: &Path) -> String {
        self.matcher.match_tier(path).to_string()
    }
}

pub(crate) fn infer_file_attributes(path: &Path) -> (String, Vec<String>) {
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

    #[test]
    fn test_scanner_noise_filtering_behavior() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let scanner = Scanner::new(storage.clone()).expect("new scanner");

        let tmp = tempdir().expect("tempdir");
        let root = tmp.path();

        // 1. 创建用户工作目录下的有效代码
        let work_dir = root.join("work").join("my_app");
        create_dir_all(&work_dir).unwrap();
        File::create(work_dir.join("app.rs")).unwrap().write_all(b"fn run() {}").unwrap();

        // 2. 创建临时垃圾日志文件
        let tmp_dir = root.join("tmp");
        create_dir_all(&tmp_dir).unwrap();
        File::create(tmp_dir.join("debug.log")).unwrap().write_all(b"debug info").unwrap();

        let report = scanner.scan_path(root).expect("scan_path");
        assert!(report.deep_indexed_files >= 1);
        assert!(report.noise_skipped_files >= 1, "临时 log 文件应当被统计为跳过噪声");

        let app_asset = storage.get_asset_by_path(&work_dir.join("app.rs").to_string_lossy().to_string()).unwrap();
        let log_asset = storage.get_asset_by_path(&tmp_dir.join("debug.log").to_string_lossy().to_string()).unwrap();

        assert!(app_asset.is_some(), "有效代码资产必须已入库");
        assert!(log_asset.is_none(), "垃圾日志文件绝不应该被入库");
    }
}
