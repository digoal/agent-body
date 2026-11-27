use std::fs;
use std::io::Read;
use std::path::Path;
use crate::storage::models::SubGraph;
use super::property_graph::PropertyGraphOrganizer;

impl PropertyGraphOrganizer {
    /// 将子图格式化为适合 Agent 阅读的结构化 Markdown 与 Mermaid 关系图
    pub fn format_graph_report(&self, subgraph: &SubGraph) -> String {
        let mut out = String::new();
        out.push_str("# 🕸️ 宿主机实体属性图关联透视 (Property Graph)\n\n");

        if subgraph.nodes.is_empty() {
            out.push_str("未检索到与查询匹配的实体图节点。可尝试执行全局事实构建或换用项目名/端口号/服务名重新检索。\n");
            return out;
        }

        if let Some(ref root) = subgraph.root_node {
            out.push_str(&format!("**根探测节点**: `{}` ({}) - 类型: `{}`\n\n", root.name, root.label, root.node_type));
        }

        out.push_str(&format!("- **关联节点总数**: {}\n", subgraph.nodes.len()));
        out.push_str(&format!("- **实体关联边数**: {}\n\n", subgraph.edges.len()));

        out.push_str("### 📋 关联实体节点清单\n\n");
        out.push_str("| 节点 ID | 类型 | 实体名称 | 标签与语义 |\n");
        out.push_str("|---------|------|----------|------------|\n");
        for n in &subgraph.nodes {
            out.push_str(&format!("| `{}` | `{}` | `{}` | {} |\n", n.id, n.node_type, n.name, n.label));
        }
        out.push('\n');

        if !subgraph.edges.is_empty() {
            out.push_str("### 🔗 关系依赖拓扑 (含证据层级与权重)\n\n");
            out.push_str("| 源实体 | 关系类型 | 目标实体 | 证据层级 | 权重 | 方向语义 |\n");
            out.push_str("|--------|:--------:|----------|:--------:|:----:|:--------:|\n");
            for e in &subgraph.edges {
                out.push_str(&format!(
                    "| `{}` | **-- {} -->** | `{}` | `{}` | {:.2} | `{}` |\n",
                    e.source_id, e.relation, e.target_id, e.evidence_tier, e.weight, e.direction_kind
                ));
            }
            out.push('\n');
        }

        out.push_str("### 📊 可视化拓扑关系图 (Mermaid)\n\n");
        out.push_str("```mermaid\n");
        out.push_str(&self.render_mermaid(subgraph));
        out.push_str("\n```\n");

        out
    }

    /// 将子图渲染为规整、美观、带有中文注释与安全框线的 Mermaid 流程图
    pub fn render_mermaid(&self, subgraph: &SubGraph) -> String {
        let mut lines = Vec::new();
        lines.push("flowchart LR".to_string());

        // 节点声明与样式
        for n in &subgraph.nodes {
            let clean_id = sanitize_mermaid_id(&n.id);
            let clean_label = n.name.replace(['"', '(', ')'], "");
            let shape = match n.node_type.as_str() {
                "project" => format!("[\"📦 {}\"]", clean_label),
                "service" => format!("([\"⚙️ {}\"])", clean_label),
                "port" => format!("{{\"🔌 端口:{}\"}}", clean_label),
                "cron" => format!("{{\"⏰ {}\"}}", clean_label),
                "tool" => format!("[\"🛠️ {}\"]", clean_label),
                "file" => format!("[\"📄 {}\"]", clean_label),
                _ => format!("[\"{}\"]", clean_label),
            };
            lines.push(format!("    {}{}", clean_id, shape));
        }

        // 边关系（L3 隐式语义使用虚线箭头 -.-> 区分物理/结构硬依赖）
        for e in &subgraph.edges {
            let src = sanitize_mermaid_id(&e.source_id);
            let tgt = sanitize_mermaid_id(&e.target_id);
            let is_semantic = e.evidence_tier == "L3_SEMANTIC";
            let label = match e.relation.as_str() {
                "listens_on" => "监听",
                "depends_on" => "依赖",
                "scheduled_by" => "调度",
                "part_of" => "归属",
                "reads" => "读取",
                "writes" => "写入",
                "uses_tool" => "使用工具",
                "runs_as" => "运行实例",
                "documented_by" => "衍生文档",
                "semantically_related" => "语义关联",
                other => other,
            };
            let arrow = if is_semantic {
                format!("-.->|\"{} {:.2}\"|", label, e.weight)
            } else {
                format!("-->|\"{} {:.2}\"|", label, e.weight)
            };
            lines.push(format!("    {} {} {}", src, arrow, tgt));
        }

        lines.join("\n")
    }
}

