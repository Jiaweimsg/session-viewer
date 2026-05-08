use crate::opencode::parser::db_reader::{open_db, query_all_text_parts};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpencodeSearchResult {
    pub project_id: String,
    pub session_id: String,
    pub first_prompt: Option<String>,
    pub matched_text: String,
    pub role: String,
    pub timestamp: Option<String>,
    pub message_id: String,
}

pub fn global_search(query: String, max_results: usize) -> Result<Vec<OpencodeSearchResult>, String> {
    if query.is_empty() {
        return Ok(vec![]);
    }

    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => return Ok(vec![]),
    };

    let query_lower = query.to_lowercase();
    let all_parts = query_all_text_parts(&conn);

    let results = all_parts
        .into_iter()
        .filter_map(|(part, project_id)| {
            let text = part.data.get("text")?.as_str()?;
            if !text.to_lowercase().contains(&query_lower) {
                return None;
            }

            let matched_text = extract_match_context(text, &query, 200);
            let timestamp = Some(
                chrono::DateTime::from_timestamp(part.time_created / 1000, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default(),
            );

            Some(OpencodeSearchResult {
                project_id,
                session_id: part.session_id.clone(),
                first_prompt: None,
                matched_text,
                role: String::new(),
                timestamp,
                message_id: part.message_id.clone(),
            })
        })
        .take(max_results)
        .collect();

    Ok(results)
}

fn extract_match_context(text: &str, query: &str, context_len: usize) -> String {
    let text_lower = text.to_lowercase();
    let query_lower = query.to_lowercase();

    if let Some(pos) = text_lower.find(&query_lower) {
        let mut start = pos.saturating_sub(context_len / 2);
        while start > 0 && !text.is_char_boundary(start) {
            start -= 1;
        }
        let raw_end = pos + query_lower.len() + context_len / 2;
        let mut end = std::cmp::min(raw_end, text.len());
        while end < text.len() && !text.is_char_boundary(end) {
            end += 1;
        }
        let mut snippet = text[start..end].to_string();
        if start > 0 { snippet = format!("...{}", snippet); }
        if end < text.len() { snippet = format!("{}...", snippet); }
        snippet
    } else {
        text.chars().take(context_len).collect()
    }
}
