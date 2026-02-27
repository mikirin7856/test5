// src/rules_ban.rs
use anyhow::{Context, Result};
use dashmap::DashSet;
use std::{path::Path, sync::Arc};
use tokio::{fs::OpenOptions, io::AsyncWriteExt, sync::Mutex};

#[derive(Clone)]
pub struct BanList {
    blocked: Arc<DashSet<i64>>,
    file_path: Arc<String>,
    file_lock: Arc<Mutex<()>>, // сериализуем append
}

impl BanList {
    pub async fn load(file_path: String) -> Result<Self> {
        let set = DashSet::new();

        if Path::new(&file_path).exists() {
            let content = tokio::fs::read_to_string(&file_path)
                .await
                .with_context(|| format!("read blocked file {}", file_path))?;
            for line in content.lines() {
                if let Ok(id) = line.trim().parse::<i64>() {
                    set.insert(id);
                }
            }
        } else {
            // создадим пустой файл, чтобы append всегда работал
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file_path)
                .await?;
        }

        Ok(Self {
            blocked: Arc::new(set),
            file_path: Arc::new(file_path),
            file_lock: Arc::new(Mutex::new(())),
        })
    }

    pub fn is_blocked(&self, user_id: i64) -> bool {
        self.blocked.contains(&user_id)
    }

    pub async fn ban(&self, user_id: i64) -> Result<()> {
        if self.blocked.insert(user_id) {
            let _g = self.file_lock.lock().await;
            let mut f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.file_path.as_str())
                .await?;

            f.write_all(format!("{}\n", user_id).as_bytes()).await?;
            f.flush().await?;
        }
        Ok(())
    }
}
