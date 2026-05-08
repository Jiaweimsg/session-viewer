use crate::cursor::parser::cli_chats;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorCliSession {
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
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = hex(bytes[i + 1]);
            let lo = hex(bytes[i + 2]);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub fn get_sessions(project_key: String) -> Result<Vec<CursorCliSession>, String> {
    let target = percent_decode(&project_key);

    let mut sessions: Vec<CursorCliSession> = cli_chats::load_all_sessions()
        .into_iter()
        .filter(|s| s.cwd() == target)
        .filter_map(|s| {
            let message_count = s.message_count();
            let first_prompt = s.first_prompt().or_else(|| s.meta.name.clone());
            if message_count == 0 && first_prompt.is_none() {
                return None;
            }
            Some(CursorCliSession {
                session_id: cli_chats::encode_session_key(&s.project_hash, &s.session_id),
                name: s.meta.name.clone(),
                mode: s.meta.mode.clone(),
                first_prompt,
                message_count,
                created: s.created(),
                modified: s.modified(),
                is_archived: false,
            })
        })
        .collect();

    sessions.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(sessions)
}
