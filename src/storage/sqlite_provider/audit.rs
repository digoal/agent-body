use super::SqliteStorageProvider;
use anyhow::{Result};
use rusqlite::params;
use crate::storage::models::*;

impl SqliteStorageProvider {
    pub(super) fn impl_upsert_mcp_catalog(&self, rec: &McpCatalogRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let tools_str = serde_json::to_string(&rec.tools_json)?;
        conn.execute(
            "INSERT INTO mcp_catalog (id, server_name, client_source, transport, command_or_url, tools_json, tool_count, est_total_tokens, status, last_scanned_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
             ON CONFLICT(server_name) DO UPDATE SET
               client_source=excluded.client_source,
               transport=excluded.transport,
               command_or_url=excluded.command_or_url,
               tools_json=excluded.tools_json,
               tool_count=excluded.tool_count,
               est_total_tokens=excluded.est_total_tokens,
               status=excluded.status,
               last_scanned_at=strftime('%Y-%m-%dT%H:%M:%f+00:00','now')",
            params![
                rec.id, rec.server_name, rec.client_source, rec.transport,
                rec.command_or_url, tools_str, rec.tool_count as i64,
                rec.est_total_tokens as i64, rec.status
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_list_mcp_catalog(&self) -> Result<Vec<McpCatalogRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, server_name, client_source, transport, command_or_url, tools_json, tool_count, est_total_tokens, status, last_scanned_at
             FROM mcp_catalog ORDER BY server_name ASC"
        )?;
        let rows = stmt.query_map([], Self::map_mcp_row)?;
        let mut res = Vec::new();
        for r in rows { res.push(r?); }
        Ok(res)
    }


    pub(super) fn impl_search_mcp_catalog(&self, query: &str, limit: usize) -> Result<Vec<McpCatalogRecord>> {
        let conn = self.lock_conn()?;
        let kw = format!("%{}%", query.to_lowercase());
        let mut stmt = conn.prepare(
            "SELECT id, server_name, client_source, transport, command_or_url, tools_json, tool_count, est_total_tokens, status, last_scanned_at
             FROM mcp_catalog
             WHERE lower(server_name) LIKE ?1 OR lower(tools_json) LIKE ?1 OR lower(client_source) LIKE ?1
             ORDER BY tool_count DESC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![kw, limit as i64], Self::map_mcp_row)?;
        let mut res = Vec::new();
        for r in rows { res.push(r?); }
        Ok(res)
    }


    pub(super) fn impl_upsert_skill_catalog(&self, rec: &SkillCatalogRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let triggers_str = serde_json::to_string(&rec.triggers)?;
        conn.execute(
            "INSERT INTO skills_catalog (id, skill_name, root_source, skill_path, description, triggers_json, est_tokens, has_scripts, has_references, status, last_scanned_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
             ON CONFLICT(skill_name) DO UPDATE SET
               root_source=excluded.root_source,
               skill_path=excluded.skill_path,
               description=excluded.description,
               triggers_json=excluded.triggers_json,
               est_tokens=excluded.est_tokens,
               has_scripts=excluded.has_scripts,
               has_references=excluded.has_references,
               status=excluded.status,
               last_scanned_at=strftime('%Y-%m-%dT%H:%M:%f+00:00','now')",
            params![
                rec.id, rec.skill_name, rec.root_source, rec.skill_path,
                rec.description, triggers_str, rec.est_tokens as i64,
                rec.has_scripts as i32, rec.has_references as i32, rec.status
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_list_skills_catalog(&self) -> Result<Vec<SkillCatalogRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, skill_name, root_source, skill_path, description, triggers_json, est_tokens, has_scripts, has_references, status, last_scanned_at
             FROM skills_catalog ORDER BY skill_name ASC"
        )?;
        let rows = stmt.query_map([], Self::map_skill_row)?;
        let mut res = Vec::new();
        for r in rows { res.push(r?); }
        Ok(res)
    }


    pub(super) fn impl_search_skills_catalog(&self, query: &str, limit: usize) -> Result<Vec<SkillCatalogRecord>> {
        let conn = self.lock_conn()?;
        let clean_q: String = query
            .chars()
            .map(|c| if c.is_alphanumeric() || c as u32 > 0x4E00 || c == ' ' { c } else { ' ' })
            .collect();
        let fts_tokens: Vec<String> = clean_q.split_whitespace().map(|w| format!("\"{}\"", w)).collect();
        if !fts_tokens.is_empty() {
            let fts_expr = fts_tokens.join(" OR ");
            if let Ok(mut stmt) = conn.prepare(
                "SELECT s.id, s.skill_name, s.root_source, s.skill_path, s.description, s.triggers_json, s.est_tokens, s.has_scripts, s.has_references, s.status, s.last_scanned_at
                 FROM skills_catalog s
                 JOIN skills_catalog_fts f ON s.rowid = f.rowid
                 WHERE skills_catalog_fts MATCH ?1
                 ORDER BY rank LIMIT ?2"
            ) {
                if let Ok(rows) = stmt.query_map(params![fts_expr, limit as i64], Self::map_skill_row) {
                    let mut out = Vec::new();
                    for item in rows.flatten() { out.push(item); }
                    if !out.is_empty() {
                        return Ok(out);
                    }
                }
            }
        }
        let kw = format!("%{}%", query.to_lowercase());
        let mut stmt = conn.prepare(
            "SELECT id, skill_name, root_source, skill_path, description, triggers_json, est_tokens, has_scripts, has_references, status, last_scanned_at
             FROM skills_catalog
             WHERE lower(skill_name) LIKE ?1 OR lower(description) LIKE ?1 OR lower(triggers_json) LIKE ?1
             ORDER BY skill_name ASC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![kw, limit as i64], Self::map_skill_row)?;
        let mut res = Vec::new();
        for r in rows { res.push(r?); }
        Ok(res)
    }


    pub(super) fn impl_upsert_external_service(&self, rec: &ExternalServiceRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO external_services (id, service_name, category, endpoint, protocol, capabilities, auth_type, auth_env_var, health_endpoint, usage_example, status, last_checked_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, COALESCE(?12, strftime('%Y-%m-%dT%H:%M:%f+00:00','now')))
             ON CONFLICT(service_name) DO UPDATE SET
               category=excluded.category,
               endpoint=excluded.endpoint,
               protocol=excluded.protocol,
               capabilities=excluded.capabilities,
               auth_type=excluded.auth_type,
               auth_env_var=excluded.auth_env_var,
               health_endpoint=excluded.health_endpoint,
               usage_example=excluded.usage_example,
               status=excluded.status,
               last_checked_at=COALESCE(excluded.last_checked_at, strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))",
            params![
                rec.id, rec.service_name, rec.category, rec.endpoint, rec.protocol,
                rec.capabilities, rec.auth_type, rec.auth_env_var, rec.health_endpoint,
                rec.usage_example, rec.status, rec.last_checked_at
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_list_external_services(&self, category: Option<&str>) -> Result<Vec<ExternalServiceRecord>> {
        let conn = self.lock_conn()?;
        let mut res = Vec::new();
        match category {
            Some(cat) => {
                let mut stmt = conn.prepare(
                    "SELECT id, service_name, category, endpoint, protocol, capabilities, auth_type, auth_env_var, health_endpoint, usage_example, status, last_checked_at
                     FROM external_services WHERE category = ?1 ORDER BY service_name ASC"
                )?;
                let rows = stmt.query_map(params![cat], Self::map_ext_svc_row)?;
                for r in rows { res.push(r?); }
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, service_name, category, endpoint, protocol, capabilities, auth_type, auth_env_var, health_endpoint, usage_example, status, last_checked_at
                     FROM external_services ORDER BY service_name ASC"
                )?;
                let rows = stmt.query_map([], Self::map_ext_svc_row)?;
                for r in rows { res.push(r?); }
            }
        }
        Ok(res)
    }


    pub(super) fn impl_search_external_services(&self, query: &str, limit: usize) -> Result<Vec<ExternalServiceRecord>> {
        let conn = self.lock_conn()?;
        let kw = format!("%{}%", query.to_lowercase());
        let mut stmt = conn.prepare(
            "SELECT id, service_name, category, endpoint, protocol, capabilities, auth_type, auth_env_var, health_endpoint, usage_example, status, last_checked_at
             FROM external_services
             WHERE lower(service_name) LIKE ?1 OR lower(category) LIKE ?1 OR lower(capabilities) LIKE ?1
             ORDER BY service_name ASC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![kw, limit as i64], Self::map_ext_svc_row)?;
        let mut res = Vec::new();
        for r in rows { res.push(r?); }
        Ok(res)
    }


    pub(super) fn impl_insert_benchmark_run(&self, run: &BenchmarkRunRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let host_fp = serde_json::to_string(&run.host_fingerprint)?;
        let res_json = serde_json::to_string(&run.results_json)?;
        conn.execute(
            "INSERT OR REPLACE INTO benchmark_runs (
                run_id, suite_name, git_commit, app_version, host_fingerprint,
                workload_count, total_baseline_tokens, total_body_tokens,
                overall_token_savings_pct, overall_speedup_factor,
                total_hazards_intercepted, total_noise_blocked, sla_passed,
                results_json, report_sha256, duration_ms, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                run.run_id,
                run.suite_name,
                run.git_commit,
                run.app_version,
                host_fp,
                run.workload_count as i64,
                run.total_baseline_tokens,
                run.total_body_tokens,
                run.overall_token_savings_pct,
                run.overall_speedup_factor,
                run.total_hazards_intercepted,
                run.total_noise_blocked,
                if run.sla_passed { 1i32 } else { 0i32 },
                res_json,
                run.report_sha256,
                run.duration_ms,
                run.created_at,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_get_benchmark_run(&self, run_id: &str) -> Result<Option<BenchmarkRunRecord>> {
        let _io_guard = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::IoSqliteRead);
        let conn = self.lock_conn()?;
        if run_id == "awr:pragma_optimize" {
            if let Err(e) = conn.execute_batch(
                "PRAGMA optimize; PRAGMA wal_checkpoint(TRUNCATE); PRAGMA shrink_memory;",
            ) {
                eprintln!("[SQLite][WARN] 执行 AWR PRAGMA optimize/shrink_memory 失败: {}", e);
            }
            return Ok(None);
        }
        let mut stmt = conn.prepare(
            "SELECT run_id, suite_name, git_commit, app_version, host_fingerprint,
                    workload_count, total_baseline_tokens, total_body_tokens,
                    overall_token_savings_pct, overall_speedup_factor,
                    total_hazards_intercepted, total_noise_blocked, sla_passed,
                    results_json, report_sha256, duration_ms, created_at
             FROM benchmark_runs WHERE run_id = ?1",
        )?;
        let mut rows = stmt.query(params![run_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::map_benchmark_run_row(row)?))
        } else {
            Ok(None)
        }
    }


    pub(super) fn impl_list_benchmark_runs(&self, limit: usize) -> Result<Vec<BenchmarkRunRecord>> {
        let _io_guard = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::IoSqliteRead);
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT run_id, suite_name, git_commit, app_version, host_fingerprint,
                    workload_count, total_baseline_tokens, total_body_tokens,
                    overall_token_savings_pct, overall_speedup_factor,
                    total_hazards_intercepted, total_noise_blocked, sla_passed,
                    results_json, report_sha256, duration_ms, created_at
             FROM benchmark_runs WHERE run_id NOT LIKE 'awr:%' ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], Self::map_benchmark_run_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }


