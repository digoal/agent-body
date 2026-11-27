//! OpenMetrics / Prometheus 0.0.4 文本导出、W3C `traceparent` 与 JSONL 结构化事件格式化

use std::fmt::{Arguments, Write as _};

use agent_body_sdk::{
    types::rfc3339_beijing_now, KernelObservabilitySnapshot, SlowOpRecord,
    KERNEL_LATENCY_BUCKET_BOUNDS_US,
};

fn escape_label_value(raw: &str) -> String {
    raw.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[inline]
fn append_line(out: &mut String, args: Arguments<'_>) {
    if out.write_fmt(args).is_err() {
        out.clear();
    }
    out.push('\n');
}

/// 读取当前进程零外部依赖的 `/proc/self` RSS（字节）与开放 FD 数量（非 Linux 优雅回退为 0）
pub fn read_proc_self_rss_and_fds() -> (u64, usize) {
    let mut rss_bytes = 0u64;
    if let Ok(status_txt) = std::fs::read_to_string("/proc/self/status") {
        for line in status_txt.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:") {
                if let Some(kb) = rest
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse::<u64>().ok())
                {
                    rss_bytes = kb.saturating_mul(1024);
                    break;
                }
            }
        }
    }
    let open_fds = std::fs::read_dir("/proc/self/fd")
        .map(|entries| entries.count())
        .unwrap_or(0);
    (rss_bytes, open_fds)
}

/// 生成严格符合 OpenMetrics / Prometheus 0.0.4 规范的指标文本
pub fn render_prometheus_text(
    snap: &KernelObservabilitySnapshot,
    service_name: &str,
    host_id: &str,
    siem_dropped: Option<u64>,
) -> String {
    let svc = escape_label_value(service_name);
    let host = escape_label_value(host_id);
    let mut out = String::with_capacity(4096);

    append_line(&mut out, format_args!("# HELP agent_body_kernel_uptime_seconds Microkernel process uptime in seconds."));
    append_line(&mut out, format_args!("# TYPE agent_body_kernel_uptime_seconds gauge"));
    append_line(
        &mut out,
        format_args!(
            "agent_body_kernel_uptime_seconds{{service=\"{svc}\",host=\"{host}\"}} {:.6}",
            snap.uptime_ms as f64 / 1000.0
        ),
    );

    append_line(&mut out, format_args!("# HELP agent_body_kernel_in_flight_ops Currently executing kernel operations."));
    append_line(&mut out, format_args!("# TYPE agent_body_kernel_in_flight_ops gauge"));
    append_line(
        &mut out,
        format_args!(
            "agent_body_kernel_in_flight_ops{{service=\"{svc}\",host=\"{host}\"}} {}",
            snap.in_flight.len()
        ),
    );

    append_line(&mut out, format_args!("# HELP agent_body_kernel_slow_ops_buffered Slow or error operations retained in bounded ring."));
    append_line(&mut out, format_args!("# TYPE agent_body_kernel_slow_ops_buffered gauge"));
    append_line(
        &mut out,
        format_args!(
            "agent_body_kernel_slow_ops_buffered{{service=\"{svc}\",host=\"{host}\"}} {}",
            snap.slow_ops.len()
        ),
    );

    append_line(&mut out, format_args!("# HELP agent_body_kernel_wait_duration_seconds Cumulative kernel wait event latency histogram."));
    append_line(&mut out, format_args!("# TYPE agent_body_kernel_wait_duration_seconds histogram"));
    for w in &snap.wait_events {
        let ev = escape_label_value(&w.event_name);
        let mut cumulative = 0u64;
        for (idx, count) in w.histogram_buckets.iter().enumerate() {
            cumulative = cumulative.saturating_add(*count);
            let bound = KERNEL_LATENCY_BUCKET_BOUNDS_US[idx];
            let le = if bound == u64::MAX {
                "+Inf".to_string()
            } else {
                format_secs_bound(bound)
            };
            append_line(
                &mut out,
                format_args!(
                    "agent_body_kernel_wait_duration_seconds_bucket{{service=\"{svc}\",host=\"{host}\",event=\"{ev}\",le=\"{le}\"}} {cumulative}"
                ),
            );
        }
        append_line(
            &mut out,
            format_args!(
                "agent_body_kernel_wait_duration_seconds_sum{{service=\"{svc}\",host=\"{host}\",event=\"{ev}\"}} {:.6}",
                w.total_us as f64 / 1_000_000.0
            ),
        );
        append_line(
            &mut out,
            format_args!(
                "agent_body_kernel_wait_duration_seconds_count{{service=\"{svc}\",host=\"{host}\",event=\"{ev}\"}} {}",
                w.calls
            ),
        );
    }

    append_line(&mut out, format_args!("# HELP agent_body_kernel_op_calls_total Kernel operation invocation counters by kind, target and outcome."));
    append_line(&mut out, format_args!("# TYPE agent_body_kernel_op_calls_total counter"));
    for op in &snap.op_stats {
        let kind = escape_label_value(&op.kind);
        let target = escape_label_value(&op.target);
        for (outcome, val) in [
            ("ok", op.ok_count),
            ("error", op.error_count),
            ("denied", op.denied_count),
            ("timeout", op.timeout_count),
        ] {
            append_line(
                &mut out,
                format_args!(
                    "agent_body_kernel_op_calls_total{{service=\"{svc}\",host=\"{host}\",kind=\"{kind}\",target=\"{target}\",outcome=\"{outcome}\"}} {val}"
                ),
            );
        }
    }

    append_line(&mut out, format_args!("# HELP agent_body_kernel_op_duration_seconds_sum Total accumulated execution seconds per kernel operation."));
    append_line(&mut out, format_args!("# TYPE agent_body_kernel_op_duration_seconds_sum counter"));
    for op in &snap.op_stats {
        let kind = escape_label_value(&op.kind);
        let target = escape_label_value(&op.target);
        append_line(
            &mut out,
            format_args!(
                "agent_body_kernel_op_duration_seconds_sum{{service=\"{svc}\",host=\"{host}\",kind=\"{kind}\",target=\"{target}\"}} {:.6}",
                op.total_us as f64 / 1_000_000.0
            ),
        );
    }

    let (rss_bytes, open_fds) = read_proc_self_rss_and_fds();
    append_line(&mut out, format_args!("# HELP agent_body_process_rss_bytes Current process resident set size bytes (from /proc/self when available)."));
    append_line(&mut out, format_args!("# TYPE agent_body_process_rss_bytes gauge"));
    append_line(
        &mut out,
        format_args!("agent_body_process_rss_bytes{{service=\"{svc}\",host=\"{host}\"}} {rss_bytes}"),
    );

    append_line(&mut out, format_args!("# HELP agent_body_process_open_fds Open file descriptor count (from /proc/self/fd when available)."));
    append_line(&mut out, format_args!("# TYPE agent_body_process_open_fds gauge"));
    append_line(
        &mut out,
        format_args!("agent_body_process_open_fds{{service=\"{svc}\",host=\"{host}\"}} {open_fds}"),
    );

    if let Some(dropped) = siem_dropped {
        append_line(&mut out, format_args!("# HELP agent_body_siem_dropped_total Total SIEM ring buffer dropped audit events."));
        append_line(&mut out, format_args!("# TYPE agent_body_siem_dropped_total counter"));
        append_line(
            &mut out,
            format_args!("agent_body_siem_dropped_total{{service=\"{svc}\",host=\"{host}\"}} {dropped}"),
        );
    }

    out
}

