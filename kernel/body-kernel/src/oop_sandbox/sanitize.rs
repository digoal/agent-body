//! 轨 B 沙盒提示词注入清洗辅助（过滤零宽字符及常见注入标签）

/// 递归清洗 JSON 载荷中的零宽隐藏字符及常见提示词注入标签
pub fn sanitize_prompt_injection(val: &mut serde_json::Value) -> bool {
    let mut changed = false;
    match val {
        serde_json::Value::String(s) => {
            let cleaned = clean_injection_string(s);
            if cleaned != *s {
                *s = cleaned;
                changed = true;
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                if sanitize_prompt_injection(item) {
                    changed = true;
                }
            }
        }
        serde_json::Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                if sanitize_prompt_injection(v) {
                    changed = true;
                }
            }
        }
        _ => {}
    }
    changed
}

fn clean_injection_string(input: &str) -> String {
    let mut no_zw: String = input
        .chars()
        .filter(|&c| !is_zero_width_or_bidi(c))
        .collect();
    const TAGS: &[&str] = &[
        "<system>",
        "</system>",
        "<|im_start|>",
        "<|im_end|>",
        "<|endoftext|>",
        "<instructions>",
        "</instructions>",
        "[INST]",
        "[/INST]",
        "<<SYS>>",
        "<</SYS>>",
        "IGNORE PREVIOUS INSTRUCTIONS",
    ];
    for tag in TAGS {
        no_zw = replace_case_insensitive(&no_zw, tag, "[FILTERED_TAG]");
    }
    no_zw
}

fn is_zero_width_or_bidi(c: char) -> bool {
    matches!(
        c,
        '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2064}' | '\u{FEFF}'
    )
}

fn replace_case_insensitive(haystack: &str, needle: &str, replacement: &str) -> String {
    let lower_hay = haystack.to_ascii_lowercase();
    let lower_needle = needle.to_ascii_lowercase();
    if !lower_hay.contains(&lower_needle) {
        return haystack.to_string();
    }
    let mut out = String::with_capacity(haystack.len());
    let mut last = 0usize;
    let mut search_from = 0usize;
    while let Some(rel) = lower_hay[search_from..].find(&lower_needle) {
        let start = search_from + rel;
        let end = start + lower_needle.len();
        if haystack.is_char_boundary(start) && haystack.is_char_boundary(end) {
            out.push_str(&haystack[last..start]);
            out.push_str(replacement);
            last = end;
        }
        search_from = end;
    }
    out.push_str(&haystack[last..]);
    out
}
