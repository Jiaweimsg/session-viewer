use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::claude::models::stats::{
    AdvancedStats, DailyActivity, DailyModelTokens, DailyTokenEntry, EfficiencyBucket,
    ModelUsageEntry, ProjectTokenEntry, SessionEfficiency, StatsCache, ToolCallEntry,
    TokenUsageSummary,
};
use crate::claude::parser::path_encoder::{get_projects_dir, get_stats_cache_path, short_name_from_path};

pub fn get_global_stats() -> Result<StatsCache, String> {
    let path = get_stats_cache_path().ok_or("Could not find stats cache path")?;

    if path.exists() {
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read stats cache: {}", e))?;

        let stats: StatsCache = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse stats cache: {}", e))?;

        // If cache has data, use it
        if !stats.daily_activity.is_empty() {
            return Ok(stats);
        }
    }

    // Fallback: compute stats dynamically from session files
    compute_stats_from_sessions()
}

pub fn get_token_summary() -> Result<TokenUsageSummary, String> {
    let stats = get_global_stats()?;

    let mut total_tokens: u64 = 0;
    let mut tokens_by_model: HashMap<String, u64> = HashMap::new();
    let mut daily_tokens: Vec<DailyTokenEntry> = Vec::new();

    // Compute total input/output from modelUsage for ratio calculation
    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;
    for usage in stats.model_usage.values() {
        total_input_tokens +=
            usage.input_tokens + usage.cache_read_input_tokens + usage.cache_creation_input_tokens;
        total_output_tokens += usage.output_tokens;
    }

    // Compute input ratio for proportional daily split
    let global_total = total_input_tokens + total_output_tokens;
    let input_ratio = if global_total > 0 {
        total_input_tokens as f64 / global_total as f64
    } else {
        0.5
    };

    for day in &stats.daily_model_tokens {
        let mut day_total: u64 = 0;
        for (model, tokens) in &day.tokens_by_model {
            total_tokens += tokens;
            day_total += tokens;
            *tokens_by_model.entry(model.clone()).or_insert(0) += tokens;
        }

        // Distribute daily total into input/output using global ratio
        let day_input = (day_total as f64 * input_ratio) as u64;
        let day_output = day_total.saturating_sub(day_input);

        daily_tokens.push(DailyTokenEntry {
            date: day.date.clone(),
            input_tokens: day_input,
            output_tokens: day_output,
            total_tokens: day_total,
        });
    }

    Ok(TokenUsageSummary {
        total_input_tokens,
        total_output_tokens,
        total_tokens,
        tokens_by_model,
        daily_tokens,
    })
}

/// Compute stats dynamically by scanning all session JSONL files
fn compute_stats_from_sessions() -> Result<StatsCache, String> {
    let projects_dir = get_projects_dir().ok_or("Could not find Claude projects directory")?;

    if !projects_dir.exists() {
        return Ok(StatsCache {
            version: None,
            last_computed_date: None,
            daily_activity: Vec::new(),
            daily_model_tokens: Vec::new(),
            model_usage: HashMap::new(),
        });
    }

    // daily_date -> (messages, sessions, tool_calls)
    let mut daily_stats: HashMap<String, (u64, u64, u64)> = HashMap::new();
    // daily_date -> model -> tokens
    let mut daily_model_tokens: HashMap<String, HashMap<String, u64>> = HashMap::new();
    // model -> ModelUsageEntry
    let mut model_usage: HashMap<String, ModelUsageEntry> = HashMap::new();

    let entries = fs::read_dir(&projects_dir)
        .map_err(|e| format!("Failed to read projects dir: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let files = match fs::read_dir(&path) {
            Ok(f) => f,
            Err(_) => continue,
        };

        for file_entry in files.flatten() {
            let file_path = file_entry.path();
            if file_path
                .extension()
                .map(|e| e == "jsonl")
                .unwrap_or(false)
            {
                scan_session_for_stats(
                    &file_path,
                    &mut daily_stats,
                    &mut daily_model_tokens,
                    &mut model_usage,
                );
            }
        }
    }

    // Convert to sorted vectors
    let mut daily_activity: Vec<DailyActivity> = daily_stats
        .into_iter()
        .map(
            |(date, (messages, sessions, tool_calls))| DailyActivity {
                date,
                message_count: messages,
                session_count: sessions,
                tool_call_count: tool_calls,
            },
        )
        .collect();
    daily_activity.sort_by(|a, b| a.date.cmp(&b.date));

    let mut daily_model_tokens_vec: Vec<DailyModelTokens> = daily_model_tokens
        .into_iter()
        .map(|(date, tokens_by_model)| DailyModelTokens {
            date,
            tokens_by_model,
        })
        .collect();
    daily_model_tokens_vec.sort_by(|a, b| a.date.cmp(&b.date));

    Ok(StatsCache {
        version: Some(1),
        last_computed_date: Some(chrono::Utc::now().format("%Y-%m-%d").to_string()),
        daily_activity,
        daily_model_tokens: daily_model_tokens_vec,
        model_usage,
    })
}

