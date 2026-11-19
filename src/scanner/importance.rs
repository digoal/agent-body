use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportanceScore {
    pub score: f32, // 0.0 ~ 1.0
    pub reason: String,
    pub is_noise: bool,
}

pub trait ImportanceScorer: Send + Sync {
    fn score_asset(
        &self,
        path: &Path,
        asset_type: &str,
        size_bytes: Option<i64>,
        mtime_secs: Option<u64>,
    ) -> ImportanceScore;
}

/// 默认内置规则重要性评分引擎（纯规则，0 LLM 开销，纳秒级响应）
pub struct RuleBasedImportanceScorer {
    threshold: f32,
}

impl RuleBasedImportanceScorer {
    pub fn new(threshold: f32) -> Self {
        Self {
            threshold: threshold.clamp(0.0, 1.0),
        }
    }

    pub fn default_scorer() -> Self {
        Self::new(0.35)
    }
}

impl ImportanceScorer for RuleBasedImportanceScorer {
    fn score_asset(
        &self,
        path: &Path,
        _asset_type: &str,
        size_bytes: Option<i64>,
        mtime_secs: Option<u64>,
    ) -> ImportanceScore {
        let mut score = 0.5f32;
        let mut reasons = Vec::new();

        let path_str = path.to_string_lossy().to_lowercase();
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        // 0. 确定性高噪音/编译产物/编辑器交换文件一票否决（不被工作区加分抵消）
        let hard_noise_ext = matches!(
            ext.as_str(),
            "pyc" | "pyo" | "swp" | "swx" | "swo" | "tmp" | "log" | "pid" | "sock" | "o" | "a" | "so" | "dylib"
        );
        let hard_noise_name = matches!(
            file_name.as_str(),
            "cachedir.tag" | ".rustc_info.json" | ".ds_store" | "thumbs.db" | "desktop.ini"
        ) || file_name.ends_with('~');
        let hard_noise_dir = path_str.contains("/.git/")
            || path_str.contains("/node_modules/")
            || path_str.contains("/__pycache__/")
            || path_str.contains("/.steward/")
            || path_str.contains("/.state/")
            || path_str.contains("/target/debug/")
            || path_str.contains("/target/release/");

        if hard_noise_ext || hard_noise_name || hard_noise_dir {
            return ImportanceScore {
                score: 0.05,
                reason: "确定性阻断: 编译缓存/临时交换/日志/黑名单路径噪声".to_string(),
                is_noise: true,
            };
        }

        // 1. 路径核心度评估
        if path_str.contains("/work/") || path_str.contains("/shared_proj/") || path_str.contains("/skills/") || path_str.contains("/note/") {
            score += 0.25;
            reasons.push("位于用户核心工作/沉淀目录 (+0.25)");
        } else if path_str.contains("/tmp/") || path_str.contains("/temp/") || path_str.contains("/target/") || path_str.contains("/cache/") || path_str.contains("/.cache/") {
            score -= 0.35;
            reasons.push("位于临时/构建缓存目录 (-0.35)");
        }

        // 2. 噪声路径命中 (logs)
        if path_str.contains("/logs/") {
            score -= 0.50;
            reasons.push("命中日志目录路径 (-0.50)");
        }

        // 3. 文件扩展名价值评估
        match ext.as_str() {
            "rs" | "py" | "go" | "cpp" | "c" | "ts" | "sql" | "sh" => {
                score += 0.20;
                reasons.push("核心源码工程脚本 (+0.20)");
            }
            "md" | "org" | "txt" | "pdf" => {
                score += 0.15;
                reasons.push("知识文档资料 (+0.15)");
            }
            "toml" | "json" | "yaml" | "yml" | "ini" | "conf" | "service" => {
                score += 0.10;
                reasons.push("关键系统/工程配置 (+0.10)");
            }
            "tmp" | "bak" | "swp" | "lock" | "pid" | "sock" | "pyc" => {
                score -= 0.30;
                reasons.push("临时暂存/中间锁文件 (-0.30)");
            }
            _ => {}
        }

        // 4. 文件尺寸与空文件评估
        if let Some(size) = size_bytes {
            if size == 0 {
                score -= 0.30;
                reasons.push("0字节空文件 (-0.30)");
            } else if size > 100 * 1024 * 1024 {
                score -= 0.20;
                reasons.push("超大单体文件 (>100MB) (-0.20)");
            }
        }

        // 5. 修改时间时效性（若传入）
        if let Some(mtime) = mtime_secs {
            let now_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(mtime);

            if now_secs >= mtime {
                let age_days = (now_secs - mtime) / 86400;
                if age_days <= 7 {
                    score += 0.15;
                    reasons.push("近7天内高频活跃更新 (+0.15)");
                } else if age_days > 90 {
                    score -= 0.10;
                    reasons.push("超过90天未更新 (-0.10)");
                }
            }
        }

        let final_score = score.clamp(0.0, 1.0);
        let is_noise = final_score < self.threshold;

        ImportanceScore {
            score: final_score,
            reason: reasons.join("; "),
            is_noise,
        }
    }
}

/// 噪声过滤器
pub struct NoiseFilter {
    scorer: Box<dyn ImportanceScorer>,
}

impl Default for NoiseFilter {
    fn default() -> Self {
        Self {
            scorer: Box::new(RuleBasedImportanceScorer::default_scorer()),
        }
    }
}

impl NoiseFilter {
    pub fn new(scorer: Box<dyn ImportanceScorer>) -> Self {
        Self { scorer }
    }

    pub fn with_threshold(threshold: f32) -> Self {
        Self {
            scorer: Box::new(RuleBasedImportanceScorer::new(threshold)),
        }
    }

    /// 判定是否应当入库录入资产
    pub fn should_record(
        &self,
        path: &Path,
        asset_type: &str,
        size_bytes: Option<i64>,
        mtime_secs: Option<u64>,
    ) -> (bool, ImportanceScore) {
        let score = self.scorer.score_asset(path, asset_type, size_bytes, mtime_secs);
        (!score.is_noise, score)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_importance_scoring_rules() {
        let filter = NoiseFilter::default();

        // 1. 核心目录下的 Rust 源码
        let path1 = Path::new("/root/new/work/agent-steward/src/main.rs");
        let (record1, s1) = filter.should_record(path1, "script", Some(1024), None);
        assert!(record1, "核心 Rust 源码必须被记录");
        assert!(s1.score >= 0.7, "核心源码得分应高: {}", s1.score);

        // 2. 临时目录下的 log 文件
        let path2 = Path::new("/root/new/tmp/debug.log");
        let (record2, s2) = filter.should_record(path2, "other", Some(500), None);
        assert!(!record2, "临时目录 log 文件必须被过滤判定为噪声");
        assert!(s2.score < 0.35, "临时 log 得分应低于阈值: {}", s2.score);

        // 3. node_modules 内部依赖
        let path3 = Path::new("/root/new/work/my_web/node_modules/lodash/index.js");
        let (record3, s3) = filter.should_record(path3, "script", Some(2000), None);
        assert!(!record3, "node_modules 内部文件必须被过滤判定为噪声");
        assert!(s3.is_noise);

        // 4. 0 字节空文件
        let path4 = Path::new("/root/new/tmp/empty.tmp");
        let (record4, _) = filter.should_record(path4, "other", Some(0), None);
        assert!(!record4, "0 字节临时文件必须被过滤");
    }
}
