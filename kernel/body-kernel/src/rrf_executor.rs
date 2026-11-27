//! 多通道索引 RRF 融合执行器 (`RrfMergeExecutor`)：动态权重重归一化 + RLS + 脱敏二次拦截

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use agent_body_sdk::{
    CallerIdentity, EvidenceTuple, FieldPathContext, IndexAmRoutine, IndexHit, IndexScanDesc,
    PlanRewriteVerdict, RedactDirection, RedactVerdict, RowFilterPredicate, SearchPlan,
    TableAmRoutine,
};
use anyhow::bail;
use serde::{Deserialize, Serialize};

use crate::hook_chain::HookChain;
use crate::wait_events::{KernelWaitEventId, KernelWaitRegistry};

/// 标准 RRF 平滑常数 $k = 60$
pub const DEFAULT_RRF_K: f64 = 60.0;

/// 融合检索输入查询描述
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchQuery {
    pub query_text: String,
    pub strategy_num: u16,
    pub scalar_filters: BTreeMap<String, String>,
    pub rls_filter: Option<RowFilterPredicate>,
    pub limit: usize,
    pub allowed_channels: Option<Vec<String>>,
    pub disabled_channels: Vec<String>,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            query_text: String::new(),
            strategy_num: 1,
            scalar_filters: BTreeMap::new(),
            rls_filter: None,
            limit: 20,
            allowed_channels: None,
            disabled_channels: Vec::new(),
        }
    }
}

impl SearchQuery {
    pub fn to_scan_desc(&self) -> IndexScanDesc {
        IndexScanDesc {
            query_text: self.query_text.clone(),
            strategy_num: self.strategy_num,
            scalar_filters: self.scalar_filters.clone(),
            rls_filter: self.rls_filter.clone(),
            limit: self.limit,
        }
    }
}

/// RRF 融合后单条命中记录
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FusedHit {
    pub entity_id: String,
    pub entity_type: String,
    pub rrf_score: f64,
    pub matched_channels: Vec<String>,
    pub channel_scores: BTreeMap<String, f64>,
    pub reasoning_paths: Vec<String>,
    pub tuple: Option<EvidenceTuple>,
}

/// RRF 多通道融合执行结果
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RrfSearchResult {
    pub hits: Vec<FusedHit>,
    pub normalized_weights: BTreeMap<String, f64>,
    pub dropped_channels: Vec<String>,
    pub blocked_count: usize,
}

/// RRF 多通道融合执行器
#[derive(Clone)]
pub struct RrfMergeExecutor {
    channels: Vec<Arc<dyn IndexAmRoutine>>,
    rrf_k: f64,
}

impl Default for RrfMergeExecutor {
    fn default() -> Self {
        Self::new(DEFAULT_RRF_K)
    }
}

impl RrfMergeExecutor {
    pub fn new(rrf_k: f64) -> Self {
        Self {
            channels: Vec::new(),
            rrf_k: if rrf_k > 0.0 { rrf_k } else { DEFAULT_RRF_K },
        }
    }

    pub fn register_channel(&mut self, am: Arc<dyn IndexAmRoutine>) {
        self.channels.push(am);
    }

