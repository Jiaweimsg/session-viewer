use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Cursor data directory: ~/Library/Application Support/Cursor/User
pub fn get_cursor_user_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().map(|h| h.join("Library/Application Support/Cursor/User"))
    }
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir().map(|c| c.join("Cursor/User"))
    }
    #[cfg(target_os = "linux")]
    {
        dirs::home_dir().map(|h| h.join(".config/Cursor/User"))
    }
}

pub fn get_global_state_db() -> Option<PathBuf> {
    get_cursor_user_dir().map(|d| d.join("globalStorage/state.vscdb"))
}

pub fn get_workspace_storage_dir() -> Option<PathBuf> {
    get_cursor_user_dir().map(|d| d.join("workspaceStorage"))
}

// ── Data structures parsed from SQLite ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorComposerHeader {
    pub composer_id: String,
    pub name: Option<String>,
    pub unified_mode: Option<String>,
    pub created_at: Option<u64>,
    pub last_updated_at: Option<u64>,
    pub subtitle: Option<String>,
    pub is_archived: bool,
    pub workspace_path: Option<String>,
    pub workspace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorBubble {
    /// 1 = user, 2 = assistant
    #[serde(rename = "type")]
    pub msg_type: u32,
    pub text: Option<String>,
    pub created_at: Option<String>,
    pub token_count: Option<TokenCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenCount {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Read workspace.json from a hashed workspace dir to get the project folder path
pub fn read_workspace_folder(ws_dir: &std::path::Path) -> Option<String> {
    let ws_json = ws_dir.join("workspace.json");
    let content = std::fs::read_to_string(&ws_json).ok()?;
    let v: serde_json::Value = serde_json::from_str(&content).ok()?;
    v.get("folder")
        .and_then(|f| f.as_str())
        .map(|s| s.strip_prefix("file://").unwrap_or(s).to_string())
}

/// Read all composer headers from globalStorage/state.vscdb
pub fn read_composer_headers() -> Vec<CursorComposerHeader> {
    let db_path = match get_global_state_db() {
        Some(p) if p.exists() => p,
        _ => return Vec::new(),
    };

    let db = match rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(db) => db,
        Err(_) => return Vec::new(),
    };

    let raw: String = match db.query_row(
        "SELECT value FROM ItemTable WHERE key = 'composer.composerHeaders'",
        [],
        |row| row.get(0),
    ) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let parsed: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let composers = match parsed.get("allComposers").and_then(|a| a.as_array()) {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    composers
        .iter()
        .filter_map(|c| {
            let id = c.get("composerId")?.as_str()?.to_string();
            let name = c.get("name").and_then(|n| n.as_str()).map(|s| s.to_string());
            let mode = c.get("unifiedMode").and_then(|m| m.as_str()).map(|s| s.to_string());
            let created = c.get("createdAt").and_then(|t| t.as_u64());
            let updated = c.get("lastUpdatedAt").and_then(|t| t.as_u64());
            let subtitle = c.get("subtitle").and_then(|s| s.as_str()).map(|s| s.to_string());
            let archived = c.get("isArchived").and_then(|a| a.as_bool()).unwrap_or(false);
            let ws_path = c
                .get("workspaceIdentifier")
                .and_then(|w| w.get("uri"))
                .and_then(|u| u.get("path"))
                .and_then(|p| p.as_str())
                .map(|s| s.to_string());
            let ws_id = c
                .get("workspaceIdentifier")
                .and_then(|w| w.get("id"))
                .and_then(|i| i.as_str())
                .map(|s| s.to_string());

            Some(CursorComposerHeader {
                composer_id: id,
                name,
                unified_mode: mode,
                created_at: created,
                last_updated_at: updated,
                subtitle,
                is_archived: archived,
                workspace_path: ws_path,
                workspace_id: ws_id,
            })
        })
        .collect()
}

/// Count bubbles (messages) for a given composer by reading composerData
pub fn count_bubbles(composer_id: &str) -> usize {
    let db_path = match get_global_state_db() {
        Some(p) if p.exists() => p,
        _ => return 0,
    };
    let db = match rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(db) => db,
        Err(_) => return 0,
    };

    let key = format!("composerData:{}", composer_id);
    let raw: String = match db.query_row(
        "SELECT value FROM cursorDiskKV WHERE key = ?1",
        [&key],
        |row| row.get(0),
    ) {
        Ok(v) => v,
        Err(_) => return 0,
    };

    let parsed: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return 0,
    };

    parsed
        .get("fullConversationHeadersOnly")
        .and_then(|h| h.as_array())
        .map(|a| a.len())
        .unwrap_or(0)
}

/// Read all bubble messages for a composer session
pub fn read_bubbles(composer_id: &str) -> Vec<CursorBubble> {
    let db_path = match get_global_state_db() {
        Some(p) if p.exists() => p,
        _ => return Vec::new(),
    };
    let db = match rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(db) => db,
        Err(_) => return Vec::new(),
    };

    // First get bubble IDs from composerData
    let key = format!("composerData:{}", composer_id);
    let raw: String = match db.query_row(
        "SELECT value FROM cursorDiskKV WHERE key = ?1",
        [&key],
        |row| row.get(0),
    ) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let parsed: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let headers = match parsed
        .get("fullConversationHeadersOnly")
        .and_then(|h| h.as_array())
    {
        Some(arr) => arr.clone(),
        None => return Vec::new(),
    };

    let mut bubbles = Vec::new();
    for header in &headers {
        let bubble_id = match header.get("bubbleId").and_then(|b| b.as_str()) {
            Some(id) => id,
            None => continue,
        };
        let msg_type = header.get("type").and_then(|t| t.as_u64()).unwrap_or(0) as u32;

        let bubble_key = format!("bubbleId:{}:{}", composer_id, bubble_id);
        let bubble_raw: String = match db.query_row(
            "SELECT value FROM cursorDiskKV WHERE key = ?1",
            [&bubble_key],
            |row| row.get(0),
        ) {
            Ok(v) => v,
            Err(_) => {
                // Bubble content might not exist, still record the header
                bubbles.push(CursorBubble {
                    msg_type,
                    text: None,
                    created_at: None,
                    token_count: None,
                });
                continue;
            }
        };

        let bv: serde_json::Value = match serde_json::from_str(&bubble_raw) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let text = bv.get("text").and_then(|t| t.as_str()).map(|s| s.to_string());
        let created_at = bv.get("createdAt").and_then(|c| c.as_str()).map(|s| s.to_string());
        let token_count = bv.get("tokenCount").and_then(|tc| {
            let input = tc.get("inputTokens").and_then(|t| t.as_u64()).unwrap_or(0);
            let output = tc.get("outputTokens").and_then(|t| t.as_u64()).unwrap_or(0);
            Some(TokenCount {
                input_tokens: input,
                output_tokens: output,
            })
        });

        bubbles.push(CursorBubble {
            msg_type,
            text,
            created_at,
            token_count,
        });
    }

    bubbles
}

/// Convert epoch milliseconds to RFC3339 string
pub fn epoch_ms_to_rfc3339(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    let nanos = ((ms % 1000) * 1_000_000) as u32;
    chrono::DateTime::from_timestamp(secs, nanos)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}
