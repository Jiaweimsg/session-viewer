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
    // auth0|user_XXX → user_XXX
    let user = auth_id
        .split('|')
        .find(|s| s.starts_with("user_"))
        .ok_or_else(|| "no user_* segment in authId".to_string())?;
    Ok(user.to_string())
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
    sub.split('|')
        .find(|s| s.starts_with("user_"))
        .map(|s| s.to_string())
        .ok_or_else(|| "no user_* in jwt.sub".to_string())
}
