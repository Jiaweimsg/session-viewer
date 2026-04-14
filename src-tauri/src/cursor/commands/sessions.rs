use serde::Serialize;
use crate::cursor::parser::project_scanner::{
    read_composer_headers, count_bubbles, epoch_ms_to_rfc3339,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorSession {
    pub session_id: String,
    pub name: Option<String>,
    pub mode: Option<String>,
    pub first_prompt: Option<String>,
    pub message_count: usize,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub is_archived: bool,
}

fn percent_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                result.push(hex as char);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}

pub fn get_sessions(project_key: String) -> Result<Vec<CursorSession>, String> {
    let target = percent_decode(&project_key);
    let headers = read_composer_headers();

    let mut sessions: Vec<CursorSession> = headers
        .into_iter()
        .filter(|h| {
            let ws = h.workspace_path.as_deref().unwrap_or("(no workspace)");
            ws == target
        })
        .map(|h| {
            let msg_count = count_bubbles(&h.composer_id);
            CursorSession {
                session_id: h.composer_id,
                name: h.name,
                mode: h.unified_mode,
                first_prompt: h.subtitle,
                message_count: msg_count,
                created: h.created_at.map(epoch_ms_to_rfc3339),
                modified: h.last_updated_at.map(epoch_ms_to_rfc3339),
                is_archived: h.is_archived,
            }
        })
        .collect();

    sessions.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(sessions)
}