fn format_secs_bound(us: u64) -> String {
    match us {
        100 => "0.0001".to_string(),
        500 => "0.0005".to_string(),
        2_000 => "0.002".to_string(),
        10_000 => "0.01".to_string(),
        50_000 => "0.05".to_string(),
        250_000 => "0.25".to_string(),
        1_000_000 => "1".to_string(),
        other => format!("{:.6}", other as f64 / 1_000_000.0),
    }
}

/// 确定性将内核 `trace_id` 归一化为合法 W3C `traceparent` (`00-<32hex>-<16hex>-<flags>`)
pub fn format_w3c_traceparent(trace_id: &str, sampled: bool) -> String {
    let mut hex_chars: String = trace_id
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if hex_chars.is_empty() {
        for b in trace_id.as_bytes().iter().take(16) {
            hex_chars.push_str(&format!("{b:02x}"));
        }
    }
    while hex_chars.len() < 32 {
        hex_chars.push('0');
    }
    hex_chars.truncate(32);
    if hex_chars.bytes().all(|b| b == b'0') {
        hex_chars = "1".to_string() + &"0".repeat(31);
    }

    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in trace_id.as_bytes() {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    if hash == 0 {
        hash = 1;
    }
    let flags = if sampled { "01" } else { "00" };
    format!("00-{hex_chars}-{hash:016x}-{flags}")
}

/// 格式化单行 JSONL 结构化观测事件（兼容 Loki / Filebeat / OTel Collector）
pub fn format_jsonl_obs_event(service_name: &str, host_id: &str, record: &SlowOpRecord) -> String {
    let level = if record.outcome == "ok" {
        "WARN"
    } else if record.outcome == "timeout" || record.outcome == "error" {
        "ERROR"
    } else {
        "WARN"
    };
    let val = serde_json::json!({
        "timestamp": if record.recorded_at.is_empty() { rfc3339_beijing_now() } else { record.recorded_at.clone() },
        "level": level,
        "service": service_name,
        "host_id": host_id,
        "trace_id": record.trace_id,
        "traceparent": format_w3c_traceparent(&record.trace_id, true),
        "kind": record.kind,
        "target": record.target,
        "caller_id": record.caller_id,
        "outcome": record.outcome,
        "elapsed_us": record.elapsed_us,
        "detail": record.detail,
    });
    serde_json::to_string(&val).unwrap_or_else(|_| "{}".to_string())
}
