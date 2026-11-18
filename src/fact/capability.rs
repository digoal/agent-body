use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 宿主机工具能力模型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCapability {
    /// 二进制可执行文件名（如 pdftotext, ffmpeg, jq）
    pub tool_name: String,
    /// 能力大类（如 pdf, video_audio, data_json, text_search, doc_convert, network, archive, dev_build）
    pub category: String,
    /// 核心能力描述
    pub description: String,
    /// 语义标签列表（用于意图匹配）
    pub tags: Vec<String>,
    /// 是否在当前宿主机物理安装并就绪
    pub installed: bool,
    /// 宿主机物理绝对路径（如 /usr/bin/pdftotext）
    pub path: Option<String>,
    /// 探测到的实际版本字符串（如 pdftotext version 25.03.0）
    pub version: Option<String>,
    /// 官方与工业界最佳实践参数组合（如 -layout input.pdf output.txt）
    pub best_practice_args: String,
    /// 资深架构师/DBA 避坑指南与最佳实践备忘
    pub notes: String,
    /// 宿主机上的同类替代工具列表（如 pdftoppm, pdfunite, pypdf）
    pub alternatives: Vec<String>,
}

/// 语义查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityQueryResult {
    pub query: String,
    pub matched_category: Option<String>,
    pub top_recommendation: Option<ToolCapability>,
    pub alternatives: Vec<ToolCapability>,
    pub all_matching_tools: Vec<ToolCapability>,
}

/// 工具能力语义图谱引擎
pub struct CapabilityEngine {
    /// 知识库中的工具能力字典 (tool_name -> ToolCapability)
    registry: HashMap<String, ToolCapability>,
    /// 缓存的 PATH 目录列表
    path_dirs: Vec<PathBuf>,
}

impl Default for CapabilityEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CapabilityEngine {
    /// 初始化能力引擎并加载内置知识库
    pub fn new() -> Self {
        let mut engine = Self {
            registry: HashMap::new(),
            path_dirs: Self::resolve_path_dirs(),
        };
        engine.load_builtin_capabilities();
        engine
    }

    /// 获取当前系统的 PATH 目录列表
    fn resolve_path_dirs() -> Vec<PathBuf> {
        let path_var = env::var("PATH").unwrap_or_default();
        env::split_paths(&path_var)
            .filter(|p| p.is_dir())
            .collect()
    }

