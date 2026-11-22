pub mod handler;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;
use anyhow::{Context, Result};
use notify::{Config as NotifyConfig, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChangeKind {
    Created,
    Modified,
    Deleted,
}

#[derive(Debug, Clone)]
pub struct FileChangeEvent {
    pub path: PathBuf,
    pub kind: FileChangeKind,
}

pub struct FileWatcher {
    watcher: std::sync::Mutex<RecommendedWatcher>,
    rx: Receiver<notify::Result<Event>>,
}

impl FileWatcher {
    /// 创建文件监听器并绑定待监听目录（跳过 target/.git/node_modules 等黑名单目录，避免数千个无效 inotify 句柄）
    pub fn new(watch_paths: &[PathBuf]) -> Result<Self> {
        let (tx, rx) = channel();

        let mut watcher = RecommendedWatcher::new(tx, NotifyConfig::default().with_poll_interval(Duration::from_millis(500)))
            .context("创建 inotify Watcher 失败")?;

        for p in watch_paths {
            if p.exists() {
                watch_dir_pruned(&mut watcher, p)?;
            }
        }

        Ok(Self {
            watcher: std::sync::Mutex::new(watcher),
            rx,
        })
    }

    /// 尝试拉取一组防抖后的变更事件 (非阻塞)
    pub fn poll_events(&self) -> Vec<FileChangeEvent> {
        let mut events = Vec::new();

        while let Ok(res) = self.rx.try_recv() {
            if let Ok(event) = res {
                let kind = match event.kind {
                    EventKind::Create(_) => FileChangeKind::Created,
                    EventKind::Modify(_) => FileChangeKind::Modified,
                    EventKind::Remove(_) => FileChangeKind::Deleted,
                    _ => continue,
                };

                for path in event.paths {
                    // 忽略隐藏文件、临时交换文件、target 目录
                    if should_ignore_path(&path) {
                        continue;
                    }
                    // 若新创建了非黑名单子目录，动态补挂非递归监听
                    if kind == FileChangeKind::Created && path.is_dir() {
                        if let Ok(mut w) = self.watcher.lock() {
                            let _ = watch_dir_pruned(&mut w, &path);
                        }
                    }
                    events.push(FileChangeEvent { path, kind: kind.clone() });
                }
            }
        }

        // 简易防抖: 同一路径保留最新事件
        dedup_events(events)
    }
}

fn is_ignored_dir_name(name: &str) -> bool {
    matches!(
        name,
        "target" | ".git" | ".agent-body" | ".state" | "__pycache__" | ".codegraph" | ".venv" | "node_modules" | ".cache"
    )
}

fn watch_dir_pruned(watcher: &mut RecommendedWatcher, root: &Path) -> Result<()> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Some(name) = dir.file_name().and_then(|s| s.to_str()) {
            if is_ignored_dir_name(name) {
                continue;
            }
        }
        let _ = watcher.watch(&dir, RecursiveMode::NonRecursive);
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_dir() && !p.is_symlink() {
                    if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                        if !is_ignored_dir_name(name) {
                            stack.push(p);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn should_ignore_path(path: &Path) -> bool {
    let s = path.to_string_lossy();
    if s.ends_with('~')
        || s.ends_with(".tmp")
        || s.ends_with(".swp")
        || s.ends_with(".swx")
        || s.ends_with(".swo")
        || s.ends_with(".log")
        || s.ends_with(".pyc")
        || s.ends_with(".pyo")
    {
        return true;
    }
    if s.contains("/target/")
        || s.contains("/.git/")
        || s.contains("/.agent-body/")
        || s.contains("/.state/")
        || s.contains("/__pycache__/")
        || s.contains("/.codegraph/")
        || s.contains("/.venv/")
        || s.contains("/node_modules/")
    {
        return true;
    }
    false
}

fn dedup_events(events: Vec<FileChangeEvent>) -> Vec<FileChangeEvent> {
    use std::collections::HashMap;
    let mut map = HashMap::new();
    for e in events {
        map.insert(e.path.clone(), e);
    }
    map.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs::File;
    use std::io::Write;
    use std::thread::sleep;

    #[test]
    fn test_file_watcher_create() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().to_path_buf();

        let watcher = FileWatcher::new(&[path.clone()]).expect("new watcher");
        
        // 创建一个文件触发事件
        let file_path = path.join("test_watch.txt");
        let mut f = File::create(&file_path).expect("create file");
        writeln!(f, "hello watcher").expect("write");
        drop(f);

        // 等待 inotify 事件捕获
        sleep(Duration::from_millis(600));

        let events = watcher.poll_events();
        assert!(!events.is_empty(), "必须能捕获到文件创建/修改事件");
        assert!(events.iter().any(|e| e.path == file_path));
    }
}
