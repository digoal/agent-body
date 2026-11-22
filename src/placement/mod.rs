use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use crate::storage::Storage;
use crate::storage::models::PlacementRule;

pub mod learner;
pub mod engine_v2;
pub mod conventions;

pub use learner::{PlacementLearner, LearnedRuleCandidate, LearnReport, CorrectionFeedback};
pub use engine_v2::{PlacementEngineV2, PlacementRequestV2, PlacementSuggestionV2, sanitize_name_v2};
pub use conventions::{ConventionsEngine, ConventionEngine, EngineeringConvention, ContextualConventionsReport};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementRequest {
    pub object_type: String, // project, script, skill, note, tmp, src, other
    pub name: String,
    pub intent: String,
    pub tags: Vec<String>,
    pub security_level: String, // public, internal, confidential, secret
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementSuggestion {
    pub suggested_path: PathBuf,
    pub rule_matched: String,
    pub reason: String,
    pub naming_convention: String,
    pub conflict_detected: bool,
    pub guidelines: Vec<String>,
}

pub struct PlacementEngine {
    storage: Storage,
}

impl PlacementEngine {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    /// 获取目录建议
    pub fn suggest(&self, req: &PlacementRequest) -> Result<PlacementSuggestion> {
        let rules = self.storage.get_placement_rules()?;
        if rules.is_empty() {
            anyhow::bail!("placement_rules 表为空，请先初始化宿主机放置规则");
        }

        // 1. 规则匹配
        let matched_rule = self.match_rule(&rules, req)?;

        // 2. 格式化名称为 snake_case 风格
        let clean_name = sanitize_name(&req.name);

        // 3. 计算建议目标路径
        let base_dir = PathBuf::from(&matched_rule.target_base_dir);
        let pattern = matched_rule.naming_pattern.as_deref().unwrap_or("{name}/");
        let sub_path = pattern.replace("{name}", &clean_name);
        let suggested_path = base_dir.join(sub_path);

        // 4. 冲突检查 (检查本地文件系统或 assets 表中是否存在)
        let conflict_detected = suggested_path.exists() || 
            self.storage.get_asset_by_path(&suggested_path.to_string_lossy())?.is_some();

        // 5. 组合规范与安全指引
        let mut guidelines = Vec::new();
        if let Some(ref g) = matched_rule.guideline {
            guidelines.push(g.clone());
        }

        if req.security_level == "confidential" || req.security_level == "secret" {
            guidelines.push("安全隔离要求: 该对象密级较高，严禁包含未脱敏 API Key/Token，禁止推送到外部公开仓库。".to_string());
        }

        if req.object_type == "script" {
            guidelines.push("工程规范: 建议在同目录下配备 README.md 说明用途、依赖库及执行示例。".to_string());
        }

        Ok(PlacementSuggestion {
            suggested_path,
            rule_matched: matched_rule.rule_name.clone(),
            reason: matched_rule.description.clone().unwrap_or_else(|| "符合宿主机放置规范".to_string()),
            naming_convention: format!("推荐命名格式: {}", pattern),
            conflict_detected,
            guidelines,
        })
    }

    /// 获取当前放置规则摘要（供情境组装器使用）
    pub fn get_rules_summary(&self) -> String {
        match self.storage.get_placement_rules() {
            Ok(rules) => {
                let parts: Vec<String> = rules
                    .iter()
                    .take(6)
                    .map(|r| format!("{}: {}", r.pattern_type, r.target_base_dir))
                    .collect();
                if parts.is_empty() {
                    let b = crate::config::Config::load_effective_or_default().effective_workspace_base();
                    format!("工程目录规范: {}/{{work,shared_proj,skills,note,src,tmp}}", b.display())
                } else {
                    parts.join(" | ")
                }
            }
            Err(_) => {
                let b = crate::config::Config::load_effective_or_default().effective_workspace_base();
                format!("工程目录规范: {}/{{work,shared_proj,skills,note,src,tmp}}", b.display())
            }
        }
    }

    fn match_rule<'a>(&self, rules: &'a [PlacementRule], req: &PlacementRequest) -> Result<&'a PlacementRule> {
        let req_type = req.object_type.to_lowercase();
        let intent = req.intent.to_lowercase();

        // 意图关键词优先
        if intent.contains("临时") || intent.contains("debug") || intent.contains("测试日志") {
            if let Some(r) = rules.iter().find(|r| r.pattern_type == "tmp") {
                return Ok(r);
            }
        }
        if intent.contains("技能") || intent.contains("skill") {
            if let Some(r) = rules.iter().find(|r| r.pattern_type == "skill") {
                return Ok(r);
            }
        }
        if intent.contains("开源") || intent.contains("clone") || intent.contains("github") {
            if let Some(r) = rules.iter().find(|r| r.pattern_type == "src") {
                return Ok(r);
            }
        }
        if intent.contains("笔记") || intent.contains("清单") || intent.contains("备忘") {
            if let Some(r) = rules.iter().find(|r| r.pattern_type == "note") {
                return Ok(r);
            }
        }

        // 类型匹配
        for rule in rules {
            if rule.pattern_type.eq_ignore_ascii_case(&req_type) {
                return Ok(rule);
            }
        }

        // 兜底返回默认第一条规则 (shared_tools 或其他)
        rules
            .first()
            .ok_or_else(|| anyhow::anyhow!("placement_rules 表为空，无法匹配放置规则"))
    }
}

