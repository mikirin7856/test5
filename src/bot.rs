use anyhow::Result;
use dashmap::DashMap;
use std::{sync::Arc, time::SystemTime};
use teloxide::{prelude::*, types::InputFile};
use tokio::{fs::OpenOptions, io::AsyncWriteExt, sync::mpsc};

use crate::{
    helper,
    i18n::{
        BTN_LANG_BACK, BTN_LANG_EN, BTN_LANG_RU, Lang, btn_buy_3m, btn_buy_all, btn_buy_old,
        btn_cancel, lang_of, user_lang_store,
    },
    input_filter::{
        validate_domain, validate_login_or_email, validate_path_prefix, validate_port,
        validate_subdomain_prefix,
    },
    keyboards::{
        amount_keyboard, btn_search_domain, btn_search_login, btn_search_path, btn_search_port,
        btn_search_subdomain, input_keyboard, language_keyboard, main_keyboard,
        purchase_action_keyboard,
    },
    queue::{DbTask, SearchKind},
    rate_limit::RateLimiter,
    rules_ban::BanList,
    sold_store::{SoldCandidate, SoldStore},
};

#[derive(Clone, Debug)]
pub struct PurchaseData {
    pub kind: SearchKind,
    pub query: String,
    pub file_new: String,
    pub file_old: String,
    pub cnt_new: usize,
    pub cnt_old: usize,
    pub updated_at: SystemTime,
}

static PURCHASE_STORE: std::sync::OnceLock<DashMap<i64, PurchaseData>> = std::sync::OnceLock::new();

pub fn purchase_store() -> &'static DashMap<i64, PurchaseData> {
    PURCHASE_STORE.get_or_init(DashMap::new)
}

#[derive(Clone, Debug)]
pub enum UserState {
    ChoosingLanguage,
    Idle,
    WaitingDomain,
    WaitingPort,
    WaitingSubdomain,
    WaitingPath,
    WaitingLogin,
    WaitingPurchaseAction,
    WaitingPurchaseAmount {
        kind: PurchaseKind,
        available: usize,
    },
}

#[derive(Clone, Debug)]
pub enum PurchaseKind {
    Last3Month,
    Old,
    All,
}

#[derive(Clone)]
pub struct BotState {
    pub db_tx: mpsc::Sender<DbTask>,
    pub active_requests: Arc<DashMap<i64, SearchKind>>,
    pub rate: RateLimiter,
    pub banlist: BanList,
    pub user_states: Arc<DashMap<i64, UserState>>,
    pub sold_store: SoldStore,
}

fn t_main_title(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Choose an action:",
        Lang::Ru => "Выберите действие:",
    }
}
fn t_choose_lang(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Choose language / Выберите язык:",
        Lang::Ru => "Выберите язык / Choose language:",
    }
}
fn t_cancelled(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Cancelled.",
        Lang::Ru => "Отменено.",
    }
}
fn t_enter_number(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Enter a number.",
        Lang::Ru => "Введите число.",
    }
}

fn t_invalid_action_selection(lang: Lang) -> &'static str {
    match lang {
        Lang::En => {
            "You did not select a valid option. Please choose an action using the buttons or press Back."
        }
        Lang::Ru => {
            "Вы выбрали неверный вариант. Пожалуйста, выберите действие кнопками или нажмите Назад."
        }
    }
}
fn t_available_prefix(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Available:",
        Lang::Ru => "Доступно:",
    }
}
fn t_no_data(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "No data.",
        Lang::Ru => "Нет данных.",
    }
}
fn t_first_search(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Run a search first.",
        Lang::Ru => "Сначала выполните поиск.",
    }
}
fn t_no_lines(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "No lines.",
        Lang::Ru => "Нет строк.",
    }
}
fn t_no_available_lines(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "No available lines.",
        Lang::Ru => "Нет доступных строк.",
    }
}
fn t_ready_sending(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Done. Sending file.",
        Lang::Ru => "Готово. Отправляю файл.",
    }
}
fn t_queue_overloaded(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Queue is overloaded.",
        Lang::Ru => "Очередь перегружена.",
    }
}

fn t_busy_with_kind(lang: Lang, kind: &SearchKind) -> String {
    let label = search_kind_label(lang, kind);
    match lang {
        Lang::En => format!("You already have an active request [{label}]. Please wait."),
        Lang::Ru => format!("У вас уже есть активный запрос [{label}]. Дождитесь завершения."),
    }
}

