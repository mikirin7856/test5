// src/main.rs
mod bot;
mod config;
mod helper;
mod i18n;
mod input_filter;
mod keyboards;
mod queue;
mod rate_limit;
mod rules_ban;
mod shutdown;
mod sold_store;
mod worker;

use anyhow::Result;
use dashmap::DashMap;
use std::sync::Arc;
use teloxide::prelude::*;
use tokio::sync::mpsc;

use crate::{
    config::Config,
    queue::SearchKind,
    rate_limit::RateLimiter,
    rules_ban::BanList,
    shutdown::shutdown_channel,
    sold_store::SoldStore,
    worker::{WorkerDeps, run_db_worker},
};

#[tokio::main]
async fn main() -> Result<()> {
    // ==========================
    // CONFIG + BOT
    // ==========================
    let cfg = Config::from_env()?;
    let bot = Bot::new(cfg.bot_token.clone());

    // ==========================
    // LOAD STORES
    // ==========================
    let banlist = BanList::load(cfg.blocked_file.clone()).await?;
    let rate = RateLimiter::new(banlist.clone());

    // ✅ Активные запросы: user_id -> SearchKind (чтобы показывать какой запрос активен)
    let active_requests = Arc::new(DashMap::<i64, SearchKind>::new());

    // FSM states
    let user_states = Arc::new(DashMap::<i64, bot::UserState>::new());

    // RocksDB sold store
    let sold_store = SoldStore::new("rocksdb_sold_lines").await?;

    // ==========================
    // DB QUEUE
    // ==========================
    let (db_tx, db_rx) = mpsc::channel::<queue::DbTask>(cfg.db_queue_maxsize);

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(cfg.query_timeout))
        .build()?;

    let (trigger, shutdown) = shutdown_channel();

    // ==========================
    // WORKER
    // ==========================
    let worker_deps = WorkerDeps {
        cfg: cfg.clone(),
        http,
        active_requests: active_requests.clone(),
        bot: bot.clone(),
        sold_store: sold_store.clone(),
        user_states: user_states.clone(),
    };

    let worker_handle = tokio::spawn(run_db_worker(shutdown.clone(), db_rx, worker_deps));

    // ==========================
    // TELEGRAM STATE
    // ==========================
    let state = bot::BotState {
        db_tx: db_tx.clone(),
        active_requests: active_requests.clone(),
        rate: rate.clone(),
        banlist: banlist.clone(),
        user_states: user_states.clone(),
        sold_store: sold_store.clone(),
    };

    let handler = Update::filter_message().endpoint({
        let state = state.clone();
        move |bot: Bot, msg: Message| {
            let state = state.clone();
            async move {
                if let Err(e) = bot::handle_message(bot, msg, state).await {
                    eprintln!("Bot handler error: {:?}", e);
                }
                Ok::<(), teloxide::RequestError>(())
            }
        }
    });

    let mut dispatcher = Dispatcher::builder(bot, handler).build();

    // ==========================
    // CTRL+C
    // ==========================
    let shutdown_signal = async {
        tokio::signal::ctrl_c().await.expect("ctrl+c");
        trigger.trigger();
    };

    // ==========================
    // RUN LOOP
    // ==========================
    tokio::select! {
        _ = dispatcher.dispatch() => {},
        _ = shutdown_signal => {},
    }

    // ==========================
    // GRACEFUL STOP
    // ==========================
    drop(db_tx);
    let _ = worker_handle.await;

    println!("Bot shutdown complete.");

    Ok(())
}
