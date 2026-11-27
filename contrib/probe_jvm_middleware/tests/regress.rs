//! `contrib/probe_jvm_middleware` 集成回归测试

use std::fs;
use std::path::Path;

use agent_body_kernel::KernelRuntime;
use agent_body_sdk::{
    BodyExtension, ClientAuthContext, ExtensionKind, ExtensionMeta, ExtensionRegistrar,
    ProbeContext, SDK_API_VERSION, TransportKind,
};
use ext_probe_jvm_middleware::parsers::{parse_jvm_cmdline, parse_nginx_hierarchy};
use ext_probe_jvm_middleware::ProbeJvmMiddlewareExtension;

struct StubLinuxOpsExtension;

impl BodyExtension for StubLinuxOpsExtension {
    fn meta(&self) -> ExtensionMeta {
        ExtensionMeta {
            ext_id: "probe_linux_ops".to_string(),
            version: "0.1.0".to_string(),
            kind: ExtensionKind::CompiledCrate,
            min_sdk_version: SDK_API_VERSION.to_string(),
            table_prefix: "ext_probe_linux_ops__".to_string(),
            requires: vec![],
        }
    }

    fn register(&self, _reg: &mut ExtensionRegistrar) -> anyhow::Result<()> {
        Ok(())
    }
}

fn write_file(p: &Path, content: &str) {
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(p, content).unwrap();
}

#[test]
fn parses_dual_jvm_instances_and_nginx_include_upstream_topology() {
    let tmp = tempfile::tempdir().unwrap();
    let r = tmp.path();

    write_file(
        &r.join("proc/3001/cmdline"),
        "java\0-Xms1g\0-Xmx2g\0-Dspring.datasource.password=Secret99\0-jar\0/opt/biz/portal.jar\0--spring.config.location=/etc/biz/v1.yml\0--server.port=8080\0",
    );
    write_file(
        &r.join("proc/3002/cmdline"),
        "/opt/tongweb/bin/java\0-Dtongweb.home=/opt/tongweb\0-jar\0/opt/biz/portal-v2.jar\0--server.port=8081\0",
    );
    write_file(
        &r.join("etc/nginx/nginx.conf"),
        "http {\n  include /etc/nginx/conf.d/*.conf;\n}\n",
    );
    write_file(
        &r.join("etc/nginx/conf.d/gov_portal.conf"),
        "upstream gov_backend {\n  server 127.0.0.1:8080;\n}\nserver {\n  listen 80;\n  server_name gov.local;\n  location /api/ {\n    proxy_pass http://gov_backend;\n  }\n}\n",
    );

    let j1 = parse_jvm_cmdline(
        3001,
        "java -Xms1g -Xmx2g -Dspring.datasource.password=Secret99 -jar /opt/biz/portal.jar --spring.config.location=/etc/biz/v1.yml --server.port=8080",
    )
    .unwrap();
    assert_eq!(j1.middleware_kind, "spring_boot");
    assert_eq!(j1.listen_port, Some(8080));
    assert_eq!(j1.spring_configs, vec!["/etc/biz/v1.yml"]);

    let topo = parse_nginx_hierarchy(&r.join("etc/nginx/nginx.conf"), r);
    assert_eq!(topo.upstreams.len(), 1);
    assert_eq!(topo.proxy_routes.len(), 1);
    assert_eq!(topo.proxy_routes[0].resolved_backends, vec!["127.0.0.1:8080"]);

    let mut rt = KernelRuntime::open_sqlite_in_memory(None).unwrap();
    let order = rt
        .install_extensions(&[
            Box::new(ProbeJvmMiddlewareExtension::new(r)),
            Box::new(StubLinuxOpsExtension),
        ])
        .unwrap();
    assert_eq!(order, vec!["probe_linux_ops", "probe_jvm_middleware"]);

    let auth_ctx = ClientAuthContext {
        transport: TransportKind::Internal,
        cert: None,
        bearer_token: None,
        method: "GET".to_string(),
        path: "/api/v1/fdw".to_string(),
        is_loopback: true,
        is_read_only: true,
    };
    let mut ctx = ProbeContext::default();
    let tuples = rt
        .scan_fdw("fdw:jvm_middleware", None, &mut ctx, &auth_ctx)
        .unwrap();
    assert_eq!(tuples.len(), 4);
}
