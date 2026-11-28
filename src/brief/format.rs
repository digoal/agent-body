//! 简报文本清洗、词边界/反引号优雅截断与历史经验语义去重工具。

const DECORATIVE_CHARS: &[char] = &['=', '-', '*', '_', '~', '─', '═'];
const MAX_BACKTICK_EXTENSION: usize = 24;

/// 清洗连续装饰线（如 `# ======================`、`--------`）并折叠空白为紧凑单行文本
pub(crate) fn strip_decorative_noise(raw: &str) -> String {
    let normalized = raw.replace("\r\n", "\n");
    let mut cleaned_lines: Vec<String> = Vec::new();

    for line in normalized.lines() {
        let stripped = strip_decorative_runs_in_line(line);
        let compact: String = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
        if compact.is_empty() || compact.chars().all(|c| c == '#') {
            continue;
        }
        cleaned_lines.push(compact);
    }

    cleaned_lines.join(" ")
}

fn strip_decorative_runs_in_line(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        if DECORATIVE_CHARS.contains(&ch) {
            let mut j = i + 1;
            while j < chars.len() && chars[j] == ch {
                j += 1;
            }
            if j - i >= 4 {
                out.push(' ');
                i = j;
                continue;
            }
        }
        out.push(ch);
        i += 1;
    }

    out
}

/// 清洗并按自然词/标点边界及反引号跨度优雅截断文本
pub(crate) fn truncate_clean(raw: &str, max_chars: usize) -> String {
    let cleaned = strip_decorative_noise(raw);
    if max_chars == 0 {
        return String::new();
    }

    let chars: Vec<char> = cleaned.chars().collect();
    if chars.len() <= max_chars {
        return cleaned;
    }

    let target = max_chars.saturating_sub(1);
    if target == 0 {
        return "…".to_string();
    }

    // 1. 反引号跨度保护：避免截断在 `...` 内部导致后缀残缺（如 .md -> .m）与语法破损
    let backticks_before = chars[..target].iter().filter(|&&c| c == '`').count();
    let mut cut_idx = target;
    let mut extended_backtick = false;

    if backticks_before % 2 == 1 {
        if let Some(rel_close) = chars[target..]
            .iter()
            .take(MAX_BACKTICK_EXTENSION + 1)
            .position(|&c| c == '`')
        {
            let close_end = target + rel_close + 1;
            if close_end >= chars.len() {
                return cleaned;
            }
            cut_idx = close_end;
            extended_backtick = true;
        } else {
            // 行内代码过长：回退至少 1 个字符留出闭合反引号位
            cut_idx = target.saturating_sub(1).max(1);
        }
    }

    // 2. 若未顺延至闭合反引号，则向后回退到自然词/标点边界，避免劈断英文单词或后缀
    if !extended_backtick {
        cut_idx = find_natural_boundary(&chars, cut_idx);
    }

    // 3. 清理末尾悬空引导符（保留闭合反引号）
    while cut_idx > 1 {
        let prev = chars[cut_idx - 1];
        if !extended_backtick && prev == '`' {
            // 如果正好退到了一个孤立开反引号前，一并修剪
            let bt_count = chars[..cut_idx].iter().filter(|&&c| c == '`').count();
            if bt_count % 2 == 1 {
                cut_idx -= 1;
                continue;
            }
        }
        if prev.is_whitespace() || matches!(prev, '(' | '[' | '{' | ':' | '：' | '-' | ',' | '，' | '、') {
            cut_idx -= 1;
        } else {
            break;
        }
    }

    let mut result: String = chars[..cut_idx].iter().collect();
    if result.chars().filter(|&c| c == '`').count() % 2 == 1 {
        result.push('`');
    }
    result.push('…');
    result
}

fn find_natural_boundary(chars: &[char], target: usize) -> usize {
    if target >= chars.len() {
        return chars.len();
    }
    // 如果截断点后一位本就是空白或分隔符，说明当前正好落在完整词末尾
    if is_soft_delimiter(chars[target]) {
        return target;
    }

    let min_idx = (target * 65) / 100;
    for i in (min_idx..=target).rev() {
        let ch = chars[i];
        if ch.is_whitespace() || is_drop_delimiter(ch) {
            return i;
        }
        if i > min_idx && is_keep_after_delimiter(chars[i - 1]) {
            return i;
        }
    }
    target
}

fn is_soft_delimiter(ch: char) -> bool {
    ch.is_whitespace() || is_drop_delimiter(ch)
}

fn is_drop_delimiter(ch: char) -> bool {
    matches!(ch, ',' | ';' | ':' | '，' | '；' | '：' | '、')
}

fn is_keep_after_delimiter(ch: char) -> bool {
    matches!(ch, ')' | ']' | '}' | '/' | '。' | '！' | '？')
}

/// 生成记忆去重键：`(归一化内容键, 可选语义目标锚点键)`
///
/// 语义锚点用于合并同会话多次 `close` 沉淀及对应 `[sleep_summary:session]`/`[sleep_summary:md]` 摘要。
pub(crate) fn memory_dedup_keys(raw: &str) -> (String, Option<String>) {
    let cleaned = strip_decorative_noise(raw);
    let core = strip_sleep_summary_prefix(&cleaned);

    let norm_key: String = core
        .chars()
        .take(80)
        .collect::<String>()
        .to_lowercase();

    let target_key = extract_session_close_anchor(core)
        .or_else(|| extract_file_output_anchor(core));

    (norm_key, target_key)
}