/// Scan a single JSONL session file and accumulate stats
fn scan_session_for_stats(
    path: &Path,
    daily_stats: &mut HashMap<String, (u64, u64, u64)>,
    daily_model_tokens: &mut HashMap<String, HashMap<String, u64>>,
    model_usage: &mut HashMap<String, ModelUsageEntry>,
) {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let reader = BufReader::new(file);

    let mut session_date: Option<String> = None;

    // Track per-day message and tool counts within this session
    let mut day_messages: HashMap<String, u64> = HashMap::new();
    let mut day_tools: HashMap<String, u64> = HashMap::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Quick pre-filter: skip known large/irrelevant record types
        if trimmed.contains("\"type\":\"file-history-snapshot\"")
            || trimmed.contains("\"type\":\"progress\"")
        {
            continue;
        }

        let v: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let record_type = match v.get("type").and_then(|t| t.as_str()) {
            Some(t) => t,
            None => continue,
        };

        if record_type != "user" && record_type != "assistant" {
            continue;
        }

        // Extract date from timestamp
        let date = v
            .get("timestamp")
            .and_then(|t| t.as_str())
            .and_then(|ts| {
                if ts.len() >= 10 {
                    Some(ts[..10].to_string())
                } else {
                    None
                }
            });

        let date = match date {
            Some(d) => d,
            None => continue,
        };

        if session_date.is_none() {
            session_date = Some(date.clone());
        }

        // Count messages per day
        *day_messages.entry(date.clone()).or_insert(0) += 1;

        if record_type == "assistant" {
            if let Some(msg) = v.get("message") {
                // Count tool_use blocks
                if let Some(content) = msg.get("content").and_then(|c| c.as_array()) {
                    for block in content {
                        if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                            *day_tools.entry(date.clone()).or_insert(0) += 1;
                        }
                    }
                }

                // Extract token usage
                let model = msg
                    .get("model")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown");

                // Skip synthetic/placeholder messages (error responses, no real API call)
                if model == "<synthetic>" || model == "unknown" {
                    continue;
                }

                if let Some(usage) = msg.get("usage") {
                    let input = usage
                        .get("input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let output = usage
                        .get("output_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let cache_read = usage
                        .get("cache_read_input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let cache_creation = usage
                        .get("cache_creation_input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);

                    let total_tokens = input + output + cache_read + cache_creation;

                    // Update daily model tokens
                    let day_tokens = daily_model_tokens.entry(date.clone()).or_default();
                    *day_tokens.entry(model.to_string()).or_insert(0) += total_tokens;

                    // Update model usage totals
                    let entry =
                        model_usage
                            .entry(model.to_string())
                            .or_insert_with(|| ModelUsageEntry {
                                input_tokens: 0,
                                output_tokens: 0,
                                cache_read_input_tokens: 0,
                                cache_creation_input_tokens: 0,
                            });
                    entry.input_tokens += input;
                    entry.output_tokens += output;
                    entry.cache_read_input_tokens += cache_read;
                    entry.cache_creation_input_tokens += cache_creation;
                }
            }
        }
    }

    // Add per-day message/tool counts to global daily stats
    for (date, msg_count) in &day_messages {
        let entry = daily_stats.entry(date.clone()).or_insert((0, 0, 0));
        entry.0 += msg_count;
        entry.2 += day_tools.get(date).copied().unwrap_or(0);
    }

    // Add session count to the session's first date
    if let Some(date) = session_date {
        let entry = daily_stats.entry(date).or_insert((0, 0, 0));
        entry.1 += 1;
    }
}

// ============ Advanced Stats ============

pub fn get_advanced_stats() -> Result<AdvancedStats, String> {
    let projects_dir = get_projects_dir().ok_or("Could not find Claude projects directory")?;

    if !projects_dir.exists() {
        return Ok(AdvancedStats {
            project_token_ranking: Vec::new(),
            tool_call_ranking: Vec::new(),
            efficiency: SessionEfficiency {
                avg_messages_per_session: 0.0,
                avg_tokens_per_session: 0.0,
                total_sessions: 0,
                total_messages: 0,
                distribution: Vec::new(),
            },
        });
    }

    // project_name -> (total_tokens, input_tokens, output_tokens)
    let mut project_tokens: HashMap<String, (u64, u64, u64)> = HashMap::new();
    // tool_name -> call_count
    let mut tool_calls: HashMap<String, u64> = HashMap::new();
    // per-session message counts for efficiency analysis
    let mut session_msg_counts: Vec<u64> = Vec::new();
    let mut total_tokens_all: u64 = 0;

    let entries = fs::read_dir(&projects_dir)
        .map_err(|e| format!("Failed to read projects dir: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let project_name = resolve_project_name(&path);

        let files = match fs::read_dir(&path) {
            Ok(f) => f,
            Err(_) => continue,
        };

        for file_entry in files.flatten() {
            let file_path = file_entry.path();
            if file_path
                .extension()
                .map(|e| e == "jsonl")
                .unwrap_or(false)
            {
                let result = scan_session_advanced(&file_path);
                // Accumulate project tokens
                let proj = project_tokens.entry(project_name.clone()).or_insert((0, 0, 0));
                proj.0 += result.total_tokens;
                proj.1 += result.input_tokens;
                proj.2 += result.output_tokens;
                total_tokens_all += result.total_tokens;

                // Accumulate tool calls
                for (name, count) in &result.tool_calls {
                    *tool_calls.entry(name.clone()).or_insert(0) += count;
                }

                // Track session message count
                if result.message_count > 0 {
                    session_msg_counts.push(result.message_count);
                }
            }
        }
    }

    // Build project token ranking (top 10)
    let mut project_token_ranking: Vec<ProjectTokenEntry> = project_tokens
        .into_iter()
        .map(|(name, (total, input, output))| ProjectTokenEntry {
            project_name: name,
            total_tokens: total,
            input_tokens: input,
            output_tokens: output,
        })
        .collect();
    project_token_ranking.sort_by_key(|b| std::cmp::Reverse(b.total_tokens));
    project_token_ranking.truncate(10);

    // Build tool call ranking (top 15)
    let mut tool_call_ranking: Vec<ToolCallEntry> = tool_calls
        .into_iter()
        .map(|(name, count)| ToolCallEntry {
            tool_name: name,
            call_count: count,
        })
        .collect();
    tool_call_ranking.sort_by_key(|b| std::cmp::Reverse(b.call_count));
    tool_call_ranking.truncate(15);

    // Build session efficiency
    let total_sessions = session_msg_counts.len() as u64;
    let total_messages: u64 = session_msg_counts.iter().sum();
    let avg_messages = if total_sessions > 0 {
        total_messages as f64 / total_sessions as f64
    } else {
        0.0
    };
    let avg_tokens = if total_sessions > 0 {
        total_tokens_all as f64 / total_sessions as f64
    } else {
        0.0
    };

    // Message count distribution buckets
    let buckets = [
        ("1-5", 1u64, 5u64),
        ("6-10", 6, 10),
        ("11-20", 11, 20),
        ("21-50", 21, 50),
        ("50+", 51, u64::MAX),
    ];
    let distribution: Vec<EfficiencyBucket> = buckets
        .iter()
        .map(|(label, lo, hi)| {
            let count = session_msg_counts
                .iter()
                .filter(|&&c| c >= *lo && c <= *hi)
                .count() as u64;
            EfficiencyBucket {
                label: label.to_string(),
                count,
            }
        })
        .collect();

    Ok(AdvancedStats {
        project_token_ranking,
        tool_call_ranking,
        efficiency: SessionEfficiency {
            avg_messages_per_session: (avg_messages * 10.0).round() / 10.0,
            avg_tokens_per_session: (avg_tokens * 10.0).round() / 10.0,
            total_sessions,
            total_messages,
            distribution,
        },
    })
}

/// Resolve project name from a project directory by reading cwd from session files
fn resolve_project_name(project_dir: &Path) -> String {
    // Try to read cwd from a session file
    if let Ok(dir_entries) = fs::read_dir(project_dir) {
        for entry in dir_entries.flatten() {
            let file_path = entry.path();
            if file_path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                let file = match fs::File::open(&file_path) {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                let reader = BufReader::new(file);
                for line in reader.lines().take(10) {
                    let line = match line {
                        Ok(l) => l,
                        Err(_) => continue,
                    };
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(line.trim()) {
                        if let Some(cwd) = v.get("cwd").and_then(|c| c.as_str()) {
                            if !cwd.is_empty() {
                                return short_name_from_path(cwd);
                            }
                        }
                    }
                }
            }
        }
    }
    // Fallback: use directory name
    project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}

struct SessionScanResult {
    message_count: u64,
    total_tokens: u64,
    input_tokens: u64,
    output_tokens: u64,
    tool_calls: HashMap<String, u64>,
}

/// Scan a single JSONL session file for advanced stats
fn scan_session_advanced(path: &Path) -> SessionScanResult {
    let mut result = SessionScanResult {
        message_count: 0,
        total_tokens: 0,
        input_tokens: 0,
        output_tokens: 0,
        tool_calls: HashMap::new(),
    };

    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return result,
    };
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.contains("\"type\":\"file-history-snapshot\"")
            || trimmed.contains("\"type\":\"progress\"")
        {
            continue;
        }

        let v: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let record_type = match v.get("type").and_then(|t| t.as_str()) {
            Some(t) => t,
            None => continue,
        };

        if record_type != "user" && record_type != "assistant" {
            continue;
        }

        result.message_count += 1;

        if record_type == "assistant" {
            if let Some(msg) = v.get("message") {
                // Collect tool names
                if let Some(content) = msg.get("content").and_then(|c| c.as_array()) {
                    for block in content {
                        if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                            if let Some(name) = block.get("name").and_then(|n| n.as_str()) {
                                *result.tool_calls.entry(name.to_string()).or_insert(0) += 1;
                            }
                        }
                    }
                }

                let model = msg.get("model").and_then(|m| m.as_str()).unwrap_or("");
                if model == "<synthetic>" || model.is_empty() {
                    continue;
                }

                if let Some(usage) = msg.get("usage") {
                    let input = usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    let output = usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    let cache_read = usage.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    let cache_creation = usage.get("cache_creation_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);

                    result.input_tokens += input + cache_read + cache_creation;
                    result.output_tokens += output;
                    result.total_tokens += input + output + cache_read + cache_creation;
                }
            }
        }
    }

    result
}
