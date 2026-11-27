use std::path::Path;
use serde_json::{Map, Value};

/// 敏感文件判定（严禁读取内容送入提取或 Ollama）
pub fn is_sensitive_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let path_str = path.to_string_lossy().to_lowercase();

    if path_str.contains("/.ssh/") || path_str.contains("/.gnupg/") {
        return true;
    }
    if name == ".env" || name.starts_with(".env.") || name == "id_rsa" || name == "id_ed25519" {
        return true;
    }
    if name.ends_with(".pem") || name.ends_with(".key") || name.ends_with(".p12") || name.ends_with(".pfx") {
        return true;
    }
    if name.contains("secret") || name.contains("credential") || name.contains("password") {
        // 允许代码/文档自身讨论脱敏规则，仅拦截数据/配置文件
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        if matches!(ext, "json" | "toml" | "yaml" | "yml" | "ini" | "conf" | "env" | "txt" | "") {
            return true;
        }
    }
    false
}

/// 密钥与敏感凭据脱敏器（覆盖文件摘要、/proc cmdline、记忆与变更流全链路）
pub struct SecretRedactor;

impl SecretRedactor {
    /// 多行脱敏（用于送入本地大模型的文件摘录等，保留文本行结构）
    pub fn redact(input: &str) -> String {
        let mut out = Self::redact_preserving_newlines(input);
        if !input.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out
    }