fn sanitize_name(name: &str) -> String {
    let s = name.trim().replace([' ', '-', '.'], "_");
    s.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placement_suggest_script() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let engine = PlacementEngine::new(storage);

        let req = PlacementRequest {
            object_type: "script".to_string(),
            name: "sec-scraper".to_string(),
            intent: "长期复用的美股财报抓取工具".to_string(),
            tags: vec!["python".to_string(), "sec".to_string()],
            security_level: "internal".to_string(),
        };

        let suggestion = engine.suggest(&req).expect("suggest");
        assert_eq!(suggestion.rule_matched, "shared_tools");
        assert!(suggestion.suggested_path.to_string_lossy().contains("/root/new/shared_proj/sec_scraper"));
        assert!(!suggestion.conflict_detected);
    }

    #[test]
    fn test_placement_suggest_tmp_intent() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let engine = PlacementEngine::new(storage);

        let req = PlacementRequest {
            object_type: "other".to_string(),
            name: "test_output".to_string(),
            intent: "临时调试抓取结果缓存".to_string(),
            tags: vec!["debug".to_string()],
            security_level: "internal".to_string(),
        };

        let suggestion = engine.suggest(&req).expect("suggest");
        assert_eq!(suggestion.rule_matched, "temp_files");
        assert!(suggestion.suggested_path.to_string_lossy().contains("/root/new/tmp/test_output"));
    }

    #[test]
    fn test_placement_confidential_guideline() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let engine = PlacementEngine::new(storage);

        let req = PlacementRequest {
            object_type: "note".to_string(),
            name: "api_keys".to_string(),
            intent: "私密凭证记录".to_string(),
            tags: vec!["auth".to_string()],
            security_level: "secret".to_string(),
        };

        let suggestion = engine.suggest(&req).expect("suggest");
        assert!(suggestion.guidelines.iter().any(|g| g.contains("安全隔离要求")));
    }

    #[test]
    fn test_placement_empty_rules_returns_err_instead_of_panic() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let engine = PlacementEngine::new(storage);
        let req = PlacementRequest {
            object_type: "script".to_string(),
            name: "demo".to_string(),
            intent: "test".to_string(),
            tags: vec![],
            security_level: "internal".to_string(),
        };
        assert!(engine.match_rule(&[], &req).is_err(), "空规则切片应返回 Err 而非 Panic");
    }
}
