use base64::Engine;
use serde::Deserialize;
use std::path::PathBuf;

use crate::cursor::parser::project_scanner::get_global_state_db;

const ACCESS_TOKEN_SQL: &str =
    "SELECT value FROM ItemTable WHERE key = 'cursorAuth/accessToken'";

#[derive(Debug)]
pub struct CursorSession {
    pub cookie: String,
    #[allow(dead_code)]
    pub user_id: String,
}

pub fn extract_session() -> Result<CursorSession, String> {
    let db_path = get_global_state_db()
        .ok_or_else(|| "cursor user dir not resolvable".to_string())?;
    if !db_path.exists() {
        return Err(format!("cursor state.vscdb not found at {}", db_path.display()));
    }

    let jwt = read_access_token(&db_path)?;
    if jwt.len() < 10 {
        return Err("cursor access token missing".to_string());
    }

    let user_id = read_user_id_from_cli_config()
        .or_else(|_| user_id_from_jwt(&jwt))
        .map_err(|e| format!("cursor userId unavailable: {}", e))?;

    let cookie = format!("WorkosCursorSessionToken={}%3A%3A{}", user_id, jwt);
    Ok(CursorSession { cookie, user_id })
}

fn read_access_token(db_path: &PathBuf) -> Result<String, String> {
    let db = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("open state.vscdb: {}", e))?;

    let token: String = db
        .query_row(ACCESS_TOKEN_SQL, [], |row| row.get::<_, String>(0))
        .map_err(|e| format!("query cursorAuth/accessToken: {}", e))?;

    Ok(token.trim().to_string())
}

#[derive(Deserialize)]
struct CliConfig {
    #[serde(rename = "authInfo")]
    auth_info: Option<AuthInfo>,
}

#[derive(Deserialize)]
struct AuthInfo {
    #[serde(rename = "authId")]
    auth_id: Option<String>,
}

fn read_user_id_from_cli_config() -> Result<String, String> {
    let home = dirs::home_dir().ok_or_else(|| "home dir missing".to_string())?;
    let path = home.join(".cursor").join("cli-config.json");
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("read {}: {}", path.display(), e))?;
    let cfg: CliConfig =
        serde_json::from_str(&raw).map_err(|e| format!("parse cli-config.json: {}", e))?;
    let auth_id = cfg
        .auth_info
        .and_then(|a| a.auth_id)
        .ok_or_else(|| "authInfo.authId missing".to_string())?;
    normalize_cursor_subject(&auth_id)
        .ok_or_else(|| format!("unparseable cli-config authId: {:?}", auth_id))
}

fn user_id_from_jwt(jwt: &str) -> Result<String, String> {
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 {
        return Err("jwt not 3-part".to_string());
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|e| format!("jwt payload b64: {}", e))?;
    let json: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("jwt payload json: {}", e))?;
    let sub = json
        .get("sub")
        .and_then(|s| s.as_str())
        .ok_or_else(|| "jwt.sub missing".to_string())?;
    normalize_cursor_subject(sub).ok_or_else(|| format!("unparseable jwt.sub: {:?}", sub))
}

/// Map a Cursor "subject" (from JWT.sub 或 cli-config.json authId) 到 cookie
/// 里要用的 user identifier。规则参考 mm7894215/TokenTracker —— Cursor 实际
/// 在用 WorkOS bridge,sub 形态分两类:
///
/// 1. **Native account** (Cursor 邮箱密码注册): `auth0|user_XXXXX`
///    → cookie 里需要 *剥掉* provider 前缀,只用 `user_XXXXX`
/// 2. **OAuth via WorkOS** (Google / GitHub / 通用 OIDC): `google-oauth2|123`,
///    `github|45678`, `oidc|...`, 或者罕见的 `auth0|<非 user_ id>`
///    → cookie 里需要 **整段保留** `<provider>|<id>`
/// 3. **直接 user_XXX(无 provider 段)**: WorkOS 未来格式预留
///    → 整段保留
///
/// 之前的实现只识别 #1,Google 登录的客户卡在 "no user_* in jwt.sub",再之前
/// 的"取最后一段"兜底又会把 #2 错拼成只剩 ID 部分,cursor.com API 返回 401。
const WORKOS_OAUTH_PROVIDERS: &[&str] = &["google-oauth2", "github", "oidc", "auth0"];

fn normalize_cursor_subject(subject: &str) -> Option<String> {
    let s = subject.trim();
    if s.is_empty() {
        return None;
    }
    // Case 1: native — "<provider>|user_XXX" → "user_XXX"
    if let Some(idx) = s.rfind('|') {
        let last = &s[idx + 1..];
        if last.starts_with("user_")
            && last.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Some(last.to_string());
        }
        // Case 2: WorkOS OAuth — keep "<provider>|<id>" verbatim
        let provider = &s[..idx];
        let id_part = &s[idx + 1..];
        if WORKOS_OAUTH_PROVIDERS.contains(&provider)
            && !id_part.is_empty()
            && !id_part.contains('|')
        {
            return Some(s.to_string());
        }
        return None;
    }
    // Case 3: no '|' — bare "user_XXX"
    if s.starts_with("user_") && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Some(s.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_native_strips_provider() {
        assert_eq!(
            normalize_cursor_subject("auth0|user_01HXXX"),
            Some("user_01HXXX".to_string())
        );
    }

    #[test]
    fn normalize_google_keeps_full_subject() {
        assert_eq!(
            normalize_cursor_subject("google-oauth2|112233445566"),
            Some("google-oauth2|112233445566".to_string())
        );
    }

    #[test]
    fn normalize_github_keeps_full_subject() {
        assert_eq!(
            normalize_cursor_subject("github|12345"),
            Some("github|12345".to_string())
        );
    }

    #[test]
    fn normalize_bare_user_id() {
        assert_eq!(
            normalize_cursor_subject("user_01HXXX"),
            Some("user_01HXXX".to_string())
        );
    }

    #[test]
    fn normalize_rejects_email_or_plain_id() {
        assert_eq!(normalize_cursor_subject("alice@example.com"), None);
        assert_eq!(normalize_cursor_subject("12345"), None);
    }

    #[test]
    fn normalize_rejects_unknown_provider() {
        assert_eq!(normalize_cursor_subject("twitter|123"), None);
    }

    #[test]
    fn normalize_rejects_empty_and_whitespace() {
        assert_eq!(normalize_cursor_subject(""), None);
        assert_eq!(normalize_cursor_subject("   "), None);
    }
}
