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
    _watcher: RecommendedWatcher,
    rx: Receiver<notify::Result<Event>>,
}

impl FileWatcher {
    /// 创建文件监听器并绑定待监听目录
    pub fn new(watch_paths: &[PathBuf]) -> Result<Self> {
        let (tx, rx) = channel();

        let mut watcher = RecommendedWatcher::new(tx, NotifyConfig::default().with_poll_interval(Duration::from_millis(500)))
            .context("创建 inotify Watcher 失败")?;

        for p in watch_paths {
            if p.exists() {
                watcher.watch(p, RecursiveMode::Recursive)
                    .with_context(|| format!("监听目录失败: {:?}", p))?;
            }
        }

        Ok(Self {
            _watcher: watcher,
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
                    events.push(FileChangeEvent { path, kind: kind.clone() });
                }
            }
        }

        // 简易防抖: 同一路径保留最新事件
        dedup_events(events)
    }
}

fn should_ignore_path(path: &Path) -> bool {
    let s = path.to_string_lossy();
    if s.ends_with('~') || s.ends_with(".tmp") || s.ends_with(".swp") {
        return true;
    }
    if s.contains("/target/") || s.contains("/.git/") || s.contains("/.steward/") {
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
