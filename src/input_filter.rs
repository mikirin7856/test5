use anyhow::{Result, anyhow, bail};
use regex::Regex;
use std::sync::OnceLock;

fn domain_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-z0-9-]+\.[a-z0-9.-]+$").expect("domain regex"))
}

pub fn validate_domain(domain: &str) -> Result<()> {
    if domain.len() < 5 || !domain.contains('.') {
        bail!("domain too short or missing dot");
    }
    if !domain_regex().is_match(domain) {
        bail!("domain format invalid");
    }
    Ok(())
}

pub fn validate_port(port: &str) -> Result<()> {
    let port = port.trim();
    if port.is_empty() || port.len() > 5 || !port.chars().all(|c| c.is_ascii_digit()) {
        bail!("bad port");
    }
    let n: u16 = port.parse().map_err(|_| anyhow!("bad port"))?;
    if n == 0 {
        bail!("bad port");
    }
    Ok(())
}

pub fn validate_subdomain_prefix(s: &str) -> Result<()> {
    let s = s.trim();
    if s.is_empty() || s.len() > 63 {
        bail!("bad subdomain prefix");
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        bail!("bad subdomain prefix");
    }
    if s.starts_with('-') || s.ends_with('-') {
        bail!("bad subdomain prefix");
    }
    Ok(())
}

pub fn validate_path_prefix(p: &str) -> Result<()> {
    let p = p.trim();
    if p.is_empty() || p.len() < 2 || p.len() > 300 {
        bail!("bad path");
    }
    if !p.starts_with('/') {
        bail!("path must start with /");
    }
    if p.chars().any(|c| c.is_control()) || p.contains(' ') {
        bail!("bad path");
    }
    Ok(())
}

pub fn validate_login_or_email(s: &str) -> Result<()> {
    let s = s.trim();
    if s.is_empty() || s.len() < 3 || s.len() > 254 {
        bail!("bad login length");
    }
    if s.chars().any(|c| c.is_control()) {
        bail!("control chars not allowed");
    }
    if s.contains(' ') || s.contains('\t') || s.contains('\n') || s.contains('\r') {
        bail!("spaces not allowed");
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+' | '@'))
    {
        bail!("bad chars in login");
    }

    if let Some(at) = s.find('@') {
        if at == 0 || at == s.len() - 1 {
            bail!("bad email format");
        }
        let domain = &s[at + 1..];
        if !domain.contains('.') {
            bail!("bad email domain");
        }
    }

    Ok(())
}
