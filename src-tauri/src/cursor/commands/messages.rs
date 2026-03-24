use crate::shared_models::PaginatedMessages;

pub fn get_messages(
    _session_key: String,
    _page: usize,
    _page_size: usize,
) -> Result<PaginatedMessages, String> {
    Ok(PaginatedMessages {
        messages: Vec::new(),
        total: 0,
        page: 0,
        page_size: 0,
        has_more: false,
    })
}
