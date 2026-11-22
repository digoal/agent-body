use super::PgStorageProvider;
use crate::storage::spi::StorageProvider;
use anyhow::{Result};
use crate::storage::models::*;

impl PgStorageProvider {
    pub(super) fn impl_add_link(&self, link: &LinkRecord) -> Result<()> {
        let mut client = self.lock_client()?;
        client.execute(
            "INSERT INTO links (source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             ON CONFLICT (source_type, source_id, target_type, target_id, relation) DO UPDATE SET
                confidence = EXCLUDED.confidence,
                line_number = EXCLUDED.line_number,
                snippet = EXCLUDED.snippet,
                provenance = EXCLUDED.provenance",
            &[
                &link.source_type,
                &link.source_id,
                &link.target_type,
                &link.target_id,
                &link.relation,
                &link.confidence,
                &link.auto_discovered,
                &link.line_number,
                &link.snippet,
                &link.provenance.as_deref().unwrap_or("static_scan"),
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_delete_link(&self, id: i64) -> Result<()> {
        let mut client = self.lock_client()?;
        client.execute("DELETE FROM links WHERE id = $1", &[&id])?;
        Ok(())
    }


    pub(super) fn impl_delete_links_for_target(&self, target_id: &str) -> Result<usize> {
        let mut client = self.lock_client()?;
        let count = client.execute("DELETE FROM links WHERE target_id = $1", &[&target_id])?;
        Ok(count as usize)
    }


    pub(super) fn impl_delete_auto_links_for_source(&self, source_id: &str) -> Result<usize> {
        let mut client = self.lock_client()?;
        let count = client.execute(
            "DELETE FROM links WHERE source_id = $1 AND auto_discovered = TRUE",
            &[&source_id],
        )?;
        Ok(count as usize)
    }


    pub(super) fn impl_update_link_target(&self, source_id: &str, old_target: &str, new_target: &str) -> Result<usize> {
        let mut client = self.lock_client()?;
        let count = client.execute(
            "UPDATE links SET target_id = $3 WHERE source_id = $1 AND target_id = $2",
            &[&source_id, &old_target, &new_target],
        )?;
        Ok(count as usize)
    }


    pub(super) fn impl_query_links_for_node(&self, node_type: &str, node_id: &str) -> Result<Vec<LinkRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance, created_at
             FROM links
             WHERE (source_type = $1 AND source_id = $2) OR (target_type = $1 AND target_id = $2)
             ORDER BY created_at DESC",
            &[&node_type, &node_id],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let id_i64: i64 = row.get(0);
            let created: Option<chrono::DateTime<chrono::Utc>> = row.get(11);
            results.push(LinkRecord {
                id: Some(id_i64),
                source_type: row.get(1),
                source_id: row.get(2),
                target_type: row.get(3),
                target_id: row.get(4),
                relation: row.get(5),
                confidence: row.get(6),
                auto_discovered: row.get(7),
                line_number: row.get(8),
                snippet: row.get(9),
                provenance: row.get(10),
                created_at: created.map(|c| c.to_rfc3339()),
            });
        }
        Ok(results)
    }


    pub(super) fn impl_list_all_links(&self, limit: usize) -> Result<Vec<LinkRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance, created_at
             FROM links
             ORDER BY created_at DESC
             LIMIT $1",
            &[&(limit as i64)],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let id_i64: i64 = row.get(0);
            let created: Option<chrono::DateTime<chrono::Utc>> = row.get(11);
            results.push(LinkRecord {
                id: Some(id_i64),
                source_type: row.get(1),
                source_id: row.get(2),
                target_type: row.get(3),
                target_id: row.get(4),
                relation: row.get(5),
                confidence: row.get(6),
                auto_discovered: row.get(7),
                line_number: row.get(8),
                snippet: row.get(9),
                provenance: row.get(10),
                created_at: created.map(|c| c.to_rfc3339()),
            });
        }
        Ok(results)
    }


    pub(super) fn impl_upsert_fact_node(&self, node: &FactNodeRecord) -> Result<()> {
        let mut client = self.lock_client()?;
        client.execute(
            "INSERT INTO fact_nodes (id, node_type, name, label, properties, updated_at)
             VALUES ($1, $2, $3, $4, $5, NOW())
             ON CONFLICT (id) DO UPDATE SET
                node_type = EXCLUDED.node_type,
                name = EXCLUDED.name,
                label = EXCLUDED.label,
                properties = EXCLUDED.properties,
                updated_at = NOW()",
            &[&node.id, &node.node_type, &node.name, &node.label, &node.properties],
        )?;
        Ok(())
    }


    pub(super) fn impl_upsert_fact_edge(&self, edge: &FactEdgeRecord) -> Result<()> {
        let mut client = self.lock_client()?;
        client.execute(
            "INSERT INTO fact_edges (source_id, target_id, relation, properties, confidence, evidence_tier, weight, direction_kind, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
             ON CONFLICT (source_id, target_id, relation) DO UPDATE SET
                properties = EXCLUDED.properties,
                confidence = EXCLUDED.confidence,
                evidence_tier = EXCLUDED.evidence_tier,
                weight = EXCLUDED.weight,
                direction_kind = EXCLUDED.direction_kind,
                updated_at = NOW()",
            &[
                &edge.source_id,
                &edge.target_id,
                &edge.relation,
                &edge.properties,
                &edge.confidence,
                &edge.evidence_tier,
                &edge.weight,
                &edge.direction_kind,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_get_fact_node(&self, id: &str) -> Result<Option<FactNodeRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, node_type, name, label, properties, updated_at FROM fact_nodes WHERE id = $1",
            &[&id],
        )?;

        if let Some(row) = rows.first() {
            let updated: Option<chrono::DateTime<chrono::Utc>> = row.get(5);
            Ok(Some(FactNodeRecord {
                id: row.get(0),
                node_type: row.get(1),
                name: row.get(2),
                label: row.get(3),
                properties: row.get(4),
                updated_at: updated.map(|u| u.to_rfc3339()),
            }))
        } else {
            Ok(None)
        }
    }


    pub(super) fn impl_find_fact_nodes_by_name(&self, name: &str) -> Result<Vec<FactNodeRecord>> {
        let mut client = self.lock_client()?;
        let pattern = format!("%{}%", name);
        let rows = client.query(
            "SELECT id, node_type, name, label, properties, updated_at FROM fact_nodes 
             WHERE name ILIKE $1 OR label ILIKE $1 LIMIT 50",
            &[&pattern],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let updated: Option<chrono::DateTime<chrono::Utc>> = row.get(5);
            results.push(FactNodeRecord {
                id: row.get(0),
                node_type: row.get(1),
                name: row.get(2),
                label: row.get(3),
                properties: row.get(4),
                updated_at: updated.map(|u| u.to_rfc3339()),
            });
        }
        Ok(results)
    }


    pub(super) fn impl_traverse_fact_graph(&self, root_id: &str, max_hops: usize) -> Result<SubGraph> {
        let root = match self.get_fact_node(root_id)? {
            Some(r) => r,
            None => return Ok(SubGraph::default()),
        };

        let mut client = self.lock_client()?;
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
                let rows = client.query(
                    "SELECT id, source_id, target_id, relation, properties, confidence, evidence_tier, weight, direction_kind, updated_at
                     FROM fact_edges
                     WHERE source_id = $1 OR target_id = $1",
                    &[&node_id],
                )?;

                for row in rows {
                    let id_i64: i64 = row.get(0);
                    let src: String = row.get(1);
                    let tgt: String = row.get(2);
                    let rel: String = row.get(3);
                    let props: serde_json::Value = row.get(4);
                    let conf: f32 = row.get(5);
                    let tier: String = row.get(6);
                    let weight: f32 = row.get(7);
                    let dir: String = row.get(8);
                    let updated: Option<chrono::DateTime<chrono::Utc>> = row.get(9);

                    let other_id = if &src == node_id {
                        tgt.clone()
                    } else {
                        src.clone()
                    };

                    if !visited_node_ids.contains(&other_id) {
                        visited_node_ids.insert(other_id.clone());
                        next_frontier.push(other_id);
                    }

                    if !collected_edges.iter().any(|existing: &FactEdgeRecord| {
                        existing.source_id == src && existing.target_id == tgt && existing.relation == rel
                    }) {
                        collected_edges.push(FactEdgeRecord {
                            id: Some(id_i64),
                            source_id: src,
                            target_id: tgt,
                            relation: rel,
                            properties: props,
                            confidence: conf,
                            evidence_tier: tier,
                            weight,
                            direction_kind: dir,
                            updated_at: updated.map(|u| u.to_rfc3339()),
                        });
                    }
                }
            }
            current_frontier = next_frontier;
        }

        let mut collected_nodes = Vec::new();
        for nid in &visited_node_ids {
            let rows = client.query(
                "SELECT id, node_type, name, label, properties, updated_at FROM fact_nodes WHERE id = $1",
                &[&nid],
            )?;
            if let Some(row) = rows.first() {
                let updated: Option<chrono::DateTime<chrono::Utc>> = row.get(5);
                collected_nodes.push(FactNodeRecord {
                    id: row.get(0),
                    node_type: row.get(1),
                    name: row.get(2),
                    label: row.get(3),
                    properties: row.get(4),
                    updated_at: updated.map(|u| u.to_rfc3339()),
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
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, node_type, name, label, properties, updated_at FROM fact_nodes ORDER BY id LIMIT $1",
            &[&(limit as i64)],
        )?;
        let mut nodes = Vec::new();
        for row in rows {
            let updated: Option<chrono::DateTime<chrono::Utc>> = row.get(5);
            nodes.push(FactNodeRecord {
                id: row.get(0),
                node_type: row.get(1),
                name: row.get(2),
                label: row.get(3),
                properties: row.get(4),
                updated_at: updated.map(|u| u.to_rfc3339()),
            });
        }
        Ok(nodes)
    }


    pub(super) fn impl_list_all_fact_edges(&self, limit: usize) -> Result<Vec<FactEdgeRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, source_id, target_id, relation, properties, confidence, evidence_tier, weight, direction_kind, updated_at FROM fact_edges ORDER BY id LIMIT $1",
            &[&(limit as i64)],
        )?;
        let mut edges = Vec::new();
        for row in rows {
            let id_i64: i64 = row.get(0);
            let updated: Option<chrono::DateTime<chrono::Utc>> = row.get(9);
            edges.push(FactEdgeRecord {
                id: Some(id_i64),
                source_id: row.get(1),
                target_id: row.get(2),
                relation: row.get(3),
                properties: row.get(4),
                confidence: row.get(5),
                evidence_tier: row.get(6),
                weight: row.get(7),
                direction_kind: row.get(8),
                updated_at: updated.map(|u| u.to_rfc3339()),
            });
        }
        Ok(edges)
    }


    pub(super) fn impl_delete_fact_node(&self, id: &str) -> Result<usize> {
        let mut client = self.lock_client()?;
        client.execute(
            "DELETE FROM fact_edges WHERE source_id = $1 OR target_id = $1",
            &[&id],
        )?;
        let affected = client.execute("DELETE FROM fact_nodes WHERE id = $1", &[&id])?;
        Ok(affected as usize)
    }


    pub(super) fn impl_delete_fact_edges_by_tier(&self, tier: &str) -> Result<usize> {
        let mut client = self.lock_client()?;
        let affected = client.execute(
            "DELETE FROM fact_edges WHERE evidence_tier = $1",
            &[&tier],
        )?;
        Ok(affected as usize)
    }


    pub(super) fn impl_update_fact_node_embedding(&self, id: &str, embedding: &[f32]) -> Result<()> {
        let mut client = self.lock_client()?;
        let emb_str = serde_json::to_string(embedding)?;
        client.execute(
            "UPDATE fact_nodes SET embedding = $1, updated_at = NOW() WHERE id = $2",
            &[&emb_str, &id],
        )?;
        Ok(())
    }


    pub(super) fn impl_get_fact_node_embedding(&self, id: &str) -> Result<Vec<f32>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT COALESCE(embedding, '[]') FROM fact_nodes WHERE id = $1",
            &[&id],
        )?;
        if let Some(row) = rows.first() {
            let s: String = row.get(0);
            Ok(serde_json::from_str(&s).unwrap_or_default())
        } else {
            Ok(Vec::new())
        }
    }

    // ─── Multi-Agent Coordination Locks (多 Agent 协作锁) ────────────

}
