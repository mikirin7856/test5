// src/helper.rs
pub fn usage_domain() -> &'static str {
    "Неверный формат.\nПример: /domain \"example.com\""
}

pub fn blocked_msg() -> &'static str {
    "Доступ заблокирован."
}

pub fn busy_msg() -> &'static str {
    "У вас уже есть активный запрос. Дождитесь завершения."
}

pub fn queued_msg() -> &'static str {
    "Ваш запрос поставлен в очередь."
}

pub fn rate_limited_msg() -> &'static str {
    "Слишком много запросов. Доступ заблокирован."
}

pub fn internal_err() -> &'static str {
    "Внутренняя ошибка. Попробуйте позже."
}
