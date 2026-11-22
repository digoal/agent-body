use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use crate::storage::Storage;
use crate::storage::models::PlacementRule;

/// 2.0 增强型放置请求实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementRequestV2 {
    /// 资产类型: "project", "script", "skill", "note", "tmp", "src", "article", "svg", "other"
    pub object_type: String,
    /// 建议资产名或文件名
    pub name: String,
    /// 详细使用意图与生命周期描述
    pub intent: String,
    /// 标签集合
    pub tags: Vec<String>,
    /// 安全保密级别: "public", "internal", "confidential", "secret"
    pub security_level: String,
    /// 可选：当前执行任务所在的工作区目录（用于探测项目级覆盖规则）
    pub current_workspace: Option<PathBuf>,
}

/// 2.0 增强型放置建议结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementSuggestionV2 {
    /// 建议生成的物理绝对路径
    pub suggested_path: PathBuf,
    /// 规范化后的安全名称（小写蛇形）
    pub sanitized_name: String,
    /// 命中的规则名与层级 ("project_override" | "global_rule")
    pub rule_matched: String,
    /// 意图匹配置信度打分 (0.0 ~ 1.0)
    pub confidence_score: f64,
    /// 建议理由与依据
    pub reason: String,
    /// 推荐命名规范说明
    pub naming_convention: String,
    /// 目标物理路径是否已存在冲突
    pub conflict_detected: bool,
    /// 伴随的工程与安全强制规约指引
    pub guidelines: Vec<String>,
}

/// 项目局部放置规则配置模型（读取 .steward-rules.toml）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectPlacementRulesConfig {
    #[serde(default)]
    pub rules: Vec<ProjectCustomRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCustomRule {
    pub object_type: String,
    pub target_sub_dir: String,
    pub naming_pattern: Option<String>,
    pub guideline: Option<String>,
}

/// 放置规则引擎 2.0
pub struct PlacementEngineV2 {
    storage: Storage,
}

impl PlacementEngineV2 {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    /// 获取智能放置建议（支持项目级覆盖 + 意图打分 + 命名校验）
    pub fn suggest(&self, req: &PlacementRequestV2) -> Result<PlacementSuggestionV2> {
        // 1. 规范化资产名称为小写下划线（杜绝空格、特殊字符、大写）
        let sanitized = sanitize_name_v2(&req.name);

        // 2. 优先检查是否存在项目级配置覆盖 (.steward-rules.toml)
        if let Some(ref ws) = req.current_workspace {
            if let Some(custom_suggestion) = Self::try_project_override(ws, req, &sanitized) {
                return Ok(custom_suggestion);
            }
        }

        // 3. 全局宿主机规则库打分匹配
        let global_rules = self.storage.get_placement_rules()?;
        if global_rules.is_empty() {
            anyhow::bail!("placement_rules 表为空，请先初始化宿主机放置规则");
        }
        let (best_rule, score) = self.score_and_match_rule(&global_rules, req)?;

        // 4. 计算建议绝对路径
        let base_dir = PathBuf::from(&best_rule.target_base_dir);
        let pattern = best_rule.naming_pattern.as_deref().unwrap_or("{name}/");
        let sub_path = pattern.replace("{name}", &sanitized);
        let suggested_path = base_dir.join(sub_path);

        // 5. 冲突探测
        let conflict_detected = suggested_path.exists()
            || self.storage.get_asset_by_path(&suggested_path.to_string_lossy())?.is_some();

        // 6. 整合工程规约与安全底线
        let mut guidelines = Vec::new();
        if let Some(ref g) = best_rule.guideline {
            guidelines.push(g.clone());
        }

        Self::enrich_engineering_conventions(req, &mut guidelines);

        Ok(PlacementSuggestionV2 {
            suggested_path,
            sanitized_name: sanitized,
            rule_matched: format!("global:{}", best_rule.rule_name),
            confidence_score: score,
            reason: best_rule.description.clone().unwrap_or_else(|| "符合宿主机全局资产规划".to_string()),
            naming_convention: format!("推荐命名格式: {}", pattern),
            conflict_detected,
            guidelines,
        })
    }

