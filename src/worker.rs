// src/worker.rs
use anyhow::{Context, Result};
use chrono::{Months, NaiveDate, Utc};
use dashmap::DashMap;
use futures_util::TryStreamExt;
use reqwest::Client;
use std::{collections::HashSet, sync::Arc};
use teloxide::{
    prelude::*,
    types::ParseMode,
};
use tokio::{
    fs::OpenOptions,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
};

use crate::{
    bot::{UserState, purchase_store},
    config::Config,
    i18n::{Lang, lang_of},
    keyboards::purchase_action_keyboard,
    queue::{DbTask, SearchKind},
    shutdown::Shutdown,
    sold_store::SoldStore,
};

const CHUNK_SIZE: usize = 2000;

#[derive(Clone)]
pub struct WorkerDeps {
    pub cfg: Config,
    pub http: Client,

    /// ✅ активный kind для каждого user_id
    pub active_requests: Arc<DashMap<i64, SearchKind>>,

    pub bot: Bot,
    pub sold_store: SoldStore,
    pub user_states: Arc<DashMap<i64, UserState>>,
}

// =========================
// I18N (worker)
// =========================
fn t_no_available_including_sold(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "No available lines (including already sold).",
        Lang::Ru => "Нет доступных строк (включая уже проданные)",
    }
}

fn t_choose_action(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Choose an action:",
        Lang::Ru => "Выберите действие:",
    }
}

fn t_report_date(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "REPORT DATE",
        Lang::Ru => "📊 REPORT DATE",
    }
}

fn t_query(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "QUERY",
        Lang::Ru => "QUERY",
    }
}

fn t_lines(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "LINES",
        Lang::Ru => "LINES",
    }
}

// Функции возвращают только текст без форматирования
fn t_last3m_label(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "New lines",
        Lang::Ru => "Новые строки",
    }
}

fn t_old_label(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Old lines",
        Lang::Ru => "Старые строки",
    }
}

fn t_total_label(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Total",
        Lang::Ru => "Total",
    }
}

fn kind_label(lang: Lang, kind: &SearchKind) -> &'static str {
    match kind {
        SearchKind::Domain => match lang {
            Lang::En => "domain",
            Lang::Ru => "domain",
        },
        SearchKind::Port => match lang {
            Lang::En => "port",
            Lang::Ru => "port",
        },
        SearchKind::Subdomain => match lang {
            Lang::En => "subdomain",
            Lang::Ru => "subdomain",
        },
        SearchKind::Path => match lang {
            Lang::En => "path",
            Lang::Ru => "path",
        },
        SearchKind::Login => match lang {
            Lang::En => "login",
            Lang::Ru => "login",
        },
    }
}

pub async fn run_db_worker(
    mut shutdown: Shutdown,
    mut rx: tokio::sync::mpsc::Receiver<DbTask>,
    deps: WorkerDeps,
) {
    while !shutdown.is_cancelled() {
        tokio::select! {
            _ = shutdown.cancelled() => break,

            msg = rx.recv() => {
                let Some(task) = msg else { break };

                let result = handle_task(&deps, &task).await;

                if let Err(e) = result {
                    // Ошибку можно тоже локализовать, но оставим как есть (не критично)
                    let _ = deps.bot
                        .send_message(task.chat_id, format!("Ошибка выполнения запроса: {}", e))
                        .await;
                }

                // ✅ всегда снимаем "активный запрос" после завершения (успех/ошибка)
                deps.active_requests.remove(&task.user_id);
            }
        }
    }
}

