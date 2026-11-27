use std::path::PathBuf;
use serde_json::Value;
use anyhow::{bail, Result};
use std::sync::Arc;


use super::McpToolRegistry;

impl McpToolRegistry {
    pub(super) fn call_infra_tool(&self, name: &str, args: &Value) -> Option<Result<String>> {
        if !matches!(name, "body_session_close" | "body_links_repair" | "body_placement_learn" | "body_env_gc" | "body_memory_audit" | "body_fact_query" | "body_hook_pre" | "body_hook_post" | "body_hook_generate" | "body_analytics_heatmap" | "body_analytics_trend" | "body_analytics_export" | "body_lock_acquire" | "body_lock_release" | "body_lock_list" | "body_mcp_catalog" | "body_skill_route" | "body_service_contact" | "body_lock_wait" | "body_lock_deadlock_check" | "body_mesh_query") {
            return None;
        }
        Some(self.call_infra_tool_inner(name, args))
    }

    fn call_infra_tool_inner(&self, name: &str, args: &Value) -> Result<String> {
        match name {
            "body_session_close" => {
                let arc_storage = Arc::new(self.storage.clone());
                let sm = crate::session::SessionManager::new(arc_storage);
                let session_id = args.get("session_id").and_then(|v| v.as_str()).unwrap_or("default");
                let clean_temp = args.get("clean_temp").and_then(|v| v.as_bool()).unwrap_or(true);
                let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);
                let res = sm.close_session_with_dry_run(session_id, clean_temp, dry_run)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "body_links_repair" => {
                let arc_storage = Arc::new(self.storage.clone());
                let repairer = crate::links::repair::LinkRepairer::new(arc_storage);
                let apply = args.get("apply").and_then(|v| v.as_bool()).unwrap_or(false);
                let res = repairer.repair_links(apply)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "body_placement_learn" => {
                let learner = crate::placement::learner::PlacementLearner::new(&self.storage);
                let min_samples = args.get("min_samples").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
                let accept = args.get("accept").and_then(|v| v.as_bool()).unwrap_or(false);
                let rep = learner.learn_rules(min_samples)?;
                let mut accepted_rules = Vec::new();
                if accept {
                    for c in &rep.candidates {
                        let rule = learner.apply_candidate(c)?;
                        accepted_rules.push(rule);
                    }
                }
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "learn_report": rep,
                    "accepted_count": accepted_rules.len(),
                    "accepted_rules": accepted_rules,
                }))?)
            }

            "body_env_gc" => {
                let default_tmp = crate::config::Config::load_effective_or_default().effective_temp_dir();
                let tmp_dir_buf = args
                    .get("tmp_dir")
                    .and_then(|v| v.as_str())
                    .map(PathBuf::from)
                    .unwrap_or(default_tmp);
                let max_age_hours = args.get("max_age_hours").and_then(|v| v.as_u64()).unwrap_or(72);
                let apply = args.get("apply").and_then(|v| v.as_bool()).unwrap_or(false);
                let res = crate::env::gc::HostGarbageCollector::run_gc(&tmp_dir_buf, max_age_hours, !apply)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "body_memory_audit" => {
                let mgr = crate::memory::lifecycle::MemoryLifecycleManager::new(&self.storage);
                if let Some(aid) = args.get("archive_id").and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()) {
                    mgr.archive_memory(aid)?;
                }
                let rep = mgr.audit_memories()?;
                Ok(serde_json::to_string_pretty(&rep)?)
            }

            "body_fact_query" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("all");
                let fact_type = args.get("fact_type").and_then(|v| v.as_str()).unwrap_or("auto");
                crate::fact::FactFacade::query_facts(fact_type, query)
            }

            "body_hook_pre" => {
                let tool_name = args.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
                let tool_args = args.get("args").cloned().unwrap_or(serde_json::json!({}));
                let engine = crate::api::hooks::AgentHookEngine::new(self.storage.clone());
                let res = engine.pre_tool_call(tool_name, &tool_args);
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "body_hook_post" => {
                let tool_name = args.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
                let tool_args = args.get("args").cloned().unwrap_or(serde_json::json!({}));
                let success = args.get("success").and_then(|v| v.as_bool()).unwrap_or(true);
                let error_output = args.get("error_output").and_then(|v| v.as_str());
                let engine = crate::api::hooks::AgentHookEngine::new(self.storage.clone());
                let res = engine.post_tool_call_with_error(tool_name, &tool_args, success, error_output);
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "body_hook_generate" => {
                let client_str = args.get("client").and_then(|v| v.as_str()).unwrap_or("agy");
                let client_type = client_str.parse::<crate::hooks::ClientType>()?;
                let out_dir = args.get("out_dir").and_then(|v| v.as_str()).map(std::path::Path::new);
                let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);

                let res = crate::hooks::ClientHookGenerator::generate(client_type, out_dir, dry_run)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "body_analytics_heatmap" => {
                use crate::storage::AnalyticsProvider;
                let days = args.get("days").and_then(|v| v.as_u64()).unwrap_or(30) as usize;
                let engine = crate::storage::DuckDbAnalyticsEngine::from_synced_storage(self.storage.provider().clone())?;
                let heatmap = engine.changes_heatmap(days)?;
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "time_span_days": days,
                    "heatmap": heatmap,
                }))?)
            }

            "body_analytics_trend" => {
                use crate::storage::AnalyticsProvider;
                let days = args.get("days").and_then(|v| v.as_u64()).unwrap_or(30) as usize;
                let engine = crate::storage::DuckDbAnalyticsEngine::from_synced_storage(self.storage.provider().clone())?;
                let trend = engine.asset_growth_trend(days)?;
                let decay = engine.memory_decay_curve()?;
                let stats = engine.fact_graph_stats()?;
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "time_span_days": days,
                    "asset_growth_trend": trend,
                    "memory_decay": decay,
                    "graph_stats": stats,
                }))?)
            }

            "body_analytics_export" => {
                use crate::storage::AnalyticsProvider;
                let table = args.get("table").and_then(|v| v.as_str()).unwrap_or("assets");
                let default_out = crate::config::Config::load_effective_or_default()
                    .effective_temp_dir()
                    .join("export.parquet");
                let default_out_str = default_out.to_string_lossy().to_string();
                let output_path = args.get("output_path").and_then(|v| v.as_str()).unwrap_or(&default_out_str);
                let engine = crate::storage::DuckDbAnalyticsEngine::from_synced_storage(self.storage.provider().clone())?;
                let summary = engine.export_to_parquet(table, output_path)?;
                Ok(serde_json::to_string_pretty(&summary)?)
            }

            "body_lock_acquire" => {
                use crate::coordination::{CoordinationEngine, LockRequest, ResourceType, LockScope};
                let rt_str = args.get("resource_type").and_then(|v| v.as_str()).unwrap_or("custom");
                let res_id = args.get("resource_id").and_then(|v| v.as_str()).unwrap_or("");
                let scope_str = args.get("scope").and_then(|v| v.as_str()).unwrap_or("exclusive");
                let holder = args.get("holder_agent").and_then(|v| v.as_str()).unwrap_or("current-agent");
                let pid = args.get("holder_pid").and_then(|v| v.as_u64()).map(|p| p as u32);
                let ttl = args.get("ttl_seconds").and_then(|v| v.as_u64()).unwrap_or(300);
                let meta = args.get("metadata").cloned();

                let engine = CoordinationEngine::new(self.storage.provider().clone());
                let req = LockRequest {
                    resource_type: ResourceType::from_str_loose(rt_str),
                    resource_id: res_id.to_string(),
                    scope: LockScope::from_str_loose(scope_str),
                    holder_agent: holder.to_string(),
                    holder_pid: pid,
                    ttl_seconds: ttl,
                    metadata: meta,
                };
                let res = engine.try_acquire(&req)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "body_lock_release" => {
                use crate::coordination::CoordinationEngine;
                let lock_id = args.get("lock_id").and_then(|v| v.as_str());
                let resource_type = args.get("resource_type").and_then(|v| v.as_str());
                let resource_id = args.get("resource_id").and_then(|v| v.as_str());
                let holder = args
                    .get("holder_agent")
                    .or_else(|| args.get("agent_id"))
                    .and_then(|v| v.as_str())
                    .or(Some("current-agent"));
                let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
                let engine = CoordinationEngine::new(self.storage.provider().clone());
                let (resolved_id, released) = engine.release_flexible(
                    lock_id,
                    resource_type,
                    resource_id,
                    holder,
                    force,
                )?;
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "lock_id": resolved_id.unwrap_or_default(),
                    "resource_type": resource_type,
                    "resource_id": resource_id,
                    "released": released,
                    "force": force,
                }))?)
            }

            "body_lock_list" => {
                use crate::coordination::{CoordinationEngine, ResourceType};
                let rt_opt = args.get("resource_type").and_then(|v| v.as_str()).map(ResourceType::from_str_loose);
                let engine = CoordinationEngine::new(self.storage.provider().clone());
                let list = engine.list_active_locks(rt_opt.as_ref())?;
                Ok(serde_json::to_string_pretty(&list)?)
            }

            "body_mcp_catalog" => {
                use crate::ecosystem::McpConfigScanner;
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("route");
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
                let scanner = McpConfigScanner::new(self.storage.clone());
                match action {
                    "scan" => {
                        let (cnt, sanitized) = scanner.scan_default_clients()?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "mcp_servers_discovered": cnt,
                            "sanitized_env_keys_count": sanitized
                        }))?)
                    }
                    "list" => {
                        let list = self.storage.list_mcp_catalog()?;
                        Ok(serde_json::to_string_pretty(&list)?)
                    }
                    _ => {
                        let list = if query.trim().is_empty() {
                            self.storage.list_mcp_catalog()?.into_iter().take(limit).collect()
                        } else {
                            self.storage.search_mcp_catalog(query, limit)?
                        };
                        Ok(serde_json::to_string_pretty(&list)?)
                    }
                }
            }

            "body_skill_route" => {
                use crate::ecosystem::{ContextBudgetRouter, ContextRouteRequest, SkillCatalogScanner};
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("route");
                let intent = args.get("intent").and_then(|v| v.as_str()).unwrap_or("");
                let budget_tokens = args.get("budget_tokens").and_then(|v| v.as_u64()).unwrap_or(600) as usize;
                let max_skills = args.get("max_skills").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
                match action {
                    "scan" => {
                        let scanner = SkillCatalogScanner::new(self.storage.clone());
                        let (cnt, tokens) = scanner.scan_default_roots()?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "skills_discovered": cnt,
                            "total_full_tokens_estimated": tokens
                        }))?)
                    }
                    "list" => {
                        let list = self.storage.list_skills_catalog()?;
                        Ok(serde_json::to_string_pretty(&list)?)
                    }
                    _ => {
                        let router = ContextBudgetRouter::new(self.storage.clone());
                        let plan = router.route(&ContextRouteRequest {
                            intent: intent.to_string(),
                            budget_tokens,
                            max_skills,
                            max_mcps: 4,
                            max_services: 3,
                        })?;
                        Ok(serde_json::to_string_pretty(&plan)?)
                    }
                }
            }

            "body_service_contact" => {
                use crate::ecosystem::ExternalContactsManager;
                use crate::storage::models::ExternalServiceRecord;
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");
                let mgr = ExternalContactsManager::new(self.storage.clone());
                match action {
                    "register" => {
                        let service_name = args.get("service_name").and_then(|v| v.as_str()).unwrap_or("unnamed-service");
                        let rec = ExternalServiceRecord {
                            id: format!("svc:{}", service_name),
                            service_name: service_name.to_string(),
                            category: args.get("category").and_then(|v| v.as_str()).unwrap_or("api").to_string(),
                            endpoint: args.get("endpoint").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            protocol: args.get("protocol").and_then(|v| v.as_str()).unwrap_or("https").to_string(),
                            capabilities: args.get("capabilities").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            auth_type: args.get("auth_type").and_then(|v| v.as_str()).unwrap_or("none").to_string(),
                            auth_env_var: args.get("auth_env_var").and_then(|v| v.as_str()).map(String::from),
                            health_endpoint: args.get("health_endpoint").and_then(|v| v.as_str()).map(String::from),
                            usage_example: args.get("usage_example").and_then(|v| v.as_str()).map(String::from),
                            status: "unknown".to_string(),
                            last_checked_at: None,
                        };
                        mgr.register_service(&rec)?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "registered": true,
                            "service_name": rec.service_name,
                            "auth_env_ready": ExternalContactsManager::is_auth_env_ready(rec.auth_env_var.as_deref())
                        }))?)
                    }
                    "check" => {
                        let name_arg = args.get("service_name").and_then(|v| v.as_str()).unwrap_or("");
                        let rep = mgr.check_health(name_arg)?;
                        Ok(serde_json::to_string_pretty(&rep)?)
                    }
                    "query" => {
                        let q = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                        let list = self.storage.search_external_services(q, 10)?;
                        Ok(serde_json::to_string_pretty(&list)?)
                    }
                    "bootstrap" => {
                        let cfg = crate::config::Config::load_or_default(None).unwrap_or_default();
                        let count = mgr.bootstrap_from_config(&cfg)?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({ "bootstrapped": count }))?)
                    }
                    _ => {
                        let cat = args.get("category").and_then(|v| v.as_str());
                        let list = self.storage.list_external_services(cat)?;
                        Ok(serde_json::to_string_pretty(&list)?)
                    }
                }
            }

            "body_lock_wait" => {
                use crate::coordination::{CoordinationEngine, LockRequest, LockScope, ResourceType};
                let rt_str = args.get("resource_type").and_then(|v| v.as_str()).unwrap_or("custom");
                let res_id = args.get("resource_id").and_then(|v| v.as_str()).unwrap_or("");
                let scope_str = args.get("scope").and_then(|v| v.as_str()).unwrap_or("exclusive");
                let holder = args.get("holder_agent").and_then(|v| v.as_str()).unwrap_or("current-agent");
                let ttl = args.get("ttl_seconds").and_then(|v| v.as_u64()).unwrap_or(300);
                let wait_ms = args.get("wait_timeout_ms").and_then(|v| v.as_u64()).unwrap_or(5000);
                let priority = args.get("priority").and_then(|v| v.as_i64()).unwrap_or(100) as i32;

                let engine = CoordinationEngine::new(self.storage.provider().clone());
                let req = LockRequest {
                    resource_type: ResourceType::from_str_loose(rt_str),
                    resource_id: res_id.to_string(),
                    scope: LockScope::from_str_loose(scope_str),
                    holder_agent: holder.to_string(),
                    holder_pid: None,
                    ttl_seconds: ttl,
                    metadata: None,
                };
                let res = engine.wait_and_acquire(&req, wait_ms, priority)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "body_lock_deadlock_check" => {
                use crate::coordination::CoordinationEngine;
                let engine = CoordinationEngine::new(self.storage.provider().clone());
                let rep = engine.detect_deadlocks()?;
                let waits = engine.list_wait_queue(None)?;
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "deadlock_report": rep,
                    "wait_queue_count": waits.len(),
                    "wait_queue": waits,
                }))?)
            }

            "body_mesh_query" => {
                use crate::coordination::{LockRequest, LockScope, ResourceType};
                use crate::mesh::{build_local_facts, MeshClient};
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("ping");
                let cfg = crate::config::Config::load_or_default(None).unwrap_or_default();
                let client = MeshClient::new(&cfg.mesh, cfg.web.auth_token.clone());
                match action {
                    "local" => Ok(serde_json::to_string_pretty(&build_local_facts())?),
                    "peers" => Ok(serde_json::to_string_pretty(&serde_json::json!({
                        "enabled": cfg.mesh.enabled,
                        "peers": client.peers(),
                        "timeout_ms": cfg.mesh.timeout_ms,
                    }))?),
                    "facts" => Ok(serde_json::to_string_pretty(&client.facts_all())?),
                    "locks" => Ok(serde_json::to_string_pretty(&client.locks_all())?),
                    "port" => {
                        let port = args.get("port").and_then(|v| v.as_u64()).unwrap_or(0) as u16;
                        Ok(serde_json::to_string_pretty(&client.federated_port_check(port))?)
                    }
                    "acquire" => {
                        let peer = args.get("peer").and_then(|v| v.as_str()).unwrap_or("");
                        if peer.is_empty() {
                            bail!("action=acquire 必须提供 peer 参数");
                        }
                        let req = LockRequest {
                            resource_type: ResourceType::from_str_loose(args.get("resource_type").and_then(|v| v.as_str()).unwrap_or("custom")),
                            resource_id: args.get("resource_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            scope: LockScope::from_str_loose(args.get("scope").and_then(|v| v.as_str()).unwrap_or("exclusive")),
                            holder_agent: args.get("holder_agent").and_then(|v| v.as_str()).unwrap_or("current-agent").to_string(),
                            holder_pid: None,
                            ttl_seconds: args.get("ttl_seconds").and_then(|v| v.as_u64()).unwrap_or(300),
                            metadata: None,
                        };
                        Ok(serde_json::to_string_pretty(&client.acquire_on_peer(peer, &req)?)?)
                    }
                    "graph" => {
                        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                        let hops = args.get("hops").and_then(|v| v.as_u64()).unwrap_or(2) as usize;
                        Ok(serde_json::to_string_pretty(&client.federated_graph_query(query, hops))?)
                    }
                    "dialogue_send" => {
                        let peer = args.get("peer").and_then(|v| v.as_str()).unwrap_or("");
                        if peer.is_empty() {
                            bail!("action=dialogue_send 必须提供 peer 参数");
                        }
                        let thread_id = args.get("thread_id").and_then(|v| v.as_str()).unwrap_or("");
                        let intent = args.get("intent").and_then(|v| v.as_str()).unwrap_or("ASK_CONTEXT");
                        let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
                        let to_agent = args.get("to_agent").and_then(|v| v.as_str()).unwrap_or("");
                        let from_agent = args.get("holder_agent").and_then(|v| v.as_str()).unwrap_or("mcp-agent");
                        Ok(serde_json::to_string_pretty(&client.send_dialogue(
                            peer, thread_id, intent, content, from_agent, to_agent,
                        )?)?)
                    }
                    "dialogue_inbox" => {
                        let rows = self.storage.list_mesh_dialogues(30)?;
                        Ok(serde_json::to_string_pretty(&rows)?)
                    }
                    "dialogue_ack" => {
                        let did = args.get("dialogue_id").and_then(|v| v.as_str()).unwrap_or("");
                        let updated = self.storage.update_mesh_dialogue_status(did, "acked")?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({ "dialogue_id": did, "acked": updated }))?)
                    }
                    "transfer_send" => {
                        let peer = args.get("peer").and_then(|v| v.as_str()).unwrap_or("");
                        let file_path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
                        if peer.is_empty() || file_path.is_empty() {
                            bail!("action=transfer_send 必须提供 peer 与 file_path 参数");
                        }
                        let from_agent = args.get("holder_agent").and_then(|v| v.as_str()).unwrap_or("mcp-agent");
                        Ok(serde_json::to_string_pretty(&client.send_transfer(
                            peer, std::path::Path::new(file_path), from_agent,
                        )?)?)
                    }
                    "transfer_inbox" => {
                        let rows = self.storage.list_mesh_transfers(30)?;
                        Ok(serde_json::to_string_pretty(&rows)?)
                    }
                    "transfer_accept" => {
                        let tid = args.get("transfer_id").and_then(|v| v.as_str()).unwrap_or("");
                        let default_dest = crate::config::Config::load_effective_or_default()
                            .effective_temp_dir()
                            .join("mesh_accepted");
                        let dest_buf = args
                            .get("dest_dir")
                            .and_then(|v| v.as_str())
                            .map(PathBuf::from)
                            .unwrap_or(default_dest);
                        let rec = crate::mesh::transfer::accept_transfer_to_dir(
                            self.storage.provider().as_ref(),
                            tid,
                            &dest_buf,
                        )?;
                        Ok(serde_json::to_string_pretty(&rec)?)
                    }
                    "transfer_reject" => {
                        let tid = args.get("transfer_id").and_then(|v| v.as_str()).unwrap_or("");
                        let ok = crate::mesh::transfer::reject_transfer(self.storage.provider().as_ref(), tid)?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({ "transfer_id": tid, "rejected": ok }))?)
                    }
                    _ => Ok(serde_json::to_string_pretty(&client.ping_all())?),
                }
            }

            _ => bail!("Unknown tool: {}", name),
        }
    }
}