fn prompt_enter_domain(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Enter domain (example: example.com)",
        Lang::Ru => "Введите домен (пример: example.com)",
    }
}
fn prompt_enter_port(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Enter port (example: 22)",
        Lang::Ru => "Введите порт (пример: 22)",
    }
}
fn prompt_enter_subdomain(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Enter subdomain prefix (example: admin)",
        Lang::Ru => "Введите начало субдомена (пример: admin)",
    }
}
fn prompt_enter_path(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Enter URL path prefix (example: /wp-login.php)",
        Lang::Ru => "Введите начало пути урла (пример: /wp-login.php)",
    }
}
fn prompt_enter_login(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Enter login (example: example@mail.com)",
        Lang::Ru => "Введите login (пример: example@mail.com)",
    }
}
fn err_bad_domain(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Invalid domain format.",
        Lang::Ru => "Неверный формат домена.",
    }
}
fn err_bad_port(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Invalid port format.",
        Lang::Ru => "Неверный формат порта.",
    }
}
fn err_bad_generic(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Invalid format.",
        Lang::Ru => "Неверный формат.",
    }
}
fn err_bad_login(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Invalid login/email format.",
        Lang::Ru => "Неверный формат login/email.",
    }
}
fn prompt_enter_amount(lang: Lang, available: usize) -> String {
    match lang {
        Lang::En => format!("Enter number of lines (available: {available})."),
        Lang::Ru => format!("Введите количество строк (доступно: {available})."),
    }
}

pub async fn handle_message(bot: Bot, msg: Message, state: BotState) -> Result<()> {
    let Some(text) = msg.text() else {
        return Ok(());
    };
    let text = text.trim();

    let user_id = msg.from().map(|u| u.id.0 as i64).unwrap_or(0);
    let chat_id = msg.chat.id;

    if state.banlist.is_blocked(user_id) {
        bot.send_message(chat_id, helper::blocked_msg()).await?;
        return Ok(());
    }

    if !state.rate.check(user_id).await? {
        bot.send_message(chat_id, helper::rate_limited_msg())
            .await?;
        return Ok(());
    }

    let lang = lang_of(user_id);
    let current_state = state
        .user_states
        .get(&user_id)
        .map(|s| s.clone())
        .unwrap_or(UserState::ChoosingLanguage);

    if text == "/start" || text == BTN_LANG_BACK {
        set_state(&state, user_id, UserState::ChoosingLanguage);
        bot.send_message(chat_id, t_choose_lang(lang))
            .reply_markup(language_keyboard())
            .await?;
        return Ok(());
    }

    if text == BTN_LANG_EN {
        user_lang_store().insert(user_id, Lang::En);
        set_state(&state, user_id, UserState::Idle);
        show_main_menu(&bot, chat_id, Lang::En).await?;
        return Ok(());
    }
    if text == BTN_LANG_RU {
        user_lang_store().insert(user_id, Lang::Ru);
        set_state(&state, user_id, UserState::Idle);
        show_main_menu(&bot, chat_id, Lang::Ru).await?;
        return Ok(());
    }

    if text == btn_cancel(lang) {
        set_state(&state, user_id, UserState::Idle);
        bot.send_message(chat_id, t_cancelled(lang))
            .reply_markup(main_keyboard(lang))
            .await?;
        return Ok(());
    }

    if handle_search_buttons(&bot, chat_id, &state, user_id, text).await? {
        return Ok(());
    }

    match current_state {
        UserState::ChoosingLanguage => {
            bot.send_message(chat_id, t_choose_lang(lang))
                .reply_markup(language_keyboard())
                .await?;
        }
        UserState::WaitingDomain => {
            let q = text.to_lowercase();
            if validate_domain(&q).is_err() {
                bot.send_message(chat_id, err_bad_domain(lang))
                    .reply_markup(input_keyboard(lang))
                    .await?;
                return Ok(());
            }
            enqueue(&bot, &state, user_id, chat_id, SearchKind::Domain, q).await?;
        }
        UserState::WaitingPort => {
            let q = text.to_string();
            if validate_port(&q).is_err() {
                bot.send_message(chat_id, err_bad_port(lang))
                    .reply_markup(input_keyboard(lang))
                    .await?;
                return Ok(());
            }
            enqueue(&bot, &state, user_id, chat_id, SearchKind::Port, q).await?;
        }
        UserState::WaitingSubdomain => {
            let q = text.to_lowercase();
            if validate_subdomain_prefix(&q).is_err() {
                bot.send_message(chat_id, err_bad_generic(lang))
                    .reply_markup(input_keyboard(lang))
                    .await?;
                return Ok(());
            }
            enqueue(&bot, &state, user_id, chat_id, SearchKind::Subdomain, q).await?;
        }
        UserState::WaitingPath => {
            let q = text.to_string();
            if validate_path_prefix(&q).is_err() {
                bot.send_message(chat_id, err_bad_generic(lang))
                    .reply_markup(input_keyboard(lang))
                    .await?;
                return Ok(());
            }
            enqueue(&bot, &state, user_id, chat_id, SearchKind::Path, q).await?;
        }
        UserState::WaitingLogin => {
            let q = text.to_string();
            if validate_login_or_email(&q).is_err() {
                bot.send_message(chat_id, err_bad_login(lang))
                    .reply_markup(input_keyboard(lang))
                    .await?;
                return Ok(());
            }
            enqueue(&bot, &state, user_id, chat_id, SearchKind::Login, q).await?;
        }
        UserState::WaitingPurchaseAmount { kind, available } => {
            handle_purchase_amount(&bot, chat_id, &state, user_id, kind, available, text).await?;
        }
        UserState::WaitingPurchaseAction => {
            if is_buy_button(lang, text) {
                handle_buy_button(&bot, chat_id, &state, user_id, text).await?;
            } else if let Some(data_ref) = purchase_store().get(&user_id) {
                let data = data_ref.clone();
                bot.send_message(chat_id, t_invalid_action_selection(lang))
                    .reply_markup(purchase_action_keyboard(
                        lang,
                        &data.kind,
                        data.cnt_new,
                        data.cnt_old,
                    ))
                    .await?;
            } else {
                set_state(&state, user_id, UserState::Idle);
                show_main_menu(&bot, chat_id, lang).await?;
            }
        }
        UserState::Idle => {
            if is_buy_button(lang, text) {
                handle_buy_button(&bot, chat_id, &state, user_id, text).await?;
            } else {
                show_main_menu(&bot, chat_id, lang).await?;
            }
        }
    }

    Ok(())
}

