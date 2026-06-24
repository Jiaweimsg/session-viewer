use crate::claude::parser::jsonl::parse_session_messages;
use crate::claude::parser::path_encoder::get_all_projects_dirs;
use crate::shared_models::PaginatedMessages;

pub fn get_messages(
    encoded_name: String,
    session_id: String,
    page: usize,
    page_size: usize,
) -> Result<PaginatedMessages, String> {
    // 会话文件可能在默认 ~/.claude 或任一额外账号目录下;session_id 是 UUID,
    // 全局唯一,取首个命中的文件即可。
    for projects_dir in get_all_projects_dirs() {
        let session_path = projects_dir
            .join(&encoded_name)
            .join(format!("{}.jsonl", session_id));
        if session_path.exists() {
            return parse_session_messages(&session_path, page, page_size);
        }
    }

    Err(format!("Session file not found: {}", session_id))
}
