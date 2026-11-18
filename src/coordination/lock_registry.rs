use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceType {
    Port,
    Directory,
    Service,
    File,
    Custom,
}

impl ResourceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResourceType::Port => "port",
            ResourceType::Directory => "directory",
            ResourceType::Service => "service",
            ResourceType::File => "file",
            ResourceType::Custom => "custom",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().trim() {
            "port" => ResourceType::Port,
            "dir" | "directory" => ResourceType::Directory,
            "service" => ResourceType::Service,
            "file" => ResourceType::File,
            _ => ResourceType::Custom,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LockScope {
    Exclusive,
    Shared,
}

impl LockScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            LockScope::Exclusive => "exclusive",
            LockScope::Shared => "shared",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().trim() {
            "shared" => LockScope::Shared,
            _ => LockScope::Exclusive,
        }
    }
}

pub struct LockRegistry;

impl LockRegistry {
    /// 资源标识规范化清洗
    pub fn normalize_resource_id(resource_type: &ResourceType, raw_id: &str) -> String {
        let trimmed = raw_id.trim();
        match resource_type {
            ResourceType::Port => {
                // 统一纯端口数字
                trimmed.trim_start_matches(':').to_string()
            }
            ResourceType::Directory => {
                // 统一绝对路径末尾斜杠
                let mut path = trimmed.to_string();
                if !path.ends_with('/') {
                    path.push('/');
                }
                path
            }
            ResourceType::File => {
                // 统一去除结尾多余斜杠
                trimmed.trim_end_matches('/').to_string()
            }
            ResourceType::Service => {
                // 统一带 .service 后缀
                if !trimmed.ends_with(".service") {
                    format!("{}.service", trimmed)
                } else {
                    trimmed.to_string()
                }
            }
            ResourceType::Custom => trimmed.to_string(),
        }
    }

    /// 计算 64 位整数哈希，用于 PostgreSQL Advisory Lock (pg_try_advisory_lock)
    pub fn hash_to_i64(resource_type: &ResourceType, resource_id: &str) -> i64 {
        let mut hasher = DefaultHasher::new();
        resource_type.as_str().hash(&mut hasher);
        resource_id.hash(&mut hasher);
        hasher.finish() as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_registry_normalization() {
        assert_eq!(LockRegistry::normalize_resource_id(&ResourceType::Port, ":8080"), "8080");
        assert_eq!(LockRegistry::normalize_resource_id(&ResourceType::Directory, "/etc/nginx"), "/etc/nginx/");
        assert_eq!(LockRegistry::normalize_resource_id(&ResourceType::Service, "redis"), "redis.service");
        assert_eq!(LockRegistry::normalize_resource_id(&ResourceType::File, "/tmp/foo/"), "/tmp/foo");
    }

    #[test]
    fn test_hash_to_i64_deterministic() {
        let h1 = LockRegistry::hash_to_i64(&ResourceType::Port, "8080");
        let h2 = LockRegistry::hash_to_i64(&ResourceType::Port, "8080");
        assert_eq!(h1, h2);

        let h3 = LockRegistry::hash_to_i64(&ResourceType::Port, "8081");
        assert_ne!(h1, h3);
    }
}
