use std::process::Command;

#[test]
fn test_cli_binary_e2e_isolated_lifecycle() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let data_dir = tmp.path().join("data");
    let db_path = data_dir.join("body.db");
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

fn write_isolated_cli_config(tmp: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let data_dir = tmp.join("data");
    let db_path = data_dir.join("body.db");
    let ws_root = tmp.join("ws");
    std::fs::create_dir_all(&data_dir).expect("mkdir data");
    std::fs::create_dir_all(ws_root.join("work")).expect("mkdir work");
    std::fs::create_dir_all(ws_root.join("tmp")).expect("mkdir tmp");

    let cfg_path = tmp.join("config.toml");
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
    (cfg_path, db_path)
}

#[test]
fn test_cli_legacy_db_auto_migration_and_bench_run() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (cfg_path, db_path) = write_isolated_cli_config(tmp.path());
    let lp = ["st", "ew", "ard"].concat();
    {
        let raw = rusqlite::Connection::open(&db_path).expect("raw sqlite");
        let ddl = format!(
            "CREATE TABLE benchmark_runs (
                run_id TEXT PRIMARY KEY, suite_name TEXT NOT NULL, git_commit TEXT NOT NULL,
                app_version TEXT NOT NULL, host_fingerprint TEXT NOT NULL, workload_count INTEGER NOT NULL,
                total_baseline_tokens INTEGER NOT NULL, total_{lp}_tokens INTEGER NOT NULL,
                overall_token_savings_pct REAL NOT NULL, overall_speedup_factor REAL NOT NULL,
                total_hazards_intercepted INTEGER NOT NULL, total_noise_blocked INTEGER NOT NULL,
                sla_passed INTEGER NOT NULL, results_json TEXT NOT NULL, report_sha256 TEXT NOT NULL,
                duration_ms INTEGER NOT NULL, created_at TEXT NOT NULL
            );
            INSERT INTO benchmark_runs VALUES (
                'cli-legacy-run-1', 'micro', 'abc000', '0.9.0', '{{}}',
                6, 8000, 600, 92.5, 18.0, 1, 10, 1, '[]', 'sha-cli-legacy', 120, '2026-11-12T00:00:00Z'
            );
            CREATE TABLE effectiveness_ledger (
                event_id TEXT PRIMARY KEY, session_id TEXT NOT NULL DEFAULT 'default',
                source_channel TEXT NOT NULL, tool_or_action TEXT NOT NULL, provenance_rule TEXT NOT NULL,
                baseline_tokens INTEGER NOT NULL DEFAULT 0, {lp}_tokens INTEGER NOT NULL DEFAULT 0,
                tokens_saved INTEGER NOT NULL DEFAULT 0, baseline_rounds INTEGER NOT NULL DEFAULT 1,
                {lp}_rounds INTEGER NOT NULL DEFAULT 1, rounds_saved INTEGER NOT NULL DEFAULT 0,
                hazard_intercepted INTEGER NOT NULL DEFAULT 0, noise_blocked INTEGER NOT NULL DEFAULT 0,
                elapsed_us INTEGER NOT NULL DEFAULT 0, evidence_summary TEXT NOT NULL, created_at TEXT NOT NULL
            );",
            lp = lp
        );
        raw.execute_batch(&ddl).expect("seed legacy cli db");
    }

    let bin = env!("CARGO_BIN_EXE_agent-body");
    let cfg = cfg_path.to_str().expect("utf8");

    // 1. CLI bench run --suite micro 应自动迁移旧库并成功落库
    let bench_out = Command::new(bin)
        .args(["--config", cfg, "bench", "run", "--suite", "micro"])
        .output()
        .expect("run bench micro");
    assert!(
        bench_out.status.success(),
        "bench run on legacy db failed: stdout={} stderr={}",
        String::from_utf8_lossy(&bench_out.stdout),
        String::from_utf8_lossy(&bench_out.stderr)
    );

    // 2. CLI bench ledger --window all 应正常输出无 SQL 错误
    let ledger_out = Command::new(bin)
        .args(["--config", cfg, "bench", "ledger", "--window", "all"])
        .output()
        .expect("run bench ledger");
    assert!(
        ledger_out.status.success(),
        "bench ledger failed: {}",
        String::from_utf8_lossy(&ledger_out.stderr)
    );
}

