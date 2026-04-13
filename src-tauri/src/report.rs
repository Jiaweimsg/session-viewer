use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// A single usage record to be sent to the server
#[derive(Debug, Clone, Serialize)]
pub struct UsageRecord {
    pub date: String,
    pub project: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub session_count: u64,
    pub message_count: u64,
}

/// The payload sent to POST /api/report
#[derive(Debug, Serialize)]
pub struct ReportPayload {
    pub user_email: String,
    pub user_name: String,
    pub machine_id: String,
    pub tool: String,
    pub records: Vec<UsageRecord>,
    pub reported_at: String,
}

/// Response from the server
#[derive(Debug, Serialize, Deserialize)]
pub struct ReportResponse {
    pub ok: Option<bool>,
    pub received: Option<u64>,
    pub error: Option<String>,
}

/// Get user email from git config
pub fn get_git_user_email() -> String {
    std::process::Command::new("git")
        .args(["config", "--global", "user.email"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown@localhost".to_string())
}

/// Get user name from git config
pub fn get_git_user_name() -> String {
    std::process::Command::new("git")
        .args(["config", "--global", "user.name"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default()
}

/// Get machine hostname
pub fn get_machine_id() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Send a single tool's usage report to the server
async fn send_tool_report(
    client: &reqwest::Client,
    url: &str,
    tool: &str,
    records: Vec<UsageRecord>,
    email: &str,
    name: &str,
    machine_id: &str,
) -> Result<ReportResponse, String> {
    if records.is_empty() {
        return Ok(ReportResponse { ok: Some(true), received: Some(0), error: None });
    }

    let payload = ReportPayload {
        user_email: email.to_string(),
        user_name: name.to_string(),
        machine_id: machine_id.to_string(),
        tool: tool.to_string(),
        records,
        reported_at: chrono::Utc::now().to_rfc3339(),
    };

    let resp = client
        .post(url)
        .json(&payload)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("Failed to send report: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Server returned {}: {}", status, body));
    }

    resp.json::<ReportResponse>()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// Send usage reports for ALL tools to the server
pub async fn send_all_reports(server_url: &str) -> Result<u64, String> {
    let url = format!("{}/api/report", server_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let email = get_git_user_email();
    let name = get_git_user_name();
    let machine_id = get_machine_id();

    let mut total_received: u64 = 0;

    // Collect from each tool
    let tools: Vec<(&str, Result<Vec<UsageRecord>, String>)> = vec![
        ("claude_code", crate::claude::commands::report::collect_usage_records()),
        ("codex", collect_codex_records()),
        ("opencode", collect_opencode_records()),
        ("copilot", collect_copilot_records()),
    ];

    for (tool_name, result) in tools {
        match result {
            Ok(records) if !records.is_empty() => {
                match send_tool_report(&client, &url, tool_name, records, &email, &name, &machine_id).await {
                    Ok(resp) => {
                        let n = resp.received.unwrap_or(0);
                        eprintln!("[AutoReport] {}: sent {} records", tool_name, n);
                        total_received += n;
                    }
                    Err(e) => eprintln!("[AutoReport] {} error: {}", tool_name, e),
                }
            }
            Ok(_) => {} // empty, skip
            Err(e) => eprintln!("[AutoReport] {} collect error: {}", tool_name, e),
        }
    }

    Ok(total_received)
}

// ── Codex collection ─────────────────────────────────────────

fn collect_codex_records() -> Result<Vec<UsageRecord>, String> {
    use crate::codex::parser::session_scanner::{scan_all_session_files, extract_date_from_path, short_name_from_path};
    use crate::codex::parser::jsonl::{extract_session_meta, extract_token_info, count_messages};

    let files = scan_all_session_files();
    if files.is_empty() {
        return Ok(Vec::new());
    }

    // Key: (date, project, model)
    type AggKey = (String, String, String);
    let mut agg: HashMap<AggKey, (u64, u64, u64, u64)> = HashMap::new(); // input, output, session, messages

    for file_path in &files {
        let date = match extract_date_from_path(file_path) {
            Some(d) => d,
            None => continue,
        };

        let meta = extract_session_meta(file_path);
        let project = meta.as_ref()
            .map(|m| short_name_from_path(&m.cwd))
            .unwrap_or_else(|| "unknown".to_string());
        let model = meta.as_ref()
            .and_then(|m| m.model_provider.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let (input, output) = match extract_token_info(file_path) {
            Some(t) => (t.input_tokens, t.output_tokens),
            None => (0, 0),
        };

        let msg_count = count_messages(file_path) as u64;

        let key = (date, project, model);
        let entry = agg.entry(key).or_insert((0, 0, 0, 0));
        entry.0 += input;
        entry.1 += output;
        entry.2 += 1; // session count
        entry.3 += msg_count;
    }

    Ok(agg.into_iter().map(|((date, project, model), (input, output, sessions, messages))| {
        UsageRecord {
            date,
            project,
            model,
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            session_count: sessions,
            message_count: messages,
        }
    }).collect())
}

// ── OpenCode collection ──────────────────────────────────────

fn collect_opencode_records() -> Result<Vec<UsageRecord>, String> {
    use crate::opencode::parser::session_scanner::{scan_all_session_files, get_message_dir};
    use std::fs;

    let session_files = scan_all_session_files();
    if session_files.is_empty() {
        return Ok(Vec::new());
    }

    let message_dir = get_message_dir().ok_or("OpenCode message dir not found")?;
    let mut records = Vec::new();

    // Each session file is a JSON with session metadata
    for path in &session_files {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let v: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let session_id = v.get("id").and_then(|i| i.as_str()).unwrap_or("");
        let project = v.get("worktree").and_then(|w| w.as_str())
            .map(|p| p.rsplit('/').next().unwrap_or("unknown").to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Get date from updatedAt or createdAt
        let date = v.get("updatedAt").or_else(|| v.get("createdAt"))
            .and_then(|d| d.as_str())
            .and_then(|s| s.get(..10))
            .unwrap_or("")
            .to_string();

        if date.is_empty() || session_id.is_empty() {
            continue;
        }

        // Count messages in message dir
        let msg_dir = message_dir.join(session_id);
        let msg_count = if msg_dir.exists() {
            fs::read_dir(&msg_dir)
                .map(|e| e.filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().map(|ext| ext == "json").unwrap_or(false))
                    .count() as u64)
                .unwrap_or(0)
        } else {
            0
        };

        records.push(UsageRecord {
            date,
            project,
            model: "opencode".to_string(),
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            session_count: 1,
            message_count: msg_count,
        });
    }

    Ok(records)
}

// ── Copilot collection ───────────────────────────────────────

fn collect_copilot_records() -> Result<Vec<UsageRecord>, String> {
    use crate::copilot::parser::session_scanner::scan_all_sessions;

    let sessions = scan_all_sessions();
    if sessions.is_empty() {
        return Ok(Vec::new());
    }

    let mut records = Vec::new();
    for s in &sessions {
        let date = s.created_at.get(..10).unwrap_or("").to_string();
        if date.is_empty() {
            continue;
        }
        let project = s.cwd.rsplit('/').next().unwrap_or("unknown").to_string();

        records.push(UsageRecord {
            date,
            project,
            model: "copilot".to_string(),
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            session_count: 1,
            message_count: s.message_count as u64,
        });
    }

    Ok(records)
}
