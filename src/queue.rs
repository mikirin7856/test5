use teloxide::types::ChatId;

#[derive(Debug, Clone)]
pub enum SearchKind {
    Domain,
    Port,
    Subdomain,
    Path,
    Login, // ✅ новое: поиск по login/email
}

#[derive(Debug, Clone)]
pub struct DbTask {
    pub user_id: i64,
    pub chat_id: ChatId,
    pub kind: SearchKind,
    pub query: String, // domain / port / sub / path / login(email)
}
