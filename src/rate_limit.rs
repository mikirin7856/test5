// src/rate_limit.rs
use anyhow::Result;
use dashmap::DashMap;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use crate::rules_ban::BanList;

#[derive(Clone)]
pub struct RateLimiter {
    map: Arc<DashMap<i64, Vec<Instant>>>,
    limit: usize,
    window: Duration,
    banlist: BanList,
}

impl RateLimiter {
    pub fn new(banlist: BanList) -> Self {
        Self {
            map: Arc::new(DashMap::new()),
            limit: 8,
            window: Duration::from_secs(10),
            banlist,
        }
    }

    /// Ok(true) -> разрешено
    /// Ok(false) -> запрещено (и уже забанен)
    pub async fn check(&self, user_id: i64) -> Result<bool> {
        if self.banlist.is_blocked(user_id) {
            return Ok(false);
        }

        let now = Instant::now();
        let mut entry = self.map.entry(user_id).or_insert_with(Vec::new);
        entry.retain(|t| now.duration_since(*t) <= self.window);
        entry.push(now);

        if entry.len() > self.limit {
            // баним
            self.banlist.ban(user_id).await?;
            self.map.remove(&user_id);
            return Ok(false);
        }

        Ok(true)
    }
}
