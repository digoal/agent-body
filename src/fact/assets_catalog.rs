use anyhow::Result;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// 宿主机代码仓库客观事实实体
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoFact {
    /// 仓库目录名（如 duckdb, mem0, agent-body）
    pub name: String,
    /// 物理绝对路径（如 /root/new/src/duckdb）
    pub abs_path: String,
    /// 是否为标准 Git 仓库
    pub is_git_repo: bool,
    /// 当前活跃 Git 分支名
    pub current_branch: Option<String>,
    /// 是否已构建 CodeGraph 代码语义图谱 (.codegraph 就绪)
    pub codegraph_ready: bool,
    /// 探测到的主要开发语言/技术栈 (Rust, C/C++, Python, Go, Node, Unknown)
    pub primary_lang: Option<String>,
    /// 面向 Agent 的代码探索操作指引
    pub recommendation: String,
}

/// 宿主机本地权威知识与核心笔记事实实体
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocFact {
    /// 文档标题或文件名
    pub title: String,
    /// 物理绝对路径
    pub abs_path: String,
    /// 知识分类 ("authority_list" | "troubleshooting" | "research" | "general")
    pub category: String,
    /// 文件体积字节数
    pub size_bytes: u64,
    /// 最近更新时间
    pub updated_at: String,
    /// 核心主题或第一行摘要
    pub summary: String,
}

/// 源码与资料资产总览
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogOverview {
    pub total_repos: usize,
    pub repos: Vec<RepoFact>,
    pub total_documents: usize,
    pub documents: Vec<DocFact>,
}

/// 源码工程与知识资料编目器
pub struct AssetsCatalogEngine;