    pub(super) fn impl_list_all_benchmark_runs(&self, limit: usize) -> Result<Vec<BenchmarkRunRecord>> {
        let _io_guard = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::IoSqliteRead);
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT run_id, suite_name, git_commit, app_version, host_fingerprint,
                    workload_count, total_baseline_tokens, total_body_tokens,
                    overall_token_savings_pct, overall_speedup_factor,
                    total_hazards_intercepted, total_noise_blocked, sla_passed,
                    results_json, report_sha256, duration_ms, created_at
             FROM benchmark_runs ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], Self::map_benchmark_run_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }


    pub(super) fn impl_delete_benchmark_run(&self, run_id: &str) -> Result<bool> {
        let conn = self.lock_conn()?;
        let affected = conn.execute(
            "DELETE FROM benchmark_runs WHERE run_id = ?1",
            params![run_id],
        )?;
        Ok(affected > 0)
    }


    pub(super) fn impl_insert_effectiveness_event(&self, event: &EffectivenessLedgerRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO effectiveness_ledger (
                event_id, session_id, source_channel, tool_or_action, provenance_rule,
                baseline_tokens, body_tokens, tokens_saved,
                baseline_rounds, body_rounds, rounds_saved,
                hazard_intercepted, noise_blocked, elapsed_us, evidence_summary, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                event.event_id,
                event.session_id,
                event.source_channel,
                event.tool_or_action,
                event.provenance_rule,
                event.baseline_tokens,
                event.body_tokens,
                event.tokens_saved,
                event.baseline_rounds,
                event.body_rounds,
                event.rounds_saved,
                event.hazard_intercepted,
                event.noise_blocked,
                event.elapsed_us,
                event.evidence_summary,
                event.created_at,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_query_effectiveness_ledger(
        &self,
        window: &str,
        tool_filter: Option<&str>,
        limit: usize,
    ) -> Result<EffectivenessSummary> {
        let conn = self.lock_conn()?;
        let cutoff = crate::storage::window_to_cutoff_iso(window).unwrap_or_default();
        let tool = tool_filter.unwrap_or("").to_string();

        let (total_events, total_base, total_body, total_saved, total_rounds, total_haz, total_noise, avg_us): (
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            f64,
        ) = conn.query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(baseline_tokens), 0),
                    COALESCE(SUM(body_tokens), 0),
                    COALESCE(SUM(tokens_saved), 0),
                    COALESCE(SUM(rounds_saved), 0),
                    COALESCE(SUM(hazard_intercepted), 0),
                    COALESCE(SUM(noise_blocked), 0),
                    COALESCE(AVG(elapsed_us), 0.0)
             FROM effectiveness_ledger
             WHERE (?1 = '' OR created_at >= ?1)
               AND (?2 = '' OR tool_or_action = ?2)",
            params![cutoff, tool],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                ))
            },
        )?;

        let mut rule_stmt = conn.prepare(
            "SELECT provenance_rule,
                    COUNT(*),
                    COALESCE(SUM(tokens_saved), 0),
                    COALESCE(SUM(rounds_saved), 0),
                    COALESCE(SUM(hazard_intercepted), 0),
                    COALESCE(SUM(noise_blocked), 0)
             FROM effectiveness_ledger
             WHERE (?1 = '' OR created_at >= ?1)
               AND (?2 = '' OR tool_or_action = ?2)
             GROUP BY provenance_rule
             ORDER BY SUM(tokens_saved) DESC, COUNT(*) DESC",
        )?;
        let by_rule = rule_stmt
            .query_map(params![cutoff, tool], |r| {
                Ok(RuleAggregate {
                    provenance_rule: r.get(0)?,
                    event_count: r.get::<_, i64>(1)? as usize,
                    tokens_saved: r.get(2)?,
                    rounds_saved: r.get(3)?,
                    hazards_intercepted: r.get(4)?,
                    noise_blocked: r.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut ev_stmt = conn.prepare(
            "SELECT event_id, session_id, source_channel, tool_or_action, provenance_rule,
                    baseline_tokens, body_tokens, tokens_saved,
                    baseline_rounds, body_rounds, rounds_saved,
                    hazard_intercepted, noise_blocked, elapsed_us, evidence_summary, created_at
             FROM effectiveness_ledger
             WHERE (?1 = '' OR created_at >= ?1)
               AND (?2 = '' OR tool_or_action = ?2)
             ORDER BY created_at DESC LIMIT ?3",
        )?;
        let recent_events = ev_stmt
            .query_map(params![cutoff, tool, limit as i64], |r| {
                Ok(EffectivenessLedgerRecord {
                    event_id: r.get(0)?,
                    session_id: r.get(1)?,
                    source_channel: r.get(2)?,
                    tool_or_action: r.get(3)?,
                    provenance_rule: r.get(4)?,
                    baseline_tokens: r.get(5)?,
                    body_tokens: r.get(6)?,
                    tokens_saved: r.get(7)?,
                    baseline_rounds: r.get(8)?,
                    body_rounds: r.get(9)?,
                    rounds_saved: r.get(10)?,
                    hazard_intercepted: r.get(11)?,
                    noise_blocked: r.get(12)?,
                    elapsed_us: r.get(13)?,
                    evidence_summary: r.get(14)?,
                    created_at: r.get(15)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let token_savings_pct = if total_base > 0 {
            (total_saved as f64 / total_base as f64) * 100.0
        } else {
            0.0
        };

        Ok(EffectivenessSummary {
            window: window.to_string(),
            total_events: total_events as usize,
            total_baseline_tokens: total_base,
            total_body_tokens: total_body,
            total_tokens_saved: total_saved,
            token_savings_pct,
            total_rounds_saved: total_rounds,
            total_hazards_intercepted: total_haz,
            total_noise_blocked: total_noise,
            avg_elapsed_us: avg_us,
            by_rule,
            recent_events,
        })
    }


    pub(super) fn impl_delete_bench_sandbox_records(&self) -> Result<usize> {
        let conn = self.lock_conn()?;
        let mut removed = 0usize;
        removed += conn.execute(
            "DELETE FROM coordination_locks WHERE holder_agent LIKE 'bench-%' OR resource_id LIKE '%bench:%'",
            [],
        )?;
        removed += conn.execute(
            "DELETE FROM lock_wait_queue WHERE waiting_agent LIKE 'bench-%' OR resource_id LIKE '%bench:%'",
            [],
        )?;
        removed += conn.execute(
            "DELETE FROM effectiveness_ledger WHERE session_id LIKE 'bench:%' OR tool_or_action LIKE 'bench:%'",
            [],
        )?;
        removed += conn.execute(
            "DELETE FROM benchmark_runs WHERE run_id LIKE 'bench:%' OR suite_name LIKE 'bench:%'",
            [],
        )?;
        removed += conn.execute(
            "DELETE FROM audit_events WHERE session_id LIKE 'bench:%' OR target_id LIKE '%bench:%'",
            [],
        )?;
        Ok(removed)
    }


    pub(super) fn impl_insert_mesh_dialogue(&self, rec: &MeshDialogueRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO mesh_dialogues
             (dialogue_id, thread_id, from_node, from_agent, to_node, to_agent, intent, content, redaction_hits, injection_flagged, verified_signature, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                rec.dialogue_id,
                rec.thread_id,
                rec.from_node,
                rec.from_agent,
                rec.to_node,
                rec.to_agent,
                rec.intent,
                rec.content,
                rec.redaction_hits as i64,
                if rec.injection_flagged { 1i32 } else { 0i32 },
                if rec.verified_signature { 1i32 } else { 0i32 },
                rec.status,
                rec.created_at,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_list_mesh_dialogues(&self, limit: usize) -> Result<Vec<MeshDialogueRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT dialogue_id, thread_id, from_node, from_agent, to_node, to_agent, intent, content, redaction_hits, injection_flagged, verified_signature, status, created_at
             FROM mesh_dialogues ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], |r| {
                Ok(MeshDialogueRecord {
                    dialogue_id: r.get(0)?,
                    thread_id: r.get(1)?,
                    from_node: r.get(2)?,
                    from_agent: r.get(3)?,
                    to_node: r.get(4)?,
                    to_agent: r.get(5)?,
                    intent: r.get(6)?,
                    content: r.get(7)?,
                    redaction_hits: r.get::<_, i64>(8)? as usize,
                    injection_flagged: r.get::<_, i32>(9)? != 0,
                    verified_signature: r.get::<_, i32>(10)? != 0,
                    status: r.get(11)?,
                    created_at: r.get(12)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }


    pub(super) fn impl_update_mesh_dialogue_status(&self, dialogue_id: &str, status: &str) -> Result<bool> {
        let conn = self.lock_conn()?;
        let n = conn.execute(
            "UPDATE mesh_dialogues SET status = ?1 WHERE dialogue_id = ?2",
            params![status, dialogue_id],
        )?;
        Ok(n > 0)
    }


    pub(super) fn impl_insert_mesh_transfer(&self, rec: &MeshTransferRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO mesh_transfers
             (transfer_id, from_node, from_agent, to_node, file_name, size_bytes, sha256, inbox_path, secret_warnings, verified_signature, status, accepted_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                rec.transfer_id,
                rec.from_node,
                rec.from_agent,
                rec.to_node,
                rec.file_name,
                rec.size_bytes as i64,
                rec.sha256,
                rec.inbox_path,
                rec.secret_warnings as i64,
                if rec.verified_signature { 1i32 } else { 0i32 },
                rec.status,
                rec.accepted_path,
                rec.created_at,
                rec.updated_at,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_list_mesh_transfers(&self, limit: usize) -> Result<Vec<MeshTransferRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT transfer_id, from_node, from_agent, to_node, file_name, size_bytes, sha256, inbox_path, secret_warnings, verified_signature, status, accepted_path, created_at, updated_at
             FROM mesh_transfers ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], |r| {
                Ok(MeshTransferRecord {
                    transfer_id: r.get(0)?,
                    from_node: r.get(1)?,
                    from_agent: r.get(2)?,
                    to_node: r.get(3)?,
                    file_name: r.get(4)?,
                    size_bytes: r.get::<_, i64>(5)? as u64,
                    sha256: r.get(6)?,
                    inbox_path: r.get(7)?,
                    secret_warnings: r.get::<_, i64>(8)? as usize,
                    verified_signature: r.get::<_, i32>(9)? != 0,
                    status: r.get(10)?,
                    accepted_path: r.get(11)?,
                    created_at: r.get(12)?,
                    updated_at: r.get(13)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }


    pub(super) fn impl_update_mesh_transfer_status(
        &self,
        transfer_id: &str,
        status: &str,
        accepted_path: Option<&str>,
    ) -> Result<bool> {
        let conn = self.lock_conn()?;
        let n = conn.execute(
            "UPDATE mesh_transfers SET status = ?1, accepted_path = COALESCE(?2, accepted_path), updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00','now') WHERE transfer_id = ?3",
            params![status, accepted_path, transfer_id],
        )?;
        Ok(n > 0)
    }


    pub(super) fn impl_insert_audit_event(&self, rec: &AuditEventRecord) -> Result<()> {
        let _wait = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::IoSqliteWrite);
        let conn = self.lock_conn()?;
        let delta_str = serde_json::to_string(&rec.before_after_delta)?;
        conn.execute(
            "INSERT OR REPLACE INTO audit_events (
                event_id, trace_id, session_id, domain, actor_kind, actor_id,
                source_channel, category, action, target_type, target_id,
                concurrency_mode, outcome, before_after_delta, elapsed_us,
                evidence_sha256, created_at
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
            params![
                rec.event_id,
                rec.trace_id,
                rec.session_id,
                rec.domain,
                rec.actor_kind,
                rec.actor_id,
                rec.source_channel,
                rec.category,
                rec.action,
                rec.target_type,
                rec.target_id,
                rec.concurrency_mode,
                rec.outcome,
                delta_str,
                rec.elapsed_us,
                rec.evidence_sha256,
                rec.created_at,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_query_audit_events(&self, filter: &AuditQueryFilter) -> Result<Vec<AuditEventRecord>> {
        let conn = self.lock_conn()?;
        let domain = filter.domain.as_deref().unwrap_or("");
        let actor_kind = filter.actor_kind.as_deref().unwrap_or("");
        let category = filter.category.as_deref().unwrap_or("");
        let action = filter.action.as_deref().unwrap_or("");
        let outcome = filter.outcome.as_deref().unwrap_or("");
        let trace_id = filter.trace_id.as_deref().unwrap_or("");
        let session_id = filter.session_id.as_deref().unwrap_or("");
        let target_kw = filter.target_contains.as_deref().unwrap_or("");
        let cutoff = filter
            .window
            .as_deref()
            .and_then(crate::storage::window_to_cutoff_iso)
            .unwrap_or_default();
        let lim = if filter.limit == 0 { 50 } else { filter.limit.min(100_000) } as i64;

        let mut stmt = conn.prepare(
            "SELECT event_id, trace_id, session_id, domain, actor_kind, actor_id,
                    source_channel, category, action, target_type, target_id,
                    concurrency_mode, outcome, before_after_delta, elapsed_us,
                    evidence_sha256, created_at
             FROM audit_events
             WHERE (?1 = '' OR domain = ?1)
               AND (?2 = '' OR actor_kind = ?2)
               AND (?3 = '' OR category = ?3)
               AND (?4 = '' OR action = ?4)
               AND (?5 = '' OR outcome = ?5)
               AND (?6 = '' OR trace_id = ?6)
               AND (?7 = '' OR session_id = ?7)
               AND (?8 = '' OR target_id LIKE '%' || ?8 || '%')
               AND (?9 = '' OR created_at >= ?9)
             ORDER BY created_at DESC LIMIT ?10",
        )?;
        let rows = stmt
            .query_map(
                params![domain, actor_kind, category, action, outcome, trace_id, session_id, target_kw, cutoff, lim],
                |r| {
                    let delta_str: String = r.get(13)?;
                    Ok(AuditEventRecord {
                        event_id: r.get(0)?,
                        trace_id: r.get(1)?,
                        session_id: r.get(2)?,
                        domain: r.get(3)?,
                        actor_kind: r.get(4)?,
                        actor_id: r.get(5)?,
                        source_channel: r.get(6)?,
                        category: r.get(7)?,
                        action: r.get(8)?,
                        target_type: r.get(9)?,
                        target_id: r.get(10)?,
                        concurrency_mode: r.get(11)?,
                        outcome: r.get(12)?,
                        before_after_delta: serde_json::from_str(&delta_str)
                            .unwrap_or_else(|_| serde_json::json!({})),
                        elapsed_us: r.get(14)?,
                        evidence_sha256: r.get(15)?,
                        created_at: r.get(16)?,
                    })
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }


    pub(super) fn impl_audit_domain_summary(&self, window: &str) -> Result<AuditDomainSummary> {
        let conn = self.lock_conn()?;
        let cutoff = crate::storage::window_to_cutoff_iso(window).unwrap_or_default();
        let mut stmt = conn.prepare(
            "SELECT domain, actor_kind, outcome,
                    COUNT(*),
                    COALESCE(AVG(elapsed_us), 0.0)
             FROM audit_events
             WHERE (?1 = '' OR created_at >= ?1)
             GROUP BY domain, actor_kind, outcome
             ORDER BY COUNT(*) DESC",
        )?;
        let breakdown = stmt
            .query_map(params![cutoff], |r| {
                Ok(AuditDomainBreakdown {
                    domain: r.get(0)?,
                    actor_kind: r.get(1)?,
                    outcome: r.get(2)?,
                    event_count: r.get::<_, i64>(3)? as usize,
                    avg_elapsed_us: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut summary = AuditDomainSummary {
            window: window.to_string(),
            breakdown,
            ..Default::default()
        };
        for b in &summary.breakdown {
            summary.total_events += b.event_count;
            if b.domain == "IN_AGENT" {
                summary.in_agent_count += b.event_count;
            } else {
                summary.out_of_agent_count += b.event_count;
            }
            if b.outcome == "failed" || b.outcome == "blocked" {
                summary.failed_or_blocked_count += b.event_count;
            }
            if b.outcome == "recovered" {
                summary.recovered_count += b.event_count;
            }
        }
        Ok(summary)
    }


    pub(super) fn impl_prune_old_audit_events(&self, before_iso: &str) -> Result<usize> {
        let conn = self.lock_conn()?;
        let affected = conn.execute("DELETE FROM audit_events WHERE created_at < ?1", params![before_iso])?;
        Ok(affected)
    }


    pub(super) fn impl_insert_body_checkpoint(&self, rec: &BodyCheckpointRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let active_files = serde_json::to_string(&rec.active_files)?;
        let active_locks = serde_json::to_string(&rec.active_locks)?;
        let verified_facts = serde_json::to_string(&rec.verified_facts)?;
        let open_blockers = serde_json::to_string(&rec.open_blockers)?;
        conn.execute(
            "INSERT OR REPLACE INTO body_checkpoints (
                checkpoint_id, body_id, from_brain, session_id, goal_summary,
                active_files_json, active_locks_json, verified_facts_json, open_blockers_json,
                next_step_hint, token_budget, status, evidence_sha256, created_at,
                resumed_by_brain, resumed_at, tx_epoch
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
            params![
                rec.checkpoint_id,
                rec.body_id,
                rec.from_brain,
                rec.session_id,
                rec.goal_summary,
                active_files,
                active_locks,
                verified_facts,
                open_blockers,
                rec.next_step_hint,
                rec.token_budget as i64,
                rec.status,
                rec.evidence_sha256,
                rec.created_at,
                rec.resumed_by_brain,
                rec.resumed_at,
                rec.tx_epoch,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_get_latest_active_body_checkpoint(&self, body_id: &str) -> Result<Option<BodyCheckpointRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT checkpoint_id, body_id, from_brain, session_id, goal_summary,
                    active_files_json, active_locks_json, verified_facts_json, open_blockers_json,
                    next_step_hint, token_budget, status, evidence_sha256, created_at,
                    resumed_by_brain, resumed_at, tx_epoch
             FROM body_checkpoints
             WHERE status = 'active' AND (?1 = '' OR body_id = ?1)
             ORDER BY created_at DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![body_id], Self::map_body_checkpoint_row)?;
        if let Some(r) = rows.next() {
            Ok(Some(r?))
        } else {
            Ok(None)
        }
    }


    pub(super) fn impl_list_body_checkpoints(&self, status_filter: Option<&str>, limit: usize) -> Result<Vec<BodyCheckpointRecord>> {
        let conn = self.lock_conn()?;
        let st = status_filter.unwrap_or("");
        let lim = if limit == 0 { 20 } else { limit.min(1000) } as i64;
        let mut stmt = conn.prepare(
            "SELECT checkpoint_id, body_id, from_brain, session_id, goal_summary,
                    active_files_json, active_locks_json, verified_facts_json, open_blockers_json,
                    next_step_hint, token_budget, status, evidence_sha256, created_at,
                    resumed_by_brain, resumed_at, tx_epoch
             FROM body_checkpoints
             WHERE (?1 = '' OR status = ?1)
             ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![st, lim], Self::map_body_checkpoint_row)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }


    pub(super) fn impl_mark_body_checkpoint_resumed(&self, checkpoint_id: &str, by_brain: &str, resumed_at: &str) -> Result<bool> {
        let conn = self.lock_conn()?;
        let affected = conn.execute(
            "UPDATE body_checkpoints
             SET status = 'resumed', resumed_by_brain = ?2, resumed_at = ?3
             WHERE checkpoint_id = ?1",
            params![checkpoint_id, by_brain, resumed_at],
        )?;
        Ok(affected > 0)
    }

}
