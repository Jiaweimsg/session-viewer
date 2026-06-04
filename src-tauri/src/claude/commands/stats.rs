use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::claude::models::stats::{
    AdvancedStats, DailyActivity, DailyModelTokens, DailyTokenEntry, EfficiencyBucket,
    ModelUsageEntry, ProjectTokenEntry, SessionEfficiency, StatsCache, ToolCallEntry,
    TokenUsageSummary,
};
use crate::claude::parser::path_encoder::{
    get_projects_dir, get_stats_cache_path, list_session_jsonl_files, short_name_from_path,
};

pub fn get_global_stats() -> Result<StatsCache, String> {
    let path = get_stats_cache_path().ok_or("Could not find stats cache path")?;

    // Cache file is owned by Claude Code itself and may be days/weeks stale.
    // If usable, treat it as a baseline and merge in any JSONL records newer
    // than `last_computed_date` so the current month is always covered.
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(cached) = serde_json::from_str::<StatsCache>(&content) {
                let cutoff = cached
                    .last_computed_date
                    .as_ref()
                    .filter(|d| d.len() >= 10)
                    .cloned();
                if !cached.daily_activity.is_empty() {
                    if let Some(cutoff_date) = cutoff {
                        return merge_incremental(cached, cutoff_date);
                    }
                }
            }
        }
    }

    // Fallback: cache missing/empty/unparseable — compute everything from JSONL.
    compute_stats_from_sessions()
}

/// Merge fresh JSONL records into a cached `StatsCache`.
/// Only files with mtime strictly after the cutoff day are opened, and only
/// records dated strictly after `cutoff_date` are accumulated, so the cache's
/// historical values stay untouched.
fn merge_incremental(mut base: StatsCache, cutoff_date: String) -> Result<StatsCache, String> {
    let projects_dir = get_projects_dir().ok_or("Could not find Claude projects directory")?;
    if !projects_dir.exists() {
        return Ok(base);
    }

    // Skip files whose mtime is on or before end-of-cutoff_date (00:00 UTC of
    // cutoff_date+1). Anything written that early can't contain records dated
    // after cutoff_date.
    let cutoff_mtime = match chrono::NaiveDate::parse_from_str(&cutoff_date, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.succ_opt())
        .and_then(|d| d.and_hms_opt(0, 0, 0))
    {
        Some(naive) => {
            let ts = naive.and_utc().timestamp();
            if ts < 0 {
                return Ok(base);
            }
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(ts as u64)
        }
        None => return Ok(base),
    };

    let mut delta_daily: HashMap<String, (u64, u64, u64)> = HashMap::new();
    let mut delta_model_tokens: HashMap<String, HashMap<String, u64>> = HashMap::new();
    let mut delta_model_usage: HashMap<String, ModelUsageEntry> = HashMap::new();
    // Global dedup of (message.id, requestId) across the incremental scan.
    let mut seen: HashSet<(String, String)> = HashSet::new();

    let entries = fs::read_dir(&projects_dir)
        .map_err(|e| format!("Failed to read projects dir: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        for file_path in list_session_jsonl_files(&path) {
            let mtime = match file_path.metadata().and_then(|m| m.modified()) {
                Ok(t) => t,
                Err(_) => continue,
            };
            if mtime <= cutoff_mtime {
                continue;
            }

            scan_session_incremental(
                &file_path,
                &cutoff_date,
                &mut delta_daily,
                &mut delta_model_tokens,
                &mut delta_model_usage,
                &mut seen,
            );
        }
    }

    // Merge daily_activity (add only — delta only contains dates > cutoff).
    let mut daily_map: HashMap<String, DailyActivity> = base
        .daily_activity
        .into_iter()
        .map(|d| (d.date.clone(), d))
        .collect();
    for (date, (msgs, sessions, tools)) in delta_daily {
        let entry = daily_map.entry(date.clone()).or_insert(DailyActivity {
            date,
            message_count: 0,
            session_count: 0,
            tool_call_count: 0,
        });
        entry.message_count += msgs;
        entry.session_count += sessions;
        entry.tool_call_count += tools;
    }
    let mut daily_activity: Vec<DailyActivity> = daily_map.into_values().collect();
    daily_activity.sort_by(|a, b| a.date.cmp(&b.date));
    base.daily_activity = daily_activity;

    // Merge daily_model_tokens.
    let mut dmt_map: HashMap<String, HashMap<String, u64>> = base
        .daily_model_tokens
        .into_iter()
        .map(|d| (d.date, d.tokens_by_model))
        .collect();
    for (date, tokens) in delta_model_tokens {
        let entry = dmt_map.entry(date).or_default();
        for (model, t) in tokens {
            *entry.entry(model).or_insert(0) += t;
        }
    }
    let mut dmt: Vec<DailyModelTokens> = dmt_map
        .into_iter()
        .map(|(date, tokens_by_model)| DailyModelTokens {
            date,
            tokens_by_model,
        })
        .collect();
    dmt.sort_by(|a, b| a.date.cmp(&b.date));
    base.daily_model_tokens = dmt;

    // Merge model_usage all-time totals.
    for (model, delta) in delta_model_usage {
        let entry = base
            .model_usage
            .entry(model)
            .or_insert_with(|| ModelUsageEntry {
                input_tokens: 0,
                output_tokens: 0,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            });
        entry.input_tokens += delta.input_tokens;
        entry.output_tokens += delta.output_tokens;
        entry.cache_read_input_tokens += delta.cache_read_input_tokens;
        entry.cache_creation_input_tokens += delta.cache_creation_input_tokens;
    }

    base.last_computed_date = Some(chrono::Utc::now().format("%Y-%m-%d").to_string());
    Ok(base)
}

