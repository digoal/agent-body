use std::path::Path;
use anyhow::{anyhow, Context, Result};
use chrono::Local;
use walkdir::WalkDir;
use crate::storage::Storage;
use crate::storage::models::{AssetRecord, ScanJobRecord};
use crate::scanner::matcher::PolicyMatcher;
use crate::scanner::infer_file_attributes;
use crate::scanner::importance::NoiseFilter;

/// 断点续扫状态机
pub struct ScanStateMachine {
    storage: Storage,
    matcher: PolicyMatcher,
    noise_filter: NoiseFilter,
    checkpoint_batch_size: usize,
}

impl ScanStateMachine {
    pub fn new(storage: Storage) -> Result<Self> {
        Self::with_batch_size(storage, 50)
    }

    pub fn with_batch_size(storage: Storage, batch_size: usize) -> Result<Self> {
        let policies = storage.get_scan_policies()?;
        let matcher = PolicyMatcher::from_policies(&policies)?;
        let noise_filter = NoiseFilter::default();
        Ok(Self {
            storage,
            matcher,
            noise_filter,
            checkpoint_batch_size: batch_size.max(1),
        })
    }

    /// 创建一个新的扫描任务（初始状态为 pending）
    pub fn create_job(&self, root_path: &Path) -> Result<ScanJobRecord> {
        if !root_path.exists() {
            return Err(anyhow!("目标扫描根路径不存在: {:?}", root_path));
        }

        let canonical = root_path.canonicalize()
            .with_context(|| format!("解析目标根路径绝对规范路径失败: {:?}", root_path))?;
        let root_str = canonical.to_string_lossy().to_string();

        let now = Local::now().to_rfc3339();
        let job = ScanJobRecord {
            id: uuid::Uuid::new_v4().to_string(),
            root_path: root_str,
            status: "pending".to_string(),
            last_path: None,
            scanned_dirs: 0,
            scanned_files: 0,
            total_assets: 0,
            shallow_blocked: 0,
            error_message: None,
            started_at: None,
            paused_at: None,
            completed_at: None,
            created_at: Some(now.clone()),
            updated_at: Some(now),
        };

        self.storage.create_scan_job(&job)?;
        Ok(job)
    }

