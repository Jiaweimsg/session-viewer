use std::collections::HashMap;
use std::sync::OnceLock;
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
/// On Windows we set `CREATE_NO_WINDOW` (0x0800_0000) so the periodic
/// 5-min spawn inside `send_all_reports` doesn't flash a cmd window.
fn git_config(key: &str) -> Option<String> {
    let mut cmd = std::process::Command::new("git");
    cmd.args(["config", "--global", key]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.output()
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

/// User email: identity override → cached git config → `{os_user}@{hostname}.local`.
/// Override 让用户在设置页订正不准确的 hostname/git 身份；保存后下一轮上报立即生效。
pub fn get_user_email() -> String {
    if let Some(v) = crate::identity::load().user_email {
        if !v.trim().is_empty() {
            return v.trim().to_string();
        }
    }
    if let Some(email) = cached_git_config("user.email") {
        return email;
    }
    format!("{}@{}.local", get_os_username(), get_machine_id())
}

/// User name: identity override → cached git config → OS username.
pub fn get_user_name() -> String {
    if let Some(v) = crate::identity::load().user_name {
        if !v.trim().is_empty() {
            return v.trim().to_string();
        }
    }
    cached_git_config("user.name").unwrap_or_else(get_os_username)
}

/// Get machine hostname (cached, hostname doesn't change at runtime).
pub fn get_machine_id() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            hostname::get()
                .ok()
                .and_then(|h| h.into_string().ok())
                .unwrap_or_else(|| "unknown".to_string())
        })
        .clone()
}

/// `git config --global {key}` with per-key OnceLock cache. We only spawn `git`
/// once per process for each key — Windows users with broken git installs
/// (0xC0000142) won't keep paying the spawn cost on every 5-min cycle.
fn cached_git_config(key: &str) -> Option<String> {
    match key {
        "user.email" => {
            static CACHE: OnceLock<Option<String>> = OnceLock::new();
            CACHE.get_or_init(|| git_config(key)).clone()
        }
        "user.name" => {
            static CACHE: OnceLock<Option<String>> = OnceLock::new();
            CACHE.get_or_init(|| git_config(key)).clone()
        }
        _ => git_config(key),
    }
}

