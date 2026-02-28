use dashmap::DashMap;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Ru,
}

static USER_LANGS: OnceLock<DashMap<i64, Lang>> = OnceLock::new();

pub fn user_lang_store() -> &'static DashMap<i64, Lang> {
    USER_LANGS.get_or_init(DashMap::new)
}

pub fn lang_of(user_id: i64) -> Lang {
    user_lang_store()
        .get(&user_id)
        .map(|v| *v)
        .unwrap_or(Lang::Ru)
}

pub const BTN_LANG_EN: &str = "🇬🇧 English Language";
pub const BTN_LANG_RU: &str = "🇷🇺 Русский Язык";
pub const BTN_LANG_BACK: &str = "🔙 Назад / Back (Language)";

pub fn btn_cancel(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "🔙 Back / Назад",
        Lang::Ru => "🔙 Назад / Back",
    }
}

pub fn btn_buy_3m(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "🛒 Buy lines for last [3 month] 🔥",
        Lang::Ru => "🛒 Купить строки за последние [3 месяца] 🔥",
    }
}
pub fn btn_buy_old(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "🛒 Buy old lines ⏳",
        Lang::Ru => "🛒 Купить cтарые cтроки ⏳",
    }
}
pub fn btn_buy_all(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "🛒 Buy lines",
        Lang::Ru => "🛒 Купить строки",
    }
}
