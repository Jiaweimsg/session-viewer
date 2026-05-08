use crate::shared_models::{DisplayContentBlock, DisplayMessage};
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const SESSION_KEY_PREFIX: &str = "cli";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliMeta {
    #[serde(default)]
    pub agent_id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub created_at: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct CliSessionKey {
    pub project_hash: String,
    pub session_id: String,
}

#[derive(Debug, Clone)]
pub struct CliBlobRow {
    pub rowid: i64,
    pub value: Value,
}

#[derive(Debug, Clone)]
pub struct CliSessionData {
    pub meta: CliMeta,
    pub project_hash: String,
    pub session_id: String,
    pub db_path: PathBuf,
    pub workspace_path: Option<String>,
    pub rows: Vec<CliBlobRow>,
    pub file_mtime_ms: u64,
}

pub fn encode_session_key(project_hash: &str, session_id: &str) -> String {
    format!("{}:{}:{}", SESSION_KEY_PREFIX, project_hash, session_id)
}

pub fn decode_session_key(key: &str) -> Option<CliSessionKey> {
    let mut parts = key.splitn(3, ':');
    if parts.next()? != SESSION_KEY_PREFIX {
        return None;
    }
    let project_hash = parts.next()?.to_string();
    let session_id = parts.next()?.to_string();
    if project_hash.is_empty() || session_id.is_empty() {
        return None;
    }
    Some(CliSessionKey {
        project_hash,
        session_id,
    })
}

pub fn decode_meta_value(raw: &str) -> Option<CliMeta> {
    let trimmed = raw.trim();
    let json = if trimmed.starts_with('{') {
        trimmed.to_string()
    } else {
        String::from_utf8(hex_decode(trimmed)?).ok()?
    };
    serde_json::from_str(&json).ok()
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_val(bytes[i])?;
        let lo = hex_val(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Some(out)
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub fn extract_user_query_text(v: &Value) -> Option<String> {
    if v.get("role")?.as_str()? != "user" {
        return None;
    }
    let text = content_text(v.get("content")?)?;
    extract_wrapped(&text, "<user_query>", "</user_query>")
}

pub fn extract_workspace_path(v: &Value) -> Option<String> {
    if v.get("role")?.as_str()? != "user" {
        return None;
    }
    let text = content_text(v.get("content")?)?;
    let info = extract_wrapped(&text, "<user_info>", "</user_info>")?;
    for line in info.lines() {
        let line = line.trim();
        if let Some(path) = line.strip_prefix("Workspace Path:") {
            let path = path.trim();
            if !path.is_empty() {
                return Some(path.to_string());
            }
        }
    }
    None
}

fn content_text(content: &Value) -> Option<String> {
    match content {
        Value::String(s) => Some(s.clone()),
        Value::Array(items) => {
            let parts: Vec<String> = items
                .iter()
                .filter_map(|item| {
                    let ty = item.get("type")?.as_str()?;
                    if ty == "text" {
                        Some(item.get("text")?.as_str()?.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n\n"))
            }
        }
        _ => None,
    }
}

fn extract_wrapped(text: &str, prefix: &str, suffix: &str) -> Option<String> {
    let trimmed = text.trim();
    if !trimmed.starts_with(prefix) || !trimmed.ends_with(suffix) {
        return None;
    }
    let inner = &trimmed[prefix.len()..trimmed.len() - suffix.len()];
    let inner = inner.trim();
    if inner.is_empty() {
        None
    } else {
        Some(inner.to_string())
    }
}

pub fn display_messages_from_rows(session_id: &str, rows: Vec<CliBlobRow>) -> Vec<DisplayMessage> {
    rows.into_iter()
        .filter_map(|row| display_message_from_row(session_id, row))
        .collect()
}

fn display_message_from_row(session_id: &str, row: CliBlobRow) -> Option<DisplayMessage> {
    let role = row.value.get("role")?.as_str()?;
    let content = row.value.get("content");
    let display_role = match role {
        "user" => "user",
        "assistant" => "assistant",
        "tool" => "tool",
        "system" => return None,
        _ => return None,
    };

    let blocks = match display_role {
        "user" => {
            if let Some(text) = extract_user_query_text(&row.value) {
                vec![DisplayContentBlock::Text { text }]
            } else {
                return None;
            }
        }
        "assistant" => content
            .and_then(blocks_from_assistant_content)
            .unwrap_or_default(),
        "tool" => content.and_then(blocks_from_tool_content).unwrap_or_default(),
        _ => Vec::new(),
    };

    if blocks.is_empty() {
        return None;
    }

    Some(DisplayMessage {
        uuid: Some(format!("{}-{}", session_id, row.rowid)),
        role: display_role.to_string(),
        timestamp: extract_timestamp(&row.value),
        content: blocks,
    })
}

fn blocks_from_assistant_content(content: &Value) -> Option<Vec<DisplayContentBlock>> {
    match content {
        Value::String(s) => Some(vec![DisplayContentBlock::Text { text: s.clone() }]),
        Value::Array(items) => {
            let mut out = Vec::new();
            for item in items {
                match item.get("type").and_then(|v| v.as_str()) {
                    Some("text") => {
                        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                            if !text.is_empty() {
                                out.push(DisplayContentBlock::Text {
                                    text: text.to_string(),
                                });
                            }
                        }
                    }
                    Some("reasoning") | Some("redacted-reasoning") => {
                        let text = item
                            .get("text")
                            .or_else(|| item.get("data"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        if !text.is_empty() {
                            out.push(DisplayContentBlock::Reasoning { text });
                        }
                    }
                    Some("tool-call") | Some("tool_call") => {
                        let id = item
                            .get("toolCallId")
                            .or_else(|| item.get("tool_call_id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let name = item
                            .get("toolName")
                            .or_else(|| item.get("tool_name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("tool")
                            .to_string();
                        let input = item
                            .get("args")
                            .or_else(|| item.get("input"))
                            .map(|v| {
                                if let Some(s) = v.as_str() {
                                    s.to_string()
                                } else {
                                    serde_json::to_string_pretty(v).unwrap_or_default()
                                }
                            })
                            .unwrap_or_default();
                        out.push(DisplayContentBlock::ToolUse { id, name, input });
                    }
                    _ => {}
                }
            }
            Some(out)
        }
        _ => None,
    }
}

fn blocks_from_tool_content(content: &Value) -> Option<Vec<DisplayContentBlock>> {
    match content {
        Value::String(s) => Some(vec![DisplayContentBlock::Text { text: s.clone() }]),
        Value::Array(items) => {
            let mut out = Vec::new();
            for item in items {
                if item.get("type").and_then(|v| v.as_str()) == Some("tool-result") {
                    let tool_use_id = item
                        .get("toolCallId")
                        .or_else(|| item.get("tool_call_id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let content = item
                        .get("result")
                        .or_else(|| item.get("content"))
                        .map(|v| {
                            if let Some(s) = v.as_str() {
                                s.to_string()
                            } else {
                                serde_json::to_string_pretty(v).unwrap_or_default()
                            }
                        })
                        .unwrap_or_default();
                    out.push(DisplayContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error: item
                            .get("isError")
                            .or_else(|| item.get("is_error"))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                    });
                }
            }
            Some(out)
        }
        _ => None,
    }
}

fn extract_timestamp(v: &Value) -> Option<String> {
    v.get("createdAt")
        .or_else(|| v.get("timestamp"))
        .and_then(|ts| {
            if let Some(s) = ts.as_str() {
                Some(s.to_string())
            } else {
                ts.as_u64().map(epoch_ms_to_rfc3339)
            }
        })
}

pub fn epoch_ms_to_rfc3339(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    let nanos = ((ms % 1000) * 1_000_000) as u32;
    chrono::DateTime::from_timestamp(secs, nanos)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}

pub fn get_chats_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".cursor").join("chats"))
}

pub fn scan_all_store_dbs() -> Vec<PathBuf> {
    let Some(chats) = get_chats_dir() else { return Vec::new() };
    scan_store_dbs_under(&chats)
}

pub fn scan_store_dbs_under(chats: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(projects) = fs::read_dir(chats) else { return out };
    for project in projects.flatten() {
        let project_path = project.path();
        if !project_path.is_dir() {
            continue;
        }
        let Ok(sessions) = fs::read_dir(&project_path) else { continue };
        for session in sessions.flatten() {
            let db = session.path().join("store.db");
            if db.is_file() {
                out.push(db);
            }
        }
    }
    out
}

pub fn load_session_from_key(key: &str) -> Option<CliSessionData> {
    let key = decode_session_key(key)?;
    let db = get_chats_dir()?
        .join(&key.project_hash)
        .join(&key.session_id)
        .join("store.db");
    load_session_from_db(&db)
}

pub fn load_all_sessions() -> Vec<CliSessionData> {
    scan_all_store_dbs()
        .into_iter()
        .filter_map(|p| load_session_from_db(&p))
        .collect()
}

pub fn load_session_from_db(db_path: &Path) -> Option<CliSessionData> {
    let session_id = db_path.parent()?.file_name()?.to_str()?.to_string();
    let project_hash = db_path.parent()?.parent()?.file_name()?.to_str()?.to_string();
    let metadata = fs::metadata(db_path).ok()?;
    let file_mtime_ms = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis() as u64;
    let db = open_readonly_db(db_path).ok()?;
    let raw_meta: String = db
        .query_row("SELECT value FROM meta WHERE key = '0'", [], |row| row.get(0))
        .ok()?;
    let meta = decode_meta_value(&raw_meta)?;
    let rows = read_json_blob_rows(&db);
    let workspace_path = rows.iter().find_map(|r| extract_workspace_path(&r.value));

    Some(CliSessionData {
        meta,
        project_hash,
        session_id,
        db_path: db_path.to_path_buf(),
        workspace_path,
        rows,
        file_mtime_ms,
    })
}

fn open_readonly_db(path: &Path) -> rusqlite::Result<rusqlite::Connection> {
    let uri = format!("file:{}?mode=ro&immutable=1", path.to_string_lossy());
    rusqlite::Connection::open_with_flags(
        uri,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
            | rusqlite::OpenFlags::SQLITE_OPEN_URI
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
}

fn read_json_blob_rows(db: &rusqlite::Connection) -> Vec<CliBlobRow> {
    let mut stmt = match db.prepare("SELECT rowid, data FROM blobs ORDER BY rowid") {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map([], |row| {
        let rowid: i64 = row.get(0)?;
        let data: Vec<u8> = row.get(1)?;
        Ok((rowid, data))
    }) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };

    rows.filter_map(|row| {
        let Ok((rowid, data)) = row else { return None };
        if data.first().copied() != Some(b'{') {
            return None;
        }
        let value: Value = serde_json::from_slice(&data).ok()?;
        Some(CliBlobRow { rowid, value })
    })
    .collect()
}

impl CliSessionData {
    pub fn cwd(&self) -> String {
        self.workspace_path
            .clone()
            .unwrap_or_else(|| format!("~/.cursor/chats/{}", self.project_hash))
    }

    pub fn first_prompt(&self) -> Option<String> {
        self.rows
            .iter()
            .find_map(|row| extract_user_query_text(&row.value))
    }

    pub fn user_prompt_rows_after(&self, start_rowid: i64) -> Vec<(i64, String)> {
        self.rows
            .iter()
            .filter(|row| row.rowid > start_rowid)
            .filter_map(|row| extract_user_query_text(&row.value).map(|text| (row.rowid, text)))
            .collect()
    }

    pub fn message_count(&self) -> usize {
        display_messages_from_rows(&self.session_id, self.rows.clone()).len()
    }

    pub fn modified(&self) -> Option<String> {
        Some(epoch_ms_to_rfc3339(self.file_mtime_ms))
    }

    pub fn created(&self) -> Option<String> {
        self.meta.created_at.map(epoch_ms_to_rfc3339)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn decodes_hex_encoded_meta_json() {
        let raw = "7b226167656e744964223a22616263222c226e616d65223a224e6577204167656e74222c22637265617465644174223a3132337d";
        let meta = super::decode_meta_value(raw).unwrap();
        assert_eq!(meta.agent_id, "abc");
        assert_eq!(meta.name.as_deref(), Some("New Agent"));
        assert_eq!(meta.created_at, Some(123));
    }

    #[test]
    fn extracts_user_query_from_string_and_array_content() {
        let from_string = json!({
            "role": "user",
            "content": "<user_query>\nhello\n</user_query>"
        });
        assert_eq!(
            super::extract_user_query_text(&from_string).as_deref(),
            Some("hello")
        );

        let from_array = json!({
            "role": "user",
            "content": [
                {"type": "text", "text": "<user_query>\npart a"},
                {"type": "text", "text": "part b\n</user_query>"}
            ]
        });
        assert_eq!(
            super::extract_user_query_text(&from_array).as_deref(),
            Some("part a\n\npart b")
        );
    }

    #[test]
    fn rejects_system_injected_user_context() {
        let v = json!({
            "role": "user",
            "content": "<user_info>\nWorkspace Path: /tmp/project\n</user_info>"
        });
        assert_eq!(super::extract_user_query_text(&v), None);
    }

    #[test]
    fn extracts_workspace_path_from_user_info() {
        let v = json!({
            "role": "user",
            "content": "<user_info>\nOS Version: darwin\n\nWorkspace Path: /Users/me/work/app\n\nIs directory a git repo: true\n</user_info>"
        });
        assert_eq!(
            super::extract_workspace_path(&v).as_deref(),
            Some("/Users/me/work/app")
        );
    }

    #[test]
    fn encodes_and_decodes_cli_session_keys() {
        let key = super::encode_session_key("project-hash", "session-id");
        assert_eq!(key, "cli:project-hash:session-id");
        let decoded = super::decode_session_key(&key).unwrap();
        assert_eq!(decoded.project_hash, "project-hash");
        assert_eq!(decoded.session_id, "session-id");
        assert!(super::decode_session_key("not-cli").is_none());
    }

    #[test]
    fn converts_json_rows_to_display_messages() {
        let rows = vec![
            super::CliBlobRow {
                rowid: 1,
                value: json!({
                    "role": "user",
                    "content": [{"type": "text", "text": "<user_query>\nhello\n</user_query>"}]
                }),
            },
            super::CliBlobRow {
                rowid: 2,
                value: json!({
                    "role": "assistant",
                    "content": [{"type": "text", "text": "hi"}]
                }),
            },
            super::CliBlobRow {
                rowid: 3,
                value: json!({
                    "role": "tool",
                    "content": [{"type": "tool-result", "toolCallId": "call-1", "toolName": "Read", "result": "file text"}]
                }),
            },
        ];

        let messages = super::display_messages_from_rows("session", rows);
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[2].role, "tool");
    }
}