    /// 启动或从断点恢复扫描任务
    /// 如果传入 step_limit，则在处理指定数量的条目后主动挂起（返回 paused 状态）
    pub fn run_or_resume(&self, job_id: &str, step_limit: Option<usize>) -> Result<ScanJobRecord> {
        let mut job = self.storage.get_scan_job(job_id)?
            .ok_or_else(|| anyhow!("未找到扫描任务: {}", job_id))?;

        if job.status == "completed" {
            return Ok(job);
        }

        let now = Local::now().to_rfc3339();
        if job.started_at.is_none() {
            job.started_at = Some(now.clone());
        }
        job.status = "running".to_string();
        job.error_message = None;
        job.paused_at = None;
        self.storage.update_scan_job(&job)?;

        let root_path_buf = std::path::PathBuf::from(&job.root_path);
        let root_path = root_path_buf.as_path();
        if !root_path.exists() {
            job.status = "failed".to_string();
            job.error_message = Some(format!("根路径不存在: {:?}", root_path));
            self.storage.update_scan_job(&job)?;
            return Ok(job);
        }

        // 使用确定性文件排序迭代器
        let mut it = WalkDir::new(root_path)
            .sort_by_file_name()
            .into_iter();

        let mut skipping = job.last_path.is_some();
        let target_last = job.last_path.clone();

        let mut batch_records = Vec::new();
        let mut steps_in_this_run = 0usize;
        let mut reached_limit = false;
        let links_engine = crate::links::LinksEngine::new(std::sync::Arc::new(self.storage.clone()));

        while let Some(entry_res) = it.next() {
            let entry = match entry_res {
                Ok(e) => e,
                Err(err) => {
                    eprintln!("警告: 扫描路径读取错误: {:?}", err);
                    continue;
                }
            };

            let path = entry.path();
            let path_str = path.to_string_lossy().to_string();
            let is_dir = entry.file_type().is_dir();
            let tier = self.determine_tier(path);

            // 断点恢复阶段：跳过已经处理过的路径
            if skipping {
                if let Some(ref target) = target_last {
                    if path_str == *target {
                        // 命中上次完成的最后一条路径，从下一个条目开始扫描
                        skipping = false;
                        if is_dir && tier == "shallow" {
                            it.skip_current_dir();
                        }
                        continue;
                    } else {
                        // 依然处于已处理前缀中
                        if is_dir && tier == "shallow" {
                            // 保持遍历树形态一致，shallow 目录必须跳过其内部
                            it.skip_current_dir();
                        }
                        continue;
                    }
                } else {
                    skipping = false;
                }
            }

            // 正式扫描新节点
            if is_dir {
                job.scanned_dirs += 1;
                if tier == "shallow" {
                    it.skip_current_dir();

                    let metadata = entry.metadata().ok();
                    let size_bytes = metadata.as_ref().map(|m| m.len() as i64);

                    let record = AssetRecord {
                        id: uuid::Uuid::new_v4().to_string(),
                        path: path_str.clone(),
                        asset_type: "directory".to_string(),
                        owner: "system".to_string(),
                        security_level: "confidential".to_string(),
                        scan_tier: "shallow".to_string(),
                        description: Some("黑名单目录: 止于当前目录层级，未向下递归解析".to_string()),
                        desc_kv: serde_json::json!({
                            "policy": "shallow_tier",
                            "blocked_descendants": true,
                            "reason": "黑名单阻断区: 避免无差别扫描打爆存储并防敏感泄露"
                        }),
                        usage: None,
                        size_bytes,
                        file_count: None,
                        tags: vec!["shallow_blacklist".to_string()],
                        status: "active".to_string(),
                    };

                    batch_records.push(record);
                    job.shallow_blocked += 1;
                    job.total_assets += 1;
                }
            } else if entry.file_type().is_file() {
                job.scanned_files += 1;
                if tier == "shallow" {
                    // 黑名单目录内文件绝不入库
                    job.last_path = Some(path_str);
                    steps_in_this_run += 1;
                    continue;
                }
                if tier == "restricted" && !crate::scanner::should_index_in_restricted(root_path, path) {
                    job.last_path = Some(path_str);
                    steps_in_this_run += 1;
                    continue;
                }
                let (asset_type, mut tags) = infer_file_attributes(path);
                let metadata = entry.metadata().ok();
                let size_bytes = metadata.as_ref().map(|m| m.len() as i64);
                let mtime_secs = metadata.as_ref()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs());

                // 智能重要性与噪声过滤
                let (should_record, imp_score) = self.noise_filter.should_record(path, &asset_type, size_bytes, mtime_secs);
                if should_record {
                    let extracted = crate::scanner::metadata::HeuristicMetadataExtractor::extract(
                        path,
                        &asset_type,
                        &tier,
                        Some(&imp_score),
                        size_bytes,
                    );
                    for t in extracted.extra_tags {
                        if !tags.contains(&t) {
                            tags.push(t);
                        }
                    }

                    let record = AssetRecord {
                        id: uuid::Uuid::new_v4().to_string(),
                        path: path_str.clone(),
                        asset_type,
                        owner: "user:digoal".to_string(),
                        security_level: extracted.security_level,
                        scan_tier: tier.clone(),
                        description: Some(extracted.description),
                        desc_kv: extracted.desc_kv,
                        usage: extracted.usage,
                        size_bytes,
                        file_count: None,
                        tags,
                        status: "active".to_string(),
                    };

                    batch_records.push(record);
                    job.total_assets += 1;

                    if tier == "deep" {
                        let _ = links_engine.refresh_links_for_asset(path);
                    }
                }
            }

            job.last_path = Some(path_str);
            steps_in_this_run += 1;

            // 检查批次刷写
            if batch_records.len() >= self.checkpoint_batch_size {
                self.flush_checkpoint(&mut job, &batch_records)?;
                batch_records.clear();
            }

            // 单步或步数限制挂起判定
            if let Some(limit) = step_limit {
                if steps_in_this_run >= limit {
                    reached_limit = true;
                    break;
                }
            }
        }

        // 刷写剩余批次
        if !batch_records.is_empty() {
            self.flush_checkpoint(&mut job, &batch_records)?;
            batch_records.clear();
        }

        let now = Local::now().to_rfc3339();
        if reached_limit {
            job.status = "paused".to_string();
            job.paused_at = Some(now);
        } else {
            job.status = "completed".to_string();
            job.completed_at = Some(now);
        }

