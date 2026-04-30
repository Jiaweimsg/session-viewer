use crate::cursor::parser::project_scanner::{
    count_bubbles, epoch_ms_to_rfc3339, read_composer_headers,
};
use serde::Serialize;

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
        .filter_map(|h| {
            let msg_count = count_bubbles(&h.composer_id);
            if !has_visible_session_content(h.subtitle.as_deref(), msg_count) {
                return None;
            }
            Some(CursorSession {
                session_id: h.composer_id,
                name: h.name,
                mode: h.unified_mode,
                first_prompt: h.subtitle,
                message_count: msg_count,
                created: h.created_at.map(epoch_ms_to_rfc3339),
                modified: h.last_updated_at.map(epoch_ms_to_rfc3339),
                is_archived: h.is_archived,
            })
        })
        .collect();

    sessions.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(sessions)
}

pub(super) fn has_visible_session_content(
    first_prompt: Option<&str>,
    message_count: usize,
) -> bool {
    message_count > 0 || first_prompt.map(|p| !p.trim().is_empty()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    #[test]
    fn hides_untitled_empty_cursor_sessions() {
        assert!(!super::has_visible_session_content(None, 0));
        assert!(!super::has_visible_session_content(Some(""), 0));
        assert!(!super::has_visible_session_content(Some("  "), 0));
    }

    #[test]
    fn keeps_cursor_sessions_with_messages_or_title() {
        assert!(super::has_visible_session_content(None, 1));
        assert!(super::has_visible_session_content(
            Some("Investigate bug"),
            0
        ));
    }
}
