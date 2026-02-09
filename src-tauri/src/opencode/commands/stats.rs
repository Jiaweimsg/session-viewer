use crate::opencode::models::stats::TokenSummary;
use crate::opencode::parser::session_scanner::{get_message_dir, scan_all_session_files};
use std::fs;

/// Get statistics for OpenCode
pub fn get_stats() -> Result<TokenSummary, String> {
    // Count sessions
    let session_files = scan_all_session_files();
    let session_count = session_files.len();

    // Count messages
    let message_dir_base = get_message_dir()
        .ok_or_else(|| "Could not find OpenCode message directory".to_string())?;

    let mut message_count = 0;
    
    if let Ok(entries) = fs::read_dir(&message_dir_base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Ok(msg_entries) = fs::read_dir(&path) {
                    message_count += msg_entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().extension().map(|ext| ext == "json").unwrap_or(false))
                        .count();
                }
            }
        }
    }

    // OpenCode doesn't track token usage the same way, so we return counts only
    Ok(TokenSummary {
        total_input_tokens: 0,
        total_output_tokens: 0,
        total_tokens: 0,
        tokens_by_model: std::collections::HashMap::new(),
        daily_tokens: vec![],
        session_count,
        message_count,
    })
}
