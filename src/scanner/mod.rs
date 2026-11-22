pub mod matcher;
pub mod state_machine;
pub mod importance;
pub mod metadata;
pub mod ollama_annotator;

pub use state_machine::ScanStateMachine;
pub use importance::{ImportanceScorer, NoiseFilter, ImportanceScore};
pub use metadata::{
    DetailLevel, ExtractedMetadata, HeuristicMetadataExtractor, SecretRedactor,
    format_asset_line, merge_with_existing_asset, project_asset_view,
};
pub use ollama_annotator::{EnrichReport, EnrichedSample, OllamaAnnotator};

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
    ignore_patterns: Vec<String>,
    default_owner: String,
}

impl Scanner {
    pub fn new(storage: Storage) -> Result<Self> {
        let cfg = crate::config::Config::load_effective_or_default();
        Self::with_options(storage, cfg.assets.ignore_patterns.clone(), cfg.effective_default_owner())
    }

    pub fn with_options(
        storage: Storage,
        ignore_patterns: Vec<String>,
        default_owner: String,
    ) -> Result<Self> {
        let policies = storage.get_scan_policies()?;
        let matcher = PolicyMatcher::from_policies(&policies)?;
        let noise_filter = NoiseFilter::with_ignore_patterns(ignore_patterns.clone());
        Ok(Self {
            storage,
            matcher,
            noise_filter,
            ignore_patterns,
            default_owner,
        })
    }

    pub fn with_noise_filter(storage: Storage, noise_filter: NoiseFilter) -> Result<Self> {
        let cfg = crate::config::Config::load_effective_or_default();
        let default_owner = cfg.effective_default_owner();
        let policies = storage.get_scan_policies()?;
        let matcher = PolicyMatcher::from_policies(&policies)?;
        Ok(Self {
            storage,
            matcher,
            noise_filter,
            ignore_patterns: cfg.assets.ignore_patterns,
            default_owner,
        })
    }

    pub fn with_policies(storage: Storage, extra_policies: &[crate::storage::models::ScanPolicy]) -> Result<Self> {
        let cfg = crate::config::Config::load_effective_or_default();
        let default_owner = cfg.effective_default_owner();
        let mut policies = storage.get_scan_policies()?;
        policies.extend_from_slice(extra_policies);
        let matcher = PolicyMatcher::from_policies(&policies)?;
        let noise_filter = NoiseFilter::with_ignore_patterns(cfg.assets.ignore_patterns.clone());
        Ok(Self {
            storage,
            matcher,
            noise_filter,
            ignore_patterns: cfg.assets.ignore_patterns,
            default_owner,
        })
    }

    fn matches_configured_ignore(&self, path: &Path) -> bool {
        self.ignore_patterns
            .iter()
            .any(|pat| crate::scanner::importance::path_matches_ignore_pattern(path, pat))
    }