async fn handle_task(deps: &WorkerDeps, task: &DbTask) -> Result<()> {
    let lang = lang_of(task.user_id);

    tokio::fs::create_dir_all("Notes").await.ok();

    // Для Login: один файл
    // Для остальных: 2 файла (3month/old)
    let (file_new, file_old) = match task.kind {
        SearchKind::Login => {
            let one = format!(
                "Notes/{}_{}.txt",
                format_kind(&task.kind),
                sanitize(&task.query),
            );
            (one, String::new())
        }
        _ => {
            let f_new = format!(
                "Notes/{}_{}_3month.txt",
                format_kind(&task.kind),
                sanitize(&task.query)
            );
            let f_old = format![
                "Notes/{}_{}_old.txt",
                format_kind(&task.kind),
                sanitize(&task.query)
            ];
            (f_new, f_old)
        }
    };

    let mut f_new = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&file_new)
        .await?;

    let mut f_old_opt = if matches!(task.kind, SearchKind::Login) {
        None
    } else {
        Some(
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&file_old)
                .await?,
        )
    };

    let today = Utc::now().date_naive();
    let today_str = today.format("%Y-%m-%d").to_string();

    // threshold для split
    let threshold = today.checked_sub_months(Months::new(3)).unwrap();

    // SQL + params
    let (sql, params) = build_sql(&task.kind, &task.query);

    let resp = deps
        .http
        .post(deps.cfg.ch_base_url())
        .basic_auth(&deps.cfg.ch_user, Some(&deps.cfg.ch_password))
        .query(&[("database", deps.cfg.ch_database.as_str())])
        .query(&params)
        .body(sql)
        .send()
        .await
        .context("clickhouse request failed")?
        .error_for_status()?;

    let stream = resp.bytes_stream();
    let reader = tokio_util::io::StreamReader::new(
        stream.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)),
    );
    let mut lines = BufReader::new(reader).lines();

    let mut cnt_new = 0u64;
    let mut cnt_old = 0u64;

    let mut unique: HashSet<(String, String, String)> = HashSet::new();
    let mut preview_entries: Vec<String> = Vec::new();
    let mut buf: Vec<String> = Vec::with_capacity(CHUNK_SIZE);

    while let Some(line) = lines.next_line().await? {
        buf.push(line);

        if buf.len() >= CHUNK_SIZE {
            if matches!(task.kind, SearchKind::Login) {
                process_chunk_nosplit(
                    deps,
                    &task.kind,
                    &mut buf,
                    &mut f_new,
                    &mut cnt_new,
                    &mut unique,
                    &mut preview_entries,
                )
                .await?;
            } else {
                let f_old = f_old_opt.as_mut().expect("old file must exist");
                process_chunk_split(
                    deps,
                    &task.kind,
                    &mut buf,
                    threshold,
                    &mut f_new,
                    f_old,
                    &mut cnt_new,
                    &mut cnt_old,
                    &mut unique,
                    &mut preview_entries,
                )
                .await?;
            }
        }
    }

    if !buf.is_empty() {
        if matches!(task.kind, SearchKind::Login) {
            process_chunk_nosplit(
                deps,
                &task.kind,
                &mut buf,
                &mut f_new,
                &mut cnt_new,
                &mut unique,
                &mut preview_entries,
            )
            .await?;
        } else {
            let f_old = f_old_opt.as_mut().expect("old file must exist");
            process_chunk_split(
                deps,
                &task.kind,
                &mut buf,
                threshold,
                &mut f_new,
                f_old,
                &mut cnt_new,
                &mut cnt_old,
                &mut unique,
                &mut preview_entries,
            )
            .await?;
        }
    }

    f_new.flush().await?;
    if let Some(f_old) = f_old_opt.as_mut() {
        f_old.flush().await?;
    }

    let total = cnt_new + cnt_old;
    if total == 0 {
        deps.bot
            .send_message(task.chat_id, t_no_available_including_sold(lang))
            .await?;
        return Ok(());
    }

    // ИСПРАВЛЕНО: Формирование таблицы с фиксированными размерами колонок и пробелами вокруг даты
    // Колонка 1: 14 символов (текст влево)
    // Колонка 2: 27 символов (25 дата + пробелы по бокам)
    // Колонка 3: 12 символов (текст вправо)
    
    // Форматируем числа с выравниванием вправо (12 символов)
    let cnt_new_str = format!("{:>12}", cnt_new);
    let cnt_old_str = format!("{:>12}", cnt_old);
    let total_str = format!("{:>12}", total);
    
    // Формируем даты с пробелами по бокам
    let start_3m = (today - chrono::Duration::days(90)).format("%d_%b_%Y");
    let end_3m = today.format("%d_%b_%Y");
    let date_range_3m = format!(" {} → {} ", start_3m, end_3m); // Пробелы в начале и конце
    // Убеждаемся что дата с пробелами занимает ровно 27 символов
    let date_range_3m = format!("{:^27}", date_range_3m);
    
    let start_old = NaiveDate::from_ymd_opt(2018, 6, 1).unwrap();
    let date_range_old = format!(" {} → {} ", start_old.format("%d_%b_%Y"), end_3m); // Пробелы в начале и конце
    let date_range_old = format!("{:^27}", date_range_old);
    
    // Получаем локализованные метки
    let new_lines_label = t_last3m_label(lang);
    let old_lines_label = t_old_label(lang);
    let total_label = t_total_label(lang);
    
    // Формируем заголовок с фиксированными размерами
    let header = format!(
        "{:<14}|{:^27}|{:>12}",
        "CATEGORY",
        " DATE RANGE ",
        "LINES"
    );
    
    // Разделитель (14 + 1 + 27 + 1 + 12 = 55 символов)
    let separator = "-".repeat(55);
    
    // Формируем строки данных с фиксированными размерами
    let new_lines_row = format!(
        "{:<14}|{}|{}",
        new_lines_label,
        date_range_3m,
        cnt_new_str
    );
    
    let old_lines_row = format!(
        "{:<14}|{}|{}",
        old_lines_label,
        date_range_old,
        cnt_old_str
    );
    
    // Формируем итоговую строку (Total с пустым центром)
    let total_row = format!(
        "{:<14}|{:^27}|{}",
        total_label,
        "",
        total_str
    );

    // Собираем весь отчет
    let report = format!(
        "<pre>📊 REPORT DATE: {}\n\n{}\n{}\n{}\n{}\n{}\n{}\n</pre>",
        today_str,
        header,
        separator,
        new_lines_row,
        old_lines_row,
        separator,
        total_row,
    );

    deps.bot
        .send_message(task.chat_id, report)
        .parse_mode(ParseMode::Html)
        .await?;

    // Purchase store
    purchase_store().insert(
        task.user_id,
        crate::bot::PurchaseData {
            kind: task.kind.clone(),
            query: task.query.clone(),
            file_new: file_new.clone(),
            file_old: file_old.clone(),
            cnt_new: cnt_new as usize,
            cnt_old: cnt_old as usize,
            updated_at: std::time::SystemTime::now(),
        },
    );

    // =========================
    // Клавиатура покупки
    deps.user_states
        .insert(task.user_id, UserState::WaitingPurchaseAction);

    deps.bot
        .send_message(task.chat_id, t_choose_action(lang))
        .reply_markup(purchase_action_keyboard(
            lang,
            &task.kind,
            cnt_new as usize,
            cnt_old as usize,
        ))
        .await?;

    Ok(())
}

