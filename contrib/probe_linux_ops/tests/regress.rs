//! `contrib/probe_linux_ops` 集成回归测试

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use agent_body_kernel::KernelRuntime;
use agent_body_sdk::{
    CancellationToken, ClientAuthContext, EventQueryFilter, ProbeContext, TransportKind,
};
use ext_probe_linux_ops::automation_parser::{scan_cron_all, scan_systemd_units};
use ext_probe_linux_ops::proc_parser::scan_proc_root;
use ext_probe_linux_ops::ProbeLinuxOpsExtension;

fn write_fixture_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

#[test]
fn probes_started_at_long_cmd_multi_listener_established_rhel_cron_and_systemd() {
    let tmp = tempfile::tempdir().unwrap();
    let r = tmp.path();

    // 1. /proc/stat + /proc/net/tcp + /proc/self/status
    write_fixture_file(&r.join("proc/stat"), "cpu  0 0 0\nbtime 1764200000\n");
    write_fixture_file(
        &r.join("proc/self/status"),
        "Name:\tagent\nUid:\t1000\t1000\t1000\t1000\nCapEff:\t0000000000000000\n",
    );
    // 8080 LISTEN (inode 1001), 8081 LISTEN (inode 1002), 8080 second worker SO_REUSEPORT (inode 1003),
    // and Nginx ESTABLISHED -> 127.0.0.1:8080 (inode 1004)
    let tcp_content = "\
  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 00000000:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 1001 1 00000000 100 0 0 10 0
   1: 0100007F:1F91 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 1002 1 00000000 100 0 0 10 0
   2: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 1003 1 00000000 100 0 0 10 0
   3: 0100007F:C350 0100007F:1F90 01 00000000:00000000 00:00000000 00000000  1000        0 1004 1 00000000 100 0 0 10 0
";
    write_fixture_file(&r.join("proc/net/tcp"), tcp_content);

    // 2. PID 2001 (老 Java 8080) & PID 2002 (14:02 新发布长命令行 Java 8081) & PID 2003 (Nginx 连接到 8080)
    let long_jvm = format!(
        "java -Xms2g -Xmx2g {} -Dspring.datasource.password=S3cret! -jar /opt/gov/app-v2.jar --spring.config.location=/opt/gov/application-prod.yml --server.port=8081",
        "-XX:+UseG1GC ".repeat(16)
    );
    assert!(long_jvm.len() > 350);
    write_fixture_file(&r.join("proc/2001/comm"), "java\n");
    write_fixture_file(
        &r.join("proc/2001/stat"),
        "2001 (java worker) S 1 2001 2001 0 -1 4194304 10 0 0 0 0 0 0 0 20 0 1 0 1000 0 0",
    );
    write_fixture_file(
        &r.join("proc/2001/cmdline"),
        "java\0-jar\0/opt/gov/app-v1.jar\0--server.port=8080\0",
    );
    write_fixture_file(&r.join("proc/2001/fd/3"), "socket:[1001]");

    write_fixture_file(&r.join("proc/2002/comm"), "java\n");
    write_fixture_file(
        &r.join("proc/2002/stat"),
        "2002 (java) S 1 2002 2002 0 -1 4194304 10 0 0 0 0 0 0 0 20 0 1 0 360000 0 0",
    );
    write_fixture_file(
        &r.join("proc/2002/cmdline"),
        &long_jvm.replace(' ', "\0"),
    );
    write_fixture_file(&r.join("proc/2002/fd/4"), "socket:[1002]");

    write_fixture_file(&r.join("proc/2003/comm"), "nginx\n");
    write_fixture_file(
        &r.join("proc/2003/stat"),
        "2003 (nginx) S 1 2003 2003 0 -1 4194304 10 0 0 0 0 0 0 0 20 0 1 0 500 0 0",
    );
    write_fixture_file(&r.join("proc/2003/cmdline"), "nginx:\0worker\0process\0");
    write_fixture_file(&r.join("proc/2003/fd/5"), "socket:[1003]");
    write_fixture_file(&r.join("proc/2003/fd/6"), "socket:[1004]");

    // 3. Systemd (/usr/lib/systemd/system + drop-in + runtime_state)
    write_fixture_file(
        &r.join("usr/lib/systemd/system/gov-app.service"),
        "[Unit]\nDescription=Gov App\n[Service]\nExecStart=/usr/bin/java -jar /opt/gov/app-v1.jar\n",
    );
    write_fixture_file(
        &r.join("etc/systemd/system/gov-app.service.d/override.conf"),
        "[Service]\nEnvironmentFile=/etc/gov/app.env\n",
    );
    write_fixture_file(
        &r.join("run/systemd/runtime_state/gov-app.service.state"),
        "ActiveState=active\nSubState=running\nMainPID=2002\nResult=success\n",
    );

    // 4. RHEL / 麒麟 /var/spool/cron/root (含 --config=/etc/app.yml 绝不能被 = 误杀)
    write_fixture_file(
        &r.join("var/spool/cron/root"),
        "PATH=/usr/local/bin:/usr/bin\n*/5 * * * * /opt/gov/check.sh --config=/etc/gov/check.yml\n",
    );

    let snap = scan_proc_root(r, 4096);
    assert_eq!(snap.processes.len(), 3);
    let p2002 = snap.processes.iter().find(|p| p.pid == 2002).unwrap();
    assert_eq!(p2002.ppid, Some(1));
    assert!(p2002.started_at.is_some());
    assert!(!p2002.cmdline.truncated);
    assert_eq!(p2002.cmdline.jar_path.as_deref(), Some("/opt/gov/app-v2.jar"));
    assert_eq!(
        p2002.cmdline.spring_config_location.as_deref(),
        Some("/opt/gov/application-prod.yml")
    );
    assert_eq!(p2002.cmdline.server_port, Some(8081));

    let p2003 = snap.processes.iter().find(|p| p.pid == 2003).unwrap();
    assert_eq!(p2003.listening_sockets.len(), 1);
    assert_eq!(p2003.established_peers.len(), 1);
    assert_eq!(p2003.established_peers[0].remote_port, 8080);

    let cap = snap.cap_check.unwrap();
    assert!(!cap.is_root);
    assert!(cap
        .degraded_reason
        .unwrap()
        .contains("CAP_DAC_READ_SEARCH"));

    let units = scan_systemd_units(r);
    assert_eq!(units.len(), 1);
    assert_eq!(units[0].drop_in_files.len(), 1);
    assert_eq!(units[0].runtime_state.as_ref().unwrap().main_pid, Some(2002));

    let crons = scan_cron_all(r);
    assert_eq!(crons.len(), 1);
    assert!(crons[0].command.contains("--config=/etc/gov/check.yml"));

    // 5. 通过 KernelRuntime 安装与执行 FDW 扫描 + BgWorker 漂移检测
    let mut rt = KernelRuntime::open_sqlite_in_memory(None).unwrap();
    let order = rt
        .install_extensions(&[Box::new(ProbeLinuxOpsExtension::new(r))])
        .unwrap();
    assert_eq!(order, vec!["probe_linux_ops"]);

    let auth_ctx = ClientAuthContext {
        transport: TransportKind::Internal,
        cert: None,
        bearer_token: None,
        method: "GET".to_string(),
        path: "/api/v1/fdw".to_string(),
        is_loopback: true,
        is_read_only: true,
    };
    let mut probe_ctx = ProbeContext {
        params: BTreeMap::new(),
        ..Default::default()
    };
    let tuples = rt
        .scan_fdw("fdw:linux_ops", Some("8081"), &mut probe_ctx, &auth_ctx)
        .unwrap();
    assert_eq!(tuples.len(), 1);
    assert_eq!(tuples[0].entity_id, "proc:2002");

    let ticks = rt.run_bgworkers_once(&CancellationToken::new()).unwrap();
    assert_eq!(ticks.len(), 1);
    assert_eq!(ticks[0].1.emitted_events, 1);

    let drift_events = rt
        .table_am()
        .event_query(
            &EventQueryFilter {
                stream: Some("host_drift".to_string()),
                limit: 10,
                ..Default::default()
            },
            None,
        )
        .unwrap();
    assert_eq!(drift_events.len(), 1);
}
