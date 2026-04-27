use serde_json::Value;
use crate::state::AppState;

const STATS_CACHE_TTL_SECS: u64 = 60;

/// Check if cached stats are still valid for a tool
fn get_cached_stats(state: &AppState, tool: &str) -> Option<(Value, Value, Value)> {
    let cache = state.stats_cache.lock();
    if let Some(cached) = cache.get(tool) {
        if cached.cached_at.elapsed().as_secs() < STATS_CACHE_TTL_SECS {
            return Some((
                cached.stats_json.clone(),
                cached.token_summary_json.clone(),
                cached.advanced_stats_json.clone(),
            ));
        }
    }
    None
}

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
        "copilot" => {
            let result = crate::copilot::commands::projects::get_projects()?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "cursor" => {
            let result = crate::cursor::commands::projects::get_projects()?;
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
        "copilot" => {
            let result = crate::copilot::commands::sessions::get_sessions(project_key)?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "cursor" => {
            let result = crate::cursor::commands::sessions::get_sessions(project_key)?;
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
        "copilot" => {
            let result = crate::copilot::commands::messages::get_messages(
                session_key,
                page,
                page_size,
            )?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "cursor" => {
            let result = crate::cursor::commands::messages::get_messages(
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
        "copilot" => {
            let result = crate::copilot::commands::search::search_sessions(query)?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "cursor" => {
            let result = crate::cursor::commands::search::global_search(query, max_results)?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        _ => Err(format!("Unknown tool: {}", tool)),
    }
}

/// Dispatch command: get_stats
/// Uses in-memory cache to avoid re-scanning files on every call.
#[tauri::command]
pub fn get_stats(tool: String, state: tauri::State<'_, AppState>) -> Result<Value, String> {
    if let Some((stats, _, _)) = get_cached_stats(&state, &tool) {
        return Ok(stats);
    }
    // Compute and cache all three at once
    compute_and_cache_stats(&tool, &state)?;
    let cache = state.stats_cache.lock();
    Ok(cache.get(&tool).map(|c| c.stats_json.clone()).unwrap_or(Value::Null))
}

/// Dispatch command: get_token_summary
/// Uses in-memory cache to avoid re-scanning files on every call.
#[tauri::command]
pub fn get_token_summary(tool: String, state: tauri::State<'_, AppState>) -> Result<Value, String> {
    if let Some((_, token_summary, _)) = get_cached_stats(&state, &tool) {
        return Ok(token_summary);
    }
    compute_and_cache_stats(&tool, &state)?;
    let cache = state.stats_cache.lock();
    Ok(cache.get(&tool).map(|c| c.token_summary_json.clone()).unwrap_or(Value::Null))
}

/// Compute all stats for a tool and cache them together (single scan)
fn compute_and_cache_stats(tool: &str, state: &AppState) -> Result<(), String> {
    let (stats_json, token_json, advanced_json) = match tool {
        "claude" => {
            let stats = crate::claude::commands::stats::get_global_stats()?;
            let token = crate::claude::commands::stats::get_token_summary()?;
            let advanced = crate::claude::commands::stats::get_advanced_stats().unwrap_or_default();
            (
                serde_json::to_value(stats).map_err(|e| e.to_string())?,
                serde_json::to_value(token).map_err(|e| e.to_string())?,
                serde_json::to_value(advanced).map_err(|e| e.to_string())?,
            )
        }
        "codex" => {
            let stats = crate::codex::commands::stats::get_stats()?;
            let json = serde_json::to_value(stats).map_err(|e| e.to_string())?;
            (json.clone(), json, Value::Null)
        }
        "opencode" => {
            let stats = crate::opencode::commands::stats::get_stats()?;
            let json = serde_json::to_value(stats).map_err(|e| e.to_string())?;
            (json.clone(), json, Value::Null)
        }
        "copilot" => {
            let stats = crate::copilot::commands::stats::get_stats()?;
            let json = serde_json::to_value(stats).map_err(|e| e.to_string())?;
            (json.clone(), json, Value::Null)
        }
        "cursor" => {
            let stats = crate::cursor::commands::stats::get_stats()?;
            let json = serde_json::to_value(stats).map_err(|e| e.to_string())?;
            (json.clone(), json, Value::Null)
        }
        _ => return Err(format!("Unknown tool: {}", tool)),
    };
    let mut cache = state.stats_cache.lock();
    cache.insert(tool.to_string(), crate::state::CachedStats {
        stats_json,
        token_summary_json: token_json,
        advanced_stats_json: advanced_json,
        cached_at: std::time::Instant::now(),
    });
    Ok(())
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
        "copilot" => crate::copilot::commands::terminal::resume_session(session_id, work_dir),
        "cursor" => crate::cursor::commands::terminal::resume_session(session_id, work_dir),
        _ => Err(format!("Unknown tool: {}", tool)),
    }
}

/// Dispatch command: report_usage
/// Send usage data for all tools to a remote server
#[tauri::command]
pub async fn report_usage(server_url: String) -> Result<Value, String> {
    let total = crate::report::send_all_reports(&server_url).await?;
    Ok(serde_json::json!({ "ok": true, "received": total }))
}

/// Dispatch command: get_advanced_stats
/// Uses in-memory cache.
#[tauri::command]
pub fn get_advanced_stats(tool: String, state: tauri::State<'_, AppState>) -> Result<Value, String> {
    if let Some((_, _, advanced)) = get_cached_stats(&state, &tool) {
        return Ok(advanced);
    }
    compute_and_cache_stats(&tool, &state)?;
    let cache = state.stats_cache.lock();
    Ok(cache.get(&tool).map(|c| c.advanced_stats_json.clone()).unwrap_or(Value::Null))
}

/// 读取上报黑名单（cwd 前缀列表）。
#[tauri::command]
pub fn get_upload_blocklist() -> crate::blocklist::UploadBlocklist {
    crate::blocklist::load()
}

/// 覆写上报黑名单。前端传完整列表，后端直接落盘。
#[tauri::command]
pub fn set_upload_blocklist(blocklist: crate::blocklist::UploadBlocklist) -> Result<(), String> {
    crate::blocklist::save(&blocklist)
}

/// 读取当前生效的身份信息：override / git / os fallback 全套。
/// 前端用来显示"目前会上报为 X，来源是 git 配置"等提示。
#[tauri::command]
pub fn get_identity_view() -> Value {
    crate::report::current_identity_view()
}

/// 读取用户保存的 identity override（不含 fallback）。
#[tauri::command]
pub fn get_identity_override() -> crate::identity::IdentityOverride {
    crate::identity::load()
}

/// 覆写 identity override。空串/None 表示清除该字段、回退到默认值。
#[tauri::command]
pub fn set_identity_override(
    identity: crate::identity::IdentityOverride,
) -> Result<(), String> {
    crate::identity::save(&identity)
}

/// 重置 conversation 上报状态：删除 conversation-state.json，下一轮 cycle 会
/// fresh scan 全部历史。配合服务端 uuid 去重，重复消息不会落盘。
#[tauri::command]
pub fn reset_conversation_state() -> Result<(), String> {
    crate::conversation::state::reset()
}
