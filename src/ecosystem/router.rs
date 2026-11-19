use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::brief::assembler::estimate_tokens;
use crate::storage::Storage;
use crate::storage::models::{ExternalServiceRecord, McpCatalogRecord, SkillCatalogRecord};
use super::contacts::ExternalContactsManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRouteRequest {
    pub intent: String,
    pub budget_tokens: usize,
    pub max_skills: usize,
    pub max_mcps: usize,
    pub max_services: usize,
}

impl Default for ContextRouteRequest {
    fn default() -> Self {
        Self {
            intent: String::new(),
            budget_tokens: 600,
            max_skills: 3,
            max_mcps: 4,
            max_services: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutedSkill {
    pub skill_name: String,
    pub skill_path: String,
    pub summary: String,
    pub full_est_tokens: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutedMcp {
    pub server_name: String,
    pub client_source: String,
    pub transport: String,
    pub command_or_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutedService {
    pub service_name: String,
    pub category: String,
    pub endpoint: String,
    pub status: String,
    pub auth_env_var: Option<String>,
    pub auth_env_ready: bool,
    pub usage_example: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRoutePlan {
    pub intent: String,
    pub skills: Vec<RoutedSkill>,
    pub mcps: Vec<RoutedMcp>,
    pub services: Vec<RoutedService>,
    pub total_skills_catalog_count: usize,
    pub total_skills_full_tokens: usize,
    pub route_tokens_used: usize,
    pub saved_tokens: usize,
    pub saved_pct: f32,
    pub markdown: String,
}

/// Context 防爆渐进式披露路由器（只喂最匹配的 Top-K 元数据与路径）
pub struct ContextBudgetRouter {
    storage: Storage,
}

impl ContextBudgetRouter {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    pub fn route(&self, req: &ContextRouteRequest) -> Result<ContextRoutePlan> {
        Self::route_with_provider(self.storage.provider().as_ref(), req)
    }

    pub fn route_with_provider(
        provider: &dyn crate::storage::StorageProvider,
        req: &ContextRouteRequest,
    ) -> Result<ContextRoutePlan> {
        let intent = req.intent.trim();
        let budget = req.budget_tokens.max(150);

        let all_skills = provider.list_skills_catalog()?;
        let total_skills_catalog_count = all_skills.len();
        let total_skills_full_tokens: usize = all_skills.iter().map(|s| s.est_tokens).sum();

        // 1. 检索并打分 SKILL
        let matched_skills = if intent.is_empty() {
            all_skills.into_iter().take(req.max_skills).collect::<Vec<_>>()
        } else {
            let mut candidates = provider.search_skills_catalog(intent, req.max_skills * 4)?;
            if candidates.is_empty() {
                candidates = all_skills;
            }
            Self::rank_skills(intent, candidates)
                .into_iter()
                .take(req.max_skills)
                .collect()
        };

        // 2. 检索 MCP Servers
        let matched_mcps: Vec<McpCatalogRecord> = if intent.is_empty() {
            provider.list_mcp_catalog()?.into_iter().take(req.max_mcps).collect()
        } else {
            let mut c = provider.search_mcp_catalog(intent, req.max_mcps)?;
            if c.is_empty() {
                c = provider.list_mcp_catalog()?.into_iter().take(req.max_mcps).collect();
            }
            c
        };

        // 3. 检索外部服务通讯录
        let matched_services: Vec<ExternalServiceRecord> = if intent.is_empty() {
            provider.list_external_services(None)?.into_iter().take(req.max_services).collect()
        } else {
            let mut c = provider.search_external_services(intent, req.max_services)?;
            if c.is_empty() {
                c = provider.list_external_services(None)?.into_iter().take(req.max_services).collect();
            }
            c
        };

        let skills: Vec<RoutedSkill> = matched_skills
            .into_iter()
            .map(|s| RoutedSkill {
                skill_name: s.skill_name,
                skill_path: s.skill_path,
                summary: s.description.chars().take(90).collect(),
                full_est_tokens: s.est_tokens,
            })
            .collect();

        let mcps: Vec<RoutedMcp> = matched_mcps
            .into_iter()
            .map(|m| RoutedMcp {
                server_name: m.server_name,
                client_source: m.client_source,
                transport: m.transport,
                command_or_url: m.command_or_url,
            })
            .collect();

        let services: Vec<RoutedService> = matched_services
            .into_iter()
            .map(|svc| {
                let auth_ready = ExternalContactsManager::is_auth_env_ready(svc.auth_env_var.as_deref());
                RoutedService {
                    service_name: svc.service_name,
                    category: svc.category,
                    endpoint: svc.endpoint,
                    status: svc.status,
                    auth_env_var: svc.auth_env_var,
                    auth_env_ready: auth_ready,
                    usage_example: svc.usage_example,
                }
            })
            .collect();

        let raw_md = Self::render_markdown(intent, &skills, &mcps, &services, total_skills_catalog_count);
        let markdown = Self::clamp_to_budget(raw_md, budget);
        let route_tokens_used = estimate_tokens(&markdown);

        let saved_tokens = total_skills_full_tokens.saturating_sub(route_tokens_used);
        let saved_pct = if total_skills_full_tokens > 0 {
            (saved_tokens as f32 / total_skills_full_tokens as f32) * 100.0
        } else {
            0.0
        };

        Ok(ContextRoutePlan {
            intent: intent.to_string(),
            skills,
            mcps,
            services,
            total_skills_catalog_count,
            total_skills_full_tokens,
            route_tokens_used,
            saved_tokens,
            saved_pct,
            markdown,
        })
    }

    fn rank_skills(intent: &str, mut list: Vec<SkillCatalogRecord>) -> Vec<SkillCatalogRecord> {
        let lower_intent = intent.to_lowercase();
        let tokens: Vec<&str> = lower_intent.split_whitespace().filter(|t| !t.is_empty()).collect();

        list.sort_by(|a, b| {
            let score_a = Self::score_skill(&lower_intent, &tokens, a);
            let score_b = Self::score_skill(&lower_intent, &tokens, b);
            score_b.cmp(&score_a)
        });
        list
    }

    fn score_skill(lower_intent: &str, tokens: &[&str], skill: &SkillCatalogRecord) -> i32 {
        let mut score = 0;
        let name_l = skill.skill_name.to_lowercase();
        let desc_l = skill.description.to_lowercase();

        if lower_intent.contains(&name_l) || name_l.contains(lower_intent) {
            score += 60;
        }
        for t in tokens {
            if name_l.contains(t) {
                score += 25;
            }
            if desc_l.contains(t) {
                score += 15;
            }
            if skill.triggers.iter().any(|tr| tr.contains(t)) {
                score += 20;
            }
        }
        score
    }

    fn render_markdown(
        intent: &str,
        skills: &[RoutedSkill],
        mcps: &[RoutedMcp],
        services: &[RoutedService],
        total_skills: usize,
    ) -> String {
        let mut out = String::new();
        out.push_str(&format!("### 🧩 按需生态路由 (Context 防爆 · 库内共 {} 个 SKILL)\n", total_skills));
        if !intent.is_empty() {
            out.push_str(&format!("- **任务意图**: `{}`\n", intent));
        }
        if !skills.is_empty() {
            out.push_str("- **推荐激活 SKILL (按需读取路径)**:\n");
            for s in skills {
                out.push_str(&format!("  - `{}` ({} tok): {} -> `{}`\n", s.skill_name, s.full_est_tokens, s.summary, s.skill_path));
            }
        }
        if !mcps.is_empty() {
            out.push_str("- **相关 MCP Server**:\n");
            for m in mcps {
                out.push_str(&format!("  - `{}` [{} / {}]: `{}`\n", m.server_name, m.client_source, m.transport, m.command_or_url));
            }
        }
        if !services.is_empty() {
            out.push_str("- **可用外部服务名片**:\n");
            for svc in services {
                let auth_note = match &svc.auth_env_var {
                    Some(v) => if svc.auth_env_ready { format!("env `{}`✓", v) } else { format!("缺 env `{}`⚠️", v) },
                    None => "无需鉴权".to_string(),
                };
                out.push_str(&format!("  - `{}` ({}, {}, {}): `{}`\n", svc.service_name, svc.category, svc.status, auth_note, svc.endpoint));
            }
        }
        out
    }

    fn clamp_to_budget(text: String, budget_tokens: usize) -> String {
        let max_chars = budget_tokens * 3;
        if text.chars().count() <= max_chars {
            text
        } else {
            let cut: String = text.chars().take(max_chars).collect();
            format!("{}…(受 {} tokens 预算保护截断)\n", cut, budget_tokens)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::{McpCatalogRecord, SkillCatalogRecord};

    #[test]
    fn test_context_budget_router_limits_tokens_and_ranks() {
        let storage = Storage::open_in_memory().expect("open");

        for i in 0..20 {
            let name = if i == 7 { "pgfaq".to_string() } else { format!("other-skill-{}", i) };
            let desc = if i == 7 {
                "PostgreSQL internals planner executor storage troubleshooting".to_string()
            } else {
                "Generic social media formatting helper".to_string()
            };
            storage
                .upsert_skill_catalog(&SkillCatalogRecord {
                    id: format!("sk-{}", i),
                    skill_name: name,
                    root_source: "/tmp".to_string(),
                    skill_path: format!("/tmp/skill-{}/SKILL.md", i),
                    description: desc,
                    triggers: vec![],
                    est_tokens: 1000,
                    has_scripts: false,
                    has_references: false,
                    status: "active".to_string(),
                    last_scanned_at: None,
                })
                .expect("upsert skill");
        }

        storage
            .upsert_mcp_catalog(&McpCatalogRecord {
                id: "mcp-steward".to_string(),
                server_name: "agent-steward".to_string(),
                client_source: "hermes".to_string(),
                transport: "stdio".to_string(),
                command_or_url: "/usr/local/bin/agent-steward mcp".to_string(),
                tools_json: serde_json::json!(["steward_brief"]),
                tool_count: 32,
                est_total_tokens: 2400,
                status: "active".to_string(),
                last_scanned_at: None,
            })
            .expect("upsert mcp");

        let router = ContextBudgetRouter::new(storage);
        let plan = router
            .route(&ContextRouteRequest {
                intent: "PostgreSQL planner".to_string(),
                budget_tokens: 400,
                max_skills: 3,
                max_mcps: 2,
                max_services: 2,
            })
            .expect("route");

        assert!(!plan.skills.is_empty() && plan.skills.len() <= 3, "返回应不超过 max_skills=3 且命中相关技能");
        assert_eq!(plan.skills[0].skill_name, "pgfaq", "pgfaq 应排在第一位");
        assert!(plan.route_tokens_used <= 450, "路由输出应严格受预算控制");
        assert!(plan.saved_tokens > 18000, "应统计节省的大量上下文 token");
        assert!(plan.saved_pct > 90.0);
    }
}