pub(crate) fn sanitize_mermaid_id(raw: &str) -> String {
    raw.replace([':', '.', '/', '-', '@'], "_")
}

pub(crate) fn infer_placement_zone(abs_path: &str) -> &'static str {
    let base = crate::config::default_workspace_base();
    let b = base.to_string_lossy().trim_end_matches('/').to_string();
    let norm = abs_path.replace('\\', "/");
    if norm.starts_with(&format!("{}/shared_proj/", b)) || norm.contains("/shared_proj/") {
        "shared_proj"
    } else if norm.starts_with(&format!("{}/shared_git/", b)) || norm.contains("/shared_git/") {
        "shared_git"
    } else if norm.starts_with(&format!("{}/src/", b)) || norm.contains("/src/") {
        "src"
    } else if norm.starts_with(&format!("{}/work/", b)) || norm.contains("/work/") {
        "work"
    } else {
        "other"
    }
}

pub(crate) fn extract_project_from_path(path: &str) -> Option<String> {
    let norm = path.replace('\\', "/");
    let cfg = crate::config::Config::load_effective_or_default();
    for root in cfg.effective_workspace_roots() {
        let r_norm = root.to_string_lossy().replace('\\', "/");
        let r_prefix = format!("{}/", r_norm.trim_end_matches('/'));
        if let Some(rest) = norm.strip_prefix(&r_prefix) {
            let name = rest.split('/').next().unwrap_or("");
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    let b = cfg
        .effective_workspace_base()
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string();
    for sub in ["work", "shared_proj", "src", "shared_git"] {
        let prefix = format!("{}/{}/", b, sub);
        if let Some(rest) = norm.strip_prefix(&prefix) {
            let name = rest.split('/').next().unwrap_or("");
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

pub(crate) fn probe_git_remote(repo_dir: &Path) -> Option<String> {
    let cfg = fs::read_to_string(repo_dir.join(".git").join("config")).ok()?;
    let mut in_origin = false;
    for line in cfg.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_origin = t.contains("remote \"origin\"");
            continue;
        }
        if in_origin {
            if let Some(url) = t.strip_prefix("url =") {
                return Some(url.trim().to_string());
            }
        }
    }
    None
}

pub(crate) fn probe_project_readme_summary(repo_dir: &Path) -> Option<String> {
    for name in &["README.md", "README_zh.md", "README.rst", "README.txt"] {
        let p = repo_dir.join(name);
        if let Ok(content) = fs::read_to_string(p) {
            for line in content.lines().take(25) {
                let t = line.trim();
                if t.is_empty() || t.starts_with("![") || t.starts_with("[![") || t.starts_with("---") || t.starts_with('<') {
                    continue;
                }
                let clean = t.trim_start_matches('#').trim_start_matches('>').trim();
                if clean.chars().count() >= 6 {
                    return Some(clean.chars().take(90).collect());
                }
            }
        }
    }
    None
}

pub(crate) fn probe_top_level_entries(repo_dir: &Path, limit: usize) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(repo_dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.starts_with('.') {
                out.push(name);
            }
        }
    }
    out.sort();
    out.truncate(limit);
    out
}

pub(crate) fn describe_well_known_port(port: u16, proc_name: Option<&str>) -> Option<&'static str> {
    if let Some(pn) = proc_name {
        if pn.contains("agent-body") {
            return Some("Agent Body Web 监控大屏与 REST API");
        }
        if pn.contains("ollama") {
            return Some("Ollama 本地大模型推理与 Embedding 向量服务");
        }
        if pn.contains("postgres") {
            return Some("PostgreSQL 关系型与向量数据库服务");
        }
    }
    match port {
        22 => Some("OpenSSH 远程安全登录服务"),
        80 | 443 => Some("HTTP/HTTPS Web 入口服务"),
        5432 => Some("PostgreSQL 数据库监听端口"),
        6379 => Some("Redis 内存缓存服务"),
        8756 => Some("Agent Body Web 监控大屏"),
        11434 => Some("Ollama 本地 LLM 与 Embedding 服务"),
        _ => None,
    }
}

