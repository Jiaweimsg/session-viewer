use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// A single usage record to be sent to the server
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub client_version: String,
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

/// Read a value from global git config; empty/missing → None.
fn git_config(key: &str) -> Option<String> {
    std::process::Command::new("git")
        .args(["config", "--global", key])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// OS login name: $USER (Unix) / $USERNAME (Windows). Falls back to "unknown".
fn get_os_username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

/// User email: git config first (devs), fall back to `{os_user}@{hostname}.local`
/// so non-dev roles (PM/QA) remain identifiable by machine owner.
pub fn get_user_email() -> String {
    if let Some(email) = git_config("user.email") {
        return email;
    }
    format!("{}@{}.local", get_os_username(), get_machine_id())
}

/// User name: git config first, fall back to OS username.
pub fn get_user_name() -> String {
    git_config("user.name").unwrap_or_else(get_os_username)
}

/// Get machine hostname
pub fn get_machine_id() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Get client version from Cargo.toml at compile time
fn get_client_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// ── High-water-mark persistence ────────────────────────────────────
//
// Rationale: the local scan is the source of truth at *this moment* only.
// If the user clears their AI tool's cache, or the tool rotates/compacts
// old sessions, the next scan will yield smaller numbers than before and
// a naive upsert on the server would overwrite previously-reported data
// with lower values. To prevent regression we keep a local high-water-mark
// per (tool, date, project, model) and always report max(scan, mark).

/// Directory that stores session-viewer's persistent client state.
fn state_dir() -> Option<std::path::PathBuf> {
    let base = dirs::data_dir().or_else(dirs::config_dir)?;
    let dir = base.join("session-viewer");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn high_water_file() -> Option<std::path::PathBuf> {
    state_dir().map(|d| d.join("report-high-water.json"))
}

/// key = "tool|date|project|model"
fn hw_key(tool: &str, r: &UsageRecord) -> String {
    format!("{}|{}|{}|{}", tool, r.date, r.project, r.model)
}

fn load_high_water() -> HashMap<String, UsageRecord> {
    let Some(path) = high_water_file() else { return HashMap::new() };
    let Ok(content) = std::fs::read_to_string(&path) else { return HashMap::new() };
    serde_json::from_str(&content).unwrap_or_default()
}

fn save_high_water(marks: &HashMap<String, UsageRecord>) {
    let Some(path) = high_water_file() else { return };
    if let Ok(json) = serde_json::to_string(marks) {
        let _ = std::fs::write(&path, json);
    }
}

/// Merge freshly-scanned records against the stored high-water marks.
/// For each metric we take the max, so reported values never decrease.
/// Also backfills any date/project/model that existed historically but is
/// now missing locally (e.g., files deleted) so the server keeps seeing it.
fn apply_high_water(tool: &str, scanned: Vec<UsageRecord>) -> Vec<UsageRecord> {
    let mut marks = load_high_water();

    // Merge each scanned record with its mark, updating the mark with the max.
    let mut merged: HashMap<String, UsageRecord> = HashMap::new();
    for rec in scanned {
        let key = hw_key(tool, &rec);
        let prev = marks.get(&key);
        let out = UsageRecord {
            date: rec.date.clone(),
            project: rec.project.clone(),
            model: rec.model.clone(),
            input_tokens: rec.input_tokens.max(prev.map(|p| p.input_tokens).unwrap_or(0)),
            output_tokens: rec.output_tokens.max(prev.map(|p| p.output_tokens).unwrap_or(0)),
            cache_read_tokens: rec.cache_read_tokens.max(prev.map(|p| p.cache_read_tokens).unwrap_or(0)),
            cache_creation_tokens: rec.cache_creation_tokens.max(prev.map(|p| p.cache_creation_tokens).unwrap_or(0)),
            session_count: rec.session_count.max(prev.map(|p| p.session_count).unwrap_or(0)),
            message_count: rec.message_count.max(prev.map(|p| p.message_count).unwrap_or(0)),
        };
        marks.insert(key.clone(), out.clone());
        merged.insert(key, out);
    }

    // Re-emit any historical marks for this tool that didn't appear in this scan,
    // so the server doesn't see them as "deleted". Other tools' marks are left untouched.
    let tool_prefix = format!("{}|", tool);
    for (key, mark) in marks.iter() {
        if key.starts_with(&tool_prefix) && !merged.contains_key(key) {
            merged.insert(key.clone(), mark.clone());
        }
    }

    save_high_water(&marks);
    merged.into_values().collect()
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
        client_version: get_client_version(),
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
    // Bypass system proxies: the report server is on an internal network (172.x)
    // and macOS GUI apps inherit system proxy settings (e.g. Clash on 127.0.0.1:7890)
    // which cause 502s even when bypass rules include the internal range.
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;
    let email = get_user_email();
    let name = get_user_name();
    let machine_id = get_machine_id();

    let mut total_received: u64 = 0;

    // Collect from each tool
    let tools: Vec<(&str, Result<Vec<UsageRecord>, String>)> = vec![
        ("claude_code", crate::claude::commands::report::collect_usage_records()),
        ("codex", collect_codex_records()),
        ("opencode", collect_opencode_records()),
        ("copilot", collect_copilot_records()),
        ("cursor", collect_cursor_records()),
    ];

    for (tool_name, result) in tools {
        match result {
            Ok(records) => {
                // Merge with persisted high-water marks so cache clears /
                // log rotation can never drive reported values downward.
                let merged = apply_high_water(tool_name, records);
                if merged.is_empty() {
                    continue;
                }
                match send_tool_report(&client, &url, tool_name, merged, &email, &name, &machine_id).await {
                    Ok(resp) => {
                        let n = resp.received.unwrap_or(0);
                        eprintln!("[AutoReport] {}: sent {} records", tool_name, n);
                        total_received += n;
                    }
                    Err(e) => eprintln!("[AutoReport] {} error: {}", tool_name, e),
                }
            }
            Err(e) => eprintln!("[AutoReport] {} collect error: {}", tool_name, e),
        }
    }

    Ok(total_received)
}

// ── Codex collection ─────────────────────────────────────────

fn collect_codex_records() -> Result<Vec<UsageRecord>, String> {
    use crate::codex::parser::session_scanner::{scan_all_session_files, extract_date_from_path, short_name_from_path};
    use crate::codex::parser::jsonl::{extract_session_meta, extract_token_deltas, count_messages};

    let files = scan_all_session_files();
    if files.is_empty() {
        return Ok(Vec::new());
    }

    // Per-day buckets: (date, project, model) -> (input_fresh, output, cached, sessions, messages)
    type AggKey = (String, String, String);
    let mut agg: HashMap<AggKey, (u64, u64, u64, u64, u64)> = HashMap::new();

    for file_path in &files {
        let meta = extract_session_meta(file_path);
        let project = meta.as_ref()
            .map(|m| short_name_from_path(&m.cwd))
            .unwrap_or_else(|| "unknown".to_string());
        let model = meta.as_ref()
            .and_then(|m| m.model_provider.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let deltas = extract_token_deltas(file_path);

        // Session count: credit to session-file's date (each file = one session).
        let session_date = extract_date_from_path(file_path);
        let msg_count = count_messages(file_path) as u64;

        if deltas.is_empty() {
            // No token events in this session — still record the session for session/message count.
            if let Some(date) = session_date {
                let key = (date, project.clone(), model.clone());
                let entry = agg.entry(key).or_insert((0, 0, 0, 0, 0));
                entry.3 += 1;
                entry.4 += msg_count;
            }
            continue;
        }

        // Token deltas split by per-event timestamp (cross-midnight safe).
        for d in &deltas {
            let key = (d.date.clone(), project.clone(), model.clone());
            let entry = agg.entry(key).or_insert((0, 0, 0, 0, 0));
            entry.0 += d.input_fresh;
            entry.1 += d.output;
            entry.2 += d.cached;
        }

        // Session + message counts credited to session-file's date.
        if let Some(date) = session_date {
            let key = (date, project, model);
            let entry = agg.entry(key).or_insert((0, 0, 0, 0, 0));
            entry.3 += 1;
            entry.4 += msg_count;
        }
    }

    Ok(agg.into_iter().map(|((date, project, model), (input_fresh, output, cached, sessions, messages))| {
        UsageRecord {
            date,
            project,
            model,
            // Align with Anthropic semantics: input_tokens = fresh (uncached) input only.
            input_tokens: input_fresh,
            output_tokens: output,
            cache_read_tokens: cached,   // Codex cached_input_tokens → cache_read
            cache_creation_tokens: 0,    // Codex has no cache-creation concept
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
            .map(crate::shared_models::basename)
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
        let project = crate::shared_models::basename(&s.cwd);

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

// ── Cursor collection ───────────────────────────────────────

fn collect_cursor_records() -> Result<Vec<UsageRecord>, String> {
    use crate::cursor::parser::project_scanner::{
        read_composer_headers, read_bubbles, epoch_ms_to_rfc3339,
    };

    let headers = read_composer_headers();
    if headers.is_empty() {
        return Ok(Vec::new());
    }

    // Aggregate by (date, project, model)
    type AggKey = (String, String, String);
    let mut agg: HashMap<AggKey, (u64, u64, u64, u64)> = HashMap::new(); // (input, output, sessions, messages)

    for h in &headers {
        let created = match h.created_at {
            Some(ms) => epoch_ms_to_rfc3339(ms),
            None => continue,
        };
        let date = if created.len() >= 10 {
            created[..10].to_string()
        } else {
            continue
        };

        let project = h
            .workspace_path
            .as_deref()
            .map(crate::shared_models::basename)
            .unwrap_or_else(|| "unknown".to_string());

        let bubbles = read_bubbles(&h.composer_id);
        let msg_count = bubbles.len() as u64;

        // Determine model from user messages (type=1), fallback to "cursor"
        let model = bubbles.iter()
            .find_map(|b| {
                if b.msg_type == 1 {
                    b.model_name.as_deref()
                        .filter(|m| !m.is_empty())
                        .map(|m| m.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "cursor".to_string());

        // Sum tokens from all bubbles
        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;
        for b in &bubbles {
            if let Some(ref tc) = b.token_count {
                input_tokens += tc.input_tokens;
                output_tokens += tc.output_tokens;
            }
        }

        let key = (date, project, model);
        let entry = agg.entry(key).or_insert((0, 0, 0, 0));
        entry.0 += input_tokens;
        entry.1 += output_tokens;
        entry.2 += 1;
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
