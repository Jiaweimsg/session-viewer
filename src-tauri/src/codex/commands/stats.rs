use std::collections::HashMap;
use std::path::PathBuf;

use crate::codex::models::stats::{DailyTokenEntry, TokenUsageSummary};
use crate::codex::parser::jsonl::{count_messages, extract_session_meta, extract_token_info};
use crate::codex::parser::session_scanner::{extract_date_from_path, scan_all_session_files};

pub fn get_stats() -> Result<TokenUsageSummary, String> {
    let files = scan_all_session_files();
    get_stats_for_files(&files)
}

fn get_stats_for_files(files: &[PathBuf]) -> Result<TokenUsageSummary, String> {
    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;
    let mut total_reasoning_tokens: u64 = 0;
    let mut total_tokens: u64 = 0;
    let mut tokens_by_model: HashMap<String, u64> = HashMap::new();
    let mut daily_map: HashMap<String, (u64, u64, u64, u64)> = HashMap::new();
    let mut session_count: u64 = 0;
    let mut message_count: u64 = 0;

    for file_path in files {
        session_count += 1;
        message_count += count_messages(file_path) as u64;

        let model_key = extract_session_meta(file_path)
            .and_then(|m| m.model.or(m.model_provider))
            .unwrap_or_else(|| "unknown".to_string());

        // Get token info
        if let Some(token_info) = extract_token_info(file_path) {
            total_input_tokens += token_info.input_tokens;
            total_output_tokens += token_info.output_tokens;
            total_reasoning_tokens += token_info.reasoning_output_tokens;
            total_tokens += token_info.total_tokens;

            *tokens_by_model.entry(model_key).or_insert(0) += token_info.total_tokens;

            // Aggregate by date
            if let Some(date) = extract_date_from_path(file_path) {
                let entry = daily_map.entry(date).or_insert((0, 0, 0, 0));
                entry.0 += token_info.input_tokens;
                entry.1 += token_info.output_tokens;
                entry.2 += token_info.reasoning_output_tokens;
                entry.3 += token_info.total_tokens;
            }
        }
    }

    // Convert daily map to sorted vec
    let mut daily_tokens: Vec<DailyTokenEntry> = daily_map
        .into_iter()
        .map(|(date, (input, output, reasoning, total))| DailyTokenEntry {
            date,
            input_tokens: input,
            output_tokens: output,
            reasoning_tokens: reasoning,
            total_tokens: total,
        })
        .collect();
    daily_tokens.sort_by(|a, b| a.date.cmp(&b.date));

    Ok(TokenUsageSummary {
        total_input_tokens,
        total_output_tokens,
        total_reasoning_tokens,
        total_tokens,
        tokens_by_model,
        daily_tokens,
        session_count,
        message_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn tokens_by_model_prefers_turn_context_model_over_provider() {
        let dir = tempfile::tempdir().unwrap();
        let day_dir = dir.path().join("sessions/2026/05/27");
        std::fs::create_dir_all(&day_dir).unwrap();
        let file_path = day_dir.join("rollout-1-test.jsonl");
        let mut file = std::fs::File::create(&file_path).unwrap();

        writeln!(
            file,
            r#"{{"timestamp":"2026-05-27T01:00:00Z","type":"session_meta","payload":{{"id":"sess-1","cwd":"/tmp/project","model_provider":"sssaicode"}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"timestamp":"2026-05-27T01:00:01Z","type":"turn_context","payload":{{"model":"gpt-5.1-codex-max"}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"timestamp":"2026-05-27T01:00:02Z","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":100,"cached_input_tokens":20,"output_tokens":50,"reasoning_output_tokens":10,"total_tokens":150}}}}}}}}"#
        )
        .unwrap();

        let stats = get_stats_for_files(&[file_path]).unwrap();

        assert_eq!(stats.tokens_by_model.get("gpt-5.1-codex-max"), Some(&150));
        assert_eq!(stats.tokens_by_model.get("sssaicode"), None);
    }
}