/// Split-режим: деление на 3 месяца / old
async fn process_chunk_split(
    deps: &WorkerDeps,
    kind: &SearchKind,
    buf: &mut Vec<String>,
    threshold: NaiveDate,
    f_new: &mut tokio::fs::File,
    f_old: &mut tokio::fs::File,
    cnt_new: &mut u64,
    cnt_old: &mut u64,
    unique: &mut HashSet<(String, String, String)>,
    preview_entries: &mut Vec<String>,
) -> Result<()> {
    struct Row {
        main_domain: String,
        id: String,
        url: String,
        login: String,
        pass: String,
        created: String,
    }

    let mut rows: Vec<Row> = Vec::new();
    let mut keys: Vec<[u8; 32]> = Vec::new();

    for line in buf.iter() {
        let mut p = line.split('\t');

        let main_domain = p.next().unwrap_or("").trim().to_string();
        let id = p.next().unwrap_or("").trim().to_string();
        let url = p.next().unwrap_or("").trim().to_string();
        let login = p.next().unwrap_or("").trim().to_string();
        let pass = p.next().unwrap_or("").trim().to_string();
        let created = p.next().unwrap_or("").trim().to_string();

        if main_domain.is_empty() || login.is_empty() || pass.is_empty() {
            continue;
        }

        let key = SoldStore::make_key(&main_domain, &login, &pass);

        rows.push(Row {
            main_domain,
            id,
            url,
            login,
            pass,
            created,
        });
        keys.push(key);
    }

    if rows.is_empty() {
        buf.clear();
        return Ok(());
    }

    let exists = deps.sold_store.filter_existing_batch(keys).await?;

    for (row, is_sold) in rows.into_iter().zip(exists.into_iter()) {
        if is_sold {
            continue;
        }

        if !unique.insert((row.main_domain.clone(), row.login.clone(), row.pass.clone())) {
            continue;
        }

        let date = match NaiveDate::parse_from_str(&row.created, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => continue,
        };

        let out_line = format!(
            "{}\t{}\t{}\t{}\t{}\t{}\n",
            row.main_domain, row.id, row.url, row.login, row.pass, row.created
        );

        if date >= threshold {
            f_new.write_all(out_line.as_bytes()).await?;
            *cnt_new += 1;
        } else {
            f_old.write_all(out_line.as_bytes()).await?;
            *cnt_old += 1;
        }

        if preview_entries.len() < 30 {
            let preview_line = make_preview_line(kind, &row.url, &row.login, &row.pass);
            preview_entries.push(preview_line);
        }
    }

    buf.clear();
    Ok(())
}