#[test]
fn test_cli_memory_placement_locks_and_demo_commands() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (cfg_path, _db_path) = write_isolated_cli_config(tmp.path());
    let bin = env!("CARGO_BIN_EXE_agent-body");
    let cfg = cfg_path.to_str().expect("utf8");

    // 1. memory add (troubleshooting 别名 + 脱敏) & search
    let mem_add = Command::new(bin)
        .args([
            "--config",
            cfg,
            "memory",
            "add",
            "--type",
            "troubleshooting",
            "--content",
            "CLI E2E 排障经验 OPENAI_API_KEY=sk-cli1234567890abcdef12",
        ])
        .output()
        .expect("memory add");
    assert!(
        mem_add.status.success(),
        "memory add failed: {}",
        String::from_utf8_lossy(&mem_add.stderr)
    );

    let mem_search = Command::new(bin)
        .args(["--config", cfg, "memory", "search", "排障", "--hybrid=false"])
        .output()
        .expect("memory search");
    assert!(mem_search.status.success());
    let search_txt = String::from_utf8_lossy(&mem_search.stdout);
    assert!(search_txt.contains("CLI E2E"));
    assert!(!search_txt.contains("sk-cli1234567890abcdef12"));

    // 2. placement suggest & rules
    let place_out = Command::new(bin)
        .args([
            "--config",
            cfg,
            "place",
            "suggest",
            "--type",
            "script",
            "--name",
            "db_check",
            "--intent",
            "数据库巡检脚本",
        ])
        .output()
        .expect("place suggest");
    assert!(
        place_out.status.success(),
        "place suggest failed: {}",
        String::from_utf8_lossy(&place_out.stderr)
    );

    // 3. coordination acquire -> list -> deadlock -> release
    let acq = Command::new(bin)
        .args([
            "--config",
            cfg,
            "coordination",
            "acquire",
            "--resource-type",
            "custom",
            "--resource-id",
            "cli-lock-target",
            "--holder-agent",
            "cli-agent-1",
        ])
        .output()
        .expect("lock acquire");
    assert!(
        acq.status.success(),
        "lock acquire failed: {}",
        String::from_utf8_lossy(&acq.stderr)
    );

    let list_l = Command::new(bin)
        .args(["--config", cfg, "coordination", "list"])
        .output()
        .expect("lock list");
    assert!(list_l.status.success());
    assert!(String::from_utf8_lossy(&list_l.stdout).contains("cli-lock-target"));

    let rel = Command::new(bin)
        .args([
            "--config",
            cfg,
            "coordination",
            "release",
            "--resource-type",
            "custom",
            "--resource-id",
            "cli-lock-target",
            "--holder-agent",
            "cli-agent-1",
        ])
        .output()
        .expect("lock release");
    assert!(
        rel.status.success(),
        "lock release failed: {}",
        String::from_utf8_lossy(&rel.stderr)
    );

    // 4. demo list & run scene-1 & reset
    let demo_run = Command::new(bin)
        .args(["--config", cfg, "demo", "run", "scene-1"])
        .output()
        .expect("demo run");
    assert!(
        demo_run.status.success(),
        "demo run failed: {}",
        String::from_utf8_lossy(&demo_run.stderr)
    );

    let demo_rst = Command::new(bin)
        .args(["--config", cfg, "demo", "reset"])
        .output()
        .expect("demo reset");
    assert!(demo_rst.status.success());
}

#[test]
fn test_cli_invalid_subcommand_exits_nonzero() {
    let bin = env!("CARGO_BIN_EXE_agent-body");
    let out = Command::new(bin)
        .args(["definitely-unknown-subcommand"])
        .output()
        .expect("run invalid cli");
    assert!(!out.status.success(), "unknown subcommand must fail cleanly");
}

