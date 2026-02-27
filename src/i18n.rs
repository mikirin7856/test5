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

pub const BTN_LANG_EN: &str = "English Language";
pub const BTN_LANG_RU: &str = "Русский Язык";
pub const BTN_LANG_BACK: &str = "Назад / Back (Language)";

pub fn btn_cancel(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Back",
        Lang::Ru => "Назад / Back",
    }
}

pub fn btn_buy_3m(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Buy lines Last 3 Month",
        Lang::Ru => "Купить строки Last 3 Month",
    }
}
pub fn btn_buy_old(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Buy Old",
        Lang::Ru => "Купить Old",
    }
}
pub fn btn_buy_all(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Buy lines",
        Lang::Ru => "Купить строки",
    }
}
