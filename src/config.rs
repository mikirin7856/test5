// src/config.rs
use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub bot_token: String,

    pub ch_host: String,
    pub ch_port: u16,
    pub ch_user: String,
    pub ch_password: String,
    pub ch_database: String,

    pub blocked_file: String,

    pub db_queue_maxsize: usize,
    pub query_timeout: u64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let bot_token = std::env::var("BOT_TOKEN")
            .context("BOT_TOKEN not found in .env file")?;

        let ch_host = std::env::var("CH_HOST")?;
        let ch_port = std::env::var("CH_PORT")?.parse()?;
        let ch_user = std::env::var("CH_USER")?;
        let ch_password = std::env::var("CH_PASSWORD")
            .context("CH_PASSWORD not found in .env file")?;
        let ch_database = std::env::var("CH_DATABASE")?;

        let blocked_file = std::env::var("BLOCKED_FILE")?;
        let db_queue_maxsize = std::env::var("DB_QUEUE_MAXSIZE")?.parse()?;
        let query_timeout = std::env::var("QUERY_TIMEOUT")?.parse()?;

        Ok(Self {
            bot_token,
            ch_host,
            ch_port,
            ch_user,
            ch_password,
            ch_database,
            blocked_file,
            db_queue_maxsize,
            query_timeout,
        })
    }

    pub fn ch_base_url(&self) -> String {
        format!("http://{}:{}/", self.ch_host, self.ch_port)
    }
}