fn set_state(state: &BotState, user_id: i64, s: UserState) {
    state.user_states.insert(user_id, s);
}
async fn show_main_menu(bot: &Bot, chat_id: ChatId, lang: Lang) -> Result<()> {
    bot.send_message(chat_id, t_main_title(lang))
        .reply_markup(main_keyboard(lang))
        .await?;
    Ok(())
}
fn is_buy_button(lang: Lang, text: &str) -> bool {
    text == btn_buy_3m(lang) || text == btn_buy_old(lang) || text == btn_buy_all(lang)
}
fn search_kind_label(lang: Lang, k: &SearchKind) -> &'static str {
    match k {
        SearchKind::Domain => btn_search_domain(lang),
        SearchKind::Port => btn_search_port(lang),
        SearchKind::Subdomain => btn_search_subdomain(lang),
        SearchKind::Path => btn_search_path(lang),
        SearchKind::Login => btn_search_login(lang),
    }
}

async fn deny_if_busy(bot: &Bot, chat_id: ChatId, state: &BotState, user_id: i64) -> Result<bool> {
    let lang = lang_of(user_id);
    if let Some(kind_ref) = state.active_requests.get(&user_id) {
        let msg = t_busy_with_kind(lang, &kind_ref);
        bot.send_message(chat_id, msg)
            .reply_markup(main_keyboard(lang))
            .await?;
        return Ok(true);
    }
    Ok(false)
}

async fn handle_search_buttons(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    user_id: i64,
    text: &str,
) -> Result<bool> {
    let lang = lang_of(user_id);
    let route = [
        (
            btn_search_domain(lang),
            UserState::WaitingDomain,
            prompt_enter_domain(lang),
        ),
        (
            btn_search_port(lang),
            UserState::WaitingPort,
            prompt_enter_port(lang),
        ),
        (
            btn_search_subdomain(lang),
            UserState::WaitingSubdomain,
            prompt_enter_subdomain(lang),
        ),
        (
            btn_search_path(lang),
            UserState::WaitingPath,
            prompt_enter_path(lang),
        ),
        (
            btn_search_login(lang),
            UserState::WaitingLogin,
            prompt_enter_login(lang),
        ),
    ];

    for (button, next_state, prompt) in route {
        if text == button {
            if deny_if_busy(bot, chat_id, state, user_id).await? {
                return Ok(true);
            }
            set_state(state, user_id, next_state);
            bot.send_message(chat_id, prompt)
                .reply_markup(input_keyboard(lang))
                .await?;
            return Ok(true);
        }
    }

    Ok(false)
}

async fn handle_buy_button(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    user_id: i64,
    text: &str,
) -> Result<()> {
    let lang = lang_of(user_id);
    let Some(data_ref) = purchase_store().get(&user_id) else {
        bot.send_message(chat_id, t_first_search(lang))
            .reply_markup(main_keyboard(lang))
            .await?;
        return Ok(());
    };

    let data = data_ref.clone();
    let (kind, available) = if text == btn_buy_3m(lang) {
        (PurchaseKind::Last3Month, data.cnt_new)
    } else if text == btn_buy_old(lang) {
        (PurchaseKind::Old, data.cnt_old)
    } else if text == btn_buy_all(lang) {
        (PurchaseKind::All, data.cnt_new)
    } else {
        return Ok(());
    };

    if available == 0 {
        bot.send_message(chat_id, t_no_lines(lang))
            .reply_markup(main_keyboard(lang))
            .await?;
        return Ok(());
    }

    set_state(
        state,
        user_id,
        UserState::WaitingPurchaseAmount {
            kind: kind.clone(),
            available,
        },
    );
    bot.send_message(chat_id, prompt_enter_amount(lang, available))
        .reply_markup(amount_keyboard(lang))
        .await?;
    Ok(())
}

