use serde_json::Value;

/// Dispatch command: get_projects
/// Routes to Claude or Codex based on `tool` parameter.
#[tauri::command]
pub fn get_projects(tool: String) -> Result<Value, String> {
    match tool.as_str() {
        "claude" => {
            let result = crate::claude::commands::projects::get_projects()?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "codex" => {
            let result = crate::codex::commands::projects::get_projects()?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "opencode" => {
            let result = crate::opencode::commands::projects::get_projects()?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        _ => Err(format!("Unknown tool: {}", tool)),
    }
}

/// Dispatch command: get_sessions
/// For Claude: project_key is encoded_name.
/// For Codex: project_key is cwd (optional filter).
#[tauri::command]
pub fn get_sessions(tool: String, project_key: String) -> Result<Value, String> {
    match tool.as_str() {
        "claude" => {
            let result = crate::claude::commands::sessions::get_sessions(project_key)?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "codex" => {
            let cwd = if project_key.is_empty() {
                None
            } else {
                Some(project_key)
            };
            let result = crate::codex::commands::sessions::get_sessions(cwd)?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "opencode" => {
            let result = crate::opencode::commands::sessions::get_sessions(project_key)?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        _ => Err(format!("Unknown tool: {}", tool)),
    }
}

/// Dispatch command: get_sessions_grouped
/// Currently only for OpenCode (returns grouped sessions with parent-child relationships)
#[tauri::command]
pub fn get_sessions_grouped(tool: String, project_key: String) -> Result<Value, String> {
    match tool.as_str() {
        "opencode" => {
            let result = crate::opencode::commands::sessions::get_sessions_grouped(project_key)?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        // For other tools, fall back to regular get_sessions
        _ => get_sessions(tool, project_key),
    }
}

/// Dispatch command: get_messages
/// For Claude: session_key is session_id, project_key is encoded_name (required).
/// For Codex: session_key is file_path, project_key is not needed.
#[tauri::command]
pub fn get_messages(
    tool: String,
    session_key: String,
    project_key: Option<String>,
    page: usize,
    page_size: usize,
) -> Result<Value, String> {
    match tool.as_str() {
        "claude" => {
            let encoded_name = project_key
                .ok_or_else(|| "project_key (encoded_name) is required for Claude".to_string())?;
            let result = crate::claude::commands::messages::get_messages(
                encoded_name,
                session_key,
                page,
                page_size,
            )?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "codex" => {
            let result = crate::codex::commands::messages::get_messages(
                session_key,
                page,
                page_size,
            )?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "opencode" => {
            let result = crate::opencode::commands::messages::get_messages(
                session_key,
                page,
                page_size,
            )?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        _ => Err(format!("Unknown tool: {}", tool)),
    }
}

/// Dispatch command: global_search
#[tauri::command]
pub fn global_search(tool: String, query: String, max_results: usize) -> Result<Value, String> {
    match tool.as_str() {
        "claude" => {
            let result = crate::claude::commands::search::global_search(query, max_results)?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "codex" => {
            let result = crate::codex::commands::search::global_search(query, max_results)?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "opencode" => {
            let result = crate::opencode::commands::search::global_search(query, max_results)?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        _ => Err(format!("Unknown tool: {}", tool)),
    }
}

/// Dispatch command: get_stats
/// For Claude: returns StatsCache from stats-cache.json.
/// For Codex: returns TokenUsageSummary computed from session files.
#[tauri::command]
pub fn get_stats(tool: String) -> Result<Value, String> {
    match tool.as_str() {
        "claude" => {
            let result = crate::claude::commands::stats::get_global_stats()?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "codex" => {
            let result = crate::codex::commands::stats::get_stats()?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "opencode" => {
            let result = crate::opencode::commands::stats::get_stats()?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        _ => Err(format!("Unknown tool: {}", tool)),
    }
}

/// Dispatch command: get_token_summary
/// Only Claude has a separate token summary; Codex uses get_stats which includes token info.
#[tauri::command]
pub fn get_token_summary(tool: String) -> Result<Value, String> {
    match tool.as_str() {
        "claude" => {
            let result = crate::claude::commands::stats::get_token_summary()?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "codex" => {
            // Codex get_stats already includes token breakdown
            let result = crate::codex::commands::stats::get_stats()?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "opencode" => {
            // OpenCode also uses get_stats
            let result = crate::opencode::commands::stats::get_stats()?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        _ => Err(format!("Unknown tool: {}", tool)),
    }
}

/// Dispatch command: resume_session
/// For Claude: work_dir is the project path, file_path is the .jsonl file path (optional).
/// For Codex: work_dir is the cwd.
#[tauri::command]
pub fn resume_session(tool: String, session_id: String, work_dir: String, file_path: Option<String>) -> Result<(), String> {
    match tool.as_str() {
        "claude" => crate::claude::commands::terminal::resume_session(session_id, work_dir, file_path),
        "codex" => crate::codex::commands::terminal::resume_session(session_id, work_dir),
        "opencode" => crate::opencode::commands::terminal::resume_session(session_id, work_dir),
        _ => Err(format!("Unknown tool: {}", tool)),
    }
}
