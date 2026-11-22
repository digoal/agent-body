use std::process::Command;

#[test]
fn test_cli_binary_e2e_isolated_lifecycle() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let data_dir = tmp.path().join("data");
    let db_path = data_dir.join("steward.db");
    let ws_root = tmp.path().join("ws");
    std::fs::create_dir_all(ws_root.join("work")).expect("mkdir work");
    std::fs::create_dir_all(ws_root.join("tmp")).expect("mkdir tmp");

    let cfg_path = tmp.path().join("config.toml");
    let cfg_content = format!(
        r#"
data_dir = {:?}
db_path = {:?}

[workspace]
base_dir = {:?}
roots = [{:?}]

[web]
enabled = false

[annotation]
enabled = false
"#,
        data_dir.to_string_lossy(),
        db_path.to_string_lossy(),
        ws_root.to_string_lossy(),
        ws_root.join("work").to_string_lossy(),
    );
    std::fs::write(&cfg_path, cfg_content).expect("write config.toml");

    let bin = env!("CARGO_BIN_EXE_agent-body");

    // 1. status
    let status_out = Command::new(bin)
        .args(["--config", cfg_path.to_str().expect("utf8"), "status"])
        .output()
        .expect("run status");
    assert!(
        status_out.status.success(),
        "status failed: {}",
        String::from_utf8_lossy(&status_out.stderr)
    );
    assert!(String::from_utf8_lossy(&status_out.stdout).contains("已纳管资产数"));

    // 2. body vitals --json
    let vitals_out = Command::new(bin)
        .args(["--config", cfg_path.to_str().expect("utf8"), "body", "vitals", "--json"])
        .output()
        .expect("run body vitals");
    assert!(vitals_out.status.success());
    let vitals_json: serde_json::Value =
        serde_json::from_slice(&vitals_out.stdout).expect("parse vitals json");
    assert!(vitals_json.get("overall_score").is_some());

    // 3. body perf-snapshot + body perf-awr
    let snap_out = Command::new(bin)
        .args([
            "--config",
            cfg_path.to_str().expect("utf8"),
            "body",
            "perf-snapshot",
            "--label",
            "cli-e2e",
            "--set-baseline",
            "--json",
        ])
        .output()
        .expect("run perf-snapshot");
    assert!(snap_out.status.success());

    let awr_out = Command::new(bin)
        .args([
            "--config",
            cfg_path.to_str().expect("utf8"),
            "body",
            "perf-awr",
            "--vs-baseline",
            "true",
            "--json",
        ])
        .output()
        .expect("run perf-awr");
    assert!(awr_out.status.success());
    let awr_json: serde_json::Value =
        serde_json::from_slice(&awr_out.stdout).expect("parse awr json");
    assert!(awr_json.get("report_id").is_some());

    // 4. audit summary --json (should record CLI calls above!)
    let audit_out = Command::new(bin)
        .args([
            "--config",
            cfg_path.to_str().expect("utf8"),
            "audit",
            "summary",
            "--window",
            "24h",
            "--json",
        ])
        .output()
        .expect("run audit summary");
    assert!(audit_out.status.success());
    let audit_json: serde_json::Value =
        serde_json::from_slice(&audit_out.stdout).expect("parse audit summary json");
    assert!(
        audit_json["summary"]["in_agent_count"].as_u64().unwrap_or(0) >= 1,
        "audit summary 应记录前置 CLI 命令行为: {}",
        audit_json
    );

    // 5. hooks generate --client agy --out-dir <tempdir>
    let hooks_dir = tmp.path().join("hooks_out");
    let hooks_out = Command::new(bin)
        .args([
            "--config",
            cfg_path.to_str().expect("utf8"),
            "hooks",
            "generate",
            "--client",
            "agy",
            "--out-dir",
            hooks_dir.to_str().expect("utf8"),
        ])
        .output()
        .expect("run hooks generate");
    assert!(hooks_out.status.success());
    assert!(hooks_dir.join(".gemini/config/hooks.json").exists());

    // 6. links audit & bench list
    let links_out = Command::new(bin)
        .args(["--config", cfg_path.to_str().expect("utf8"), "links", "audit"])
        .output()
        .expect("run links audit");
    assert!(links_out.status.success());

    let bench_workloads = Command::new(bin)
        .args(["--config", cfg_path.to_str().expect("utf8"), "bench", "workloads"])
        .output()
        .expect("run bench workloads");
    assert!(bench_workloads.status.success());
}