    /// 尝试读取项目级 `.steward-rules.toml` 覆盖规则
    fn try_project_override(
        workspace: &Path,
        req: &PlacementRequestV2,
        sanitized_name: &str,
    ) -> Option<PlacementSuggestionV2> {
        let config_file = workspace.join(".steward-rules.toml");
        if !config_file.is_file() {
            return None;
        }

        let content = fs::read_to_string(&config_file).ok()?;
        let project_config: ProjectPlacementRulesConfig = toml::from_str(&content).ok()?;

        let matched = project_config.rules.iter().find(|r| {
            r.object_type.to_lowercase() == req.object_type.to_lowercase()
                || req.tags.iter().any(|t| t.to_lowercase() == r.object_type.to_lowercase())
        })?;

        let pattern = matched.naming_pattern.as_deref().unwrap_or("{name}");
        let sub_file = pattern.replace("{name}", sanitized_name);
        let target_dir = workspace.join(&matched.target_sub_dir);
        let suggested_path = target_dir.join(sub_file);
        let conflict_detected = suggested_path.exists();

        let mut guidelines = Vec::new();
        if let Some(ref g) = matched.guideline {
            guidelines.push(g.clone());
        }
        guidelines.push("📌 已应用当前工程项目级放置覆盖规则 (.steward-rules.toml)".to_string());
        Self::enrich_engineering_conventions(req, &mut guidelines);

        Some(PlacementSuggestionV2 {
            suggested_path,
            sanitized_name: sanitized_name.to_string(),
            rule_matched: format!("project_override:{}", matched.object_type),
            confidence_score: 0.95,
            reason: format!("命中了当前工作区项目级专属放置规则 ({})", matched.object_type),
            naming_convention: format!("项目模式: {}/{}", matched.target_sub_dir, pattern),
            conflict_detected,
            guidelines,
        })
    }

    /// 多维综合打分匹配最优规则
    fn score_and_match_rule<'a>(
        &self,
        rules: &'a [PlacementRule],
        req: &PlacementRequestV2,
    ) -> Result<(&'a PlacementRule, f64)> {
        let req_type = req.object_type.to_lowercase();
        let intent = req.intent.to_lowercase();
        let name_lower = req.name.to_lowercase();

        let mut best_rule = rules
            .first()
            .ok_or_else(|| anyhow::anyhow!("placement_rules 表为空，无法评分匹配"))?;
        let mut highest_score = 0.0;

        for rule in rules {
            let mut score = 0.0;
            let p_type = rule.pattern_type.to_lowercase();

            // 1. 类型完全匹配 (基础分 0.5)
            if p_type == req_type {
                score += 0.5;
            }

            // 2. 意图关键词匹配
            match p_type.as_str() {
                "tmp" => {
                    if intent.contains("临时") || intent.contains("debug") || intent.contains("测试日志") || name_lower.contains("test") {
                        score += 0.45;
                    }
                }
                "script" => {
                    if intent.contains("脚本") || intent.contains("抓取") || intent.contains("工具") || intent.contains("crawler") || intent.contains("fetcher") {
                        score += 0.4;
                    }
                }
                "skill" => {
                    if intent.contains("技能") || intent.contains("skill") || name_lower.ends_with(".skill") {
                        score += 0.5;
                    }
                }
                "src" => {
                    if intent.contains("开源") || intent.contains("克隆") || intent.contains("clone") || intent.contains("github") {
                        score += 0.45;
                    }
                }
                "work" | "project" => {
                    if intent.contains("立项") || intent.contains("正式项目") || intent.contains("业务") {
                        score += 0.4;
                    }
                }
                "note" => {
                    if intent.contains("笔记") || intent.contains("资料") || intent.contains("清单") || intent.contains("总结") {
                        score += 0.4;
                    }
                }
                _ => {}
            }

            // 3. 标签匹配加权
            for tag in &req.tags {
                if p_type.contains(&tag.to_lowercase()) {
                    score += 0.1;
                }
            }

            // 归一化上限为 1.0
            if score > 1.0 {
                score = 1.0;
            }

            if score > highest_score {
                highest_score = score;
                best_rule = rule;
            }
        }

        // 若分数太低，兜底给一个合理解释
        if highest_score < 0.3 {
            highest_score = 0.5;
        }

        Ok((best_rule, highest_score))
    }

    /// 注入工程与安全规范指引
    fn enrich_engineering_conventions(req: &PlacementRequestV2, guidelines: &mut Vec<String>) {
        if req.security_level == "confidential" || req.security_level == "secret" {
            guidelines.push("🔒 安全隔离红线: 资产密级较高，严禁包含未脱敏 API Key/Token，禁止推送到外部公开仓库。".to_string());
        }

        if req.object_type == "script" {
            guidelines.push("📋 工程规范: 脚本必须放置于独立子目录下并提供 README.md，明确功能、依赖库与执行示例。".to_string());
        } else if req.object_type == "src" {
            guidelines.push("🧭 源码探索习惯: 浅克隆后请执行 `codegraph init` 构建代码地图，优先使用 CodeGraph 工具探索。".to_string());
        } else if req.object_type == "note" || req.object_type == "article" {
            guidelines.push("🔍 真实性门禁: 撰写技术长文与分析报告严禁凭空捏造数据，必须给出真实可查的来源出处。".to_string());
            guidelines.push("🕒 时区标准: 默认强制采用北京时间 (Asia/Shanghai) 标注产生时间。".to_string());
        } else if req.object_type == "tmp" {
            guidelines.push("⏳ 生命周期管束: /root/new/tmp/ 下文件受大管家 TTL 守护纳管，会话结清后将自动回收。".to_string());
        }
    }
}