    /// 内置 Linux 高频工具能力权威知识库
    fn load_builtin_capabilities(&mut self) {
        let builtins = vec![
            // 1. PDF 族谱
            ToolCapability {
                tool_name: "pdftotext".to_string(),
                category: "pdf".to_string(),
                description: "从 PDF 文档中抽取高保真文本内容，完美保留原始布局".to_string(),
                tags: vec![
                    "pdf".to_string(), "text".to_string(), "extract".to_string(),
                    "layout".to_string(), "排版".to_string(), "提取".to_string(),
                    "文本".to_string(), "pdftotext".to_string(), "文档".to_string(),
                ],
                installed: false,
                path: None,
                version: None,
                best_practice_args: "pdftotext -layout input.pdf output.txt".to_string(),
                notes: "基于 poppler-utils，排版保真度业界第一，极速且避免安装庞大易错的第三方 Python 库".to_string(),
                alternatives: vec!["pdftoppm".to_string(), "pdfimages".to_string(), "pdfunite".to_string()],
            },
            ToolCapability {
                tool_name: "pdftoppm".to_string(),
                category: "pdf".to_string(),
                description: "将 PDF 文档每一页渲染为高分辨率图片 (PNG/JPEG)".to_string(),
                tags: vec![
                    "pdf".to_string(), "image".to_string(), "render".to_string(),
                    "png".to_string(), "jpeg".to_string(), "转图片".to_string(),
                    "截图".to_string(), "pdftoppm".to_string(),
                ],
                installed: false,
                path: None,
                version: None,
                best_practice_args: "pdftoppm -png -r 150 input.pdf page".to_string(),
                notes: "用于大模型多模态视觉阅读或生成文档幻灯片切片，-r 150 兼顾清晰度与文件大小".to_string(),
                alternatives: vec!["pdftotext".to_string(), "convert".to_string()],
            },
            ToolCapability {
                tool_name: "pdfunite".to_string(),
                category: "pdf".to_string(),
                description: "高性能合并多个 PDF 文件为一个单一 PDF 文档".to_string(),
                tags: vec![
                    "pdf".to_string(), "merge".to_string(), "combine".to_string(),
                    "join".to_string(), "合并".to_string(), "拼接".to_string(),
                ],
                installed: false,
                path: None,
                version: None,
                best_practice_args: "pdfunite in1.pdf in2.pdf out.pdf".to_string(),
                notes: "零重编码损耗，毫秒级合并".to_string(),
                alternatives: vec!["pdftotext".to_string()],
            },

            // 2. 音视频多媒体族谱
            ToolCapability {
                tool_name: "ffmpeg".to_string(),
                category: "video_audio".to_string(),
                description: "跨平台多媒体万能处理框架，涵盖音视频转码、剪辑、抽帧、雪碧图生成".to_string(),
                tags: vec![
                    "video".to_string(), "audio".to_string(), "media".to_string(),
                    "transcode".to_string(), "convert".to_string(), "mp4".to_string(),
                    "mp3".to_string(), "视频".to_string(), "音频".to_string(), "转码".to_string(),
                    "抽帧".to_string(), "雪碧图".to_string(), "ffmpeg".to_string(),
                ],
                installed: false,
                path: None,
                version: None,
                best_practice_args: "ffmpeg -i input.mp4 -c:v libx264 -crf 23 -preset medium output.mp4".to_string(),
                notes: "生产环境推荐显式指定 -c:v 与 -crf 质量因子，避免默认编码器导致体积失控".to_string(),
                alternatives: vec!["ffprobe".to_string(), "sox".to_string()],
            },
            ToolCapability {
                tool_name: "ffprobe".to_string(),
                category: "video_audio".to_string(),
                description: "精确获取音视频多媒体文件的容器、编解码器、时长、码率等结构化元数据".to_string(),
                tags: vec![
                    "video".to_string(), "audio".to_string(), "metadata".to_string(),
                    "duration".to_string(), "inspect".to_string(), "元数据".to_string(),
                    "码率".to_string(), "时长".to_string(), "ffprobe".to_string(),
                ],
                installed: false,
                path: None,
                version: None,
                best_practice_args: "ffprobe -v quiet -print_format json -show_format -show_streams input.mp4".to_string(),
                notes: "搭配 -print_format json，可直接与 jq 配合管道解析音频时长与分辨率".to_string(),
                alternatives: vec!["ffmpeg".to_string()],
            },
            ToolCapability {
                tool_name: "sox".to_string(),
                category: "video_audio".to_string(),
                description: "音频万能瑞士军刀，支持格式转换、降噪、变调与音量归一化".to_string(),
                tags: vec![
                    "audio".to_string(), "sound".to_string(), "voice".to_string(),
                    "wav".to_string(), "mp3".to_string(), "音频".to_string(), "降噪".to_string(),
                    "播客".to_string(), "sox".to_string(),
                ],
                installed: false,
                path: None,
                version: None,
                best_practice_args: "sox input.wav -r 16000 -c 1 output.wav norm -3".to_string(),
                notes: "常用于为 ASR 语音识别预处理音频采样率 (16kHz 单声道) 和响度标准化".to_string(),
                alternatives: vec!["ffmpeg".to_string()],
            },

            // 3. 数据与 JSON 处理族谱
            ToolCapability {
                tool_name: "jq".to_string(),
                category: "data_json".to_string(),
                description: "轻量级、超高速的命令行 JSON 结构化查询、过滤与重组工具".to_string(),
                tags: vec![
                    "json".to_string(), "query".to_string(), "filter".to_string(),
                    "parse".to_string(), "数据".to_string(), "解析".to_string(),
                    "筛选".to_string(), "jq".to_string(),
                ],
                installed: false,
                path: None,
                version: None,
                best_practice_args: "jq -r '.items[] | {id: .id, name: .name}' input.json".to_string(),
                notes: "配合 -r 获得原始文本输出，利用管道可实现极高吞吐的流式 JSON 处理".to_string(),
                alternatives: vec!["yq".to_string(), "duckdb".to_string()],
            },
            ToolCapability {
                tool_name: "yq".to_string(),
                category: "data_json".to_string(),
                description: "通用的 YAML / JSON / XML 格式解析、转换与就地编辑工具".to_string(),
                tags: vec![
                    "yaml".to_string(), "json".to_string(), "xml".to_string(),
                    "config".to_string(), "配置".to_string(), "yq".to_string(),
                ],
                installed: false,
                path: None,
                version: None,
                best_practice_args: "yq -i '.service.port = 8080' config.yaml".to_string(),
                notes: "配合 -i 参数支持直接就地更新 YAML 文件且保留注释与格式".to_string(),
                alternatives: vec!["jq".to_string()],
            },
            ToolCapability {
                tool_name: "duckdb".to_string(),
                category: "data_json".to_string(),
                description: "嵌入式极速分析型 SQL 引擎，零配置直接查询 Parquet / CSV / JSON 文件".to_string(),
                tags: vec![
                    "sql".to_string(), "parquet".to_string(), "csv".to_string(),
                    "olap".to_string(), "analytics".to_string(), "分析".to_string(),
                    "查询".to_string(), "duckdb".to_string(),
                ],
                installed: false,
                path: None,
                version: None,
                best_practice_args: "duckdb -c \"SELECT * FROM read_parquet('data.parquet') LIMIT 10;\"".to_string(),
                notes: "适合百万至千万级数据的单机极速统计，避免启动大型数据库服务".to_string(),
                alternatives: vec!["jq".to_string()],
            },

            // 4. 文本检索与代码分析族谱
            ToolCapability {
                tool_name: "rg".to_string(),
                category: "text_search".to_string(),
                description: "ripgrep：全球最快的行级正文递归检索工具，自动遵循 .gitignore".to_string(),
                tags: vec![
                    "grep".to_string(), "search".to_string(), "find".to_string(),
                    "regex".to_string(), "code".to_string(), "搜索".to_string(),
                    "检索".to_string(), "代码".to_string(), "rg".to_string(), "ripgrep".to_string(),
                ],
                installed: false,
                path: None,
                version: None,
                best_practice_args: "rg -i --type rust \"fn main\"".to_string(),
                notes: "优先使用 --type 限定语言，配合 -l 快速输出匹配文件列表，杜绝全盘盲目扫盘".to_string(),
                alternatives: vec!["fd".to_string(), "codegraph".to_string()],
            },
            ToolCapability {
                tool_name: "fd".to_string(),
                category: "text_search".to_string(),
                description: "极速、直观的 find 命令现代替代品，彩色输出且默认忽略隐藏文件与 gitignore".to_string(),
                tags: vec![
                    "find".to_string(), "file".to_string(), "locate".to_string(),
                    "path".to_string(), "查找文件".to_string(), "文件名".to_string(), "fd".to_string(),
                ],
                installed: false,
                path: None,
                version: None,
                best_practice_args: "fd -e md -e txt --max-depth 3".to_string(),
                notes: "找文件扩展名配合 -e，配合 -x 可并发执行后续命令".to_string(),
                alternatives: vec!["rg".to_string()],
            },
            ToolCapability {
                tool_name: "codegraph".to_string(),
                category: "dev_build".to_string(),
                description: "基于代码语法语义的符号级调用图谱导航与探索工具".to_string(),
                tags: vec![
                    "code".to_string(), "callgraph".to_string(), "ast".to_string(),
                    "symbol".to_string(), "代码地图".to_string(), "符号".to_string(),
                    "调用链".to_string(), "codegraph".to_string(),
                ],
                installed: false,
                path: None,
                version: None,
                best_practice_args: "codegraph explore \"<symbol or query>\"".to_string(),
                notes: "针对已有 .codegraph 的仓库，优先调用 codegraph explore 获取符号精准代码段与跨文件调用".to_string(),
                alternatives: vec!["rg".to_string()],
            },

            // 5. 文档格式互转族谱
            ToolCapability {
                tool_name: "pandoc".to_string(),
                category: "doc_convert".to_string(),
                description: "万能通用文档转换工具，支持 Markdown / HTML / PDF / Word / LaTeX 互转".to_string(),
                tags: vec![
                    "markdown".to_string(), "html".to_string(), "pdf".to_string(),
                    "docx".to_string(), "convert".to_string(), "转换".to_string(),
                    "格式".to_string(), "排版".to_string(), "pandoc".to_string(),
                ],
                installed: false,
                path: None,
                version: None,
                best_practice_args: "pandoc -f markdown -t html5 --standalone input.md -o output.html".to_string(),
                notes: "生成独立 HTML 务必加上 --standalone，嵌入样式保真".to_string(),
                alternatives: vec!["pdftotext".to_string()],
            },

            // 6. 网络与探测族谱
            ToolCapability {
                tool_name: "curl".to_string(),
                category: "network".to_string(),
                description: "强大的命令行 URL 传输工具，支持 HTTP(S) / REST API 请求与文件下载".to_string(),
                tags: vec![
                    "http".to_string(), "api".to_string(), "request".to_string(),
                    "download".to_string(), "网络".to_string(), "请求".to_string(),
                    "抓取".to_string(), "curl".to_string(),
                ],
                installed: false,
                path: None,
                version: None,
                best_practice_args: "curl -sSL -H \"Content-Type: application/json\" --connect-timeout 5".to_string(),
                notes: "生产环境务必带 -sSL 与 --connect-timeout 防止悬挂阻塞会话".to_string(),
                alternatives: vec!["httpie".to_string(), "wget".to_string()],
            },
            ToolCapability {
                tool_name: "tcpdump".to_string(),
                category: "network".to_string(),
                description: "底层网络抓包与协议分析利器，直接捕获网卡原始网络数据包".to_string(),
                tags: vec![
                    "packet".to_string(), "sniff".to_string(), "network".to_string(),
                    "port".to_string(), "抓包".to_string(), "网络分析".to_string(), "tcpdump".to_string(),
                ],
                installed: false,
                path: None,
                version: None,
                best_practice_args: "tcpdump -i any -n -c 20 port 8080".to_string(),
                notes: "排查端口监听异常与跨机器丢包的首选，务必加 -c 限制包数防止填满磁盘".to_string(),
                alternatives: vec!["curl".to_string()],
            },

            // 7. 编译构建与开发
            ToolCapability {
                tool_name: "cargo".to_string(),
                category: "dev_build".to_string(),
                description: "Rust 语言官方包管理器、构建工具与测试运行器".to_string(),
                tags: vec![
                    "rust".to_string(), "build".to_string(), "compile".to_string(),
                    "test".to_string(), "构建".to_string(), "编译".to_string(), "cargo".to_string(),
                ],
                installed: false,
                path: None,
                version: None,
                best_practice_args: "cargo build --release".to_string(),
                notes: "单测调试使用 cargo test -- --nocapture 显示详细输出".to_string(),
                alternatives: vec!["rustc".to_string()],
            },
        ];

        for item in builtins {
            self.registry.insert(item.tool_name.clone(), item);
        }
    }