/// Like `scan_session_for_stats`, but only emits records strictly after
/// `cutoff_date`. Session_count is credited only when the session's first
/// record is also after the cutoff (avoids double-counting carryover sessions
/// already present in the cache).
fn scan_session_incremental(
    path: &Path,
    cutoff_date: &str,
    daily_stats: &mut HashMap<String, (u64, u64, u64)>,
    daily_model_tokens: &mut HashMap<String, HashMap<String, u64>>,
    model_usage: &mut HashMap<String, ModelUsageEntry>,
    seen: &mut HashSet<(String, String)>,
) {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let reader = BufReader::new(file);

    let mut first_record_date: Option<String> = None;
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

        if first_record_date.is_none() {
            first_record_date = Some(date.clone());
        }

        if date.as_str() <= cutoff_date {
            continue;
        }

        // Skip duplicate content-block lines of one assistant response (same
        // message.id + requestId) — see scan_session_for_stats.
        if record_type == "assistant" {
            if let Some(msg) = v.get("message") {
                if let (Some(mid), Some(rid)) = (
                    msg.get("id").and_then(|x| x.as_str()),
                    v.get("requestId").and_then(|x| x.as_str()),
                ) {
                    if !seen.insert((mid.to_string(), rid.to_string())) {
                        continue;
                    }
                }
            }
        }

        *day_messages.entry(date.clone()).or_insert(0) += 1;

        if record_type == "assistant" {
            if let Some(msg) = v.get("message") {
                if let Some(content) = msg.get("content").and_then(|c| c.as_array()) {
                    for block in content {
                        if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                            *day_tools.entry(date.clone()).or_insert(0) += 1;
                        }
                    }
                }

                let model = msg
                    .get("model")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown");

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

                    let day_tokens = daily_model_tokens.entry(date.clone()).or_default();
                    *day_tokens.entry(model.to_string()).or_insert(0) += total_tokens;

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

    for (date, msg_count) in &day_messages {
        let entry = daily_stats.entry(date.clone()).or_insert((0, 0, 0));
        entry.0 += msg_count;
        entry.2 += day_tools.get(date).copied().unwrap_or(0);
    }

    if let Some(d) = first_record_date {
        if d.as_str() > cutoff_date {
            let entry = daily_stats.entry(d).or_insert((0, 0, 0));
            entry.1 += 1;
        }
    }
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
    // Global dedup of (message.id, requestId) — see scan_session_for_stats.
    let mut seen: HashSet<(String, String)> = HashSet::new();

    let entries = fs::read_dir(&projects_dir)
        .map_err(|e| format!("Failed to read projects dir: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        for file_path in list_session_jsonl_files(&path) {
            scan_session_for_stats(
                &file_path,
                &mut daily_stats,
                &mut daily_model_tokens,
                &mut model_usage,
                &mut seen,
            );
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
    seen: &mut HashSet<(String, String)>,
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

        // Skip duplicate content-block lines of one assistant response (same
        // message.id + requestId); Claude Code stamps every block-line with the
        // same cumulative usage, so without this tokens are summed once per block.
        if record_type == "assistant" {
            if let Some(msg) = v.get("message") {
                if let (Some(mid), Some(rid)) = (
                    msg.get("id").and_then(|x| x.as_str()),
                    v.get("requestId").and_then(|x| x.as_str()),
                ) {
                    if !seen.insert((mid.to_string(), rid.to_string())) {
                        continue;
                    }
                }
            }
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

        for file_path in list_session_jsonl_files(&path) {
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
    // Try to read cwd from a session file (subagent files inherit cwd too,
    // so the recursive listing is fine here).
    for file_path in list_session_jsonl_files(project_dir) {
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
    // Per-file dedup of (message.id, requestId): same-response content-block
    // lines repeat the cumulative usage, which would otherwise inflate this
    // session's token/message totals.
    let mut seen: HashSet<(String, String)> = HashSet::new();

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

        // Skip duplicate content-block lines of one assistant response.
        if record_type == "assistant" {
            if let Some(msg) = v.get("message") {
                if let (Some(mid), Some(rid)) = (
                    msg.get("id").and_then(|x| x.as_str()),
                    v.get("requestId").and_then(|x| x.as_str()),
                ) {
                    if !seen.insert((mid.to_string(), rid.to_string())) {
                        continue;
                    }
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_jsonl(path: &Path, lines: &[&str]) {
        let mut f = fs::File::create(path).unwrap();
        for l in lines {
            writeln!(f, "{}", l).unwrap();
        }
    }

    fn assistant_line(ts: &str, model: &str, input: u64, output: u64) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"{}","message":{{"model":"{}","content":[{{"type":"tool_use","name":"Read"}}],"usage":{{"input_tokens":{},"output_tokens":{}}}}}}}"#,
            ts, model, input, output
        )
    }

    fn user_line(ts: &str) -> String {
        format!(r#"{{"type":"user","timestamp":"{}","message":{{"role":"user","content":"hi"}}}}"#, ts)
    }

    #[test]
    fn scan_incremental_only_emits_post_cutoff_records() {
        // Brand-new session entirely after cutoff: must credit a session and
        // accumulate messages/tokens.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("session.jsonl");
        write_jsonl(
            &file,
            &[
                &user_line("2026-05-13T09:00:00Z"),
                &assistant_line("2026-05-13T09:00:01Z", "claude-sonnet-4-6", 200, 80),
                &assistant_line("2026-05-14T10:00:00Z", "claude-opus-4-7", 300, 120),
            ],
        );

        let mut daily: HashMap<String, (u64, u64, u64)> = HashMap::new();
        let mut dmt: HashMap<String, HashMap<String, u64>> = HashMap::new();
        let mut usage: HashMap<String, ModelUsageEntry> = HashMap::new();
        let mut seen: HashSet<(String, String)> = HashSet::new();
        scan_session_incremental(&file, "2026-05-12", &mut daily, &mut dmt, &mut usage, &mut seen);

        let d13 = daily.get("2026-05-13").expect("missing 2026-05-13");
        assert_eq!(d13.0, 2, "2026-05-13 message_count");
        assert_eq!(d13.2, 1, "2026-05-13 tool_call_count");
        assert_eq!(d13.1, 1, "fresh session credited on first record day");

        let d14 = daily.get("2026-05-14").expect("missing 2026-05-14");
        assert_eq!(d14.0, 1, "2026-05-14 message_count");
        assert_eq!(d14.2, 1, "2026-05-14 tool_call_count");
        assert_eq!(d14.1, 0, "second day should not get a session");

        let sonnet = usage.get("claude-sonnet-4-6").expect("sonnet usage");
        assert_eq!(sonnet.input_tokens, 200);
        assert_eq!(sonnet.output_tokens, 80);
        let opus = usage.get("claude-opus-4-7").expect("opus usage");
        assert_eq!(opus.input_tokens, 300);
        assert_eq!(opus.output_tokens, 120);
    }

    #[test]
    fn scan_incremental_skips_carryover_session_and_ignores_pre_cutoff_records() {
        // Session started before cutoff and continues after — pre-cutoff records
        // must be ignored entirely (already in cache), and session_count must
        // NOT be credited again (would double-count).
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("session.jsonl");
        write_jsonl(
            &file,
            &[
                &user_line("2026-04-30T08:00:00Z"),
                &assistant_line("2026-04-30T08:00:01Z", "claude-sonnet-4-6", 50, 20),
                &user_line("2026-05-13T09:00:00Z"),
                &assistant_line("2026-05-13T09:00:01Z", "claude-sonnet-4-6", 100, 40),
            ],
        );

        let mut daily: HashMap<String, (u64, u64, u64)> = HashMap::new();
        let mut dmt: HashMap<String, HashMap<String, u64>> = HashMap::new();
        let mut usage: HashMap<String, ModelUsageEntry> = HashMap::new();
        let mut seen: HashSet<(String, String)> = HashSet::new();
        scan_session_incremental(&file, "2026-05-12", &mut daily, &mut dmt, &mut usage, &mut seen);

        assert!(!daily.contains_key("2026-04-30"), "pre-cutoff date leaked");
        assert!(!dmt.contains_key("2026-04-30"), "pre-cutoff tokens leaked");

        let d13 = daily.get("2026-05-13").expect("missing 2026-05-13");
        assert_eq!(d13.0, 2, "messages on 2026-05-13");
        assert_eq!(d13.1, 0, "carryover session must not be re-counted");

        let sonnet = usage.get("claude-sonnet-4-6").expect("sonnet usage");
        assert_eq!(sonnet.input_tokens, 100, "only post-cutoff tokens");
        assert_eq!(sonnet.output_tokens, 40);
    }
}
