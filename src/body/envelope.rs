//! 四级本体自治权限包络 (`AutonomyEnvelope`: L0_OBSERVE ~ L3_FORBIDDEN)

use serde::{Deserialize, Serialize};

/// 本体自治权限等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutonomyLevel {
    L0Observe,
    L1Autonomic,
    L2Supervised,
    L3Forbidden,
}

impl AutonomyLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::L0Observe => "L0_OBSERVE",
            Self::L1Autonomic => "L1_AUTONOMIC",
            Self::L2Supervised => "L2_SUPERVISED",
            Self::L3Forbidden => "L3_FORBIDDEN",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::L0Observe => "只读感知（永不阻塞）",
            Self::L1Autonomic => "自主稳态（可逆自愈，100% 留痕）",
            Self::L2Supervised => "受控监督（默认 dry-run / 需鉴权确认）",
            Self::L3Forbidden => "绝对禁区（物理/安全红线阻断）",
        }
    }
}

/// 单个动作的权限裁决结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvelopeDecision {
    pub action: String,
    pub target: String,
    pub level: AutonomyLevel,
    pub level_code: String,
    pub allowed: bool,
    pub requires_confirmation: bool,
    pub reason: String,
}

/// 包络策略文档项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvelopeRuleSpec {
    pub level: String,
    pub label: String,
    pub authorization: String,
    pub actions: Vec<String>,
    pub audit_requirement: String,
}

pub struct AutonomyEnvelope;

