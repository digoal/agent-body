//! `ext-probe-db-engines` 集成回归测试：多引擎 PG/达梦/人大金仓配置夹具与 SHA-256 指纹校验

use std::fs;

use agent_body_kernel::KernelRuntime;
use agent_body_sdk::{FdwRoutine, ProbeContext, TypeRoutine};
use ext_probe_db_engines::{DbInstanceType, ProbeDbEnginesExtension};
use tempfile::TempDir;

#[test]
fn probes_pg_dameng_and_kingbase_configs_and_verifies_evidence_sha256() {
    let root = TempDir::new().unwrap();

    // 1. PostgreSQL 配置夹具 (含 include, include_dir 与 pg_hba.conf)
    let pg_dir = root.path().join("pg_cluster");
    let pg_conf_d = pg_dir.join("conf.d");
    fs::create_dir_all(&pg_conf_d).unwrap();
    fs::write(pg_dir.join("PG_VERSION"), "16\n").unwrap();
    fs::write(
        pg_dir.join("postgresql.conf"),
        "listen_addresses = '*'\nport = 5432\nshared_buffers = '256MB'\nwal_level = 'replica'\ninclude = 'custom.conf'\ninclude_dir = 'conf.d'\n",
    )
    .unwrap();
    fs::write(
        pg_dir.join("custom.conf"),
        "max_wal_size = '2GB'\nwork_mem = '16MB'\n",
    )
    .unwrap();
    fs::write(
        pg_conf_d.join("99-tuning.conf"),
        "shared_buffers = '512MB'\nmaintenance_work_mem = '128MB'\n",
    )
    .unwrap();
    fs::write(
        pg_dir.join("pg_hba.conf"),
        "# TYPE DATABASE USER ADDRESS METHOD\nlocal all all peer\nhost all all 127.0.0.1/32 scram-sha-256\n",
    )
    .unwrap();

    // 2. 达梦 (Dameng) dm.ini 夹具
    let dm_dir = root.path().join("dameng_inst");
    fs::create_dir_all(&dm_dir).unwrap();
    fs::write(
        dm_dir.join("dm.ini"),
        "INSTANCE_NAME = DMSERVER\nPORT_NUM = 5236\nCTL_PATH = /opt/dmdbms/data/DAMENG/dm.ctl\nMEMORY_POOL = 500\nBUFFER = 1000\nRLOG_BUF_SIZE = 1024\nARCHIVE_MODE = 1\n",
    )
    .unwrap();

    // 3. 人大金仓 (KingbaseES) kingbase.conf + sys_hba.conf 夹具
    let kb_dir = root.path().join("kingbase_inst");
    fs::create_dir_all(&kb_dir).unwrap();
    fs::write(
        kb_dir.join("kingbase.conf"),
        "port = 54321\ndata_directory = '/var/lib/kingbase/data'\nshared_buffers = '1GB'\nwal_level = 'logical'\n",
    )
    .unwrap();
    fs::write(
        kb_dir.join("sys_hba.conf"),
        "host all system 127.0.0.1/32 scram-sha-256\n",
    )
    .unwrap();

    // 安装扩展并校验私有表与目录登记
    let mut rt = KernelRuntime::open_sqlite_in_memory(None).unwrap();
    let ext = ProbeDbEnginesExtension::with_roots(vec![root.path().to_path_buf()]);
    let fdw = ext.fdw();
    let order = rt.install_extensions(&[Box::new(ext)]).unwrap();
    assert_eq!(order, vec!["probe_db_engines"]);

    let rec = rt
        .get_extension_record("probe_db_engines")
        .unwrap()
        .expect("catalog record must exist");
    assert_eq!(rec.table_prefix, "ext_probe_db_engines__");

    rt.table_am()
        .exec_scoped_migration(
            "probe_db_engines",
            "ext_probe_db_engines__",
            "0.1.1",
            "INSERT INTO ext_probe_db_engines__snapshots (snapshot_id, engine_kind, instance_id, config_path, port, data_dir, params_json, evidence_sha256, collected_at)
             VALUES ('snap-1', 'postgresql', 'db:pg', '/pg/postgresql.conf', 5432, '/pg', '{}', 'abc', '2026-11-27T16:00:00+08:00');",
        )
        .unwrap();

    // 运行 FDW 扫描并校验多引擎参数提取与 EvidenceTuple SHA-256 指纹
    let ctx = ProbeContext {
        target_host: Some("db-node-01".to_string()),
        ..Default::default()
    };
    assert!(fdw.explain_source(&ctx).healthy);

    let mut scan = fdw.begin_scan(None, &ctx).unwrap();
    let tuples = scan.iterate_scan(10).unwrap();
    assert!(scan.iterate_scan(10).unwrap().is_empty());
    scan.end_scan().unwrap();

    assert_eq!(tuples.len(), 3);
    let ty = DbInstanceType;
    for tup in &tuples {
        assert_eq!(tup.entity_type, "entity:db_instance");
        assert_eq!(tup.source_ext, "probe_db_engines");
        assert!(tup.verify_sha256(), "SHA-256 fingerprint mismatch for {}", tup.entity_id);
        assert!(ty.validate_in(&tup.payload).is_ok());
        assert!(!ty.render_compact_summary(&tup.payload).is_empty());
    }

    let pg = tuples
        .iter()
        .find(|t| t.payload["engine"] == "postgresql")
        .expect("postgresql tuple missing");
    assert_eq!(pg.payload["port"], 5432);
    assert_eq!(pg.payload["memory_params"]["shared_buffers"], "512MB");
    assert_eq!(pg.payload["memory_params"]["work_mem"], "16MB");
    assert_eq!(pg.payload["wal_params"]["wal_level"], "replica");
    assert_eq!(pg.payload["wal_params"]["max_wal_size"], "2GB");
    let pg_refs = pg.payload["referenced_files"].as_array().unwrap();
    assert!(pg_refs.iter().any(|v| v.as_str().unwrap_or("").ends_with("custom.conf")));
    assert!(pg_refs.iter().any(|v| v.as_str().unwrap_or("").ends_with("99-tuning.conf")));
    assert!(pg_refs.iter().any(|v| v.as_str().unwrap_or("").ends_with("pg_hba.conf")));

    let dm = tuples
        .iter()
        .find(|t| t.payload["engine"] == "dameng")
        .expect("dameng tuple missing");
    assert_eq!(dm.payload["port"], 5236);
    assert_eq!(dm.payload["memory_params"]["MEMORY_POOL"], "500");
    assert_eq!(dm.payload["params"]["CTL_PATH"], "/opt/dmdbms/data/DAMENG/dm.ctl");

    let kb = tuples
        .iter()
        .find(|t| t.payload["engine"] == "kingbase")
        .expect("kingbase tuple missing");
    assert_eq!(kb.payload["port"], 54321);
    assert_eq!(kb.payload["data_dir"], "/var/lib/kingbase/data");
    assert_eq!(kb.payload["memory_params"]["shared_buffers"], "1GB");
    assert_eq!(kb.payload["wal_params"]["wal_level"], "logical");
}
