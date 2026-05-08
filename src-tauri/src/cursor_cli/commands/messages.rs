use crate::cursor::parser::cli_chats;
use crate::shared_models::PaginatedMessages;

pub fn get_messages(
    session_key: String,
    page: usize,
    page_size: usize,
) -> Result<PaginatedMessages, String> {
    let data = cli_chats::load_session_from_key(&session_key)
        .ok_or_else(|| format!("CLI session not found: {}", session_key))?;
    let all_messages = cli_chats::display_messages_from_rows(&data.session_id, data.rows.clone());

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