impl AutonomyEnvelope {
    fn is_forbidden_target(target_lower: &str) -> Option<&'static str> {
        let forbidden_patterns = [
            ("/etc/shadow", "系统影子口令文件属于 L3 绝对禁区"),
            ("/etc/sudoers", "系统提权配置属于 L3 绝对禁区"),
            ("/etc/passwd", "系统账户基线文件属于 L3 绝对禁区"),
            ("/.ssh/", "SSH 私钥与授权目录属于 L3 绝对禁区"),
            (".ssh/", "SSH 私钥目录属于 L3 绝对禁区"),
            ("id_rsa", "SSH 私钥文件属于 L3 绝对禁区"),
            ("id_ed25519", "SSH 私钥文件属于 L3 绝对禁区"),
            (".pem", "PEM 证书私钥属于 L3 绝对禁区"),
            (".key", "密钥文件属于 L3 绝对禁区"),
            (".env", "环境凭据文件受敏感路径保护，禁止明文入库或直写"),
        ];
        for (pat, reason) in forbidden_patterns {
            if target_lower.contains(pat) {
                return Some(reason);
            }
        }
        None
    }

    /// 评估动作与目标路径的自治权限等级
    pub fn evaluate(action: &str, target: Option<&str>, dry_run: bool) -> EnvelopeDecision {
        let act = action.trim().to_lowercase();
        let tgt_raw = target.unwrap_or("").trim();
        let tgt = tgt_raw.to_lowercase();

        // 1. L3_FORBIDDEN 检查：敏感路径写入/删除/跨机覆盖、明文密钥、黑名单递归扫盘
        let is_read_only_probe = matches!(
            act.as_str(),
            "brief" | "fact_query" | "asset_search" | "links_impact" | "links_audit"
                | "vitals" | "manifest" | "envelope" | "mesh_body_manifest" | "status"
        );
        if act == "plaintext_secret_persist"
            || act == "blacklist_deep_scan"
            || act == "mesh_direct_system_overwrite"
            || (!is_read_only_probe && Self::is_forbidden_target(&tgt).is_some())
        {
            let reason = Self::is_forbidden_target(&tgt)
                .unwrap_or("触犯本体安全红线（明文凭据/黑名单深扫/跨机系统覆盖）")
                .to_string();
            return EnvelopeDecision {
                action: action.to_string(),
                target: tgt_raw.to_string(),
                level: AutonomyLevel::L3Forbidden,
                level_code: AutonomyLevel::L3Forbidden.as_str().to_string(),
                allowed: false,
                requires_confirmation: false,
                reason,
            };
        }

        // 2. L2_SUPERVISED 检查：影响宿主机文件/配置/源码/规则的受控写操作
        if matches!(
            act.as_str(),
            "host_gc"
                | "env_gc"
                | "link_repair_apply"
                | "links_repair"
                | "mesh_transfer_accept"
                | "placement_rule_apply"
                | "session_close_clean"
        ) {
            return EnvelopeDecision {
                action: action.to_string(),
                target: tgt_raw.to_string(),
                level: AutonomyLevel::L2Supervised,
                level_code: AutonomyLevel::L2Supervised.as_str().to_string(),
                allowed: dry_run,
                requires_confirmation: !dry_run,
                reason: if dry_run {
                    "L2 受控操作处于 dry_run 预览模式，允许无副作用探测".to_string()
                } else {
                    "L2 受控写操作必须经大脑显式确认或 Bearer auth_token 鉴权后方可执行".to_string()
                },
            };
        }

        // 3. L1_AUTONOMIC 检查：可逆且零业务破坏的本体稳态自愈反射
        if matches!(
            act.as_str(),
            "watcher_fold"
                | "write_through_invalidate"
                | "ghost_node_prune"
                | "expired_lock_gc"
                | "lease_calibrate"
                | "duplicate_memory_archive"
                | "freshness_reconcile"
                | "homeostasis_heal"
                | "handoff_save"
                | "handoff_resume"
        ) {
            return EnvelopeDecision {
                action: action.to_string(),
                target: tgt_raw.to_string(),
                level: AutonomyLevel::L1Autonomic,
                level_code: AutonomyLevel::L1Autonomic.as_str().to_string(),
                allowed: true,
                requires_confirmation: false,
                reason: "L1 自主稳态动作（可逆且强制写入脱敏审计账本），允许自主执行".to_string(),
            };
        }

        // 4. 默认归入 L0_OBSERVE 只读感知
        EnvelopeDecision {
            action: action.to_string(),
            target: tgt_raw.to_string(),
            level: AutonomyLevel::L0Observe,
            level_code: AutonomyLevel::L0Observe.as_str().to_string(),
            allowed: true,
            requires_confirmation: false,
            reason: "L0 只读感知动作（零副作用），默认放行".to_string(),
        }
    }

    /// 返回完整的四级自治边界矩阵，供 CLI/MCP/WebUI 自解释展示
    pub fn policy_matrix() -> Vec<EnvelopeRuleSpec> {
        vec![
            EnvelopeRuleSpec {
                level: AutonomyLevel::L0Observe.as_str().to_string(),
                label: AutonomyLevel::L0Observe.label().to_string(),
                authorization: "永远允许（零副作用）".to_string(),
                actions: vec![
                    "brief".into(),
                    "fact_query".into(),
                    "asset_search".into(),
                    "links_impact".into(),
                    "manifest".into(),
                    "vitals".into(),
                    "mesh_body_manifest".into(),
                ],
                audit_requirement: "关键接入调用写入 IN_AGENT 审计流".to_string(),
            },
            EnvelopeRuleSpec {
                level: AutonomyLevel::L1Autonomic.as_str().to_string(),
                label: AutonomyLevel::L1Autonomic.label().to_string(),
                authorization: "本体自主执行（可逆且零业务破坏）".to_string(),
                actions: vec![
                    "watcher_fold".into(),
                    "write_through_invalidate".into(),
                    "ghost_node_prune".into(),
                    "expired_lock_gc".into(),
                    "lease_calibrate".into(),
                    "duplicate_memory_archive".into(),
                    "freshness_reconcile".into(),
                    "homeostasis_heal".into(),
                ],
                audit_requirement: "100% 写入 OUT_OF_AGENT 审计账本并附 before_after_delta".to_string(),
            },
            EnvelopeRuleSpec {
                level: AutonomyLevel::L2Supervised.as_str().to_string(),
                label: AutonomyLevel::L2Supervised.label().to_string(),
                authorization: "默认 dry-run；真实写入需大脑确认或 Web auth_token".to_string(),
                actions: vec![
                    "host_gc".into(),
                    "link_repair_apply".into(),
                    "mesh_transfer_accept".into(),
                    "placement_rule_apply".into(),
                ],
                audit_requirement: "强制可逆 patch_diff / 清理清单 + SHA-256 存证".to_string(),
            },
            EnvelopeRuleSpec {
                level: AutonomyLevel::L3Forbidden.as_str().to_string(),
                label: AutonomyLevel::L3Forbidden.label().to_string(),
                authorization: "物理/策略永久阻断 (allowed=false)".to_string(),
                actions: vec![
                    "write /etc/shadow & /etc/sudoers & ~/.ssh/".into(),
                    "plaintext_secret_persist".into(),
                    "blacklist_deep_scan".into(),
                    "mesh_direct_system_overwrite".into(),
                ],
                audit_requirement: "强制拦截告警并写入 CRITICAL blocked 审计事件".to_string(),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autonomy_envelope_four_levels() {
        let l0 = AutonomyEnvelope::evaluate("vitals", None, false);
        assert_eq!(l0.level, AutonomyLevel::L0Observe);
        assert!(l0.allowed);

        let l1 = AutonomyEnvelope::evaluate("homeostasis_heal", None, false);
        assert_eq!(l1.level, AutonomyLevel::L1Autonomic);
        assert!(l1.allowed);
        assert!(!l1.requires_confirmation);

        let l2_dry = AutonomyEnvelope::evaluate("host_gc", Some("/root/new/tmp/x.log"), true);
        assert_eq!(l2_dry.level, AutonomyLevel::L2Supervised);
        assert!(l2_dry.allowed);
        assert!(!l2_dry.requires_confirmation);

        let l2_real = AutonomyEnvelope::evaluate("host_gc", Some("/root/new/tmp/x.log"), false);
        assert_eq!(l2_real.level, AutonomyLevel::L2Supervised);
        assert!(!l2_real.allowed);
        assert!(l2_real.requires_confirmation);

        let l3 = AutonomyEnvelope::evaluate("write_file", Some("/etc/shadow"), false);
        assert_eq!(l3.level, AutonomyLevel::L3Forbidden);
        assert!(!l3.allowed);
    }
}