    /// 单行脱敏（用于 /proc cmdline、摘要、description 等单行字段，不追加多余换行）
    pub fn redact_single_line(input: &str) -> String {
        let _wait = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::CpuSecretRedact);
        let cleaned = Self::redact_private_keys(input);
        Self::redact_line(&cleaned)
    }

    /// 保留原始换行结构的多行脱敏（用于长期记忆 content 等）
    pub fn redact_preserving_newlines(input: &str) -> String {
        if input.is_empty() {
            return String::new();
        }
        let _wait = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::CpuSecretRedact);
        let had_trailing_nl = input.ends_with('\n');
        let pk_cleaned = Self::redact_private_keys(input);
        let lines: Vec<String> = pk_cleaned.lines().map(Self::redact_line).collect();
        let mut out = lines.join("\n");
        if had_trailing_nl && !out.ends_with('\n') {
            out.push('\n');
        }
        out
    }

    /// 递归脱敏 JSON 值中的字符串与敏感键值
    pub fn redact_json_value(val: &Value) -> Value {
        match val {
            Value::String(s) => Value::String(Self::redact_preserving_newlines(s)),
            Value::Array(arr) => Value::Array(arr.iter().map(Self::redact_json_value).collect()),
            Value::Object(map) => {
                let mut out = Map::new();
                for (k, v) in map {
                    let kl = k.to_lowercase();
                    let is_secret_key = kl.contains("password")
                        || kl.contains("passwd")
                        || kl.contains("api_key")
                        || kl.contains("apikey")
                        || kl.contains("secret_key")
                        || kl.contains("access_token")
                        || kl.contains("auth_token")
                        || kl == "token"
                        || kl == "secret";
                    if is_secret_key && (v.is_string() || v.is_number()) {
                        out.insert(k.clone(), Value::String("<REDACTED>".to_string()));
                    } else {
                        out.insert(k.clone(), Self::redact_json_value(v));
                    }
                }
                Value::Object(out)
            }
            other => other.clone(),
        }
    }

    fn redact_line(line: &str) -> String {
        let lower = line.to_ascii_lowercase();
        let trimmed_start = line.trim_start();
        let looks_like_secret_kv = (lower.contains("api_key")
            || lower.contains("apikey")
            || lower.contains("secret_key")
            || lower.contains("access_token")
            || lower.contains("auth_token")
            || lower.contains("password")
            || lower.contains("passwd"))
            && (line.contains('=') || line.contains(':'))
            && !trimmed_start.starts_with("//")
            && !trimmed_start.starts_with('#')
            // CLI 命令行（含空格与 `--flag`）交由内联参数脱敏器精准替换参数值，避免整行截断丢失命令上下文
            && !lower.contains("--password")
            && !lower.contains("--passwd")
            && !lower.contains("--token")
            && !lower.contains("://");

        if looks_like_secret_kv {
            if let Some(idx) = line.find('=').or_else(|| line.find(':')) {
                let key_part = &line[..=idx];
                return format!("{} \"<REDACTED>\"", key_part);
            }
        }

        Self::mask_inline_tokens(line)
    }

    fn redact_private_keys(input: &str) -> String {
        let mut result = input.to_string();
        while let Some(begin_idx) = result.find("-----BEGIN ") {
            let rest = &result[begin_idx..];
            let Some(header_end_rel) = rest.find("PRIVATE KEY-----") else {
                break;
            };
            let after_header = begin_idx + header_end_rel + "PRIVATE KEY-----".len();
            if let Some(end_begin_rel) = result[after_header..].find("-----END ") {
                let end_pos = after_header + end_begin_rel;
                if let Some(end_finish_rel) = result[end_pos..].find("PRIVATE KEY-----") {
                    let replace_end = end_pos + end_finish_rel + "PRIVATE KEY-----".len();
                    result.replace_range(begin_idx..replace_end, "<REDACTED_PRIVATE_KEY>");
                    continue;
                }
            }
            // 未找到闭合 END 标记（如单行/截断内容），替换至行尾
            let line_end = result[after_header..]
                .find('\n')
                .map(|o| after_header + o)
                .unwrap_or(result.len());
            result.replace_range(begin_idx..line_end, "<REDACTED_PRIVATE_KEY>");
        }
        result
    }

    fn mask_inline_tokens(line: &str) -> String {
        let mut result = Self::mask_uri_credentials(line);
        result = Self::mask_cli_flags(&result);
        result = Self::mask_bearer_tokens(&result);

        for prefix in &["sk-", "ghp_", "gho_", "glpat-", "xoxb-", "xoxp-", "AKIA"] {
            let mut search_from = 0usize;
            while search_from < result.len() {
                let Some(rel_pos) = result[search_from..].find(prefix) else {
                    break;
                };
                let pos = search_from + rel_pos;
                let end = result[pos..]
                    .find(|c: char| {
                        c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ';' | ')' | '>')
                    })
                    .map(|off| pos + off)
                    .unwrap_or(result.len());
                if end - pos >= 12 {
                    result.replace_range(pos..end, "<REDACTED_TOKEN>");
                    search_from = pos + "<REDACTED_TOKEN>".len();
                } else {
                    search_from = pos + prefix.len();
                }
            }
        }
        result
    }

    /// 脱敏 URI 内嵌凭据: scheme://user:password@host -> scheme://user:<REDACTED>@host
    fn mask_uri_credentials(input: &str) -> String {
        let mut result = input.to_string();
        let mut search_from = 0usize;
        while search_from < result.len() {
            let Some(rel) = result[search_from..].find("://") else {
                break;
            };
            let auth_start = search_from + rel + 3;
            let token_end = result[auth_start..]
                .find(|c: char| {
                    c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ';' | ')' | '>')
                })
                .map(|off| auth_start + off)
                .unwrap_or(result.len());
            let segment = &result[auth_start..token_end];
            if let Some(at_rel) = segment.find('@') {
                // 不跨越路径分隔符 `/`
                if !segment[..at_rel].contains('/') {
                    if let Some(colon_rel) = segment[..at_rel].find(':') {
                        let pwd_start = auth_start + colon_rel + 1;
                        let pwd_end = auth_start + at_rel;
                        let pwd = &result[pwd_start..pwd_end];
                        if !pwd.is_empty() && pwd != "<REDACTED>" && pwd != "***" {
                            result.replace_range(pwd_start..pwd_end, "<REDACTED>");
                            search_from = pwd_start + "<REDACTED>".len();
                            continue;
                        }
                    }
                }
            }
            search_from = auth_start;
        }
        result
    }

    /// 脱敏 CLI 敏感标志参数: --password=xxx / --password xxx / --token=xxx 等
    fn mask_cli_flags(input: &str) -> String {
        let flags = [
            "--password",
            "--passwd",
            "--secret-key",
            "--access-token",
            "--auth-token",
            "--api-key",
            "--apikey",
            "--token",
            "--secret",
        ];
        let mut result = input.to_string();
        for flag in flags {
            let mut search_from = 0usize;
            while search_from < result.len() {
                let lower = result.to_ascii_lowercase();
                let Some(rel) = lower[search_from..].find(flag) else {
                    break;
                };
                let pos = search_from + rel;
                // 确认前边界（行首、空白或引号）
                let prev_ok = pos == 0
                    || result[..pos]
                        .chars()
                        .next_back()
                        .map(|c| c.is_whitespace() || c == '"' || c == '\'')
                        .unwrap_or(true);
                let after_flag = pos + flag.len();
                if !prev_ok || after_flag >= result.len() {
                    search_from = after_flag;
                    continue;
                }

                let rest = &result[after_flag..];
                if rest.starts_with('=') {
                    let val_start = after_flag + 1;
                    let val_end = Self::scan_cli_value_end(&result, val_start);
                    if val_end > val_start && &result[val_start..val_end] != "<REDACTED>" {
                        result.replace_range(val_start..val_end, "<REDACTED>");
                        search_from = val_start + "<REDACTED>".len();
                        continue;
                    }
                } else if rest.starts_with(' ') || rest.starts_with('\t') {
                    let ws_len = rest.len() - rest.trim_start_matches([' ', '\t']).len();
                    let val_start = after_flag + ws_len;
                    if val_start < result.len() && !result[val_start..].starts_with('-') {
                        let val_end = Self::scan_cli_value_end(&result, val_start);
                        if val_end > val_start && &result[val_start..val_end] != "<REDACTED>" {
                            result.replace_range(val_start..val_end, "<REDACTED>");
                            search_from = val_start + "<REDACTED>".len();
                            continue;
                        }
                    }
                }
                search_from = after_flag;
            }
        }
        result
    }

    fn scan_cli_value_end(s: &str, val_start: usize) -> usize {
        if val_start >= s.len() {
            return val_start;
        }
        let slice = &s[val_start..];
        if let Some(quote) = slice.chars().next().filter(|&c| c == '"' || c == '\'') {
            if let Some(close_rel) = slice[1..].find(quote) {
                return val_start + 1 + close_rel + 1;
            }
        }
        slice
            .find(|c: char| c.is_whitespace() || c == ',' || c == ';')
            .map(|off| val_start + off)
            .unwrap_or(s.len())
    }

    /// 脱敏 Authorization Bearer 令牌（含 JWT eyJ...）
    fn mask_bearer_tokens(input: &str) -> String {
        let mut result = input.to_string();
        let mut search_from = 0usize;
        while search_from < result.len() {
            let lower = result.to_ascii_lowercase();
            let Some(rel) = lower[search_from..].find("bearer ") else {
                break;
            };
            let pos = search_from + rel;
            let token_start = pos + "bearer ".len();
            let trimmed_ws = result[token_start..].len() - result[token_start..].trim_start().len();
            let actual_start = token_start + trimmed_ws;
            let token_end = result[actual_start..]
                .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ';'))
                .map(|off| actual_start + off)
                .unwrap_or(result.len());
            if token_end > actual_start
                && token_end - actual_start >= 8
                && !result[actual_start..token_end].starts_with("<REDACTED")
            {
                result.replace_range(actual_start..token_end, "<REDACTED_TOKEN>");
                search_from = actual_start + "<REDACTED_TOKEN>".len();
            } else {
                search_from = token_end.max(token_start);
            }
        }
        result
    }
}

