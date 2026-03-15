use crate::copilot::models::session::CopilotSession;
use crate::copilot::parser::session_scanner::scan_sessions_for_cwd;

/// Decode percent-encoded string (e.g. %2F -> /)
fn percent_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16) {
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

/// List all sessions for a given project (identified by URL-encoded cwd)
pub fn get_sessions(project_key: String) -> Result<Vec<CopilotSession>, String> {
    let cwd = percent_decode(&project_key);
    Ok(scan_sessions_for_cwd(&cwd))
}