        self.storage.update_scan_job(&job)?;
        Ok(job)
    }

    /// 显式暂停任务
    pub fn pause_job(&self, job_id: &str) -> Result<ScanJobRecord> {
        let mut job = self.storage.get_scan_job(job_id)?
            .ok_or_else(|| anyhow!("未找到扫描任务: {}", job_id))?;

        if job.status == "running" || job.status == "pending" {
            job.status = "paused".to_string();
            job.paused_at = Some(Local::now().to_rfc3339());
            self.storage.update_scan_job(&job)?;
        }
        Ok(job)
    }

    pub fn get_job(&self, job_id: &str) -> Result<Option<ScanJobRecord>> {
        self.storage.get_scan_job(job_id)
    }

    pub fn list_jobs(&self, limit: usize) -> Result<Vec<ScanJobRecord>> {
        self.storage.list_scan_jobs(limit)
    }

    /// 判定路径的扫描级别
    pub fn determine_tier(&self, path: &Path) -> String {
        self.matcher.match_tier(path).to_string()
    }

    fn flush_checkpoint(&self, job: &mut ScanJobRecord, records: &[AssetRecord]) -> Result<()> {
        for r in records {
            let existing = self.storage.get_asset_by_path(&r.path).ok().flatten();
            let merged = crate::scanner::metadata::merge_with_existing_asset(r.clone(), existing.as_ref());
            self.storage.upsert_asset(&merged)?;
        }
        self.storage.update_scan_job(job)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs::{create_dir_all, File};
    use std::io::Write;

    #[test]
    fn test_scan_state_machine_breakpoint_resume() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let sm = ScanStateMachine::with_batch_size(storage.clone(), 2).expect("new sm");

        let tmp = tempdir().expect("tempdir");
        let root = tmp.path();

        // 构造测试目录结构：
        // root/
        //   ├── a_dir/
        //   │    ├── f1.py
        //   │    └── f2.rs
        //   ├── b_dir/
        //   │    ├── f3.md
        //   │    └── f4.toml
        //   └── c_node_modules/ (黑名单 shallow)
        //        └── ignored.js
        create_dir_all(root.join("a_dir")).unwrap();
        create_dir_all(root.join("b_dir")).unwrap();
        create_dir_all(root.join("c_node_modules")).unwrap();

        File::create(root.join("a_dir/f1.py")).unwrap().write_all(b"print(1)").unwrap();
        File::create(root.join("a_dir/f2.rs")).unwrap().write_all(b"fn main() {}").unwrap();
        File::create(root.join("b_dir/f3.md")).unwrap().write_all(b"# Doc").unwrap();
        File::create(root.join("b_dir/f4.toml")).unwrap().write_all(b"[cfg]").unwrap();
        File::create(root.join("c_node_modules/ignored.js")).unwrap().write_all(b"code").unwrap();

        // 1. 创建任务
        let job = sm.create_job(root).expect("create_job");
        assert_eq!(job.status, "pending");
        let job_id = job.id.clone();

        // 2. 第一阶段：单步执行 3 步（模拟中断/挂起）
        let paused_job = sm.run_or_resume(&job_id, Some(3)).expect("run step 1");
        assert_eq!(paused_job.status, "paused", "达到步数限制应进入 paused");
        assert!(paused_job.last_path.is_some(), "断点必须记录 last_path");
        let last_at_step_1 = paused_job.last_path.clone().unwrap();
        let total_assets_step_1 = paused_job.total_assets;
        assert!(total_assets_step_1 > 0, "第一阶段应已产生资产");

        // 3. 从数据库重新读取验证一致性
        let db_job = sm.get_job(&job_id).expect("get_job").expect("should exist");
        assert_eq!(db_job.status, "paused");
        assert_eq!(db_job.last_path, Some(last_at_step_1.clone()));

        // 4. 第二阶段：从断点恢复执行到底
        let completed_job = sm.run_or_resume(&job_id, None).expect("resume to complete");
        assert_eq!(completed_job.status, "completed", "恢复后应完成全部扫描");
        assert!(completed_job.total_assets >= total_assets_step_1, "总资产数应递增累加");
        assert!(completed_job.completed_at.is_some());

        // 5. 校验资产表中资产完整性
        let f1_asset = storage.get_asset_by_path(&root.join("a_dir/f1.py").to_string_lossy().to_string()).unwrap();
        let f4_asset = storage.get_asset_by_path(&root.join("b_dir/f4.toml").to_string_lossy().to_string()).unwrap();
        assert!(f1_asset.is_some(), "断点前的资产必须存在");
        assert!(f4_asset.is_some(), "断点后恢复扫描的资产必须存在");

        // 6. 再次调用 run_or_resume 应该幂等直接返回 completed
        let re_check = sm.run_or_resume(&job_id, None).expect("idempotent check");
        assert_eq!(re_check.status, "completed");
    }
}
