use std::path::Path;

use crate::shared_models::PaginatedMessages;
use crate::codex::parser::jsonl::parse_session_messages;

pub fn get_messages(
    file_path: String,
    page: usize,
    page_size: usize,
) -> Result<PaginatedMessages, String> {
    let path = Path::new(&file_path);

    if !path.exists() {
        return Err(format!("Session file not found: {}", file_path));
    }

    parse_session_messages(path, page, page_size)
}
