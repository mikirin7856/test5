use teloxide::types::{KeyboardButton, KeyboardMarkup};

use crate::{
    bot::PurchaseKind,
    i18n::{
        BTN_LANG_BACK, BTN_LANG_EN, BTN_LANG_RU, Lang, btn_buy_3m, btn_buy_all, btn_buy_old,
        btn_cancel,
    },
    queue::SearchKind,
};

pub fn language_keyboard() -> KeyboardMarkup {
    KeyboardMarkup::new(vec![vec![
        KeyboardButton::new(BTN_LANG_EN),
        KeyboardButton::new(BTN_LANG_RU),
    ]])
    .resize_keyboard(true)
    .one_time_keyboard(false)
}

pub fn main_keyboard(lang: Lang) -> KeyboardMarkup {
    KeyboardMarkup::new(vec![
        vec![
            KeyboardButton::new(btn_search_domain(lang)),
            KeyboardButton::new(btn_search_port(lang)),
        ],
        vec![
            KeyboardButton::new(btn_search_subdomain(lang)),
            KeyboardButton::new(btn_search_path(lang)),
        ],
        vec![KeyboardButton::new(btn_search_login(lang))],
        vec![KeyboardButton::new(BTN_LANG_BACK)],
    ])
    .resize_keyboard(true)
    .one_time_keyboard(false)
}

pub fn input_keyboard(lang: Lang) -> KeyboardMarkup {
    KeyboardMarkup::new(vec![vec![KeyboardButton::new(btn_cancel(lang))]])
        .resize_keyboard(true)
        .one_time_keyboard(false)
}

pub fn amount_keyboard(lang: Lang) -> KeyboardMarkup {
    KeyboardMarkup::new(vec![vec![KeyboardButton::new(btn_cancel(lang))]])
        .resize_keyboard(true)
        .one_time_keyboard(false)
}

pub fn buy_keyboard(lang: Lang, kind: PurchaseKind) -> KeyboardMarkup {
    let mut rows: Vec<Vec<KeyboardButton>> = Vec::new();

    match kind {
        PurchaseKind::All => rows.push(vec![KeyboardButton::new(btn_buy_all(lang))]),
    }

    rows.push(vec![KeyboardButton::new(btn_cancel(lang))]);

    KeyboardMarkup::new(rows)
        .resize_keyboard(true)
        .one_time_keyboard(false)
}

pub fn btn_search_domain(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "🔍 Search by domain",
        Lang::Ru => "🔍 Поиск по домену",
    }
}
pub fn btn_search_port(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "🔌 Search by port",
        Lang::Ru => "🔌 Поиск по порту",
    }
}
pub fn btn_search_subdomain(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "🌐 Search by subdomain",
        Lang::Ru => "🌐 Поиск по subdomain",
    }
}
pub fn btn_search_path(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "📁 Search by path",
        Lang::Ru => "📁 Поиск по пути path",
    }
}
pub fn btn_search_login(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "✉️ Search by login/email",
        Lang::Ru => "✉️ Поиск по login/email",
    }
}

pub fn purchase_action_keyboard(
    lang: Lang,
    kind: &SearchKind,
    cnt_new: usize,
    cnt_old: usize,
) -> KeyboardMarkup {
    let mut rows: Vec<Vec<KeyboardButton>> = Vec::new();

    if matches!(kind, SearchKind::Login) {
        if cnt_new > 0 {
            rows.push(vec![KeyboardButton::new(btn_buy_all(lang))]);
        }
    } else {

        }
    }

    rows.push(vec![KeyboardButton::new(btn_cancel(lang))]);

    KeyboardMarkup::new(rows)
        .resize_keyboard(true)
        .one_time_keyboard(false)
}
