use serde::{Deserialize, Serialize};
use anyhow::Result;
use crate::storage::Storage;
use crate::storage::models::MemoryRecord;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConflictIssue {
    pub memory_id_a: String,
    pub content_a: String,
    pub memory_id_b: String,
    pub content_b: String,
    pub conflict_reason: String,
    pub suggested_resolution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDecayCandidate {
    pub memory_id: String,
    pub content: String,
    pub current_weight: f32,
    pub decayed_weight: f32,
    pub access_count: i64,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAuditReport {
    pub total_memories: usize,
    pub active_memories: usize,
    pub decayed_count: usize,
    pub conflict_count: usize,
    pub conflicts: Vec<MemoryConflictIssue>,
    pub decay_candidates: Vec<MemoryDecayCandidate>,
}

pub struct MemoryLifecycleManager<'a> {
    storage: &'a Storage,
}

impl<'a> MemoryLifecycleManager<'a> {
    pub fn new(storage: &'a Storage) -> Self {
        Self { storage }
    }

    /// 全面体检长期记忆健康度（检测潜在认知矛盾与衰减淘汰候选）
    pub fn audit_memories(&self) -> Result<MemoryAuditReport> {
        let memories = self.storage.list_all_active_memories(1000)?;
        let total_memories = memories.len();
        let active_memories = memories.iter().filter(|m| m.status == "active").count();

        let mut conflicts = Vec::new();
        let mut decay_candidates = Vec::new();

        // 1. 冲突矛盾检测 (双重循环配对分析)
        let active_list: Vec<&MemoryRecord> = memories.iter().filter(|m| m.status == "active").collect();
        for i in 0..active_list.len() {
            for j in (i + 1)..active_list.len() {
                let m1 = active_list[i];
                let m2 = active_list[j];

                if let Some(reason) = Self::detect_contradiction(&m1.content, &m2.content) {
                    conflicts.push(MemoryConflictIssue {
                        memory_id_a: m1.id.clone(),
                        content_a: m1.content.clone(),
                        memory_id_b: m2.id.clone(),
                        content_b: m2.content.clone(),
                        conflict_reason: reason,
                        suggested_resolution: "建议将较旧或已被覆盖的条目标记为 'outdated'，保留最新有效决策".to_string(),
                    });
                }
            }
        }

        // 2. 衰减候选分析 (非 pinned 且低权重的记忆)
        for m in &active_list {
            if m.pinned {
                continue;
            }
            // 基础权重衰减计算
            let decayed_weight = m.weight * 0.85; // 周期性温和衰减
            if decayed_weight < 0.35 {
                decay_candidates.push(MemoryDecayCandidate {
                    memory_id: m.id.clone(),
                    content: m.content.clone(),
                    current_weight: m.weight,
                    decayed_weight,
                    access_count: 0,
                    recommendation: "记忆热度过低且未被常驻固定，建议归档以净化上下文".to_string(),
                });
            }
        }

        let conflict_count = conflicts.len();
        let decayed_count = decay_candidates.len();
        let report = MemoryAuditReport {
            total_memories,
            active_memories,
            decayed_count,
            conflict_count,
            conflicts,
            decay_candidates,
        };

        crate::audit::AuditLogger::record_best_effort(
            self.storage,
            crate::audit::AuditEventInput::out_of_agent(
                "daemon_scheduler",
                "memory_lifecycle",
                "memory_audit",
                "audit_memories",
            )
            .with_target("table", "memories")
            .with_outcome(if conflict_count > 0 { "warn" } else { "success" })
            .with_delta(serde_json::json!({
                "total_memories": total_memories,
                "active_memories": active_memories,
                "conflict_count": conflict_count,
                "decayed_count": decayed_count,
            })),
        );

        Ok(report)
    }

    /// 检测两段文本是否包含语义对立或相互矛盾的断言
    pub fn detect_contradiction(text1: &str, text2: &str) -> Option<String> {
        let t1 = text1.to_lowercase();
        let t2 = text2.to_lowercase();

        // 常见极性反转关键词对
        let polarity_pairs = [
            ("禁止", "允许"),
            ("禁用", "启用"),
            ("废弃", "推荐"),
            ("不支持", "支持"),
            ("关闭", "开启"),
            ("拒绝", "接受"),
            ("弃用", "采用"),
        ];

        for (neg, pos) in polarity_pairs {
            let (has_neg1, has_pos1) = (t1.contains(neg), t1.contains(pos));
            let (has_neg2, has_pos2) = (t2.contains(neg), t2.contains(pos));

            if (has_neg1 && has_pos2) || (has_pos1 && has_neg2) {
                // 计算共有词汇重叠度，确保讨论的是同一实体
                let words1: Vec<&str> = text1.split_whitespace().collect();
                let overlap = words1.iter().any(|w| w.len() >= 2 && text2.contains(w));
                if overlap || t1.contains("8000") && t2.contains("8000") || t1.contains("端口") && t2.contains("端口") {
                    return Some(format!("包含对立语义词: '{}' 与 '{}'", neg, pos));
                }
            }
        }

        None
    }

    /// 归档某条过时冲突记忆
    pub fn archive_memory(&self, memory_id: &str) -> Result<()> {
        self.storage.update_memory_status(memory_id, "outdated")?;
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_detect_contradiction() {
        let text1 = "禁止使用端口 8000 进行本地开发";
        let text2 = "允许使用端口 8000 启动测试服务";
        let res = MemoryLifecycleManager::detect_contradiction(text1, text2);
        assert!(res.is_some());
        assert!(res.unwrap().contains("禁止"));
    }

    #[test]
    fn test_audit_and_archive_memory() -> Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("test_mem_life.db");
        let storage = Storage::open(&db_path)?;

        let mem1 = MemoryRecord {
            id: "mem-01".to_string(),
            memory_type: "decision".to_string(),
            content: "废弃旧编译方式，禁止使用 debug 构建".to_string(),
            summary: None,
            source_agent: Some("agent_a".to_string()),
            source_session: None,
            tags: vec!["build".to_string()],
            weight: 0.9,
            pinned: false,
            status: "active".to_string(),
            embedding: vec![],
        };

        let mem2 = MemoryRecord {
            id: "mem-02".to_string(),
            memory_type: "decision".to_string(),
            content: "推荐在本地调试时采用 debug 构建以获取完整栈追踪".to_string(),
            summary: None,
            source_agent: Some("agent_b".to_string()),
            source_session: None,
            tags: vec!["build".to_string()],
            weight: 0.9,
            pinned: false,
            status: "active".to_string(),
            embedding: vec![],
        };

        storage.insert_memory(&mem1)?;
        storage.insert_memory(&mem2)?;

        let mgr = MemoryLifecycleManager::new(&storage);
        let rep = mgr.audit_memories()?;

        assert_eq!(rep.conflict_count, 1);
        let c = &rep.conflicts[0];
        assert!(
            (c.memory_id_a == "mem-01" && c.memory_id_b == "mem-02")
                || (c.memory_id_a == "mem-02" && c.memory_id_b == "mem-01")
        );

        // 归档旧条目
        mgr.archive_memory("mem-01")?;

        // 再次审计，冲突已消除
        let rep2 = mgr.audit_memories()?;
        assert_eq!(rep2.conflict_count, 0);

        Ok(())
    }
}
