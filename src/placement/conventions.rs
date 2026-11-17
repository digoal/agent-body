use serde::{Deserialize, Serialize};

/// 宿主机工程与协作操作规约实体
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineeringConvention {
    /// 规约分类: "tooling", "timezone", "integrity", "presentation", "security"
    pub category: String,
    /// 规约标题
    pub rule_title: String,
    /// 强制性操作要求细则
    pub requirement: String,
    /// 适用上下文环境: "code", "article", "script", "ops", "all"
    pub applies_to: Vec<String>,
    /// 严重等级: "CRITICAL" | "HIGH" | "RECOMMENDED"
    pub severity: String,
}

/// 面向当前任务上下文的规约组合报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextualConventionsReport {
    /// 当前任务所属上下文类型
    pub context_type: String,
    /// 当前生效的规约清单
    pub active_conventions: Vec<EngineeringConvention>,
    /// 压缩后可直接注入 Agent Prompt 的高效纪律片段
    pub brief_prompt_injection: String,
}

/// 工程操作习惯与协作纪律规约引擎
pub struct ConventionsEngine;

impl ConventionsEngine {
    /// 获取全量内置的工程操作习惯与安全红线规约库
    pub fn get_all_conventions() -> Vec<EngineeringConvention> {
        vec![
            // 1. 工具链偏好
            EngineeringConvention {
                category: "tooling".to_string(),
                rule_title: "CodeGraph 优先探索原则".to_string(),
                requirement: "在已有 .codegraph 的代码仓库中，优先使用 CodeGraph 工具 (codegraph explore / mcp) 获取高保真符号调用链与定义，禁止盲目全盘使用正则 grep 扫爆大项目。".to_string(),
                applies_to: vec!["code".to_string(), "script".to_string(), "all".to_string()],
                severity: "HIGH".to_string(),
            },
            EngineeringConvention {
                category: "tooling".to_string(),
                rule_title: "文件搜索工具选型".to_string(),
                requirement: "找文件名优先使用 `fd`；正文检索优先使用 `rg`（ripgrep）并指定 `--type`，禁止无序全盘 find。".to_string(),
                applies_to: vec!["code".to_string(), "script".to_string(), "ops".to_string()],
                severity: "RECOMMENDED".to_string(),
            },

            // 2. 本地化与时区
            EngineeringConvention {
                category: "timezone".to_string(),
                rule_title: "北京时区与中文基准".to_string(),
                requirement: "宿主机上所有时间戳、文章发布日期、日志归档与汇报，默认统一采用北京时间 (Asia/Shanghai, UTC+8)；默认采用中文进行严谨业务沟通。".to_string(),
                applies_to: vec!["all".to_string(), "article".to_string(), "ops".to_string()],
                severity: "HIGH".to_string(),
            },

            // 3. 数据真实性与权威底线
            EngineeringConvention {
                category: "integrity".to_string(),
                rule_title: "零幻觉事实与真实性门禁".to_string(),
                requirement: "撰写技术长文、行业分析与事实报告时，核心数据与叙事必须有真实权威来源出处链接（参考本地 /root/new/note 权威网站列表）；绝对严禁凭空编造虚假数字或案例，无法核实的数据必须声明未核实或直接删掉，不硬圆。".to_string(),
                applies_to: vec!["article".to_string(), "all".to_string()],
                severity: "CRITICAL".to_string(),
            },

            // 4. 排版与灵性融入
            EngineeringConvention {
                category: "presentation".to_string(),
                rule_title: "要素自然融入 (灵性写作)".to_string(),
                requirement: "撰写交付物时避免生硬照搬模板标题（如不要僵硬输出 '背景：痛点：解决方案：'），要素必须自然融入正文，逻辑连绵不绝；文章需通俗透彻，入门者与专家均能受益。".to_string(),
                applies_to: vec!["article".to_string()],
                severity: "HIGH".to_string(),
            },
            EngineeringConvention {
                category: "presentation".to_string(),
                rule_title: "SVG 图表独立落盘引用规范".to_string(),
                requirement: "若涉及架构图、流程图或可视化 SVG 图表，必须先落地为独立的 .svg 文件（如放置在 markdown/svg/ 目录），再在 Markdown 中以图片链接方式引用 `![描述](svg/xxx.svg)`，严禁将超大内联 XML 块直接贴在正文中破坏渲染排版。".to_string(),
                applies_to: vec!["article".to_string(), "code".to_string()],
                severity: "HIGH".to_string(),
            },

            // 5. 安全与脱敏
            EngineeringConvention {
                category: "security".to_string(),
                rule_title: "敏感凭据脱敏与隔离".to_string(),
                requirement: "包含 API Key、Token、私钥、数据库密码的内容严禁写入公共代码或受版本控制的资产目录，必须采用环境变量加载或做严格掩码处理。".to_string(),
                applies_to: vec!["all".to_string(), "code".to_string(), "script".to_string()],
                severity: "CRITICAL".to_string(),
            },
        ]
    }

    /// 根据当前 Agent 执行上下文动态筛选适用的工程规约
    pub fn get_conventions_for_context(
        context_type: &str,
        has_codegraph: bool,
    ) -> ContextualConventionsReport {
        let all = Self::get_all_conventions();
        let target_ctx = context_type.to_lowercase();

        let mut active: Vec<EngineeringConvention> = all
            .into_iter()
            .filter(|c| {
                c.applies_to.iter().any(|a| a == "all" || a == &target_ctx)
            })
            .collect();

        // 如果没有 codegraph，将 codegraph 规则提示降级或替换为推荐构建
        if !has_codegraph {
            for c in &mut active {
                if c.rule_title.contains("CodeGraph") {
                    c.requirement = "当前项目未检测到 .codegraph，建议浅克隆后执行 `codegraph init` 构建代码地图以加速符号定位。".to_string();
                    c.severity = "RECOMMENDED".to_string();
                }
            }
        }

        let mut injection = String::new();
        injection.push_str("【宿主机工程协作规约】\n");
        for c in &active {
            let icon = match c.severity.as_str() {
                "CRITICAL" => "🔴 [强制红线]",
                "HIGH" => "🟠 [高优遵循]",
                _ => "🟢 [最佳实践]",
            };
            injection.push_str(&format!("- {} {}: {}\n", icon, c.rule_title, c.requirement));
        }

        ContextualConventionsReport {
            context_type: target_ctx,
            active_conventions: active,
            brief_prompt_injection: injection,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_all_conventions() {
        let all = ConventionsEngine::get_all_conventions();
        assert!(all.len() >= 6);
        assert!(all.iter().any(|c| c.rule_title.contains("CodeGraph")));
        assert!(all.iter().any(|c| c.rule_title.contains("真实性门禁")));
        assert!(all.iter().any(|c| c.rule_title.contains("北京时区")));
    }

    #[test]
    fn test_contextual_filter_for_article() {
        let rep = ConventionsEngine::get_conventions_for_context("article", false);
        assert_eq!(rep.context_type, "article");
        // 文章场景必须包含真实性门禁、SVG独立落盘、北京时区
        assert!(rep.brief_prompt_injection.contains("真实性门禁"));
        assert!(rep.brief_prompt_injection.contains("SVG"));
        assert!(rep.brief_prompt_injection.contains("北京时区"));
    }

    #[test]
    fn test_contextual_filter_for_code_with_codegraph() {
        let rep = ConventionsEngine::get_conventions_for_context("code", true);
        assert_eq!(rep.context_type, "code");
        assert!(rep.brief_prompt_injection.contains("CodeGraph 优先"));
        assert!(rep.brief_prompt_injection.contains("敏感凭据"));
    }
}