fn strip_sleep_summary_prefix(text: &str) -> &str {
    let mut cur = text.trim_start();
    while cur.starts_with("[sleep_summary:") {
        if let Some(end) = cur.find(']') {
            if end <= 48 {
                cur = cur[end + 1..].trim_start();
                continue;
            }
        }
        break;
    }
    cur
}

fn extract_session_close_anchor(core: &str) -> Option<String> {
    if !(core.contains("完工环境总结") || core.contains("完工环境归档")) {
        return None;
    }
    let start_marker = "会话 [";
    let start = core.find(start_marker)? + start_marker.len();
    let rest = &core[start..];
    let end = rest.find(']')?;
    let sid = rest[..end].trim();
    if sid.is_empty() {
        return None;
    }
    Some(format!("session_close:{}", sid))
}

fn extract_file_output_anchor(core: &str) -> Option<String> {
    let marker = "产出文件 `";
    let start = core.find(marker)? + marker.len();
    let rest = &core[start..];
    let end = rest.find('`')?;
    let path = rest[..end].trim();
    if path.is_empty() {
        return None;
    }
    Some(format!("file_output:{}", path))
}

/// 按完整行边界将文本裁剪至 `budget_tokens` 以内，避免半截列表项
pub(crate) fn truncate_to_budget_clean(text: String, budget_tokens: usize) -> String {
    let max_chars = budget_tokens * 4;
    if text.chars().count() <= max_chars {
        return text;
    }

    let suffix = "…（已截断）\n";
    let limit = max_chars.saturating_sub(suffix.chars().count());
    let mut out = String::new();
    let mut used_chars = 0usize;

    for line in text.split_inclusive('\n') {
        let line_chars = line.chars().count();
        if used_chars > 0 && used_chars + line_chars > limit {
            break;
        }
        if used_chars == 0 && line_chars > limit {
            let trimmed = truncate_clean(line, limit.max(8));
            out.push_str(&trimmed);
            out.push('\n');
            break;
        }
        out.push_str(line);
        used_chars += line_chars;
    }

    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(suffix);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_decorative_noise_removes_banners_and_newlines() {
        let raw = "产出文件 `/root/new/src/agent-body/config.example.toml` 摘要:\n# ======================\n# agent-body 配置示例\n";
        let cleaned = strip_decorative_noise(raw);
        assert!(!cleaned.contains("==="), "装饰等号应被清理: {}", cleaned);
        assert!(!cleaned.contains('\n'), "应折叠为单行: {}", cleaned);
        assert!(cleaned.contains("# agent-body 配置示例"));
    }

    #[test]
    fn test_truncate_clean_preserves_backtick_path_and_word_boundary() {
        let raw = "[sleep_summary:md] 产出文件 `/root/new/src/agent-body/docs/src_doc/roadmap-phase80.md` 完成了阶段验收与全量测试";
        let res = truncate_clean(raw, 80);
        assert!(
            res.contains("`/root/new/src/agent-body/docs/src_doc/roadmap-phase80.md`"),
            "反引号路径不应被劈断为 .m: {}",
            res
        );
        assert_eq!(res.chars().filter(|&c| c == '`').count() % 2, 0);

        let session_raw = "会话 [7e171592-8786-4328-8bf1-8e5f593a260a] 完工环境总结 (dry_run: false): - 清理/待清理临时文件数: 0";
        let res2 = truncate_clean(session_raw, 65);
        assert!(!res2.contains("dry_ru…"), "英文标识符不应在词中劈断: {}", res2);
    }

    #[test]
    fn test_memory_dedup_keys_unifies_sleep_summary_and_raw_records() {
        let a = "[sleep_summary:session] 会话 [7e171592-8786-4328-8bf1-8e5f593a260a] 完工环境总结 (dry_run: false)";
        let b = "会话 [7e171592-8786-4328-8bf1-8e5f593a260a] 完工环境总结 (dry_run: false):\n- 清理/待清理临时文件数: 0";
        let (_, ka) = memory_dedup_keys(a);
        let (_, kb) = memory_dedup_keys(b);
        assert_eq!(ka, Some("session_close:7e171592-8786-4328-8bf1-8e5f593a260a".to_string()));
        assert_eq!(ka, kb);

        let f1 = "[sleep_summary:md] 产出文件 `/tmp/demo.md` 摘要";
        let f2 = "产出文件 `/tmp/demo.md` 摘要:\n# 标题";
        let (_, kf1) = memory_dedup_keys(f1);
        let (_, kf2) = memory_dedup_keys(f2);
        assert_eq!(kf1, Some("file_output:/tmp/demo.md".to_string()));
        assert_eq!(kf1, kf2);
    }

    #[test]
    fn test_truncate_to_budget_clean_cuts_at_line_boundary() {
        let text = "## 标题\n- 第一条完整内容 abcdefghij\n- 第二条完整内容 klmnopqrst\n- 第三条完整内容 uvwxyz1234\n".to_string();
        let out = truncate_to_budget_clean(text, 15); // 60 chars
        assert!(out.contains("- 第一条完整内容 abcdefghij\n"));
        assert!(!out.contains("第三条"), "超预算行应整行略过: {}", out);
        assert!(out.ends_with("…（已截断）\n"));
    }
}
