use crate::cursor::parser::cli_chats;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorCliSearchResult {
    pub session_id: String,
    pub project_key: Option<String>,
    pub project_name: Option<String>,
    pub session_name: Option<String>,
    pub matched_text: String,
    pub role: String,
    pub timestamp: Option<String>,
}

fn truncate_for_match(text: &str) -> String {
    if text.chars().count() > 200 {
        let truncated: String = text.chars().take(200).collect();
        format!("{}...", truncated)
    } else {
        text.to_string()
    }
}

fn assistant_text(value: &Value) -> Option<String> {
    if value.get("role").and_then(|r| r.as_str()) != Some("assistant") {
        return None;
    }
    match value.get("content")? {
        Value::String(s) => Some(s.clone()),
        Value::Array(items) => {
            let parts: Vec<String> = items
                .iter()
                .filter_map(|item| {
                    if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                        item.get("text").and_then(|t| t.as_str()).map(str::to_string)
                    } else {
                        None
                    }
                })
                .collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n\n"))
            }
        }
        _ => None,
    }
}

pub fn global_search(
    query: String,
    max_results: usize,
) -> Result<Vec<CursorCliSearchResult>, String> {
    let q = query.to_lowercase();
    if q.is_empty() {
        return Ok(Vec::new());
    }

    let mut results: Vec<CursorCliSearchResult> = Vec::new();

    'outer: for session in cli_chats::load_all_sessions() {
        if results.len() >= max_results {
            break;
        }
        let cwd = session.cwd();
        let project_name = Some(crate::shared_models::basename(&cwd));
        let project_key = Some(cwd);

        if let Some(name) = &session.meta.name {
            if name.to_lowercase().contains(&q) {
                results.push(CursorCliSearchResult {
                    session_id: cli_chats::encode_session_key(
                        &session.project_hash,
                        &session.session_id,
                    ),
                    project_key: project_key.clone(),
                    project_name: project_name.clone(),
                    session_name: session.meta.name.clone(),
                    matched_text: name.clone(),
                    role: "session".to_string(),
                    timestamp: session.created(),
                });
                if results.len() >= max_results {
                    break 'outer;
                }
            }
        }

        for row in &session.rows {
            if results.len() >= max_results {
                break 'outer;
            }
            let text = cli_chats::extract_user_query_text(&row.value)
                .or_else(|| assistant_text(&row.value));
            let Some(text) = text else { continue };
            if !text.to_lowercase().contains(&q) {
                continue;
            }
            results.push(CursorCliSearchResult {
                session_id: cli_chats::encode_session_key(
                    &session.project_hash,
                    &session.session_id,
                ),
                project_key: project_key.clone(),
                project_name: project_name.clone(),
                session_name: session.meta.name.clone(),
                matched_text: truncate_for_match(&text),
                role: row
                    .value
                    .get("role")
                    .and_then(|r| r.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                timestamp: None,
            });
        }
    }

    Ok(results)
}