    pub fn channel_ids(&self) -> Vec<&'static str> {
        self.channels.iter().map(|c| c.am_id()).collect()
    }

    /// 执行全流程 RRF 融合：PlannerRewriteHook → `amcostestimate` 摘除与重归一化 → RRF ($k=60$) → RLS 与 `TupleRedactHook` 二次过滤
    pub fn search(
        &self,
        query: &SearchQuery,
        caller: &CallerIdentity,
        hooks: &HookChain,
        table_am: &dyn TableAmRoutine,
        waits: &KernelWaitRegistry,
    ) -> anyhow::Result<RrfSearchResult> {
        let mut plan = SearchPlan {
            query_text: query.query_text.clone(),
            channels: query.allowed_channels.clone().unwrap_or_default(),
            disabled_channels: query.disabled_channels.clone(),
            rls_filter: query.rls_filter.clone(),
            timeout_ms: 1500,
            limit: if query.limit == 0 { 20 } else { query.limit },
        };
        if let PlanRewriteVerdict::Reject { reason } = hooks.run_planner_rewrite(&mut plan, caller)
        {
            bail!("检索计划被 PlannerRewriteHook 拒绝: {reason}");
        }

        let scan = IndexScanDesc {
            query_text: plan.query_text,
            strategy_num: query.strategy_num,
            scalar_filters: query.scalar_filters.clone(),
            rls_filter: plan.rls_filter.clone(),
            limit: plan.limit,
        };
        let allowed_set: Option<BTreeSet<String>> = if plan.channels.is_empty() {
            None
        } else {
            Some(plan.channels.into_iter().collect())
        };
        let disabled_set: BTreeSet<String> = plan.disabled_channels.into_iter().collect();

        self.execute_scan(&scan, allowed_set.as_ref(), &disabled_set, caller, hooks, table_am, waits)
    }

    /// 直接基于 `IndexScanDesc` 执行多通道检索融合
    pub fn search_scan(
        &self,
        scan: &IndexScanDesc,
        caller: &CallerIdentity,
        hooks: &HookChain,
        table_am: &dyn TableAmRoutine,
        waits: &KernelWaitRegistry,
    ) -> anyhow::Result<RrfSearchResult> {
        self.execute_scan(
            scan,
            None,
            &BTreeSet::new(),
            caller,
            hooks,
            table_am,
            waits,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_scan(
        &self,
        scan: &IndexScanDesc,
        allowed: Option<&BTreeSet<String>>,
        disabled: &BTreeSet<String>,
        caller: &CallerIdentity,
        hooks: &HookChain,
        table_am: &dyn TableAmRoutine,
        waits: &KernelWaitRegistry,
    ) -> anyhow::Result<RrfSearchResult> {
        let mut active: Vec<(&Arc<dyn IndexAmRoutine>, f64)> = Vec::new();
        let mut dropped_channels = Vec::new();

        for ch in &self.channels {
            let id = ch.am_id().to_string();
            if disabled.contains(&id) || allowed.is_some_and(|set| !set.contains(&id)) {
                dropped_channels.push(id);
                continue;
            }
            let est = ch.amcostestimate(scan);
            let raw_weight = ch.default_weight();
            if !est.available || raw_weight <= 0.0 || !raw_weight.is_finite() {
                dropped_channels.push(id);
                continue;
            }
            active.push((ch, raw_weight));
        }

        let total_weight: f64 = active.iter().map(|(_, w)| *w).sum();
        if active.is_empty() || total_weight <= 0.0 {
            return Ok(RrfSearchResult {
                hits: Vec::new(),
                normalized_weights: BTreeMap::new(),
                dropped_channels,
                blocked_count: 0,
            });
        }

        let mut normalized_weights = BTreeMap::new();
        let mut accum: BTreeMap<(String, String), AccumHit> = BTreeMap::new();

        for (ch, raw_w) in active {
            let norm_w = raw_w / total_weight;
            let ch_id = ch.am_id().to_string();
            normalized_weights.insert(ch_id.clone(), norm_w);

            let _guard = waits.enter(KernelWaitEventId::IndexScan);
            let hits = ch.amgetmulti(scan, table_am)?;
            for (idx, hit) in hits.into_iter().enumerate() {
                let rrf_delta = norm_w / (self.rrf_k + (idx as f64 + 1.0));
                let key = (hit.entity_type.clone(), hit.entity_id.clone());
                let entry = accum.entry(key).or_insert_with(|| AccumHit::from_hit(&hit));
                entry.rrf_score += rrf_delta;
                if !entry.matched_channels.contains(&ch_id) {
                    entry.matched_channels.push(ch_id.clone());
                }
                entry.channel_scores.insert(ch_id.clone(), hit.score);
                if let Some(rp) = hit.reasoning_path {
                    if !entry.reasoning_paths.contains(&rp) {
                        entry.reasoning_paths.push(rp);
                    }
                }
                if entry.tuple.is_none() {
                    entry.tuple = hit.tuple;
                }
            }
        }

        let mut fused = Vec::new();
        let mut blocked_count = 0usize;
        for ((entity_type, entity_id), mut entry) in accum {
            if entry.tuple.is_none() {
                entry.tuple =
                    table_am.entity_get(&entity_type, &entity_id, scan.rls_filter.as_ref())?;
            }
            if let Some(ref pred) = scan.rls_filter {
                match entry.tuple {
                    Some(ref t) if pred.matches_tuple(t) => {}
                    _ => continue,
                }
            }
            if let Some(ref mut tuple) = entry.tuple {
                let path_ctx = FieldPathContext {
                    entity_type: tuple.entity_type.clone(),
                    field_path: "payload".to_string(),
                    direction: RedactDirection::Outbound,
                };
                match hooks.run_tuple_redact(&mut tuple.payload, &path_ctx, caller) {
                    RedactVerdict::Unchanged => {}
                    RedactVerdict::Redacted => tuple.redacted = true,
                    RedactVerdict::Blocked { .. } => {
                        blocked_count += 1;
                        continue;
                    }
                }
            }
            fused.push(FusedHit {
                entity_id,
                entity_type,
                rrf_score: entry.rrf_score,
                matched_channels: entry.matched_channels,
                channel_scores: entry.channel_scores,
                reasoning_paths: entry.reasoning_paths,
                tuple: entry.tuple,
            });
        }

        fused.sort_by(|a, b| {
            b.rrf_score
                .total_cmp(&a.rrf_score)
                .then_with(|| a.entity_id.cmp(&b.entity_id))
        });
        let limit = if scan.limit == 0 { 20 } else { scan.limit };
        fused.truncate(limit);

        Ok(RrfSearchResult {
            hits: fused,
            normalized_weights,
            dropped_channels,
            blocked_count,
        })
    }
}

struct AccumHit {
    rrf_score: f64,
    matched_channels: Vec<String>,
    channel_scores: BTreeMap<String, f64>,
    reasoning_paths: Vec<String>,
    tuple: Option<EvidenceTuple>,
}

impl AccumHit {
    fn from_hit(_hit: &IndexHit) -> Self {
        Self {
            rrf_score: 0.0,
            matched_channels: Vec::new(),
            channel_scores: BTreeMap::new(),
            reasoning_paths: Vec::new(),
            tuple: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem_am::MemoryTableAm;
    use agent_body_sdk::{HookRegistration, IndexCostEstimate, TupleRedactHook};

    struct StubIndex {
        id: &'static str,
        weight: f64,
        available: bool,
        hits: Vec<IndexHit>,
    }

    impl IndexAmRoutine for StubIndex {
        fn am_id(&self) -> &'static str {
            self.id
        }
        fn default_weight(&self) -> f64 {
            self.weight
        }
        fn amcostestimate(&self, _scan: &IndexScanDesc) -> IndexCostEstimate {
            IndexCostEstimate {
                available: self.available,
                est_latency_us: 100,
                est_selectivity: 0.2,
            }
        }
        fn amgetmulti(
            &self,
            _scan: &IndexScanDesc,
            _am: &dyn TableAmRoutine,
        ) -> anyhow::Result<Vec<IndexHit>> {
            Ok(self.hits.clone())
        }
        fn aminsert(&self, _entity: &EvidenceTuple) -> anyhow::Result<()> {
            Ok(())
        }
        fn amdelete_by_source(&self, _source_key: &str) -> anyhow::Result<usize> {
            Ok(0)
        }
        fn amvacuum(&self) -> anyhow::Result<usize> {
            Ok(0)
        }
    }

    struct SecretMaskHook;
    impl TupleRedactHook for SecretMaskHook {
        fn hook_id(&self) -> &'static str {
            "test:mask"
        }
        fn redact(
            &self,
            value: &mut serde_json::Value,
            _path_ctx: &FieldPathContext,
            _caller: &CallerIdentity,
        ) -> anyhow::Result<RedactVerdict> {
            if let Some(obj) = value.as_object_mut() {
                if obj.contains_key("token") {
                    obj.insert("token".to_string(), serde_json::json!("***"));
                    return Ok(RedactVerdict::Redacted);
                }
            }
            Ok(RedactVerdict::Unchanged)
        }
    }

    fn make_tuple(id: &str, tenant: &str, with_token: bool) -> EvidenceTuple {
        let mut labels = BTreeMap::new();
        labels.insert("tenant_id".to_string(), tenant.to_string());
        EvidenceTuple {
            evidence_id: format!("ev-{id}"),
            host_id: "h1".to_string(),
            host_labels: labels,
            entity_type: "entity:item".to_string(),
            entity_id: id.to_string(),
            source_ext: "stub".to_string(),
            evidence_tier: "L1_EXPLICIT".to_string(),
            collected_at: "2026-11-27T16:00:00+08:00".to_string(),
            ttl_sec: 0,
            payload: if with_token {
                serde_json::json!({"token": "secret123"})
            } else {
                serde_json::json!({"name": id})
            },
            redacted: false,
            evidence_sha256: String::new(),
        }
        .with_computed_sha256()
    }

    #[test]
    fn rrf_renormalizes_weights_and_enforces_rls_and_redaction() {
        let mem = MemoryTableAm::new();
        mem.init_catalog_and_base_tables().unwrap();
        let t1 = make_tuple("a", "t1", true);
        let t2 = make_tuple("b", "t2", false);
        mem.entity_upsert(&t1).unwrap();
        mem.entity_upsert(&t2).unwrap();

        let mut hooks = HookChain::new();
        hooks.register(HookRegistration::TupleRedact(Arc::new(SecretMaskHook)));

        let mut rrf = RrfMergeExecutor::default();
        rrf.register_channel(Arc::new(StubIndex {
            id: "ch_offline",
            weight: 0.5,
            available: false,
            hits: vec![],
        }));
        rrf.register_channel(Arc::new(StubIndex {
            id: "ch_lexical",
            weight: 0.3,
            available: true,
            hits: vec![
                IndexHit {
                    entity_id: "a".to_string(),
                    entity_type: "entity:item".to_string(),
                    score: 1.0,
                    channel: "ch_lexical".to_string(),
                    reasoning_path: Some("bm25".to_string()),
                    tuple: Some(t1.clone()),
                },
                IndexHit {
                    entity_id: "b".to_string(),
                    entity_type: "entity:item".to_string(),
                    score: 0.8,
                    channel: "ch_lexical".to_string(),
                    reasoning_path: None,
                    tuple: Some(t2.clone()),
                },
            ],
        }));
        rrf.register_channel(Arc::new(StubIndex {
            id: "ch_graph",
            weight: 0.2,
            available: true,
            hits: vec![IndexHit {
                entity_id: "a".to_string(),
                entity_type: "entity:item".to_string(),
                score: 0.9,
                channel: "ch_graph".to_string(),
                reasoning_path: Some("hop1".to_string()),
                tuple: None,
            }],
        }));

        let query = SearchQuery {
            query_text: "test".to_string(),
            rls_filter: Some(RowFilterPredicate {
                tenant_id: Some("t1".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let res = rrf
            .search(
                &query,
                &CallerIdentity::default(),
                &hooks,
                &mem,
                KernelWaitRegistry::global(),
            )
            .unwrap();

        assert_eq!(res.dropped_channels, vec!["ch_offline"]);
        assert!((res.normalized_weights["ch_lexical"] - 0.6).abs() < 1e-9);
        assert!((res.normalized_weights["ch_graph"] - 0.4).abs() < 1e-9);
        assert_eq!(res.hits.len(), 1);
        assert_eq!(res.hits[0].entity_id, "a");
        let tuple = res.hits[0].tuple.as_ref().unwrap();
        assert!(tuple.redacted);
        assert_eq!(tuple.payload["token"], "***");
    }
}
