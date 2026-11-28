//! 零额外外部依赖的有界 RAII PostgreSQL 连接池（彻底消除单连接 `Arc<Mutex<Client>>` 串行瓶颈）

use std::collections::VecDeque;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use postgres::{Client, NoTls};

struct PoolState {
    idle: VecDeque<Client>,
    active_count: usize,
}

/// 线程安全有界 PostgreSQL 连接池
pub struct PgConnPool {
    url: String,
    max_size: usize,
    wait_timeout: Duration,
    state: Mutex<PoolState>,
    available: Condvar,
}

/// RAII 连接句柄：离开作用域自动归还健康连接或释放损坏槽位
pub struct PooledClient {
    pool: Arc<PgConnPool>,
    client: Option<Client>,
    broken: bool,
}

impl PgConnPool {
    pub fn new(url: String, max_size: usize) -> Result<Arc<Self>> {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            anyhow::bail!("PostgreSQL 连接串不能为空");
        }
        let capped = max_size.clamp(1, 64);
        let pool = Arc::new(Self {
            url: trimmed.to_string(),
            max_size: capped,
            wait_timeout: Duration::from_secs(15),
            state: Mutex::new(PoolState {
                idle: VecDeque::with_capacity(capped),
                active_count: 0,
            }),
            available: Condvar::new(),
        });
        // 启动期首连验证，及早暴露不可达错误供调用方降级
        let conn = Self::dial(&pool.url)?;
        {
            let mut guard = pool
                .state
                .lock()
                .map_err(|e| anyhow!("连接池状态锁异常: {}", e))?;
            guard.idle.push_back(conn);
        }
        Ok(pool)
    }

    fn dial(url: &str) -> Result<Client> {
        // 在独立线程建立同步 PG 客户端，避免调用方位于 Tokio runtime 内触发 block_on 冲突
        let owned = url.to_string();
        std::thread::spawn(move || Client::connect(&owned, NoTls))
            .join()
            .map_err(|_| anyhow!("连接 PostgreSQL 线程异常退出"))?
            .with_context(|| "连接 PostgreSQL 事务记忆库失败")
    }

    /// 借出一个可用 PostgreSQL 连接（空闲优先，自动探活重连）
    pub fn checkout(self: &Arc<Self>) -> Result<PooledClient> {
        let mut guard = self
            .state
            .lock()
            .map_err(|e| anyhow!("连接池状态锁异常: {}", e))?;

        let deadline = std::time::Instant::now() + self.wait_timeout;
        loop {
            if let Some(mut client) = guard.idle.pop_front() {
                guard.active_count += 1;
                drop(guard);
                if client.is_closed() {
                    match Self::dial(&self.url) {
                        Ok(fresh) => client = fresh,
                        Err(e) => {
                            self.release_slot();
                            return Err(e);
                        }
                    }
                }
                return Ok(PooledClient {
                    pool: Arc::clone(self),
                    client: Some(client),
                    broken: false,
                });
            }

            if guard.active_count < self.max_size {
                guard.active_count += 1;
                drop(guard);
                match Self::dial(&self.url) {
                    Ok(client) => {
                        return Ok(PooledClient {
                            pool: Arc::clone(self),
                            client: Some(client),
                            broken: false,
                        });
                    }
                    Err(e) => {
                        self.release_slot();
                        return Err(e);
                    }
                }
            }

            let now = std::time::Instant::now();
            if now >= deadline {
                anyhow::bail!(
                    "PostgreSQL 事务记忆连接池已满 (max_size={})，等待超时",
                    self.max_size
                );
            }
            let remaining = deadline.saturating_duration_since(now);
            let (next_guard, wait_res) = self
                .available
                .wait_timeout(guard, remaining)
                .map_err(|e| anyhow!("连接池条件变量等待异常: {}", e))?;
            guard = next_guard;
            if wait_res.timed_out() && guard.idle.is_empty() && guard.active_count >= self.max_size
            {
                anyhow::bail!(
                    "PostgreSQL 事务记忆连接池等待超时 (max_size={})",
                    self.max_size
                );
            }
        }
    }

    pub fn max_size(&self) -> usize {
        self.max_size
    }

    pub fn stats(&self) -> (usize, usize, usize) {
        match self.state.lock() {
            Ok(g) => (g.active_count, g.idle.len(), self.max_size),
            Err(_) => (0, 0, self.max_size),
        }
    }

    fn release_slot(&self) {
        if let Ok(mut guard) = self.state.lock() {
            guard.active_count = guard.active_count.saturating_sub(1);
            self.available.notify_one();
        }
    }

    fn return_client(&self, client: Client, broken: bool) {
        if let Ok(mut guard) = self.state.lock() {
            guard.active_count = guard.active_count.saturating_sub(1);
            if !broken && !client.is_closed() && guard.idle.len() < self.max_size {
                guard.idle.push_back(client);
            }
            self.available.notify_one();
        }
    }
}

impl PooledClient {
    pub fn mark_broken(&mut self) {
        self.broken = true;
    }
}

impl Deref for PooledClient {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        match self.client.as_ref() {
            Some(c) => c,
            None => unreachable!("PooledClient 在 Drop 前必然持有有效连接"),
        }
    }
}

impl DerefMut for PooledClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self.client.as_mut() {
            Some(c) => c,
            None => unreachable!("PooledClient 在 Drop 前必然持有有效连接"),
        }
    }
}

impl Drop for PooledClient {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            self.pool.return_client(client, self.broken);
        }
    }
}