pub(crate) fn describe_cron_schedule(expr: &str) -> String {
    let trimmed = expr.trim();
    if trimmed.starts_with('@') {
        return match trimmed {
            "@hourly" => "每小时执行一次".to_string(),
            "@daily" | "@midnight" => "每天 00:00 执行".to_string(),
            "@weekly" => "每周执行一次".to_string(),
            "@monthly" => "每月执行一次".to_string(),
            "@reboot" => "系统启动时执行".to_string(),
            _ => format!("周期调度 ({})", trimmed),
        };
    }
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() == 5 {
        let (min, hour, dom, mon, dow) = (parts[0], parts[1], parts[2], parts[3], parts[4]);
        if let Some(step) = min.strip_prefix("*/") {
            if hour == "*" && dom == "*" && mon == "*" && dow == "*" {
                return format!("每 {} 分钟执行", step);
            }
        }
        if min.chars().all(|c| c.is_ascii_digit()) && hour == "*" && dom == "*" && mon == "*" && dow == "*" {
            return format!("每小时第 {} 分执行", min);
        }
        if min.chars().all(|c| c.is_ascii_digit()) && hour.chars().all(|c| c.is_ascii_digit()) && dom == "*" && mon == "*" && dow == "*" {
            return format!("每天 {:0>2}:{:0>2} 执行", hour, min);
        }
    }
    format!("Cron 表达式 ({})", trimmed)
}

pub(crate) fn which_binary(cmd: &str) -> Option<String> {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in path_var.split(':') {
            let cand = Path::new(dir).join(cmd);
            if cand.is_file() {
                return Some(cand.to_string_lossy().to_string());
            }
        }
    }
    None
}

pub(crate) fn detect_system_file_kind(path: &Path, is_dir: bool, is_executable: bool, ext: &str) -> String {
    if is_dir {
        return "directory".to_string();
    }
    if let Ok(mut f) = fs::File::open(path) {
        let mut buf = [0u8; 4];
        if f.read(&mut buf).unwrap_or(0) == 4 && buf == [0x7f, b'E', b'L', b'F'] {
            return "elf_binary".to_string();
        }
    }
    match ext {
        "py" => "python_script".to_string(),
        "sh" | "bash" => "shell_script".to_string(),
        "md" => "markdown_doc".to_string(),
        "toml" | "json" | "yaml" | "yml" | "conf" | "ini" | "service" => "config_file".to_string(),
        _ if is_executable => "executable_script".to_string(),
        _ => "system_file".to_string(),
    }
}

pub(crate) fn file_kind_zh(kind: &str) -> &'static str {
    match kind {
        "elf_binary" => "ELF 可执行二进制",
        "python_script" => "Python 脚本",
        "shell_script" => "Shell 脚本",
        "markdown_doc" => "Markdown 文档",
        "config_file" => "系统配置文件",
        "executable_script" => "可执行脚本",
        _ => "系统文件",
    }
}

