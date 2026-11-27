//! 结构化 JSONL 日志与 W3C Trace Context (`traceparent` / `X-Trace-Id`) 透传工具

use chrono::Local;
use uuid::Uuid;

/// 检查环境变量 `AGENT_BODY_LOG_FORMAT` 是否启用结构化 JSONL 输出（默认保持人类可读文本）
pub fn is_json_log_mode() -> bool {
    std::env::var("AGENT_BODY_LOG_FORMAT")
        .map(|v| {
            let norm = v.trim().to_ascii_lowercase();
            norm == "json" || norm == "jsonl"
        })
        .unwrap_or(false)
}

fn fresh_span_hex() -> String {
    let raw = Uuid::new_v4().as_u128();
    let mut low = (raw & 0xFFFF_FFFF_FFFF_FFFF) as u64;
    if low == 0 {
        low = 1;
    }
    format!("{low:016x}")
}

fn is_valid_hex(s: &str, expected_len: usize) -> bool {
    s.len() == expected_len
        && s.bytes().all(|b| b.is_ascii_hexdigit())
        && s.bytes().any(|b| b != b'0')
}

/// 解析入站 W3C `traceparent` 与 `X-Trace-Id`，返回 `(trace_id, traceparent)`
///
/// - 若收到合法 W3C `traceparent` (`00-<32hex>-<16hex>-<2hex>`)：沿用其 32 位 `trace_id`，轮换新的 16 位 `span_id`
/// - 否则若收到合法 `X-Trace-Id`：复用该 `trace_id` 并派生标准化 32 位 hex 构建 `traceparent`
/// - 否则新建 UUIDv4 作为 `trace_id` 并生成标准 `traceparent`
pub fn resolve_trace_context(
    incoming_traceparent: Option<&str>,
    incoming_x_trace_id: Option<&str>,
) -> (String, String) {
    if let Some(tp) = incoming_traceparent.map(str::trim) {
        let parts: Vec<&str> = tp.split('-').collect();
        if parts.len() == 4
            && parts[0].len() == 2
            && is_valid_hex(parts[1], 32)
            && is_valid_hex(parts[2], 16)
            && parts[3].len() == 2
        {
            let tid_lower = parts[1].to_ascii_lowercase();
            let span = fresh_span_hex();
            let next_tp = format!("00-{tid_lower}-{span}-{}", parts[3].to_ascii_lowercase());
            let clean_trace_id = incoming_x_trace_id
                .map(str::trim)
                .filter(|s| !s.is_empty() && s.len() <= 128)
                .map(str::to_string)
                .unwrap_or_else(|| tid_lower.clone());
            return (clean_trace_id, next_tp);
        }
    }

    let trace_id = incoming_x_trace_id
        .map(str::trim)
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 128
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':')
        })
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let traceparent = to_w3c_traceparent(&trace_id);
    (trace_id, traceparent)
}

/// 将任意 `trace_id` 确定性映射为标准 `00-<32hex>-<16hex>-01`
pub fn to_w3c_traceparent(trace_id: &str) -> String {
    let mut hex: String = trace_id
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if hex.is_empty() {
        for b in trace_id.as_bytes().iter().take(16) {
            hex.push_str(&format!("{b:02x}"));
        }
    }
    while hex.len() < 32 {
        hex.push('0');
    }
    hex.truncate(32);
    if hex.bytes().all(|b| b == b'0') {
        hex = "1".to_string() + &"0".repeat(31);
    }
    format!("00-{hex}-{}-01", fresh_span_hex())
}

/// 格式化单条可观测日志（JSONL 模式输出机器可解析单行 JSON，否则输出简洁文本）
pub fn format_obs_log_line(
    json_mode: bool,
    level: &str,
    component: &str,
    trace_id: Option<&str>,
    msg: &str,
    elapsed_us: Option<u64>,
    extra: Option<serde_json::Value>,
) -> String {
    let ts = Local::now().format("%Y-%m-%dT%H:%M:%S+08:00").to_string();
    if json_mode {
        let val = serde_json::json!({
            "timestamp": ts,
            "level": level,
            "component": component,
            "trace_id": trace_id,
            "msg": msg,
            "elapsed_us": elapsed_us,
            "extra": extra,
        });
        serde_json::to_string(&val).unwrap_or_else(|_| "{}".to_string())
    } else if let Some(tid) = trace_id {
        match elapsed_us {
            Some(us) => format!("[{component}][{level}] trace={tid} ({us}us) {msg}"),
            None => format!("[{component}][{level}] trace={tid} {msg}"),
        }
    } else {
        match elapsed_us {
            Some(us) => format!("[{component}][{level}] ({us}us) {msg}"),
            None => format!("[{component}][{level}] {msg}"),
        }
    }
}

/// 输出运行期日志（受 `AGENT_BODY_LOG_FORMAT=json` 控制）
pub fn emit_obs_log(
    level: &str,
    component: &str,
    trace_id: Option<&str>,
    msg: &str,
    elapsed_us: Option<u64>,
    extra: Option<serde_json::Value>,
) {
    let json_mode = is_json_log_mode();
    let line = format_obs_log_line(json_mode, level, component, trace_id, msg, elapsed_us, extra);
    if level.eq_ignore_ascii_case("WARN") || level.eq_ignore_ascii_case("ERROR") {
        eprintln!("{line}");
    } else {
        println!("{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_trace_context_preserves_w3c_and_falls_back_cleanly() {
        let incoming = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        let (tid, tp) = resolve_trace_context(Some(incoming), None);
        assert_eq!(tid, "4bf92f3577b34da6a3ce929d0e0e4736");
        assert!(tp.starts_with("00-4bf92f3577b34da6a3ce929d0e0e4736-"));
        assert_ne!(tp, incoming, "must rotate span_id");

        let (tid2, tp2) = resolve_trace_context(Some("bad-header"), Some("custom-req-99"));
        assert_eq!(tid2, "custom-req-99");
        assert!(tp2.starts_with("00-"));

        let json_line = format_obs_log_line(
            true,
            "INFO",
            "web",
            Some("custom-req-99"),
            "GET /metrics",
            Some(42),
            None,
        );
        let parsed: serde_json::Value = serde_json::from_str(&json_line).expect("valid jsonl");
        assert_eq!(parsed["level"], "INFO");
        assert_eq!(parsed["component"], "web");
        assert_eq!(parsed["trace_id"], "custom-req-99");
        assert_eq!(parsed["elapsed_us"], 42);
    }
}