async fn handle_purchase_amount(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    user_id: i64,
    kind: PurchaseKind,
    available: usize,
    text: &str,
) -> Result<()> {
    let lang = lang_of(user_id);
    let requested: usize = match text.parse() {
        Ok(n) => n,
        Err(_) => {
            bot.send_message(chat_id, t_enter_number(lang))
                .reply_markup(amount_keyboard(lang))
                .await?;
            return Ok(());
        }
    };

    if requested == 0 || requested > available {
        bot.send_message(
            chat_id,
            format!("{} {}", t_available_prefix(lang), available),
        )
        .reply_markup(amount_keyboard(lang))
        .await?;
        return Ok(());
    }

    let Some(data_ref) = purchase_store().get(&user_id) else {
        bot.send_message(chat_id, t_no_data(lang))
            .reply_markup(main_keyboard(lang))
            .await?;
        set_state(state, user_id, UserState::Idle);
        return Ok(());
    };
    let data = data_ref.clone();

    let source_path = match kind {
        PurchaseKind::Last3Month | PurchaseKind::All => data.file_new.clone(),
        PurchaseKind::Old => data.file_old.clone(),
    };

    let content = tokio::fs::read_to_string(&source_path).await?;
    let mut ordered = Vec::new();
    let mut output_by_key = std::collections::HashMap::new();

    for line in content.lines() {
        let mut p = line.split('\t');
        let main_domain = p.next().unwrap_or("").trim();
        let _id = p.next();
        let url = p.next().unwrap_or("").trim();
        let login = p.next().unwrap_or("").trim();
        let pass = p.next().unwrap_or("").trim();

        if main_domain.is_empty() || url.is_empty() || login.is_empty() || pass.is_empty() {
            continue;
        }

        let key = format!("{main_domain}\u{0}{login}\u{0}{pass}");
        if output_by_key.contains_key(&key) {
            continue;
        }

        output_by_key.insert(key.clone(), format!("{url}\t{login}\t{pass}\n"));
        ordered.push(SoldCandidate {
            main_domain: main_domain.to_string(),
            login: login.to_string(),
            password: pass.to_string(),
        });
    }

    let claimed = state.sold_store.claim_unsold(ordered, requested).await?;

    if claimed.is_empty() {
        bot.send_message(chat_id, t_no_available_lines(lang))
            .reply_markup(main_keyboard(lang))
            .await?;
        set_state(state, user_id, UserState::Idle);
        return Ok(());
    }

    tokio::fs::create_dir_all("Notes").await.ok();
    let filename = format!(
        "result_{}_{}_{}.txt",
        format_kind(&data.kind),
        data.query,
        user_id
    );
    let out_path = format!("Notes/{}", sanitize_filename(&filename));

    let mut f = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&out_path)
        .await?;

    for row in claimed {
        let key = format!("{}\u{0}{}\u{0}", row.main_domain, row.login) + &row.password;
        if let Some(out) = output_by_key.get(&key) {
            f.write_all(out.as_bytes()).await?;
        }
    }

    f.flush().await?;

    set_state(state, user_id, UserState::Idle);
    bot.send_message(chat_id, t_ready_sending(lang))
        .reply_markup(main_keyboard(lang))
        .await?;
    bot.send_document(chat_id, InputFile::file(out_path))
        .await?;
    Ok(())
}

async fn enqueue(
    bot: &Bot,
    state: &BotState,
    user_id: i64,
    chat_id: ChatId,
    kind: SearchKind,
    query: String,
) -> Result<()> {
    use dashmap::mapref::entry::Entry;

    let lang = lang_of(user_id);
    set_state(state, user_id, UserState::Idle);

    match state.active_requests.entry(user_id) {
        Entry::Occupied(occ) => {
            let msg = t_busy_with_kind(lang, occ.get());
            bot.send_message(chat_id, msg)
                .reply_markup(input_keyboard(lang))
                .await?;
            return Ok(());
        }
        Entry::Vacant(vac) => {
            vac.insert(kind.clone());
        }
    }

    let task = DbTask {
        user_id,
        chat_id,
        kind,
        query,
    };

    if state.db_tx.try_send(task).is_err() {
        state.active_requests.remove(&user_id);
        bot.send_message(chat_id, t_queue_overloaded(lang))
            .reply_markup(input_keyboard(lang))
            .await?;
        return Ok(());
    }

    bot.send_message(chat_id, helper::queued_msg())
        .reply_markup(input_keyboard(lang))
        .await?;
    Ok(())
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

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}
