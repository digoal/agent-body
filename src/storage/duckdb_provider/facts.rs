use super::DuckDbStorageProvider;
use crate::storage::spi::StorageProvider;
use anyhow::{Result};
use duckdb::params;
use crate::storage::models::*;

impl DuckDbStorageProvider {
    pub(super) fn impl_add_link(&self, link: &LinkRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let now = chrono::Utc::now().to_rfc3339();
        let prov = link.provenance.as_deref().unwrap_or("static_scan");
        conn.execute_batch("BEGIN TRANSACTION;")?;
        let res: Result<()> = (|| {
            let updated = conn.execute(
                "UPDATE links
                 SET confidence = ?1, auto_discovered = ?2, line_number = ?3, snippet = ?4, provenance = ?5
                 WHERE source_type = ?6 AND source_id = ?7 AND target_type = ?8 AND target_id = ?9 AND relation = ?10",
                params![
                    link.confidence as f64,
                    link.auto_discovered,
                    link.line_number,
                    link.snippet,
                    prov,
                    link.source_type,
                    link.source_id,
                    link.target_type,
                    link.target_id,
                    link.relation,
                ],
            )?;
            if updated == 0 {
                conn.execute(
                    "INSERT INTO links (source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        link.source_type,
                        link.source_id,
                        link.target_type,
                        link.target_id,
                        link.relation,
                        link.confidence as f64,
                        link.auto_discovered,
                        link.line_number,
                        link.snippet,
                        prov,
                        link.created_at.as_deref().unwrap_or(&now),
                    ],
                )?;
            }
            Ok(())
        })();
        match res {
            Ok(()) => {
                conn.execute_batch("COMMIT;")?;
                Ok(())
            }
            Err(e) => {
                if let Err(rb) = conn.execute_batch("ROLLBACK;") {
                    eprintln!("[DuckDB][WARN] add_link ROLLBACK 失败: {}", rb);
                }
                Err(e)
            }
        }
    }


    pub(super) fn impl_delete_link(&self, id: i64) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute("DELETE FROM links WHERE id = ?1", params![id])?;
        Ok(())
    }


    pub(super) fn impl_delete_links_for_target(&self, target_id: &str) -> Result<usize> {
        let conn = self.lock_conn()?;
        let count = conn.execute("DELETE FROM links WHERE target_id = ?1", params![target_id])?;
        Ok(count)
    }


    pub(super) fn impl_delete_auto_links_for_source(&self, source_id: &str) -> Result<usize> {
        let conn = self.lock_conn()?;
        let count = conn.execute(
            "DELETE FROM links WHERE source_id = ?1 AND auto_discovered = TRUE",
            params![source_id],
        )?;
        Ok(count)
    }


    pub(super) fn impl_update_link_target(&self, source_id: &str, old_target: &str, new_target: &str) -> Result<usize> {
        let conn = self.lock_conn()?;
        let count = conn.execute(
            "UPDATE links SET target_id = ?1 WHERE source_id = ?2 AND target_id = ?3",
            params![new_target, source_id, old_target],
        )?;
        Ok(count)
    }


    pub(super) fn impl_query_links_for_node(&self, node_type: &str, node_id: &str) -> Result<Vec<LinkRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance, created_at
             FROM links
             WHERE (source_type = ?1 AND source_id = ?2) OR (target_type = ?1 AND target_id = ?2)"
        )?;

        let rows = stmt.query_map(params![node_type, node_id], |row| {
            let conf_f: f64 = row.get(6)?;
            Ok(LinkRecord {
                id: row.get(0)?,
                source_type: row.get(1)?,
                source_id: row.get(2)?,
                target_type: row.get(3)?,
                target_id: row.get(4)?,
                relation: row.get(5)?,
                confidence: conf_f as f32,
                auto_discovered: row.get(7)?,
                line_number: row.get(8)?,
                snippet: row.get(9)?,
                provenance: row.get(10)?,
                created_at: row.get(11)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }


    pub(super) fn impl_list_all_links(&self, limit: usize) -> Result<Vec<LinkRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance, created_at
             FROM links
             ORDER BY id DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let conf_f: f64 = row.get(6)?;
            Ok(LinkRecord {
                id: row.get(0)?,
                source_type: row.get(1)?,
                source_id: row.get(2)?,
                target_type: row.get(3)?,
                target_id: row.get(4)?,
                relation: row.get(5)?,
                confidence: conf_f as f32,
                auto_discovered: row.get(7)?,
                line_number: row.get(8)?,
                snippet: row.get(9)?,
                provenance: row.get(10)?,
                created_at: row.get(11)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }


    pub(super) fn impl_upsert_fact_node(&self, node: &FactNodeRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let props_str = serde_json::to_string(&node.properties)?;

        conn.execute(
            "INSERT INTO fact_nodes (id, node_type, name, label, properties, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, strftime(now(), '%Y-%m-%dT%H:%M:%SZ'))
             ON CONFLICT(id) DO UPDATE SET
                node_type = excluded.node_type,
                name = excluded.name,
                label = excluded.label,
                properties = excluded.properties,
                updated_at = strftime(now(), '%Y-%m-%dT%H:%M:%SZ')",
            params![
                node.id,
                node.node_type,
                node.name,
                node.label,
                props_str,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_upsert_fact_edge(&self, edge: &FactEdgeRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let props_str = serde_json::to_string(&edge.properties)?;

        conn.execute(
            "DELETE FROM fact_edges WHERE source_id = ?1 AND target_id = ?2 AND relation = ?3",
            params![edge.source_id, edge.target_id, edge.relation],
        )?;
        conn.execute(
            "INSERT INTO fact_edges (source_id, target_id, relation, properties, confidence, evidence_tier, weight, direction_kind, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, strftime(now(), '%Y-%m-%dT%H:%M:%SZ'))",
            params![
                edge.source_id,
                edge.target_id,
                edge.relation,
                props_str,
                edge.confidence as f64,
                edge.evidence_tier,
                edge.weight as f64,
                edge.direction_kind,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_get_fact_node(&self, id: &str) -> Result<Option<FactNodeRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, node_type, name, label, properties, updated_at
             FROM fact_nodes WHERE id = ?1"
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let props_str: String = row.get(4)?;
            let properties: serde_json::Value = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));

            Ok(Some(FactNodeRecord {
                id: row.get(0)?,
                node_type: row.get(1)?,
                name: row.get(2)?,
                label: row.get(3)?,
                properties,
                updated_at: row.get(5)?,
            }))
        } else {
            Ok(None)
        }
    }


    pub(super) fn impl_find_fact_nodes_by_name(&self, name: &str) -> Result<Vec<FactNodeRecord>> {
        let conn = self.lock_conn()?;
        let name_pattern = format!("%{}%", name);
        let mut stmt = conn.prepare(
            "SELECT id, node_type, name, label, properties, updated_at
             FROM fact_nodes WHERE name ILIKE ?1 OR label ILIKE ?1"
        )?;

        let rows = stmt.query_map(params![name_pattern], |row| {
            let props_str: String = row.get(4)?;
            let properties: serde_json::Value = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));

            Ok(FactNodeRecord {
                id: row.get(0)?,
                node_type: row.get(1)?,
                name: row.get(2)?,
                label: row.get(3)?,
                properties,
                updated_at: row.get(5)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }


    pub(super) fn impl_traverse_fact_graph(&self, root_id: &str, max_hops: usize) -> Result<SubGraph> {
        let root = match self.get_fact_node(root_id)? {
            Some(r) => r,
            None => return Ok(SubGraph::default()),
        };

        let conn = self.lock_conn()?;
        let mut visited_node_ids = std::collections::HashSet::new();
        visited_node_ids.insert(root_id.to_string());

        let mut current_frontier = vec![root_id.to_string()];
        let mut collected_edges = Vec::new();

        for _ in 0..max_hops {
            if current_frontier.is_empty() {
                break;
            }
            let mut next_frontier = Vec::new();

            for node_id in &current_frontier {
                let mut stmt = conn.prepare(
                    "SELECT id, source_id, target_id, relation, properties, confidence, evidence_tier, weight, direction_kind, updated_at
                     FROM fact_edges
                     WHERE source_id = ?1 OR target_id = ?1"
                )?;

                let edges = stmt.query_map(params![node_id], |row| {
                    let props_str: String = row.get(4)?;
                    let props = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));
                    let conf_f: f64 = row.get(5)?;
                    let tier: Option<String> = row.get(6)?;
                    let weight_f: Option<f64> = row.get(7)?;
                    let dir: Option<String> = row.get(8)?;
                    Ok(FactEdgeRecord {
                        id: row.get(0)?,
                        source_id: row.get(1)?,
                        target_id: row.get(2)?,
                        relation: row.get(3)?,
                        properties: props,
                        confidence: conf_f as f32,
                        evidence_tier: tier.unwrap_or_else(|| "L1_EXPLICIT".to_string()),
                        weight: weight_f.unwrap_or(conf_f) as f32,
                        direction_kind: dir.unwrap_or_else(|| "forward_dependency".to_string()),
                        updated_at: row.get(9)?,
                    })
                })?.collect::<std::result::Result<Vec<_>, _>>()?;

                for e in edges {
                    let other_id = if &e.source_id == node_id {
                        &e.target_id
                    } else {
                        &e.source_id
                    };

                    if !visited_node_ids.contains(other_id) {
                        visited_node_ids.insert(other_id.clone());
                        next_frontier.push(other_id.clone());
                    }
                    if !collected_edges.iter().any(|existing: &FactEdgeRecord| {
                        existing.source_id == e.source_id && existing.target_id == e.target_id && existing.relation == e.relation
                    }) {
                        collected_edges.push(e);
                    }
                }
            }
            current_frontier = next_frontier;
        }

        let mut collected_nodes = Vec::new();
        for nid in &visited_node_ids {
            let mut stmt = conn.prepare(
                "SELECT id, node_type, name, label, properties, updated_at FROM fact_nodes WHERE id = ?1"
            )?;
            let mut rows = stmt.query(params![nid])?;
            if let Some(row) = rows.next()? {
                let props_str: String = row.get(4)?;
                let properties: serde_json::Value = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));
                collected_nodes.push(FactNodeRecord {
                    id: row.get(0)?,
                    node_type: row.get(1)?,
                    name: row.get(2)?,
                    label: row.get(3)?,
                    properties,
                    updated_at: row.get(5)?,
                });
            }
        }

        Ok(SubGraph {
            root_node: Some(root),
            nodes: collected_nodes,
            edges: collected_edges,
        })
    }


    pub(super) fn impl_list_all_fact_nodes(&self, limit: usize) -> Result<Vec<FactNodeRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, node_type, name, label, properties, updated_at
             FROM fact_nodes
             ORDER BY updated_at DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let props_str: String = row.get(4)?;
            let properties: serde_json::Value = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));

            Ok(FactNodeRecord {
                id: row.get(0)?,
                node_type: row.get(1)?,
                name: row.get(2)?,
                label: row.get(3)?,
                properties,
                updated_at: row.get(5)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }


    pub(super) fn impl_list_all_fact_edges(&self, limit: usize) -> Result<Vec<FactEdgeRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, source_id, target_id, relation, properties, confidence, evidence_tier, weight, direction_kind, updated_at
             FROM fact_edges
             ORDER BY updated_at DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let props_str: String = row.get(4)?;
            let properties: serde_json::Value = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));
            let conf_f: f64 = row.get(5)?;
            let tier: Option<String> = row.get(6)?;
            let weight_f: Option<f64> = row.get(7)?;
            let dir: Option<String> = row.get(8)?;

            Ok(FactEdgeRecord {
                id: row.get(0)?,
                source_id: row.get(1)?,
                target_id: row.get(2)?,
                relation: row.get(3)?,
                properties,
                confidence: conf_f as f32,
                evidence_tier: tier.unwrap_or_else(|| "L1_EXPLICIT".to_string()),
                weight: weight_f.unwrap_or(conf_f) as f32,
                direction_kind: dir.unwrap_or_else(|| "forward_dependency".to_string()),
                updated_at: row.get(9)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }


    pub(super) fn impl_delete_fact_node(&self, id: &str) -> Result<usize> {
        let conn = self.lock_conn()?;
        conn.execute(
            "DELETE FROM fact_edges WHERE source_id = ?1 OR target_id = ?1",
            params![id],
        )?;
        let affected = conn.execute("DELETE FROM fact_nodes WHERE id = ?1", params![id])?;
        Ok(affected)
    }


    pub(super) fn impl_delete_fact_edges_by_tier(&self, tier: &str) -> Result<usize> {
        let conn = self.lock_conn()?;
        let affected = conn.execute(
            "DELETE FROM fact_edges WHERE evidence_tier = ?1",
            params![tier],
        )?;
        Ok(affected)
    }


    pub(super) fn impl_update_fact_node_embedding(&self, id: &str, embedding: &[f32]) -> Result<()> {
        let conn = self.lock_conn()?;
        let emb_str = serde_json::to_string(embedding)?;
        conn.execute(
            "UPDATE fact_nodes SET embedding = ?1, updated_at = strftime(now(), '%Y-%m-%dT%H:%M:%SZ') WHERE id = ?2",
            params![emb_str, id],
        )?;
        Ok(())
    }


    pub(super) fn impl_get_fact_node_embedding(&self, id: &str) -> Result<Vec<f32>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare("SELECT COALESCE(embedding, '[]') FROM fact_nodes WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let s: String = row.get(0)?;
            Ok(serde_json::from_str(&s).unwrap_or_default())
        } else {
            Ok(Vec::new())
        }
    }

    // ─── Multi-Agent Coordination Locks (多 Agent 协作锁) ────────────

}