/// 规范化资产名称为严格的蛇形小写名称 (如 "My Project Test" -> "my_project_test")
pub fn sanitize_name_v2(name: &str) -> String {
    let mut out = String::new();
    let mut prev_is_sep = false;

    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_is_sep = false;
        } else if c == '-' || c == '_' || c.is_whitespace() || c == '.' || c == '/' {
            if !prev_is_sep && !out.is_empty() {
                out.push('_');
                prev_is_sep = true;
            }
        }
    }

    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "unnamed_asset".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_sanitize_name_v2() {
        assert_eq!(sanitize_name_v2("My_Test_Script"), "my_test_script");
        assert_eq!(sanitize_name_v2("SEC Crawler 2026.py"), "sec_crawler_2026_py");
        assert_eq!(sanitize_name_v2("   ---   "), "unnamed_asset");
    }

    #[test]
    fn test_global_placement_suggest() {
        let storage = Storage::open_in_memory().expect("open memory");
        let engine = PlacementEngineV2::new(storage);

        let req = PlacementRequestV2 {
            object_type: "script".to_string(),
            name: "sec_fetcher".to_string(),
            intent: "美股上市公司财报抓取脚本".to_string(),
            tags: vec!["crawler".to_string(), "finance".to_string()],
            security_level: "internal".to_string(),
            current_workspace: None,
        };

        let res = engine.suggest(&req).expect("suggest");
        assert!(res.suggested_path.to_string_lossy().contains("shared_proj"));
        assert!(res.confidence_score >= 0.7);
        assert!(res.guidelines.iter().any(|g| g.contains("README.md")));
    }

    #[test]
    fn test_project_level_override() {
        let storage = Storage::open_in_memory().expect("open memory");
        let engine = PlacementEngineV2::new(storage);

        // 创建临时工作区并在其中写入 .steward-rules.toml
        let dir = tempdir().expect("tempdir");
        let ws = dir.path();
        let config_toml = r#"
[[rules]]
object_type = "article"
target_sub_dir = "markdown"
naming_pattern = "{name}.md"
guideline = "公众号文章统一保存到 markdown/ 目录并使用 SVG 图解"
"#;
        fs::write(ws.join(".steward-rules.toml"), config_toml).expect("write toml");

        let req = PlacementRequestV2 {
            object_type: "article".to_string(),
            name: "DuckDB Tuning Guide".to_string(),
            intent: "深度性能调优技术长文".to_string(),
            tags: vec!["duckdb".to_string()],
            security_level: "internal".to_string(),
            current_workspace: Some(ws.to_path_buf()),
        };

        let res = engine.suggest(&req).expect("suggest");
        assert!(res.rule_matched.contains("project_override"));
        assert!(res.suggested_path.to_string_lossy().ends_with("markdown/duckdb_tuning_guide.md"));
        assert!(res.guidelines.iter().any(|g| g.contains("SVG 图解")));
    }

    #[test]
    fn test_v2_empty_rules_returns_err_instead_of_panic() {
        let storage = Storage::open_in_memory().expect("open memory");
        let engine = PlacementEngineV2::new(storage);
        let req = PlacementRequestV2 {
            object_type: "script".to_string(),
            name: "demo".to_string(),
            intent: "test".to_string(),
            tags: vec![],
            security_level: "internal".to_string(),
            current_workspace: None,
        };
        assert!(engine.score_and_match_rule(&[], &req).is_err(), "V2 空规则切片应返回 Err 而非 Panic");
    }
}
