use std::path::Path;
use anyhow::Result;
use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use crate::storage::models::ScanPolicy;

pub struct PolicyMatcher {
    shallow_set: GlobSet,
    restricted_set: GlobSet,
    deep_set: GlobSet,
}

impl PolicyMatcher {
    pub fn from_policies(policies: &[ScanPolicy]) -> Result<Self> {
        let mut shallow_builder = GlobSetBuilder::new();
        let mut restricted_builder = GlobSetBuilder::new();
        let mut deep_builder = GlobSetBuilder::new();

        for p in policies {
            let pattern = normalize_pattern(&p.path_pattern);
            if let Ok(glob) = GlobBuilder::new(&pattern)
                .literal_separator(false)
                .backslash_escape(true)
                .build()
            {
                match p.tier.as_str() {
                    "shallow" => shallow_builder.add(glob),
                    "restricted" => restricted_builder.add(glob),
                    "deep" => deep_builder.add(glob),
                    _ => &mut deep_builder,
                };
            }
        }

        // 默认保底的通用黑名单
        for default_shallow in &["**/node_modules/**", "**/node_modules", "**/.cache/**", "**/.git/**", "**/target/**", "/proc/**"] {
            if let Ok(glob) = Glob::new(default_shallow) {
                shallow_builder.add(glob);
            }
        }

        Ok(Self {
            shallow_set: shallow_builder.build()?,
            restricted_set: restricted_builder.build()?,
            deep_set: deep_builder.build()?,
        })
    }

    /// 判定路径所属的层级 (shallow 优先，其次 restricted，再其次 deep)
    pub fn match_tier(&self, path: &Path) -> &'static str {
        if self.shallow_set.is_match(path) {
            return "shallow";
        }
        if self.restricted_set.is_match(path) {
            return "restricted";
        }
        if self.deep_set.is_match(path) {
            return "deep";
        }
        "deep"
    }
}

fn normalize_pattern(pat: &str) -> String {
    let mut s = pat.trim().to_string();
    if !s.starts_with('/') && !s.starts_with('*') {
        s = format!("**/{}", s);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_matcher_glob() {
        let policies = vec![
            ScanPolicy {
                id: "p1".to_string(),
                path_pattern: "/root/new/shared_proj/**".to_string(),
                tier: "deep".to_string(),
                max_depth: -1,
                enable_fts: true,
                enable_vec: true,
                enable_graph: true,
                notes: None,
            },
            ScanPolicy {
                id: "p2".to_string(),
                path_pattern: "**/node_modules/**".to_string(),
                tier: "shallow".to_string(),
                max_depth: 0,
                enable_fts: false,
                enable_vec: false,
                enable_graph: false,
                notes: None,
            },
        ];

        let matcher = PolicyMatcher::from_policies(&policies).expect("from_policies");

        assert_eq!(matcher.match_tier(Path::new("/root/new/shared_proj/my_tool/main.py")), "deep");
        assert_eq!(matcher.match_tier(Path::new("/root/new/work/app/node_modules")), "shallow");
        assert_eq!(matcher.match_tier(Path::new("/root/new/work/app/node_modules/lib/index.js")), "shallow");
        assert_eq!(matcher.match_tier(Path::new("/root/new/work/app/target/debug/build")), "shallow");
    }
}