/// 间接提示词注入检测与清洗结果
#[derive(Debug, Clone)]
pub struct PromptScanResult {
    pub suspected: bool,
    pub matched_patterns: Vec<&'static str>,
    pub sanitized: String,
}

/// 外部语料与记忆抗投毒防线（微秒级确定性规则，防范 Indirect Prompt Injection）
pub struct PromptInjectionScanner;

impl PromptInjectionScanner {
    /// 判断是否为不可见零宽或双向覆盖控制字符
    fn is_suspicious_invisible_char(c: char) -> bool {
        matches!(
            c,
            '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}'
        )
    }

    /// 扫描并清洗可疑间接提示词注入指令与隐藏控制字符
    pub fn scan_and_sanitize(input: &str) -> PromptScanResult {
        let mut had_invisible = false;
        let mut cleaned = String::with_capacity(input.len());
        for ch in input.chars() {
            if Self::is_suspicious_invisible_char(ch) {
                had_invisible = true;
            } else {
                cleaned.push(ch);
            }
        }

        let lower = cleaned.to_ascii_lowercase();
        let injection_rules: &[(&str, &str)] = &[
            ("ignore previous instructions", "ignore_previous_instructions"),
            ("ignore all previous", "ignore_all_previous"),
            ("disregard previous", "disregard_previous"),
            ("disregard all prior", "disregard_all_prior"),
            ("forget previous instructions", "forget_previous_instructions"),
            ("override system prompt", "override_system_prompt"),
            ("new system instructions", "new_system_instructions"),
            ("<system>", "fake_system_tag"),
            ("</system>", "fake_system_close_tag"),
            ("<|im_start|>system", "fake_chatml_system"),
            ("act as an unrestricted", "unrestricted_roleplay"),
            ("忽略之前的所有指令", "cn_ignore_previous"),
            ("无视上述规则", "cn_disregard_rules"),
            ("覆写系统提示词", "cn_override_system"),
        ];

        let mut matched = Vec::new();
        if had_invisible {
            matched.push("hidden_unicode_bidi_or_zero_width");
        }
        for (pat, label) in injection_rules {
            if lower.contains(pat) {
                matched.push(*label);
            }
        }

        let suspected = !matched.is_empty();
        let sanitized = if suspected {
            let escaped = cleaned
                .replace("<system>", "[system]")
                .replace("</system>", "[/system]")
                .replace("<|im_start|>", "[im_start]");
            format!("[EXTERNAL_UNTRUSTED_TEXT] {}", escaped.trim())
        } else {
            cleaned
        };

        PromptScanResult {
            suspected,
            matched_patterns: matched,
            sanitized,
        }
    }
}