/// Returns the values that *would* be reported right now, broken down by
/// source. Used by the settings page to show "currently using X (from git /
/// override / fallback)" hints.
pub fn current_identity_view() -> serde_json::Value {
    let override_ = crate::identity::load();
    let git_email = cached_git_config("user.email");
    let git_name = cached_git_config("user.name");
    let os_user = get_os_username();
    let host = get_machine_id();
    serde_json::json!({
        "effective_email": get_user_email(),
        "effective_name": get_user_name(),
        "override_email": override_.user_email,
        "override_name": override_.user_name,
        "git_email": git_email,
        "git_name": git_name,
        "os_user": os_user,
        "hostname": host,
    })
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

/// One-time migration: before 0.5.x the cursor reporter wrote bubble-derived
/// token counts (input+output only, missing cache_read/write) under the real
/// workspace project name. Newer code emits accurate API totals under the
/// special project `(cursor)` instead, and reports zero tokens for the local
/// workspace rows. The high-water "max" semantics would otherwise pin those
/// old wrong values forever, double-counting on the server dashboard.
///
/// This migration deletes every cursor HW entry whose project is NOT `(cursor)`
/// — they will be recreated this round with zero token columns and the server
/// upsert will overwrite the stale rows. We mark completion with a flag file
/// so the migration runs at most once per machine.
fn migrate_cursor_hw_2026_05() {
    let Some(dir) = state_dir() else { return };
    let flag = dir.join("cursor-hw-migrated-2026-05.flag");
    if flag.exists() {
        return;
    }
    let mut marks = load_high_water();
    let before = marks.len();
    marks.retain(|key, _| {
        // key format: "tool|date|project|model"
        if !key.starts_with("cursor|") {
            return true;
        }
        let parts: Vec<&str> = key.splitn(4, '|').collect();
        // Malformed keys: keep them as-is rather than risk dropping non-cursor data.
        if parts.len() < 4 {
            return true;
        }
        parts[2] == "(cursor)"
    });
    let removed = before.saturating_sub(marks.len());
    if removed > 0 {
        save_high_water(&marks);
        eprintln!(
            "[AutoReport] migrated cursor high-water: removed {} stale workspace-project entries",
            removed
        );
    }
    let _ = std::fs::write(&flag, b"1");
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
    // Idempotent: only deletes stale cursor HW marks the first time it runs.
    migrate_cursor_hw_2026_05();

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
        ("cursor_cli", collect_cursor_cli_records()),
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
            .and_then(|m| m.model.clone().or_else(|| m.model_provider.clone()))
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
    use crate::opencode::parser::db_reader;
    use std::collections::HashSet;

    let conn = match db_reader::open_db() {
        Ok(c) => c,
        Err(_) => return Ok(Vec::new()), // DB not found — no OpenCode data
    };

    // Pull every assistant message + its project worktree in a single query.
    // Aggregating at the message level (not session level) lets cross-midnight
    // sessions credit each day correctly, and keys the model field on the
    // real `providerID/modelID` instead of a hard-coded "opencode".
    let messages = db_reader::query_all_assistant_messages_with_worktree(&conn);
    if messages.is_empty() {
        return Ok(Vec::new());
    }

    type AggKey = (String, String, String); // (date, project, model)
    // (input, output, cache_read, cache_write, msg_count, sessions_set)
    type AggVal = (u64, u64, u64, u64, u64, HashSet<String>);
    let mut agg: HashMap<AggKey, AggVal> = HashMap::new();

    for (msg, worktree) in &messages {
        let date = chrono::DateTime::from_timestamp(msg.time_created / 1000, 0)
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_default();
        if date.is_empty() {
            continue;
        }

        let project = if worktree.is_empty() {
            "unknown".to_string()
        } else {
            crate::shared_models::basename(worktree)
        };

        let provider = msg
            .data
            .get("providerID")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let model_id = msg
            .data
            .get("modelID")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let model = format!("{}/{}", provider, model_id);

        // OpenCode message data carries the canonical token shape:
        //   tokens: { input, output, reasoning, cache: { read, write } }
        // Missing keys mean 0 (e.g. non-thinking models leave reasoning at 0).
        let tokens = msg.data.get("tokens");
        let input = tokens
            .and_then(|t| t.get("input"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let output = tokens
            .and_then(|t| t.get("output"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cache_read = tokens
            .and_then(|t| t.get("cache"))
            .and_then(|c| c.get("read"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cache_write = tokens
            .and_then(|t| t.get("cache"))
            .and_then(|c| c.get("write"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let entry = agg
            .entry((date, project, model))
            .or_insert((0, 0, 0, 0, 0, HashSet::new()));
        entry.0 += input;
        entry.1 += output;
        entry.2 += cache_read;
        entry.3 += cache_write;
        entry.4 += 1;
        entry.5.insert(msg.session_id.clone());
    }

    Ok(agg
        .into_iter()
        .map(|((date, project, model), (input, output, cr, cw, msgs, sessions))| {
            UsageRecord {
                date,
                project,
                model,
                input_tokens: input,
                output_tokens: output,
                cache_read_tokens: cr,
                cache_creation_tokens: cw,
                session_count: sessions.len() as u64,
                message_count: msgs,
            }
        })
        .collect())
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

/// Cursor stats path:
/// 1. Pull authoritative token data from Cursor's official API
///    (`cursor.com/api/dashboard/export-usage-events-csv`). This is the only
///    place that exposes cache_read / cache_write correctly — local SQLite
///    bubbles only carry input/output and miss ~80%+ of real Agent usage.
/// 2. Aggregate API rows by (date, model) under project="(cursor)" since the
///    CSV has no per-project breakdown. session_count/message_count are zeroed
///    on the API side — local records below own those dimensions to keep the
///    server's totals additive and not double-counted.
/// 3. Still scan local composer + transcripts for per-project session /
///    message counts (with tokens forced to zero to avoid double-counting).
/// 4. When the API call fails (auth expired, network blocked, etc.) we DO NOT
///    fall back to the wrong bubble token path — instead we report local
///    session/message activity with zero tokens. Server-side high-water marks
///    preserve any previously-correct API values, and the dashboard sees
///    "no token data this round" rather than half-wrong numbers.
fn collect_cursor_records() -> Result<Vec<UsageRecord>, String> {
    match crate::cursor::api::usage_csv::fetch_usage_rows() {
        Ok(rows) => {
            let api_records = aggregate_cursor_api_rows(rows);
            let mut local_records = collect_cursor_local_records()?;
            // Tokens already counted from API — zero out local side to prevent
            // double counting; keep only session/message dimensions.
            for r in local_records.iter_mut() {
                r.input_tokens = 0;
                r.output_tokens = 0;
                r.cache_read_tokens = 0;
                r.cache_creation_tokens = 0;
            }
            let mut out = api_records;
            out.extend(local_records);
            Ok(out)
        }
        Err(e) => {
            // Auth/network problem. Reporting bubble-only tokens here would
            // poison server totals (missing cache → values land 5–10× low).
            // We instead emit session/message activity per project with zero
            // tokens; the high-water mechanism keeps prior correct totals.
            eprintln!("[AutoReport] cursor api unavailable ({}), reporting sessions/messages only with zero tokens", e);
            let mut local_records = collect_cursor_local_records()?;
            for r in local_records.iter_mut() {
                r.input_tokens = 0;
                r.output_tokens = 0;
                r.cache_read_tokens = 0;
                r.cache_creation_tokens = 0;
            }
            Ok(local_records)
        }
    }
}

fn aggregate_cursor_api_rows(rows: Vec<crate::cursor::api::usage_csv::CursorUsageRow>) -> Vec<UsageRecord> {
    // Aggregate by (date, model). Matches Cursor's official dashboard:
    //   - Tokens: ALL rows count (billable + non-billable). The Cursor panel's
    //     "Total Tokens" column does the same — TokenTracker (the reference
    //     impl we compared against) likewise normalizes without filtering.
    //   - session_count/message_count: zero on the API side; local scan owns
    //     those dimensions. A CSV row is a usage event (one model call),
    //     not a session, so counting 1-per-row here would double-count when
    //     merged with the local-side records below.
    type Key = (String, String);
    let mut agg: HashMap<Key, (u64, u64, u64, u64)> = HashMap::new();
    // tuple: (input, output, cache_read, cache_write)
    for r in rows {
        let key = (r.date.clone(), r.model.clone());
        let e = agg.entry(key).or_insert((0, 0, 0, 0));
        e.0 += r.input_tokens;
        e.1 += r.output_tokens;
        e.2 += r.cache_read_tokens;
        e.3 += r.cache_write_tokens;
    }
    agg.into_iter()
        .map(|((date, model), (input, output, cread, cwrite))| UsageRecord {
            date,
            project: "(cursor)".to_string(),
            model,
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: cread,
            cache_creation_tokens: cwrite,
            session_count: 0,
            message_count: 0,
        })
        .collect()
}

fn collect_cursor_local_records() -> Result<Vec<UsageRecord>, String> {
    use crate::cursor::parser::agent_transcripts as at;
    use crate::cursor::parser::project_scanner::{
        read_composer_headers, read_bubbles, epoch_ms_to_rfc3339,
    };

    // Aggregate by (date, project, model)
    type AggKey = (String, String, String);
    let mut agg: HashMap<AggKey, (u64, u64, u64, u64)> = HashMap::new(); // (input, output, sessions, messages)

    // ── Source 1: SQLite Composer (carries token counts) ──
    let headers = read_composer_headers();
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

    // ── Source 2: Agent Transcripts (no token counts; session/message only) ──
    //
    // Newer Cursor versions write Agent conversations as jsonl under
    // `~/.cursor/projects/{workspace_encoded}/agent-transcripts/...`. They have
    // no token info and no real workspace_path — we use `workspace_encoded` as
    // the project key. The dashboard's "查看问题" detail handler uses fuzzy
    // matching (server-side) to bridge encoded vs basename names so the
    // entry-point is reachable from usage rows.
    //
    // Without this block, transcripts produce conversation jsonl on the server
    // but no usage_records row → no clickable entry on the dashboard.
    let transcript_files = at::scan_all_transcript_files();
    for tpath in &transcript_files {
        let Some(tmeta) = at::extract_transcript_meta(tpath) else { continue };
        let Some(date) = at::date_from_epoch_ms(tmeta.file_mtime_ms) else { continue };
        let msg_count = at::count_user_messages(tpath);
        if msg_count == 0 { continue; }

        let project = tmeta.workspace_encoded.clone();
        let model = "cursor".to_string();
        let key = (date, project, model);
        let entry = agg.entry(key).or_insert((0, 0, 0, 0));
        entry.2 += 1;            // session
        entry.3 += msg_count;    // messages
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

// ── Cursor CLI collection ────────────────────────────────────
//
// `~/.cursor/chats/<project_hash>/<session_id>/store.db` carries no token
// counts, so we only emit session/message activity. Date is derived from the
// db file mtime; project from workspace_path (falls back to the project hash
// directory). Model is fixed to "cursor-cli" so dashboards can distinguish
// CLI activity from IDE composer/transcript activity reported under "cursor".
fn collect_cursor_cli_records() -> Result<Vec<UsageRecord>, String> {
    use crate::cursor::parser::cli_chats;

    type AggKey = (String, String, String);
    let mut agg: HashMap<AggKey, (u64, u64, u64, u64)> = HashMap::new();

    for session in cli_chats::load_all_sessions() {
        let modified = match session.modified() {
            Some(s) => s,
            None => continue,
        };
        let date = if modified.len() >= 10 {
            modified[..10].to_string()
        } else {
            continue;
        };

        let cwd = session.cwd();
        let project = crate::shared_models::basename(&cwd);
        let model = "cursor-cli".to_string();

        let msg_count = session.message_count() as u64;
        let prompt_count = session.user_prompt_rows_after(0).len() as u64;
        if msg_count == 0 && prompt_count == 0 {
            continue;
        }

        let key = (date, project, model);
        let entry = agg.entry(key).or_insert((0, 0, 0, 0));
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
