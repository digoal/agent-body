use std::collections::{HashMap, HashSet};
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::storage::models::{CoordinationLock, LockWaitEntry};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadlockCycle {
    pub cycle_agents: Vec<String>,
    pub cycle_resources: Vec<String>,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadlockReport {
    pub has_deadlock: bool,
    pub wait_edges_count: usize,
    pub cycles: Vec<DeadlockCycle>,
}

/// 基于 Wait-For Graph (WFG) 的多 Agent 死锁环路检测引擎
pub struct DeadlockDetector;

impl DeadlockDetector {
    /// 构建有向等待图 (Waiter -> Holder) 并执行 DFS 环路检测
    /// `pending_request`: 可选 `(applicant_agent, resource_type, norm_resource_id)`，用于入队前死锁预判
    pub fn detect(
        active_locks: &[CoordinationLock],
        wait_queue: &[LockWaitEntry],
        pending_request: Option<(&str, &str, &str)>,
    ) -> DeadlockReport {
        // 邻接表: waiter -> Vec<(holder, resource_desc)>
        let mut adj: HashMap<String, Vec<(String, String)>> = HashMap::new();
        let mut edge_count = 0usize;

        let mut add_wait_edges = |waiter: &str, rt: &str, rid: &str| {
            for lock in active_locks {
                if lock.holder_agent == waiter {
                    continue;
                }
                if Self::resources_conflict(rt, rid, &lock.resource_type, &lock.resource_id) {
                    let res_desc = format!("{}:{}", lock.resource_type, lock.resource_id);
                    adj.entry(waiter.to_string())
                        .or_default()
                        .push((lock.holder_agent.clone(), res_desc));
                    edge_count += 1;
                }
            }
        };

        for w in wait_queue {
            if w.status == "waiting" || w.status == "notified" {
                add_wait_edges(&w.waiting_agent, &w.resource_type, &w.resource_id);
            }
        }

        if let Some((applicant, rt, rid)) = pending_request {
            add_wait_edges(applicant, rt, rid);
        }

        // DFS 环路检测
        let mut state: HashMap<String, u8> = HashMap::new(); // 0=unvisited, 1=visiting, 2=visited
        let mut path_agents: Vec<String> = Vec::new();
        let mut path_resources: Vec<String> = Vec::new();
        let mut cycles: Vec<DeadlockCycle> = Vec::new();
        let mut seen_cycle_sigs: HashSet<String> = HashSet::new();

        let nodes: Vec<String> = adj.keys().cloned().collect();
        for start in nodes {
            if state.get(&start).copied().unwrap_or(0) == 0 {
                Self::dfs(
                    &start,
                    &adj,
                    &mut state,
                    &mut path_agents,
                    &mut path_resources,
                    &mut cycles,
                    &mut seen_cycle_sigs,
                );
            }
        }

        DeadlockReport {
            has_deadlock: !cycles.is_empty(),
            wait_edges_count: edge_count,
            cycles,
        }
    }

    fn dfs(
        node: &str,
        adj: &HashMap<String, Vec<(String, String)>>,
        state: &mut HashMap<String, u8>,
        path_agents: &mut Vec<String>,
        path_resources: &mut Vec<String>,
        cycles: &mut Vec<DeadlockCycle>,
        seen_cycle_sigs: &mut HashSet<String>,
    ) {
        state.insert(node.to_string(), 1);
        path_agents.push(node.to_string());

        if let Some(neighbors) = adj.get(node) {
            for (next_holder, res_desc) in neighbors {
                let st = state.get(next_holder).copied().unwrap_or(0);
                if st == 1 {
                    // 发现环路！截取从 next_holder 开始的环
                    if let Some(idx) = path_agents.iter().position(|a| a == next_holder) {
                        let mut cycle_agents = path_agents[idx..].to_vec();
                        cycle_agents.push(next_holder.clone());
                        let mut cycle_resources = if idx < path_resources.len() {
                            path_resources[idx..].to_vec()
                        } else {
                            Vec::new()
                        };
                        cycle_resources.push(res_desc.clone());

                        let mut sorted_agents = path_agents[idx..].to_vec();
                        sorted_agents.sort();
                        let sig = sorted_agents.join("->");
                        if seen_cycle_sigs.insert(sig) {
                            let suggestion = format!(
                                "检测到死锁环路 [{}]，建议最后申请方 '{}' 主动放弃等待或先释放已持有资源 ({})。",
                                cycle_agents.join(" -> "),
                                node,
                                cycle_resources.join(", ")
                            );
                            cycles.push(DeadlockCycle {
                                cycle_agents,
                                cycle_resources,
                                suggestion,
                            });
                        }
                    }
                } else if st == 0 {
                    path_resources.push(res_desc.clone());
                    Self::dfs(
                        next_holder,
                        adj,
                        state,
                        path_agents,
                        path_resources,
                        cycles,
                        seen_cycle_sigs,
                    );
                    path_resources.pop();
                }
            }
        }

        path_agents.pop();
        state.insert(node.to_string(), 2);
    }

    /// 判断两个资源是否存在互斥冲突（含目录祖先后代传递）
    fn resources_conflict(rt_a: &str, rid_a: &str, rt_b: &str, rid_b: &str) -> bool {
        if rt_a == rt_b && rid_a == rid_b {
            return true;
        }
        // 目录/文件层级互斥传递
        if (rt_a == "directory" || rt_a == "file") && (rt_b == "directory" || rt_b == "file") {
            let pa = Path::new(rid_a);
            let pb = Path::new(rid_b);
            if pa.starts_with(pb) || pb.starts_with(pa) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_lock(rt: &str, rid: &str, holder: &str) -> CoordinationLock {
        CoordinationLock {
            lock_id: format!("l-{}-{}", holder, rid),
            resource_type: rt.to_string(),
            resource_id: rid.to_string(),
            scope: "exclusive".to_string(),
            holder_agent: holder.to_string(),
            holder_pid: None,
            acquired_at: "2026-11-18T00:00:00Z".to_string(),
            expires_at: "2099-01-01T00:00:00Z".to_string(),
            metadata: None,
        }
    }

    fn make_wait(rt: &str, rid: &str, waiter: &str) -> LockWaitEntry {
        LockWaitEntry {
            wait_id: format!("w-{}-{}", waiter, rid),
            resource_type: rt.to_string(),
            resource_id: rid.to_string(),
            scope: "exclusive".to_string(),
            waiting_agent: waiter.to_string(),
            priority: 100,
            enqueued_at: "2026-11-18T00:00:01Z".to_string(),
            timeout_at: "2099-01-01T00:00:00Z".to_string(),
            status: "waiting".to_string(),
        }
    }

    #[test]
    fn test_two_agent_deadlock_detected() {
        // Agent A 持 port:8080 等 port:8081；Agent B 持 port:8081 拟等 port:8080
        let locks = vec![
            make_lock("port", "8080", "agent-a"),
            make_lock("port", "8081", "agent-b"),
        ];
        let waits = vec![make_wait("port", "8081", "agent-a")];

        let rep = DeadlockDetector::detect(&locks, &waits, Some(("agent-b", "port", "8080")));
        assert!(rep.has_deadlock);
        assert_eq!(rep.cycles.len(), 1);
        assert!(rep.cycles[0].cycle_agents.contains(&"agent-a".to_string()));
        assert!(rep.cycles[0].cycle_agents.contains(&"agent-b".to_string()));
    }

    #[test]
    fn test_three_agent_cycle_with_directory_hierarchy() {
        // A 持 dir:/work/ 等 port:9000; B 持 port:9000 等 service:pg.service; C 持 service:pg.service 拟写 file:/work/a.rs
        let locks = vec![
            make_lock("directory", "/work/", "agent-a"),
            make_lock("port", "9000", "agent-b"),
            make_lock("service", "pg.service", "agent-c"),
        ];
        let waits = vec![
            make_wait("port", "9000", "agent-a"),
            make_wait("service", "pg.service", "agent-b"),
        ];

        let rep = DeadlockDetector::detect(&locks, &waits, Some(("agent-c", "file", "/work/a.rs")));
        assert!(rep.has_deadlock, "目录祖先互斥传递必须触发 3-Agent 死锁检出");
        assert_eq!(rep.cycles[0].cycle_agents.len(), 4); // C -> A -> B -> C
    }
}