    /// 执行高性能分级颗粒度扫描
    pub fn scan_path(&self, root_dir: &Path) -> Result<ScanReport> {
        let mut report = ScanReport::default();
        let mut it = WalkDir::new(root_dir).into_iter();
        let links_engine = crate::links::LinksEngine::new(std::sync::Arc::new(self.storage.clone()));

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

                if tier == "shallow" || self.matches_configured_ignore(path) {
                    // ★ 黑名单 / 配置 ignore_patterns 规则：止于当前目录层级，不再深入递归！
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
                if tier == "shallow" {
                    report.noise_skipped_files += 1;
                    continue;
                }
                if tier == "restricted" && !should_index_in_restricted(root_dir, path) {
                    report.noise_skipped_files += 1;
                    continue;
                }
                let (asset_type, mut tags) = infer_file_attributes(path);
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

                // L1 确定性微秒级元数据提取 (<1ms, 0 LLM 开销)
                let extracted = HeuristicMetadataExtractor::extract(
                    path,
                    &asset_type,
                    &tier,
                    Some(&imp_score),
                    size_bytes,
                );
                for t in extracted.extra_tags {
                    if !tags.contains(&t) {
                        tags.push(t);
                    }
                }

                let record = AssetRecord {
                    id: uuid::Uuid::new_v4().to_string(),
                    path: path_str,
                    asset_type,
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

                batch_records.push(record);
                report.deep_indexed_files += 1;
                report.total_assets_recorded += 1;

                if tier == "deep" {
                    let _ = links_engine.refresh_links_for_asset(path);
                }
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

        let blocked_total = report.shallow_blocked_dirs.len() + report.noise_skipped_files;
        if blocked_total > 0 {
            crate::benchmark::ledger::EffectivenessLedger::record(
                self.storage.provider().as_ref(),
                crate::benchmark::ledger::LedgerEventInput {
                    session_id: "scanner".into(),
                    source_channel: "scanner".into(),
                    tool_or_action: "body_asset_scan".into(),
                    rule: crate::benchmark::provenance::ProvenanceRule::Pr06GranularityNoiseGate,
                    baseline_tokens: (blocked_total as i64) * 25,
                    steward_tokens: 0,
                    baseline_rounds: 1,
                    steward_rounds: 1,
                    hazard_intercepted: report.shallow_blocked_dirs.len() as i64,
                    noise_blocked: blocked_total as i64,
                    elapsed_us: 0,
                    evidence_summary: format!(
                        "PR-06: 阻断 {} 个黑名单目录与 {} 个噪音文件入库",
                        report.shallow_blocked_dirs.len(),
                        report.noise_skipped_files
                    ),
                },
            );
        }

        Ok(report)
    }

    fn flush_batch(&self, records: &[AssetRecord]) -> Result<()> {
        for r in records {
            let existing = self.storage.get_asset_by_path(&r.path).ok().flatten();
            let merged = merge_with_existing_asset(r.clone(), existing.as_ref());
            self.storage.upsert_asset(&merged)?;
        }
        Ok(())
    }

    /// 判定给定路径所属的颗粒度等级
    pub fn determine_tier(&self, path: &Path) -> String {
        self.matcher.match_tier(path).to_string()
    }
}

/// 受限扫描（restricted）仅允许索引相对深度 <= 2 的入口文档、工程清单与顶层入口脚本，
/// 防止第三方开源仓库或对比源码树中的成千上万测试/子模块文件打爆资产库。
pub(crate) fn should_index_in_restricted(root_dir: &Path, path: &Path) -> bool {
    let rel = path.strip_prefix(root_dir).unwrap_or(path);
    let depth = rel.components().count();
    if depth > 3 {
        return false;
    }

    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    file_name.starts_with("readme")
        || file_name.starts_with("install")
        || file_name.starts_with("license")
        || matches!(
            file_name.as_str(),
            "skill.md"
                | "cargo.toml"
                | "pyproject.toml"
                | "package.json"
                | "go.mod"
                | "makefile"
                | "dockerfile"
                | "docker-compose.yml"
                | "config.example.toml"
                | "main.rs"
                | "lib.rs"
                | "main.py"
                | "__init__.py"
                | "app.py"
                | "index.js"
                | "index.ts"
        )
}

pub(crate) fn infer_file_attributes(path: &Path) -> (String, Vec<String>) {
    let mut tags = Vec::new();
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    let asset_type = match ext.as_str() {
        "rs" => {
            tags.push("rust".to_string());
            "script".to_string()
        }
        "py" => {
            tags.push("python".to_string());
            "script".to_string()
        }
        "sh" | "bash" | "zsh" => {
            tags.push("shell".to_string());
            "script".to_string()
        }
        "js" | "mjs" | "cjs" | "jsx" => {
            tags.push("javascript".to_string());
            "script".to_string()
        }
        "ts" | "tsx" => {
            tags.push("typescript".to_string());
            "script".to_string()
        }
        "go" => {
            tags.push("go".to_string());
            "script".to_string()
        }
        "c" | "cpp" | "cc" | "h" | "hpp" => {
            tags.push("c_cpp".to_string());
            "script".to_string()
        }
        "lua" | "rb" | "pl" => {
            tags.push(ext.clone());
            "script".to_string()
        }
        "md" | "markdown" | "txt" | "rst" | "adoc" | "org" | "html" | "htm" | "pdf" => {
            tags.push("document".to_string());
            if !ext.is_empty() && ext != "md" && ext != "txt" {
                tags.push(ext.clone());
            }
            "document".to_string()
        }
        "toml" | "json" | "yaml" | "yml" | "sql" | "ini" | "conf" | "cfg" | "service" | "env"
        | "xml" | "xsd" => {
            tags.push("config".to_string());
            if !ext.is_empty() {
                tags.push(ext.clone());
            }
            "config".to_string()
        }
        "csv" | "tsv" | "jsonl" | "ndjson" | "parquet" | "db" | "sqlite" | "sqlite3" | "duckdb" => {
            tags.push("dataset".to_string());
            tags.push(ext.clone());
            "dataset".to_string()
        }
        "svg" | "png" | "jpg" | "jpeg" | "webp" | "gif" | "ico" | "mp3" | "wav" | "ogg"
        | "mp4" | "webm" => {
            tags.push("media".to_string());
            tags.push(ext.clone());
            "media".to_string()
        }
        _ => {
            if matches!(
                file_name.as_str(),
                "makefile" | "dockerfile" | "cargo.lock" | "cmakelists.txt" | ".gitignore"
            ) {
                tags.push("config".to_string());
                "config".to_string()
            } else {
                "other".to_string()
            }
        }
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

    #[test]
    fn test_git_and_restricted_tier_filtering() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let tmp = tempdir().expect("tempdir");
        let root = tmp.path();

        // 显式注册一条针对 tmp/src/** 的 restricted 策略（等价于生产库的 /root/new/src/**）
        let src_root = root.join("src");
        let scanner = Scanner::with_policies(
            storage.clone(),
            &[crate::storage::models::ScanPolicy {
                id: "test-restricted-src".to_string(),
                path_pattern: format!("{}/**", src_root.to_string_lossy()),
                tier: "restricted".to_string(),
                max_depth: 2,
                enable_fts: true,
                enable_vec: false,
                enable_graph: false,
                notes: Some("test restricted".to_string()),
            }],
        )
        .expect("new scanner");

        // 1. .git 目录自身及内部子文件必须零逃逸
        let git_dir = root.join("work").join("my_repo").join(".git");
        create_dir_all(git_dir.join("refs")).unwrap();
        File::create(git_dir.join("config")).unwrap().write_all(b"[core]").unwrap();
        File::create(git_dir.join("COMMIT_EDITMSG")).unwrap().write_all(b"init").unwrap();

        // 2. restricted 目录 /src/repo/ 仅索引 README.md / Cargo.toml，不索引深层测试文件
        let src_repo = root.join("src").join("demo_repo");
        create_dir_all(src_repo.join("tests").join("deep")).unwrap();
        File::create(src_repo.join("README.md")).unwrap().write_all(b"# Demo Repo").unwrap();
        File::create(src_repo.join("Cargo.toml")).unwrap().write_all(b"[package]").unwrap();
        File::create(src_repo.join("tests/deep/heavy_test.rs")).unwrap().write_all(b"fn t() {}").unwrap();

        let report = scanner.scan_path(root).expect("scan_path");
        assert!(report.shallow_blocked_dirs.iter().any(|d| d.path.ends_with(".git")));

        assert!(
            storage.get_asset_by_path(&git_dir.join("config").to_string_lossy()).unwrap().is_none(),
            ".git/config 绝不能逃逸入库"
        );
        assert!(
            storage.get_asset_by_path(&src_repo.join("README.md").to_string_lossy()).unwrap().is_some(),
            "restricted 目录下的 README.md 必须被索引"
        );
        assert!(
            storage.get_asset_by_path(&src_repo.join("tests/deep/heavy_test.rs").to_string_lossy()).unwrap().is_none(),
            "restricted 目录下的深层测试文件不得被索引"
        );
    }
}
