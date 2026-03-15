use crate::copilot::parser::session_parser::parse_session_messages;
use crate::copilot::parser::session_scanner::get_session_state_dir;
use crate::shared_models::PaginatedMessages;

/// Get paginated messages for a Copilot CLI session
pub fn get_messages(
    session_id: String,
    page: usize,
    page_size: usize,
) -> Result<PaginatedMessages, String> {
    let state_dir =
        get_session_state_dir().ok_or("Could not find ~/.copilot/session-state directory")?;
    let events_path = state_dir.join(&session_id).join("events.jsonl");
    if !events_path.exists() {
        return Err(format!(
            "events.jsonl not found for session: {}",
            session_id
        ));
    }
    parse_session_messages(&events_path, page, page_size)
}