    /// 执行宿主机物理扫描：在 $PATH 中定位二进制并获取真实版本
    pub fn scan_host(&mut self) {
        for (name, cap) in self.registry.iter_mut() {
            if let Some(abs_path) = Self::find_binary_in_paths(name, &self.path_dirs) {
                cap.installed = true;
                cap.path = Some(abs_path.to_string_lossy().to_string());
                cap.version = Self::probe_version(&abs_path);
            } else {
                cap.installed = false;
                cap.path = None;
                cap.version = None;
            }
        }
    }

    /// 获取注册表中的所有工具能力列表
    pub fn all_capabilities(&self) -> Vec<ToolCapability> {
        self.registry.values().cloned().collect()
    }

    /// 在指定 PATH 列表中查找是否存在可执行文件
    fn find_binary_in_paths(tool_name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
        for dir in dirs {
            let candidate = dir.join(tool_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }

    /// 探测二进制工具真实版本信息（带短命令容错）
    fn probe_version(bin_path: &Path) -> Option<String> {
        // 先尝试 --version
        if let Ok(output) = Command::new(bin_path).arg("--version").output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(first_line) = stdout.lines().next() {
                    let trimmed = first_line.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }

        // 备用尝试 -v (如 pdftotext)
        if let Ok(output) = Command::new(bin_path).arg("-v").output() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if let Some(first_line) = stderr.lines().next() {
                let trimmed = first_line.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(first_line) = stdout.lines().next() {
                let trimmed = first_line.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }

        None
    }

    /// 根据自然语言或关键词意图进行打分与语义推荐
    pub fn query(&self, intent: &str) -> CapabilityQueryResult {
        let normalized = intent.to_lowercase();
        let tokens: Vec<&str> = normalized
            .split(|c: char| c.is_whitespace() || c == ',' || c == '，' || c == '。' || c == '/' || c == '-' || c == '_')
            .filter(|s| !s.is_empty())
            .collect();

        let mut scored_tools: Vec<(i32, ToolCapability)> = Vec::new();

        for cap in self.registry.values() {
            let mut score = 0;

            // 1. 精确工具名匹配 (+50)
            if normalized.contains(&cap.tool_name) {
                score += 50;
            }

            // 2. 类别名匹配 (+30)
            if normalized.contains(&cap.category) {
                score += 30;
            }

            // 3. 标签匹配 (+15/个)
            for tag in &cap.tags {
                let tag_lower = tag.to_lowercase();
                if normalized.contains(&tag_lower) {
                    score += 20;
                }
                for token in &tokens {
                    if *token == tag_lower {
                        score += 15;
                    }
                }
            }

            // 4. 描述与备注包含匹配 (+10)
            if cap.description.to_lowercase().contains(&normalized) {
                score += 15;
            }

            // 5. 物理就绪加权：已在宿主机安装的工具给予加权 (+40)，未安装的降级
            if cap.installed {
                score += 40;
            }

            if score > 0 {
                scored_tools.push((score, cap.clone()));
            }
        }

        // 按得分降序排序
        scored_tools.sort_by(|a, b| b.0.cmp(&a.0));

        let all_matching_tools: Vec<ToolCapability> = scored_tools.into_iter().map(|(_, c)| c).collect();

        let top_recommendation = all_matching_tools.first().cloned();
        let matched_category = top_recommendation.as_ref().map(|t| t.category.clone());

        // 挑选同类替代品
        let mut alternatives = Vec::new();
        if let Some(ref top) = top_recommendation {
            for tool in &all_matching_tools[1..] {
                if tool.category == top.category || top.alternatives.contains(&tool.tool_name) {
                    alternatives.push(tool.clone());
                }
            }
        }

        CapabilityQueryResult {
            query: intent.to_string(),
            matched_category,
            top_recommendation,
            alternatives,
            all_matching_tools,
        }
    }

    /// 将结果格式化为对 Agent 极具指导价值的自然语言文本
    pub fn format_for_agent(&self, result: &CapabilityQueryResult) -> String {
        if let Some(ref top) = result.top_recommendation {
            let mut out = String::new();
            out.push_str(&format!("🎯 **推荐宿主工具: `{}`**\n", top.tool_name));
            if top.installed {
                out.push_str(&format!(
                    "- **物理就绪**: ✅ 已安装 (绝对路径: `{}`)\n",
                    top.path.as_deref().unwrap_or("未知")
                ));
                if let Some(ref ver) = top.version {
                    out.push_str(&format!("- **探测版本**: {}\n", ver));
                }
            } else {
                out.push_str("- **物理状态**: ⚠️ 本地未安装（可通过包管理器安装）\n");
            }
            out.push_str(&format!("- **核心能力**: {}\n", top.description));
            out.push_str(&format!("- **推荐用法**: `{}`\n", top.best_practice_args));
            out.push_str(&format!("- **架构备忘**: {}\n", top.notes));

            if !result.alternatives.is_empty() {
                out.push_str("\n💡 **同类/替代方案**:\n");
                for alt in &result.alternatives {
                    let status = if alt.installed { "已安装" } else { "未安装" };
                    out.push_str(&format!(
                        "  * `{}` ([{}] {}): `{}`\n",
                        alt.tool_name, status, alt.description, alt.best_practice_args
                    ));
                }
            }
            out
        } else {
            format!("未在宿主机能力库中找到与意图 \"{}\" 直接匹配的工具，建议通过通用终端命令探测。", result.query)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_capabilities_load() {
        let engine = CapabilityEngine::new();
        assert!(engine.registry.len() >= 15);
        assert!(engine.registry.contains_key("pdftotext"));
        assert!(engine.registry.contains_key("ffmpeg"));
        assert!(engine.registry.contains_key("jq"));
    }

    #[test]
    fn test_host_scan() {
        let mut engine = CapabilityEngine::new();
        engine.scan_host();
        
        // 宿主机应能探测到 pdftotext, jq, curl 等已安装工具
        let pdftotext = engine.registry.get("pdftotext").unwrap();
        assert!(pdftotext.installed, "pdftotext 应该在当前宿主机被检测为 installed=true");
        assert!(pdftotext.path.is_some());

        let jq = engine.registry.get("jq").unwrap();
        assert!(jq.installed, "jq 应该被检测为 installed=true");
    }

    #[test]
    fn test_query_pdf_intent() {
        let mut engine = CapabilityEngine::new();
        engine.scan_host();

        let result = engine.query("这台机器上有没有能处理 PDF 的东西？提取排版文字");
        assert!(result.top_recommendation.is_some());
        let top = result.top_recommendation.as_ref().unwrap();
        assert_eq!(top.tool_name, "pdftotext");
        assert!(top.installed);
        assert!(top.best_practice_args.contains("-layout"));

        let agent_msg = engine.format_for_agent(&result);
        assert!(agent_msg.contains("pdftotext"));
        assert!(agent_msg.contains("-layout"));
    }

    #[test]
    fn test_query_json_intent() {
        let mut engine = CapabilityEngine::new();
        engine.scan_host();

        let result = engine.query("查询和解析 json 数据");
        assert!(result.top_recommendation.is_some());
        let top = result.top_recommendation.unwrap();
        assert_eq!(top.tool_name, "jq");
    }

    #[test]
    fn test_query_video_intent() {
        let mut engine = CapabilityEngine::new();
        engine.scan_host();

        let result = engine.query("视频转码和提取雪碧图");
        assert!(result.top_recommendation.is_some());
        let top = result.top_recommendation.unwrap();
        assert_eq!(top.tool_name, "ffmpeg");
    }
}
