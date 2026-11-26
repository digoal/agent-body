use std::fs;
use std::path::Path;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use crate::storage::Storage;
use crate::links::audit::{LinkAuditor, LinkAuditIssue};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkRepairSuggestion {
    pub source_file: String,
    pub missing_target: String,
    pub line_number: Option<i32>,
    pub original_snippet: Option<String>,
    pub candidate_target: String,
    pub similarity: f64,
    pub confidence: String, // "HIGH" | "MEDIUM" | "LOW"
    pub patch_diff: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkRepairReport {
    pub total_missing: usize,
    pub reparable_count: usize,
    pub applied_count: usize,
    pub suggestions: Vec<LinkRepairSuggestion>,
}

pub struct LinkRepairer {
    storage: Arc<Storage>,
    auditor: LinkAuditor,
}

impl LinkRepairer {
    pub fn new(storage: Arc<Storage>) -> Self {
        let auditor = LinkAuditor::new(storage.clone());
        Self { storage, auditor }
    }

    /// 计算两个字符串的 Levenshtein 相似度 (0.0 ~ 1.0)
    pub fn levenshtein_similarity(s1: &str, s2: &str) -> f64 {
        if s1 == s2 {
            return 1.0;
        }
        let len1 = s1.chars().count();
        let len2 = s2.chars().count();
        if len1 == 0 || len2 == 0 {
            return 0.0;
        }

        let mut matrix = vec![vec![0usize; len2 + 1]; len1 + 1];
        for (i, row) in matrix.iter_mut().enumerate() {
            row[0] = i;
        }
        for (j, cell) in matrix[0].iter_mut().enumerate() {
            *cell = j;
        }

        let s1_chars: Vec<char> = s1.chars().collect();
        let s2_chars: Vec<char> = s2.chars().collect();

        for i in 1..=len1 {
            for j in 1..=len2 {
                let cost = if s1_chars[i - 1] == s2_chars[j - 1] { 0 } else { 1 };
                matrix[i][j] = (matrix[i - 1][j] + 1)
                    .min(matrix[i][j - 1] + 1)
                    .min(matrix[i - 1][j - 1] + cost);
            }
        }

        let dist = matrix[len1][len2];
        let max_len = len1.max(len2);
        1.0 - (dist as f64 / max_len as f64)
    }

    /// 在已知有效资产库中寻找最匹配的替代候选
    pub fn find_best_candidate(&self, missing_target: &str) -> Option<(String, f64, String)> {
        let missing_path = Path::new(missing_target);
        let missing_filename = missing_path.file_name()?.to_str()?;

        // 从资产库获取现有资产
        let assets = self.storage.list_assets(2000).ok()?;
        let mut best_match: Option<(String, f64, String)> = None;

        for asset in assets {
            // 候选资产在磁盘上必须真实存在
            if !Path::new(&asset.path).exists() {
                continue;
            }

            let candidate_path = Path::new(&asset.path);
            let candidate_filename = match candidate_path.file_name().and_then(|f| f.to_str()) {
                Some(f) => f,
                None => continue,
            };

            // 策略 1: 同名文件迁移（文件名完全相同，只是目录移动）
            if candidate_filename == missing_filename && asset.path != missing_target {
                return Some((asset.path, 1.0, "HIGH".to_string()));
            }

            // 策略 2: 文件名相似度（小幅度重命名或中划线规范化）
            let sim = Self::levenshtein_similarity(missing_filename, candidate_filename);
            if sim >= 0.70 {
                let confidence = if sim >= 0.88 {
                    "HIGH".to_string()
                } else {
                    "MEDIUM".to_string()
                };

                if let Some((_, best_sim, _)) = &best_match {
                    if sim > *best_sim {
                        best_match = Some((asset.path, sim, confidence));
                    }
                } else {
                    best_match = Some((asset.path, sim, confidence));
                }
            }
        }

        best_match
    }

    /// 诊断断链并生成/应用修复建议
    pub fn repair_links(&self, apply: bool) -> Result<LinkRepairReport> {
        let audit_report = self.auditor.audit_links(false)?;
        let missing_issues: Vec<&LinkAuditIssue> = audit_report
            .issues
            .iter()
            .filter(|i| i.issue_type == "MISSING_ASSET")
            .collect();

        let total_missing = missing_issues.len();
        let mut suggestions = Vec::new();
        let mut applied_count = 0;

        for issue in missing_issues {
            if let Some((candidate, sim, confidence)) = self.find_best_candidate(&issue.target) {
                let patch = format!(
                    "--- a/{}\n+++ b/{}\n@@ -{} @@\n- {}\n+ {}",
                    issue.source,
                    issue.source,
                    issue.line_number.unwrap_or(1),
                    issue.target,
                    candidate
                );

                suggestions.push(LinkRepairSuggestion {
                    source_file: issue.source.clone(),
                    missing_target: issue.target.clone(),
                    line_number: issue.line_number,
                    original_snippet: issue.snippet.clone(),
                    candidate_target: candidate.clone(),
                    similarity: sim,
                    confidence,
                    patch_diff: patch,
                });

                if apply {
                    // 执行应用替换
                    let source_path = Path::new(&issue.source);
                    if source_path.exists() && source_path.is_file() {
                        if let Ok(content) = fs::read_to_string(source_path) {
                            // 替换直接路径引用，或者相对路径文件名
                            let missing_name = Path::new(&issue.target)
                                .file_name()
                                .and_then(|f| f.to_str())
                                .unwrap_or("");
                            let candidate_name = Path::new(&candidate)
                                .file_name()
                                .and_then(|f| f.to_str())
                                .unwrap_or("");

                            let mut new_content = content.replace(&issue.target, &candidate);
                            if new_content == content && !missing_name.is_empty() && !candidate_name.is_empty() {
                                new_content = content.replace(missing_name, candidate_name);
                            }

                            if new_content != content
                                && fs::write(source_path, new_content).is_ok() {
                                    applied_count += 1;
                                    // 同步更新 links 表
                                    let _ = self.storage.update_link_target(&issue.source, &issue.target, &candidate);
                                }
                        }
                    }
                }
            }
        }

        let reparable_count = suggestions.len();
        let report = LinkRepairReport {
            total_missing,
            reparable_count,
            applied_count,
            suggestions,
        };
        let patched_files: Vec<&str> = report
            .suggestions
            .iter()
            .take(15)
            .map(|s| s.source_file.as_str())
            .collect();
        crate::audit::AuditLogger::record_best_effort(
            &self.storage,
            crate::audit::AuditEventInput::out_of_agent(
                "governance_engine",
                "links_repair",
                "link_repair",
                if apply { "apply_patches" } else { "dry_run_scan" },
            )
            .with_target("graph", "links")
            .with_outcome(if apply { "success" } else { "dry_run" })
            .with_delta(serde_json::json!({
                "total_missing": report.total_missing,
                "reparable_count": report.reparable_count,
                "applied_count": report.applied_count,
                "patched_files": patched_files,
            })),
        );
        Ok(report)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::storage::models::{AssetRecord, LinkRecord};

    #[test]
    fn test_levenshtein_similarity() {
        assert_eq!(LinkRepairer::levenshtein_similarity("test.py", "test.py"), 1.0);
        assert_eq!(LinkRepairer::levenshtein_similarity("", "abc"), 0.0);
        let sim = LinkRepairer::levenshtein_similarity("my_script.py", "my-script.py");
        assert!(sim > 0.85);
    }

    #[test]
    fn test_link_repair_dry_run_and_apply() -> Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("test_repair.db");
        let storage = Arc::new(Storage::open(&db_path)?);

        // 创建引用源文件
        let src_file = dir.path().join("caller.py");
        let missing_target = dir.path().join("old_utils.py");
        let candidate_target = dir.path().join("new_utils.py");

        fs::write(&src_file, format!("import {}\n", missing_target.to_str().unwrap()))?;
        fs::write(&candidate_target, "# new utils implementation\n")?;

        // 注册资产进 storage
        let candidate_asset = AssetRecord {
            id: "asset_test_1".to_string(),
            path: candidate_target.to_str().unwrap().to_string(),
            asset_type: "script".to_string(),
            owner: "test".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some("test utils".to_string()),
            desc_kv: serde_json::json!({}),
            usage: None,
            size_bytes: Some(30),
            file_count: Some(1),
            tags: vec![],
            status: "active".to_string(),
        };
        storage.upsert_asset(&candidate_asset)?;

        // 插入断链（指向不存在的 old_utils.py）
        let broken_link = LinkRecord {
            id: None,
            source_type: "asset".to_string(),
            source_id: src_file.to_str().unwrap().to_string(),
            target_type: "asset".to_string(),
            target_id: missing_target.to_str().unwrap().to_string(),
            relation: "depends_on".to_string(),
            confidence: 0.9,
            auto_discovered: true,
            line_number: Some(1),
            snippet: Some(format!("import {}", missing_target.to_str().unwrap())),
            provenance: Some("ast_parser".to_string()),
            created_at: None,
        };
        storage.add_link(&broken_link)?;

        let repairer = LinkRepairer::new(storage.clone());

        // 1. Dry run 测试
        let report = repairer.repair_links(false)?;
        assert_eq!(report.total_missing, 1);
        assert_eq!(report.reparable_count, 1);
        assert_eq!(report.applied_count, 0);
        assert_eq!(report.suggestions[0].candidate_target, candidate_target.to_str().unwrap());

        // 2. Apply 测试
        let apply_report = repairer.repair_links(true)?;
        assert_eq!(apply_report.applied_count, 1);

        // 验证源文件内容已被替换
        let updated_content = fs::read_to_string(&src_file)?;
        assert!(updated_content.contains(candidate_target.to_str().unwrap()));

        // 验证 links audit 断链消除
        let auditor = LinkAuditor::new(storage);
        let audit_report = auditor.audit_links(false)?;
        assert_eq!(audit_report.issues.len(), 0);

        Ok(())
    }
}