impl AssetsCatalogEngine {
    /// 扫描宿主机核心源码目录，编目所有源码工程及其 CodeGraph 状态
    pub fn scan_repos(custom_roots: Option<&[&str]>) -> Result<Vec<RepoFact>> {
        let cfg_roots: Vec<String> = crate::config::Config::load_effective_or_default()
            .effective_workspace_roots()
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        let owned_roots: Vec<String> = match custom_roots {
            Some(r) => r.iter().map(|s| (*s).to_string()).collect(),
            None => cfg_roots,
        };
        let mut repos = Vec::new();

        for root in &owned_roots {
            let root_path = Path::new(root);
            if !root_path.exists() || !root_path.is_dir() {
                continue;
            }

            if let Ok(entries) = fs::read_dir(root_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        // 过滤隐藏目录
                        if name.starts_with('.') {
                            continue;
                        }

                        let is_git = path.join(".git").exists();
                        let codegraph_ready = path.join(".codegraph").exists();
                        let current_branch = if is_git {
                            Self::probe_git_branch(&path)
                        } else {
                            None
                        };
                        let primary_lang = Self::detect_primary_language(&path);

                        let recommendation = if codegraph_ready {
                            format!("✨ 该仓库已构建 CodeGraph 代码语义地图！探索源码时【必须优先调用 codegraph mcp 工具】，禁止盲目使用全局 grep。")
                        } else if is_git {
                            format!("该仓库已就绪 Git。若需深入研读架构，建议在根目录下执行 `codegraph init` 构建代码地图。")
                        } else {
                            format!("普通源码工程目录。")
                        };

                        repos.push(RepoFact {
                            name,
                            abs_path: path.to_string_lossy().to_string(),
                            is_git_repo: is_git,
                            current_branch,
                            codegraph_ready,
                            primary_lang,
                            recommendation,
                        });
                    }
                }
            }
        }

        repos.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(repos)
    }

    /// 扫描宿主机核心资料目录，编目权威网站清单与重要技术沉淀
    pub fn scan_documents(custom_roots: Option<&[&str]>) -> Result<Vec<DocFact>> {
        let cfg_roots: Vec<String> = crate::config::Config::load_effective_or_default()
            .effective_notes_dirs()
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        let owned_roots: Vec<String> = match custom_roots {
            Some(r) => r.iter().map(|s| (*s).to_string()).collect(),
            None => cfg_roots,
        };
        let mut docs = Vec::new();

        for root in &owned_roots {
            let root_path = Path::new(root);
            if !root_path.exists() || !root_path.is_dir() {
                continue;
            }

            if let Ok(entries) = fs::read_dir(root_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        if !name.ends_with(".md") && !name.ends_with(".markdown") {
                            continue;
                        }

                        let category = if name.contains("权威网站") || name.contains("网站清单") {
                            "authority_list".to_string()
                        } else if name.contains("踩坑") || name.contains("排查") || name.contains("手册") {
                            "troubleshooting".to_string()
                        } else if name.contains("research") || name.contains("调研") || name.contains("解读") {
                            "research".to_string()
                        } else {
                            "general".to_string()
                        };

                        let metadata = fs::metadata(&path).ok();
                        let size_bytes = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                        let updated_at = metadata
                            .and_then(|m| m.modified().ok())
                            .map(|time| {
                                let dt: DateTime<Local> = time.into();
                                dt.format("%Y-%m-%d %H:%M").to_string()
                            })
                            .unwrap_or_else(|| "-".to_string());

                        let summary = Self::extract_document_summary(&path);

                        docs.push(DocFact {
                            title: name,
                            abs_path: path.to_string_lossy().to_string(),
                            category,
                            size_bytes,
                            updated_at,
                            summary,
                        });
                    }
                }
            }
        }

        // 权威清单排在最前面，其他按修改时间倒序
        docs.sort_by(|a, b| {
            if a.category == "authority_list" && b.category != "authority_list" {
                std::cmp::Ordering::Less
            } else if b.category == "authority_list" && a.category != "authority_list" {
                std::cmp::Ordering::Greater
            } else {
                b.updated_at.cmp(&a.updated_at)
            }
        });

        Ok(docs)
    }

    /// 根据关键词模糊检索已编目的本地源码工程
    pub fn query_repos(keyword: &str, custom_roots: Option<&[&str]>) -> Result<Vec<RepoFact>> {
        let all_repos = Self::scan_repos(custom_roots)?;
        let kw = keyword.to_lowercase();
        let matched: Vec<RepoFact> = all_repos
            .into_iter()
            .filter(|r| {
                r.name.to_lowercase().contains(&kw)
                    || r.abs_path.to_lowercase().contains(&kw)
                    || r.primary_lang.as_deref().unwrap_or("").to_lowercase().contains(&kw)
            })
            .collect();
        Ok(matched)
    }

    /// 根据关键词模糊检索已编目的本地资料与权威清单
    pub fn query_documents(keyword: &str, custom_roots: Option<&[&str]>) -> Result<Vec<DocFact>> {
        let all_docs = Self::scan_documents(custom_roots)?;
        let kw = keyword.to_lowercase();
        let matched: Vec<DocFact> = all_docs
            .into_iter()
            .filter(|d| {
                d.title.to_lowercase().contains(&kw)
                    || d.summary.to_lowercase().contains(&kw)
                    || d.category.contains(&kw)
            })
            .collect();
        Ok(matched)
    }

    /// 读取 Git 仓库的当前分支名称
    fn probe_git_branch(repo_dir: &Path) -> Option<String> {
        let head_file = repo_dir.join(".git").join("HEAD");
        if let Ok(content) = fs::read_to_string(head_file) {
            let trimmed = content.trim();
            if let Some(ref_path) = trimmed.strip_prefix("ref: refs/heads/") {
                return Some(ref_path.to_string());
            } else if trimmed.len() >= 7 {
                return Some(trimmed[..7].to_string()); // Detached HEAD SHA
            }
        }
        None
    }

    /// 探测项目的主要开发语言与工程类型
    fn detect_primary_language(dir: &Path) -> Option<String> {
        if dir.join("Cargo.toml").exists() {
            Some("Rust".to_string())
        } else if dir.join("go.mod").exists() {
            Some("Go".to_string())
        } else if dir.join("pyproject.toml").exists() || dir.join("setup.py").exists() || dir.join("requirements.txt").exists() {
            Some("Python".to_string())
        } else if dir.join("package.json").exists() {
            Some("Node/TypeScript".to_string())
        } else if dir.join("CMakeLists.txt").exists() || dir.join("Makefile").exists() {
            Some("C/C++".to_string())
        } else if dir.join("pom.xml").exists() || dir.join("build.gradle").exists() {
            Some("Java".to_string())
        } else {
            None
        }
    }

    /// 提取文档前若干行的核心摘要
    fn extract_document_summary(file_path: &Path) -> String {
        if let Ok(content) = fs::read_to_string(file_path) {
            for line in content.lines().take(15) {
                let trimmed = line.trim();
                if trimmed.starts_with("# ") {
                    return trimmed.trim_start_matches('#').trim().to_string();
                } else if trimmed.starts_with("> ") {
                    return trimmed.trim_start_matches('>').trim().to_string();
                }
            }
        }
        file_path.file_name().unwrap_or_default().to_string_lossy().to_string()
    }

    /// 格式化为面向 Agent 的 Markdown 报告
    pub fn format_catalog_overview(repos: &[RepoFact], docs: &[DocFact]) -> String {
        let mut out = String::new();
        out.push_str("📚 **宿主机本地源码工程与权威知识库全景编目**\n\n");

        out.push_str(&format!("### 1. 本地核心源码工程 (已编目 {} 个仓库)\n\n", repos.len()));
        if repos.is_empty() {
            out.push_str("未发现源码仓库。\n\n");
        } else {
            out.push_str("| 仓库名称 | 技术栈 | 分支 | CodeGraph 地图 | 物理路径 |\n");
            out.push_str("|:---|:---:|:---:|:---:|:---|\n");
            for r in repos.iter().take(20) {
                let lang = r.primary_lang.as_deref().unwrap_or("-");
                let branch = r.current_branch.as_deref().unwrap_or("-");
                let cg_icon = if r.codegraph_ready { "✅ 就绪" } else { "未构建" };
                out.push_str(&format!(
                    "| `{}` | {} | `{}` | {} | `{}` |\n",
                    r.name, lang, branch, cg_icon, r.abs_path
                ));
            }
            if repos.len() > 20 {
                out.push_str(&format!("*... 另有 {} 个本地仓库已建立索引*\n\n", repos.len() - 20));
            } else {
                out.push_str("\n");
            }
        }

        out.push_str(&format!("### 2. 权威网站清单与重要资料库 (已编目 {} 份文档)\n\n", docs.len()));
        if docs.is_empty() {
            out.push_str("未发现资料文档。\n");
        } else {
            out.push_str("| 文档名称 | 分类 | 更新时间 | 体积 | 核心主题 |\n");
            out.push_str("|:---|:---:|:---:|:---:|:---|\n");
            for d in docs.iter().take(15) {
                let cat_label = match d.category.as_str() {
                    "authority_list" => "🌟 权威清单",
                    "troubleshooting" => "🛠️ 排障踩坑",
                    "research" => "🔬 深度调研",
                    _ => "📄 通用笔记",
                };
                let size_kb = d.size_bytes / 1024;
                out.push_str(&format!(
                    "| `{}` | {} | {} | {} KB | {} |\n",
                    d.title, cat_label, d.updated_at, size_kb, d.summary
                ));
            }
            if docs.len() > 15 {
                out.push_str(&format!("*... 另有 {} 份本地资料库文档已编目*\n", docs.len() - 15));
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_repos_real_host() {
        let repos = AssetsCatalogEngine::scan_repos(None).expect("扫描本地源码应该成功");
        assert!(!repos.is_empty(), "宿主机上应该存在已克隆的源码仓库");
        println!("扫描到宿主机源码仓库数量: {}", repos.len());

        // 验证当前 agent-body 或 duckdb 是否被探测到
        let has_steward = repos.iter().any(|r| r.name == "agent-body");
        println!("是否包含 agent-body 仓库: {}", has_steward);
    }

    #[test]
    fn test_scan_documents_real_host() {
        let docs = AssetsCatalogEngine::scan_documents(None).expect("扫描本地笔记应该成功");
        assert!(!docs.is_empty(), "宿主机 /root/new/note 应该存在笔记文档");
        println!("扫描到本地重要文档数量: {}", docs.len());

        // 验证是否优先排布权威网站清单
        let has_authority = docs.iter().any(|d| d.title.contains("权威网站"));
        assert!(has_authority, "应该包含权威网站清单");
    }

    #[test]
    fn test_query_repos_and_documents() {
        let matched_repos = AssetsCatalogEngine::query_repos("steward", None).expect("查询仓库成功");
        assert!(!matched_repos.is_empty());

        let matched_docs = AssetsCatalogEngine::query_documents("权威网站", None).expect("查询文档成功");
        assert!(!matched_docs.is_empty());
    }

    #[test]
    fn test_format_catalog_overview() {
        let dummy_repos = vec![RepoFact {
            name: "test-repo".to_string(),
            abs_path: "/root/new/src/test-repo".to_string(),
            is_git_repo: true,
            current_branch: Some("main".to_string()),
            codegraph_ready: true,
            primary_lang: Some("Rust".to_string()),
            recommendation: "CodeGraph 已就绪".to_string(),
        }];

        let dummy_docs = vec![DocFact {
            title: "全领域权威网站.md".to_string(),
            abs_path: "/root/new/note/全领域权威网站.md".to_string(),
            category: "authority_list".to_string(),
            size_bytes: 20480,
            updated_at: "2026-11-15 12:00".to_string(),
            summary: "全领域权威技术站点列表".to_string(),
        }];

        let overview = AssetsCatalogEngine::format_catalog_overview(&dummy_repos, &dummy_docs);
        assert!(overview.contains("test-repo"));
        assert!(overview.contains("权威清单"));
    }
}
