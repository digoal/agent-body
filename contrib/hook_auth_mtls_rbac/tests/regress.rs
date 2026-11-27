//! `contrib/hook_auth_mtls_rbac` 集成回归测试

use std::collections::BTreeMap;

use agent_body_kernel::KernelRuntime;
use agent_body_sdk::{
    AccessOp, AccessVerdict, CallerIdentity, ClientAuthContext, ClientCertInfo, EntityScanFilter,
    EvidenceTuple, ObjectAccessContext, PlanRewriteVerdict, SearchPlan, TransportKind,
};
use ext_hook_auth_mtls_rbac::{
    HookAuthMtlsRbacExtension, MtlsRbacConfig, PrincipalPolicy,
};

fn make_tenant_entity(id: &str, tenant: &str, env: &str, text: &str) -> EvidenceTuple {
    let mut labels = BTreeMap::new();
    labels.insert("tenant_id".to_string(), tenant.to_string());
    labels.insert("env".to_string(), env.to_string());
    EvidenceTuple {
        evidence_id: format!("ev-{id}"),
        host_id: format!("host-{tenant}"),
        host_labels: labels,
        entity_type: "entity:linux_process".to_string(),
        entity_id: id.to_string(),
        source_ext: "test".to_string(),
        evidence_tier: "L1_EXPLICIT".to_string(),
        collected_at: "2026-11-27T16:00:00+08:00".to_string(),
        ttl_sec: 60,
        payload: serde_json::json!({"summary": text}),
        redacted: false,
        evidence_sha256: String::new(),
    }
    .with_computed_sha256()
}

#[test]
fn enforces_read_auth_401_and_tenant_rls_isolation() {
    let mut tokens = BTreeMap::new();
    tokens.insert(
        "tok-tenant-a".to_string(),
        PrincipalPolicy {
            caller_id: "duty-a".to_string(),
            tenant_id: Some("dept-finance".to_string()),
            project_id: None,
            env: Some("prod".to_string()),
            roles: vec!["operator".to_string()],
            allowed_entity_types: vec!["entity:linux_process".to_string()],
        },
    );

    let mut certs = BTreeMap::new();
    certs.insert(
        "duty-b.gov.cn".to_string(),
        PrincipalPolicy {
            caller_id: "duty-b".to_string(),
            tenant_id: Some("dept-health".to_string()),
            project_id: None,
            env: Some("prod".to_string()),
            roles: vec!["operator".to_string()],
            allowed_entity_types: vec!["entity:linux_process".to_string()],
        },
    );

    let mut rt = KernelRuntime::open_sqlite_in_memory(None).unwrap();
    rt.install_extensions(&[Box::new(HookAuthMtlsRbacExtension::new(MtlsRbacConfig {
        tokens,
        cert_cn_policies: certs,
    }))])
    .unwrap();

    rt.table_am()
        .entity_upsert(&make_tenant_entity(
            "p-fin",
            "dept-finance",
            "prod",
            "payment gateway port 8080",
        ))
        .unwrap();
    rt.table_am()
        .entity_upsert(&make_tenant_entity(
            "p-hlt",
            "dept-health",
            "prod",
            "payment gateway port 8080",
        ))
        .unwrap();

    // 1. 未认证读请求 -> 401 拒绝 (即便来自 loopback)
    let unauth = ClientAuthContext {
        transport: TransportKind::HttpRest,
        cert: None,
        bearer_token: None,
        method: "GET".to_string(),
        path: "/api/v1/catalog/extensions".to_string(),
        is_loopback: true,
        is_read_only: true,
    };
    let resp = rt.handle_http(&agent_body_kernel::dispatcher::HttpRequest {
        method: "GET".to_string(),
        path: "/api/v1/catalog/extensions".to_string(),
        bearer_token: None,
        is_loopback: true,
        body: serde_json::Value::Null,
    });
    assert_eq!(resp.status_code, 401);
    let _ = unauth;

    // 2. 租户 B (mTLS CN=duty-b.gov.cn) 仅能扫描出 dept-health 数据，绝不泄露 dept-finance
    let auth_b = ClientAuthContext {
        transport: TransportKind::HttpRest,
        cert: Some(ClientCertInfo {
            subject: "O=GovCloud, CN=duty-b.gov.cn".to_string(),
            issuer: "GovCA".to_string(),
            fingerprint_sha256: "abc".to_string(),
        }),
        bearer_token: None,
        method: "GET".to_string(),
        path: "/api/v1/search".to_string(),
        is_loopback: false,
        is_read_only: true,
    };
    let verdict = rt.hooks().run_client_auth(&auth_b);
    let caller_b: CallerIdentity = match verdict {
        agent_body_sdk::AuthVerdict::Allow(id) => id,
        other => panic!("expected Allow, got {other:?}"),
    };
    let access_verdict = rt.hooks().run_object_access(&ObjectAccessContext {
        caller: caller_b.clone(),
        op: AccessOp::Read,
        entity_type: "entity:linux_process".to_string(),
        proc_name: None,
    });
    let rls = match access_verdict {
        AccessVerdict::AllowWithRls(pred) => pred,
        other => panic!("expected AllowWithRls, got {other:?}"),
    };
    let mut plan = SearchPlan {
        query_text: "gateway 8080".to_string(),
        channels: vec![],
        disabled_channels: vec![],
        rls_filter: None,
        timeout_ms: 1000,
        limit: 10,
    };
    assert!(matches!(
        rt.hooks().run_planner_rewrite(&mut plan, &caller_b),
        PlanRewriteVerdict::Modified
    ));
    let scanned = rt
        .table_am()
        .entity_scan(&EntityScanFilter::default(), plan.rls_filter.as_ref().or(Some(&rls)))
        .unwrap();
    assert_eq!(scanned.len(), 1);
    assert_eq!(scanned[0].entity_id, "p-hlt");
}
