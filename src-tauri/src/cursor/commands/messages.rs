use crate::cursor::parser::project_scanner::read_bubbles;
use crate::shared_models::{DisplayContentBlock, DisplayMessage, PaginatedMessages};

pub fn get_messages(
    session_key: String,
    page: usize,
    page_size: usize,
) -> Result<PaginatedMessages, String> {
    let bubbles = read_bubbles(&session_key);

    let all_messages: Vec<DisplayMessage> = bubbles
        .into_iter()
        .enumerate()
        .filter_map(|(i, b)| {
            let role = match b.msg_type {
                1 => "human",
                2 => "assistant",
                _ => return None,
            };
            let text = b.text.unwrap_or_default();
            if text.is_empty() && role == "human" {
                return None;
            }
            Some(DisplayMessage {
                uuid: Some(format!("{}-{}", session_key, i)),
                role: role.to_string(),
                timestamp: b.created_at,
                content: vec![DisplayContentBlock::Text { text }],
            })
        })
        .collect();

    let total = all_messages.len();
    let start = page * page_size;
    let end = (start + page_size).min(total);
    let messages = if start < total {
        all_messages[start..end].to_vec()
    } else {
        Vec::new()
    };

    Ok(PaginatedMessages {
        messages,
        total,
        page,
        page_size,
        has_more: end < total,
    })
}