/// No-split режим (Login): пишем всё в один файл
async fn process_chunk_nosplit(
    deps: &WorkerDeps,
    kind: &SearchKind,
    buf: &mut Vec<String>,
    f_out: &mut tokio::fs::File,
    cnt: &mut u64,
    unique: &mut HashSet<(String, String, String)>,
    preview_entries: &mut Vec<String>,
) -> Result<()> {
    struct Row {
        main_domain: String,
        id: String,
        url: String,
        login: String,
        pass: String,
        created: String,
    }

    let mut rows: Vec<Row> = Vec::new();
    let mut keys: Vec<[u8; 32]> = Vec::new();

    for line in buf.iter() {
        let mut p = line.split('\t');

        let main_domain = p.next().unwrap_or("").trim().to_string();
        let id = p.next().unwrap_or("").trim().to_string();
        let url = p.next().unwrap_or("").trim().to_string();
        let login = p.next().unwrap_or("").trim().to_string();
        let pass = p.next().unwrap_or("").trim().to_string();
        let created = p.next().unwrap_or("").trim().to_string();

        if main_domain.is_empty() || login.is_empty() || pass.is_empty() {
            continue;
        }

        let key = SoldStore::make_key(&main_domain, &login, &pass);

        rows.push(Row {
            main_domain,
            id,
            url,
            login,
            pass,
            created,
        });
        keys.push(key);
    }

    if rows.is_empty() {
        buf.clear();
        return Ok(());
    }

    let exists = deps.sold_store.filter_existing_batch(keys).await?;

    for (row, is_sold) in rows.into_iter().zip(exists.into_iter()) {
        if is_sold {
            continue;
        }

        if !unique.insert((row.main_domain.clone(), row.login.clone(), row.pass.clone())) {
            continue;
        }

        let out_line = format!(
            "{}\t{}\t{}\t{}\t{}\t{}\n",
            row.main_domain, row.id, row.url, row.login, row.pass, row.created
        );

        f_out.write_all(out_line.as_bytes()).await?;
        *cnt += 1;

        if preview_entries.len() < 30 {
            let preview_line = make_preview_line(kind, &row.url, &row.login, &row.pass);
            preview_entries.push(preview_line);
        }
    }

    buf.clear();
    Ok(())
}

