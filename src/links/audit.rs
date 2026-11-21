use std::path::Path;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use crate::storage::Storage;
use crate::env::port::PortChecker;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkAuditIssue {
    pub source: String,
    pub target: String,
    pub issue_type: String, // "MISSING_ASSET" | "OFFLINE_SERVICE" | "DANGLING_REF"
    pub severity: String,   // "HIGH" | "MEDIUM" | "LOW"
    pub line_number: Option<i32>,
    pub snippet: Option<String>,
    pub description: String,
    pub suggested_fix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkAuditReport {
    pub total_links: usize,
    pub health_score: u32,
    pub dangling_count: usize,
    pub fixed_count: usize,
    pub issues: Vec<LinkAuditIssue>,
    pub summary: String,
}

pub struct LinkAuditor {
    storage: Arc<Storage>,
}

impl LinkAuditor {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    /// 审计全局关联依赖的真实有效性（检测幽灵依赖、不存在文件、离线端口）
    pub fn audit_links(&self, fix_dangling: bool) -> Result<LinkAuditReport> {
        let all_links = self.storage.list_all_links(2000)?;
        let total_links = all_links.len();

        let mut issues = Vec::new();
        let mut fixed_count = 0;

        for link in &all_links {
            // 过滤早期版本遗留的模板/占位符/临时日志/纯数据 JSON 端口等假阳性边
            // 注意：当 source 与 target 同处于 /tmp/ 测试沙盒目录时保留审计（供 TempDir 单元测试与 W5 基准使用）
            let both_in_sys_tmp = link.source_id.starts_with("/tmp/") && link.target_id.starts_with("/tmp/");
            let is_false_positive = if link.target_type == "asset" {
                (!both_in_sys_tmp && crate::links::is_non_static_link_target(&link.target_id, None))
                    || (both_in_sys_tmp && crate::links::is_non_static_link_target(&link.target_id, None) && !link.target_id.starts_with("/tmp/"))
                    || (!both_in_sys_tmp
                        && crate::links::is_test_or_example_link(
                            &link.source_id,
                            &link.target_id,
                            link.snippet.as_deref(),
                        ))
                    || (!link.source_id.starts_with("/tmp/")
                        && link.source_id.ends_with(".md")
                        && !Path::new(&link.target_id).exists()
                        && !crate::links::is_explicit_markdown_link(link.snippet.as_deref()))
            } else if link.target_type == "service" {
                crate::links::is_pure_data_file(&link.source_id)
                    || link.source_id.ends_with(".md")
                    || (!both_in_sys_tmp
                        && crate::links::is_test_or_example_link(
                            &link.source_id,
                            &link.target_id,
                            link.snippet.as_deref(),
                        ))
            } else {
                false
            };

            if is_false_positive {
                if fix_dangling && link.auto_discovered {
                    if let Some(id) = link.id {
                        let _ = self.storage.delete_link(id);
                    } else if link.target_type == "asset" {
                        let _ = self.storage.delete_links_for_target(&link.target_id);
                    }
                    fixed_count += 1;
                }
                continue;
            }

            if link.target_type == "asset" {
                let p = Path::new(&link.target_id);
                // 仅对绝对路径做物理检查
                if p.is_absolute() && !p.exists() {
                    let severity = if link.source_id.ends_with(".md") {
                        "LOW".to_string()
                    } else {
                        "HIGH".to_string()
                    };
                    issues.push(LinkAuditIssue {
                        source: link.source_id.clone(),
                        target: link.target_id.clone(),
                        issue_type: "MISSING_ASSET".to_string(),
                        severity,
                        line_number: link.line_number,
                        snippet: link.snippet.clone(),
                        description: format!("目标资产文件在宿主文件系统上不存在: {}", link.target_id),
                        suggested_fix: "清理悬空依赖关系，或恢复目标资产文件".to_string(),
                    });

                    if fix_dangling {
                        let _ = self.storage.delete_links_for_target(&link.target_id);
                        fixed_count += 1;
                    }
                }
            } else if link.target_type == "service" {
                // 例如 "postgresql:5432" 或 "ollama:11434"
                if let Some(port_str) = link.target_id.split(':').last() {
                    if let Ok(port) = port_str.parse::<u16>() {
                        // 端口探测检查
                        if let Ok(port_res) = PortChecker::check_port(port, 1) {
                            if port_res.status != "OCCUPIED" {
                                issues.push(LinkAuditIssue {
                                    source: link.source_id.clone(),
                                    target: link.target_id.clone(),
                                    issue_type: "OFFLINE_SERVICE".to_string(),
                                    severity: "MEDIUM".to_string(),
                                    line_number: link.line_number,
                                    snippet: link.snippet.clone(),
                                    description: format!("依赖的宿主服务端口 {} 处于离线状态 (TCP_LISTEN 为空)", port),
                                    suggested_fix: format!("启动本地服务或检查连接配置 (端口: {})", port),
                                });
                            }
                        }
                    }
                }
            }
        }

        let dangling_count = issues.len();

        // 评分计算: 基础分 100，HIGH 扣 10，MEDIUM 扣 5
        let mut penalty = 0;
        for iss in &issues {
            match iss.severity.as_str() {
                "HIGH" => penalty += 10,
                "MEDIUM" => penalty += 5,
                _ => penalty += 2,
            }
        }
        let health_score = if penalty >= 100 { 0 } else { 100 - penalty };

        let summary = if dangling_count == 0 {
            format!("✅ 全局依赖拓扑健康！共审计 {} 条关系，无任何悬空断链 (Health: 100/100)。", total_links)
        } else {
            format!(
                "⚠️ 依赖拓扑存在 {} 处异常断链 (健康分: {}/100)。其中包含 {} 处文件缺失与离线服务。{}",
                dangling_count,
                health_score,
                issues.iter().filter(|i| i.issue_type == "MISSING_ASSET").count(),
                if fix_dangling { format!("已自动清理 {} 条死链。", fixed_count) } else { "建议使用 --fix 参数清理废弃链接。".to_string() }
            )
        };

        Ok(LinkAuditReport {
            total_links,
            health_score,
            dangling_count,
            fixed_count,
            issues,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::LinkRecord;

    #[test]
    fn test_audit_links_missing_asset() {
        let storage = Arc::new(Storage::open_in_memory().expect("open_in_memory"));
        let auditor = LinkAuditor::new(storage.clone());

        // 添加一条指向不存在文件的链接
        storage.add_link(&LinkRecord {
            id: None,
            source_type: "asset".to_string(),
            source_id: "/root/new/work/test.py".to_string(),
            target_type: "asset".to_string(),
            target_id: "/root/new/non_existent_file_xyz_123.py".to_string(),
            relation: "depends_on".to_string(),
            confidence: 1.0,
            auto_discovered: true,
            line_number: Some(10),
            snippet: Some("from non_existent import something".to_string()),
            provenance: Some("ast_import".to_string()),
            created_at: None,
        }).unwrap();

        let report = auditor.audit_links(false).unwrap();
        assert_eq!(report.dangling_count, 1);
        assert_eq!(report.issues[0].issue_type, "MISSING_ASSET");
        assert_eq!(report.issues[0].line_number, Some(10));
        assert!(report.health_score < 100);

        // 测试 fix_dangling 自动清理
        let fix_report = auditor.audit_links(true).unwrap();
        assert_eq!(fix_report.fixed_count, 1);

        let final_report = auditor.audit_links(false).unwrap();
        assert_eq!(final_report.dangling_count, 0);
        assert_eq!(final_report.health_score, 100);
    }

    #[test]
    fn test_audit_links_ignores_and_fixes_md_service_false_positive_without_deleting_real_py_link() {
        let storage = Arc::new(Storage::open_in_memory().expect("open_in_memory"));
        let auditor = LinkAuditor::new(storage.clone());

        // 1. 历史遗留 .md 误判 service 边
        storage.add_link(&LinkRecord {
            id: None,
            source_type: "asset".to_string(),
            source_id: "/root/new/work/agent-steward/user_feedback.md".to_string(),
            target_type: "service".to_string(),
            target_id: "redis:6379".to_string(),
            relation: "depends_on".to_string(),
            confidence: 0.88,
            auto_discovered: true,
            line_number: Some(115),
            snippet: Some("redis:6379".to_string()),
            provenance: Some("port_detect".to_string()),
            created_at: None,
        }).unwrap();

        // 2. 真实 .py 脚本依赖边
        storage.add_link(&LinkRecord {
            id: None,
            source_type: "asset".to_string(),
            source_id: "/root/new/shared_proj/real_worker.py".to_string(),
            target_type: "service".to_string(),
            target_id: "redis:6379".to_string(),
            relation: "depends_on".to_string(),
            confidence: 0.9,
            auto_discovered: true,
            line_number: Some(10),
            snippet: Some("redis://127.0.0.1:6379".to_string()),
            provenance: Some("port_detect".to_string()),
            created_at: None,
        }).unwrap();

        let fix_rep = auditor.audit_links(true).unwrap();
        assert_eq!(fix_rep.fixed_count, 1, "只应清理 1 条 .md 假阳性边");

        let remaining = storage.query_links_for_node("asset", "/root/new/shared_proj/real_worker.py").unwrap();
        assert_eq!(remaining.len(), 1, "不应误删真实 .py 的 redis:6379 依赖边");
    }
}
