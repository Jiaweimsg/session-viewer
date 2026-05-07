use std::collections::HashMap;

use crate::opencode::models::stats::{DailyTokenEntry, TokenSummary};
use crate::opencode::parser::db_reader::{open_db, query_all_assistant_messages};

pub fn get_stats() -> Result<TokenSummary, String> {
    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => {
            return Ok(TokenSummary {
                total_input_tokens: 0,
                total_output_tokens: 0,
                total_tokens: 0,
                tokens_by_model: HashMap::new(),
                daily_tokens: vec![],
                session_count: 0,
                message_count: 0,
            })
        }
    };

    let session_count = conn
        .query_row("SELECT COUNT(*) FROM session", [], |r| r.get::<_, i64>(0))
        .map(|c| c as usize)
        .unwrap_or(0);

    let message_count = conn
        .query_row("SELECT COUNT(*) FROM message", [], |r| r.get::<_, i64>(0))
        .map(|c| c as usize)
        .unwrap_or(0);

    let assistant_messages = query_all_assistant_messages(&conn);

    let mut total_input = 0u64;
    let mut total_output = 0u64;
    let mut tokens_by_model: HashMap<String, u64> = HashMap::new();
    let mut daily_map: HashMap<String, (u64, u64)> = HashMap::new();

    for msg in &assistant_messages {
        let tokens = match msg.data.get("tokens") {
            Some(t) => t,
            None => continue,
        };

        let input = tokens.get("input").and_then(|v| v.as_u64()).unwrap_or(0);
        let output = tokens.get("output").and_then(|v| v.as_u64()).unwrap_or(0);
        let cache_write = tokens
            .get("cache")
            .and_then(|c| c.get("write"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cache_read = tokens
            .get("cache")
            .and_then(|c| c.get("read"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let effective_input = input + cache_write + cache_read;
        total_input += effective_input;
        total_output += output;

        let provider = msg.data.get("providerID").and_then(|v| v.as_str()).unwrap_or("unknown");
        let model = msg.data.get("modelID").and_then(|v| v.as_str()).unwrap_or("unknown");
        let model_key = format!("{}/{}", provider, model);
        *tokens_by_model.entry(model_key).or_insert(0) += effective_input + output;

        let date = chrono::DateTime::from_timestamp(msg.time_created / 1000, 0)
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_default();
        if !date.is_empty() {
            let entry = daily_map.entry(date).or_insert((0, 0));
            entry.0 += effective_input;
            entry.1 += output;
        }
    }

    let mut daily_tokens: Vec<DailyTokenEntry> = daily_map
        .into_iter()
        .map(|(date, (input, output))| DailyTokenEntry {
            date,
            input_tokens: input,
            output_tokens: output,
            total_tokens: input + output,
        })
        .collect();
    daily_tokens.sort_by(|a, b| a.date.cmp(&b.date));

    Ok(TokenSummary {
        total_input_tokens: total_input,
        total_output_tokens: total_output,
        total_tokens: total_input + total_output,
        tokens_by_model,
        daily_tokens,
        session_count,
        message_count,
    })
}