/// SQL builder
fn build_sql(kind: &SearchKind, q: &str) -> (String, Vec<(&'static str, String)>) {
    match kind {
        SearchKind::Domain => (
            r#"
SELECT
    main_domain,
    id,
    url_full,
    login,
    password,
    created_date
FROM leak_data
WHERE main_domain = {q:String}
FORMAT TSV
"#
            .to_string(),
            vec![("param_q", q.to_string())],
        ),

        SearchKind::Port => (
            r#"
SELECT
    main_domain,
    id,
    url_full,
    login,
    password,
    created_date
FROM leak_data
WHERE port = {q:String}
FORMAT TSV
"#
            .to_string(),
            vec![("param_q", q.to_string())],
        ),

        SearchKind::Subdomain => (
            r#"
SELECT
    main_domain,
    id,
    url_full,
    login,
    password,
    created_date
FROM leak_data
WHERE subdomain ILIKE concat('%', {q:String}, '%')
FORMAT TSV
"#
            .to_string(),
            vec![("param_q", q.to_string())],
        ),

        SearchKind::Path => (
            r#"
SELECT
    main_domain,
    id,
    url_full,
    login,
    password,
    created_date
FROM leak_data
WHERE path ILIKE concat('%', {q:String}, '%')
FORMAT TSV
"#
            .to_string(),
            vec![("param_q", q.to_string())],
        ),

        SearchKind::Login => (
            r#"
SELECT
    main_domain,
    id,
    url_full,
    login,
    password,
    created_date
FROM leak_data
WHERE login = {q:String}
FORMAT TSV
"#
            .to_string(),
            vec![("param_q", q.to_string())],
        ),
    }
}

fn make_preview_line(kind: &SearchKind, url: &str, login: &str, pass: &str) -> String {
    match kind {
        SearchKind::Domain => {
            let masked_login = mask_alt(login);
            format!("{url}\t{masked_login}\t{pass}\n")
        }
        SearchKind::Port | SearchKind::Path => {
            let masked_url = mask_host(url);
            format!("{masked_url}\t{login}\t{pass}\n")
        }
        SearchKind::Subdomain => {
            let masked_url = mask_after_dot(url);
            format!("{masked_url}\t{login}\t{pass}\n")
        }
        SearchKind::Login => {
            let masked_url = mask_host(url);
            format!("{masked_url}\t{login}\t{pass}\n")
        }
    }
}

fn mask_alt(s: &str) -> String {
    s.chars()
        .enumerate()
        .map(|(i, c)| if i % 2 == 1 { '*' } else { c })
        .collect()
}

fn mask_host(url: &str) -> String {
    if url.len() < 8 {
        return url.to_string();
    }
    let start = 8;

    let end = url[start..]
        .find(|c| c == ':' || c == '/')
        .map(|i| start + i)
        .unwrap_or(url.len());

    let host = &url[start..end];
    if host.is_empty() {
        return url.to_string();
    }

    let masked = mask_alt(host);
    format!("{}{}{}", &url[..start], masked, &url[end..])
}

fn mask_after_dot(url: &str) -> String {
    if url.len() < 8 {
        return url.to_string();
    }
    let start = 8;

    let end = url[start..]
        .find(|c| c == ':' || c == '/')
        .map(|i| start + i)
        .unwrap_or(url.len());

    let host = &url[start..end];

    if let Some(dot) = host.find('.') {
        let (left, right) = host.split_at(dot + 1);
        if right.is_empty() {
            return url.to_string();
        }
        let masked_right = mask_alt(right);
        format!("{}{}{}{}", &url[..start], left, masked_right, &url[end..])
    } else {
        mask_host(url)
    }
}

fn format_kind(k: &SearchKind) -> &'static str {
    match k {
        SearchKind::Domain => "domain",
        SearchKind::Port => "port",
        SearchKind::Subdomain => "subdomain",
        SearchKind::Path => "path",
        SearchKind::Login => "login",
    }
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}