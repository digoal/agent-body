//! 内核自诊断规则引擎（PG `pg_stat_activity` + `pg_stat_statements` + Top-N 慢调用 + 5 条异常规则）

use agent_body_sdk::KernelObservabilitySnapshot;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::prometheus::format_w3c_traceparent;

/// 自诊断异常判定阈值
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObsDiagnoseThresholds {
    pub auth_deny_warn: u64,
    pub timeout_warn: u64,
    pub error_rate_warn_pct: f64,
    pub wait_p95_warn_us: u64,
    pub siem_drop_warn: u64,
}

impl Default for ObsDiagnoseThresholds {
    fn default() -> Self {
        Self {
            auth_deny_warn: 1,
            timeout_warn: 1,
            error_rate_warn_pct: 10.0,
            wait_p95_warn_us: 250_000,
            siem_drop_warn: 1,
        }
    }
}

/// 评估内核观测快照并生成诊断报告 JSON
pub fn evaluate_diagnostics(
    snap: &KernelObservabilitySnapshot,
    siem_dropped: u64,
    limit: usize,
    thresholds: &ObsDiagnoseThresholds,
) -> serde_json::Value {
    let top_limit = limit.clamp(1, 100);
    let total_calls: u64 = snap.op_stats.iter().map(|s| s.calls).sum();
    let total_errors: u64 = snap.op_stats.iter().map(|s| s.error_count).sum();
    let total_denied: u64 = snap.op_stats.iter().map(|s| s.denied_count).sum();
    let total_timeouts: u64 = snap.op_stats.iter().map(|s| s.timeout_count).sum();
    let error_rate_pct = if total_calls == 0 {
        0.0
    } else {
        ((total_errors as f64 / total_calls as f64) * 10_000.0).round() / 100.0
    };

    let mut anomalies = Vec::new();
    if total_denied >= thresholds.auth_deny_warn {
        anomalies.push(json!({
            "rule_id": "OBS-01",
            "severity": "warn",
            "message": format!("检测到 {total_denied} 次鉴权/访问控制拒绝 (auth/access denied)"),
            "remediation": "核查调用方 Bearer Token / mTLS 客户端证书 CN 映射与 RLS 租户标签是否一致"
        }));
    }
    if total_timeouts >= thresholds.timeout_warn {
        anomalies.push(json!({
            "rule_id": "OBS-02",
            "severity": "critical",
            "message": format!("检测到 {total_timeouts} 次外部探针或沙盒执行超时 (timeout)"),
            "remediation": "检查轨 B 沙盒外部数据源响应延迟及 .control.toml [sandbox].timeout_ms 预算"
        }));
    }
    if error_rate_pct >= thresholds.error_rate_warn_pct && total_errors > 0 {
        anomalies.push(json!({
            "rule_id": "OBS-03",
            "severity": "critical",
            "message": format!("算子/探针整体错误率 {error_rate_pct}% ({total_errors}/{total_calls}) 超过阈值"),
            "remediation": "查看 recent_slow_or_failed 中的 trace_id 与 detail 定位异常源头"
        }));
    }
    for w in &snap.wait_events {
        if w.calls > 0 && w.est_p95_us >= thresholds.wait_p95_warn_us {
            anomalies.push(json!({
                "rule_id": "OBS-04",
                "severity": "warn",
                "message": format!("等待事件 {} P95 延迟 {}us 达到预警水位", w.event_name, w.est_p95_us),
                "remediation": "检查锁竞争、外部探针耗时或后台协程周期频率"
            }));
        }
    }
    if siem_dropped >= thresholds.siem_drop_warn {
        anomalies.push(json!({
            "rule_id": "OBS-05",
            "severity": "warn",
            "message": format!("SIEM 环形审计缓冲累计丢弃 {siem_dropped} 条事件"),
            "remediation": "缩短 SiemFlushWorker 刷新周期或调大 SiemSinkConfig.ring_capacity"
        }));
    }

    let has_critical = anomalies
        .iter()
        .any(|a| a["severity"].as_str() == Some("critical"));
    let health_grade = if has_critical {
        "critical"
    } else if !anomalies.is_empty() {
        "degraded"
    } else {
        "healthy"
    };

    let mut wait_hotspots: Vec<_> = snap
        .wait_events
        .iter()
        .filter(|w| w.calls > 0)
        .cloned()
        .collect();
    wait_hotspots.sort_by_key(|w| std::cmp::Reverse(w.total_us));
    wait_hotspots.truncate(top_limit);

    let top_operations: Vec<_> = snap.op_stats.iter().take(top_limit).cloned().collect();
    let recent_slow_or_failed: Vec<_> = snap
        .slow_ops
        .iter()
        .rev()
        .take(top_limit)
        .map(|rec| {
            json!({
                "trace_id": rec.trace_id,
                "traceparent": format_w3c_traceparent(&rec.trace_id, true),
                "kind": rec.kind,
                "target": rec.target,
                "caller_id": rec.caller_id,
                "outcome": rec.outcome,
                "elapsed_us": rec.elapsed_us,
                "detail": rec.detail,
                "recorded_at": rec.recorded_at,
            })
        })
        .collect();

    json!({
        "captured_at": snap.captured_at,
        "uptime_ms": snap.uptime_ms,
        "slow_threshold_us": snap.slow_threshold_us,
        "health_grade": health_grade,
        "summary": {
            "total_calls": total_calls,
            "total_errors": total_errors,
            "total_denied": total_denied,
            "total_timeouts": total_timeouts,
            "error_rate_pct": error_rate_pct,
            "in_flight_count": snap.in_flight.len(),
            "slow_ops_count": snap.slow_ops.len(),
            "siem_dropped": siem_dropped,
        },
        "active_in_flight": snap.in_flight,
        "top_operations": top_operations,
        "wait_hotspots": wait_hotspots,
        "recent_slow_or_failed": recent_slow_or_failed,
        "anomalies": anomalies,
    })
}
