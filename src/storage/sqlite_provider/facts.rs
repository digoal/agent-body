use super::SqliteStorageProvider;
use crate::storage::spi::StorageProvider;
use anyhow::{Result};
use rusqlite::params;
use crate::storage::models::*;

impl SqliteStorageProvider {
    pub(super) fn impl_add_link(&self, link: &LinkRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO links (source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(source_type, source_id, target_type, target_id, relation) DO UPDATE SET
                confidence = excluded.confidence,
                line_number = excluded.line_number,
                snippet = excluded.snippet,
                provenance = excluded.provenance",
            params![
                link.source_type,
                link.source_id,
                link.target_type,
                link.target_id,
                link.relation,
                link.confidence,
                link.auto_discovered as i32,
                link.line_number,
                link.snippet,
                link.provenance.as_deref().unwrap_or("static_scan"),
            ],
        )?;
        Ok(())
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
            "DELETE FROM links WHERE source_id = ?1 AND auto_discovered = 1",
            params![source_id],
        )?;
        Ok(count)
    }


    pub(super) fn impl_update_link_target(&self, source_id: &str, old_target: &str, new_target: &str) -> Result<usize> {
        let conn = self.lock_conn()?;
        let count = conn.execute(
            "UPDATE links SET target_id = ?3 WHERE source_id = ?1 AND target_id = ?2",
            params![source_id, old_target, new_target],
        )?;
        Ok(count)
    }


    pub(super) fn impl_query_links_for_node(&self, node_type: &str, node_id: &str) -> Result<Vec<LinkRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance, created_at
             FROM links
             WHERE (source_type = ?1 AND source_id = ?2) OR (target_type = ?1 AND target_id = ?2)
             ORDER BY created_at DESC"
        )?;

        let rows = stmt.query_map(params![node_type, node_id], |row| {
            let auto_disc: i32 = row.get(7)?;
            Ok(LinkRecord {
                id: Some(row.get(0)?),
                source_type: row.get(1)?,
                source_id: row.get(2)?,
                target_type: row.get(3)?,
                target_id: row.get(4)?,
                relation: row.get(5)?,
                confidence: row.get(6)?,
                auto_discovered: auto_disc != 0,
                line_number: row.get(8)?,
                snippet: row.get(9)?,
                provenance: row.get(10)?,
                created_at: Some(row.get(11)?),
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }


    pub(super) fn impl_list_all_links(&self, limit: usize) -> Result<Vec<LinkRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance, created_at
             FROM links
             ORDER BY created_at DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let auto_disc: i32 = row.get(7)?;
            Ok(LinkRecord {
                id: Some(row.get(0)?),
                source_type: row.get(1)?,
                source_id: row.get(2)?,
                target_type: row.get(3)?,
                target_id: row.get(4)?,
                relation: row.get(5)?,
                confidence: row.get(6)?,
                auto_discovered: auto_disc != 0,
                line_number: row.get(8)?,
                snippet: row.get(9)?,
                provenance: row.get(10)?,
                created_at: Some(row.get(11)?),
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }


    pub(super) fn impl_upsert_fact_node(&self, node: &FactNodeRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let props_str = serde_json::to_string(&node.properties)?;
        conn.execute(
            "INSERT INTO fact_nodes (id, node_type, name, label, properties, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
             ON CONFLICT(id) DO UPDATE SET
                node_type = excluded.node_type,
                name = excluded.name,
                label = excluded.label,
                properties = excluded.properties,
                updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00','now')",
            params![node.id, node.node_type, node.name, node.label, props_str],
        )?;
        Ok(())
    }


    pub(super) fn impl_upsert_fact_edge(&self, edge: &FactEdgeRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let props_str = serde_json::to_string(&edge.properties)?;
        conn.execute(
            "INSERT INTO fact_edges (source_id, target_id, relation, properties, confidence, evidence_tier, weight, direction_kind, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
             ON CONFLICT(source_id, target_id, relation) DO UPDATE SET
                properties = excluded.properties,
                confidence = excluded.confidence,
                evidence_tier = excluded.evidence_tier,
                weight = excluded.weight,
                direction_kind = excluded.direction_kind,
                updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00','now')",
            params![
                edge.source_id,
                edge.target_id,
                edge.relation,
                props_str,
                edge.confidence,
                edge.evidence_tier,
                edge.weight,
                edge.direction_kind,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_get_fact_node(&self, id: &str) -> Result<Option<FactNodeRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, node_type, name, label, properties, updated_at FROM fact_nodes WHERE id = ?1"
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
        let mut stmt = conn.prepare(
            "SELECT id, node_type, name, label, properties, updated_at FROM fact_nodes 
             WHERE name LIKE ?1 OR label LIKE ?1 LIMIT 50"
        )?;

        let pattern = format!("%{}%", name);
        let nodes = stmt.query_map(params![pattern], |row| {
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
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(nodes)
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
                    Ok(FactEdgeRecord {
                        id: Some(row.get(0)?),
                        source_id: row.get(1)?,
                        target_id: row.get(2)?,
                        relation: row.get(3)?,
                        properties: props,
                        confidence: row.get(5)?,
                        evidence_tier: row.get(6)?,
                        weight: row.get(7)?,
                        direction_kind: row.get(8)?,
                        updated_at: row.get(9)?,
                    })
                })?.collect::<Result<Vec<_>, _>>()?;

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
            "SELECT id, node_type, name, label, properties, updated_at FROM fact_nodes ORDER BY id LIMIT ?1"
        )?;
        let nodes = stmt.query_map(params![limit as i64], |row| {
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
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(nodes)
    }


    pub(super) fn impl_list_all_fact_edges(&self, limit: usize) -> Result<Vec<FactEdgeRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, source_id, target_id, relation, properties, confidence, evidence_tier, weight, direction_kind, updated_at FROM fact_edges ORDER BY id LIMIT ?1"
        )?;
        let edges = stmt.query_map(params![limit as i64], |row| {
            let props_str: String = row.get(4)?;
            let properties: serde_json::Value = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));
            Ok(FactEdgeRecord {
                id: Some(row.get(0)?),
                source_id: row.get(1)?,
                target_id: row.get(2)?,
                relation: row.get(3)?,
                properties,
                confidence: row.get(5)?,
                evidence_tier: row.get(6)?,
                weight: row.get(7)?,
                direction_kind: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(edges)
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
            "UPDATE fact_nodes SET embedding = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00','now') WHERE id = ?2",
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
