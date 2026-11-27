//! `contrib/hook_gov_compliance` 集成回归测试

use std::collections::BTreeMap;

use agent_body_kernel::KernelRuntime;
use agent_body_sdk::{
    CallerIdentity, FieldPathContext, ProbeContext, RedactDirection, RedactVerdict,
};
use ext_hook_gov_compliance::{GovComplianceConfig, HookGovComplianceExtension};

#[test]
fn redacts_java_d_flags_uri_pem_bearer_cron_pii_and_injects_host_labels() {
    let mut host_labels = BTreeMap::new();
    host_labels.insert("tenant_id".to_string(), "dept-health".to_string());
    host_labels.insert("region".to_string(), "cn-zj-gov-1".to_string());
    host_labels.insert("env".to_string(), "prod".to_string());

    let mut rt = KernelRuntime::open_sqlite_in_memory(None).unwrap();
    rt.install_extensions(&[Box::new(HookGovComplianceExtension::new(
        GovComplianceConfig {
            host_labels,
            custom_patterns: vec![r"GOV_INTERNAL_SECRET_[A-Z0-9]+".to_string()],
            enable_pii_masking: true,
        },
    ))])
    .unwrap();

    let caller = CallerIdentity::default();
    let ctx_path = FieldPathContext {
        entity_type: "entity:jvm_app".to_string(),
        field_path: "payload".to_string(),
        direction: RedactDirection::Inbound,
    };

    let mut payload = serde_json::json!({
        "cmdline": "java -Xms1g -Dspring.datasource.password=TopSecret#2026 -jar /opt/biz/portal.jar --spring.config.location=/etc/biz/app.yml --server.port=8081",
        "dsn": "jdbc:postgresql://appuser:DbPass123!@10.2.3.4:5432/prod_db",
        "header": "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9",
        "key": "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEAtestkey\n-----END RSA PRIVATE KEY-----",
        "cron": "0 2 * * * PGPASSWORD=CronPass! /usr/bin/pg_dump -h localhost",
        "pii": "operator 110105199001011234 phone 13800138000 tag GOV_INTERNAL_SECRET_XYZ9"
    });

    let verdict = rt.hooks().run_tuple_redact(&mut payload, &ctx_path, &caller);
    assert_eq!(verdict, RedactVerdict::Redacted);

    let cmd = payload["cmdline"].as_str().unwrap();
    assert!(cmd.contains("-Dspring.datasource.password=[REDACTED]"));
    assert!(cmd.contains("-jar /opt/biz/portal.jar"));
    assert!(cmd.contains("--spring.config.location=/etc/biz/app.yml"));
    assert!(cmd.contains("--server.port=8081"));
    assert!(!cmd.contains("TopSecret#2026"));

    assert!(!payload["dsn"].as_str().unwrap().contains("DbPass123!"));
    assert!(payload["dsn"].as_str().unwrap().contains("appuser:[REDACTED]@10.2.3.4"));
    assert!(!payload["header"].as_str().unwrap().contains("eyJhbGciOi"));
    assert!(payload["key"].as_str().unwrap().contains("[REDACTED_PEM_KEY]"));
    assert!(!payload["cron"].as_str().unwrap().contains("CronPass!"));

    let pii = payload["pii"].as_str().unwrap();
    assert!(!pii.contains("110105199001011234"));
    assert!(pii.contains("110***********1234"));
    assert!(!pii.contains("13800138000"));
    assert!(pii.contains("138****8000"));
    assert!(!pii.contains("GOV_INTERNAL_SECRET_XYZ9"));

    let mut probe_ctx = ProbeContext::default();
    let rewrite = rt.hooks().run_planner_rewrite_probe(&mut probe_ctx);
    assert_eq!(rewrite, agent_body_sdk::PlanRewriteVerdict::Modified);
    assert_eq!(
        probe_ctx.params.get("host_label:tenant_id").map(String::as_str),
        Some("dept-health")
    );
    assert_eq!(
        probe_ctx.params.get("host_label:region").map(String::as_str),
        Some("cn-zj-gov-1")
    );
}
