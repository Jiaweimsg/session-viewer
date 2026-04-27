use crate::conversation::{ConversationMessage, MAX_BATCH_BYTES, state::ConversationState};
use crate::conversation::scanner::PendingMessage;
use std::collections::HashMap;
use std::path::PathBuf;

/// Split pending messages into batches where each batch's serialized size
/// (sum of `serde_json::to_vec(msg).len()`) is <= `max_bytes`. A single message
/// larger than `max_bytes` becomes its own batch (payload may exceed the limit —
/// rare, accepted).
pub fn split_into_batches(pending: Vec<PendingMessage>, max_bytes: usize) -> Vec<Vec<PendingMessage>> {
    let mut batches: Vec<Vec<PendingMessage>> = Vec::new();
    let mut current: Vec<PendingMessage> = Vec::new();
    let mut current_size: usize = 0;

    for p in pending {
        let size = serde_json::to_vec(&p.message).map(|v| v.len()).unwrap_or(0);
        if !current.is_empty() && current_size + size > max_bytes {
            batches.push(std::mem::take(&mut current));
            current_size = 0;
        }
        current.push(p);
        current_size += size;
    }
    if !current.is_empty() {
        batches.push(current);
    }
    batches
}

/// For a set of messages, return the largest `line_end` seen per source file.
pub fn max_offsets_by_file(msgs: &[PendingMessage]) -> HashMap<PathBuf, u64> {
    let mut m: HashMap<PathBuf, u64> = HashMap::new();
    for p in msgs {
        let cur = m.entry(p.file.clone()).or_insert(0);
        if p.line_end > *cur {
            *cur = p.line_end;
        }
    }
    m
}

/// Update state in place so that each file's offset advances to the max
/// line_end observed in `msgs`. Does not persist — caller must call state::save.
pub fn advance_state(state: &mut ConversationState, msgs: &[PendingMessage]) {
    for (path, end) in max_offsets_by_file(msgs) {
        // Skip synthetic "cursor:..." paths — those are handled by
        // cursor_scanner::advance_marks, which updates state.cursor_marks.
        if path.to_str().map(|s| s.starts_with("cursor:")).unwrap_or(false) {
            continue;
        }
        let current = state.offset_for(&path);
        if end > current {
            state.set_offset(path, end);
        }
    }
}

use serde::Serialize;

#[derive(Debug, Serialize)]
struct ConversationPayload<'a> {
    user_email: &'a str,
    user_name: &'a str,
    machine_id: &'a str,
    client_version: String,
    tool: &'a str,
    reported_at: String,
    messages: Vec<&'a ConversationMessage>,
}

#[derive(Debug, serde::Deserialize)]
struct ConversationResponse {
    #[serde(default)]
    ok: Option<bool>,
    #[serde(default)]
    received: Option<u64>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug)]
pub enum UploadError {
    /// 4xx: payload-level error; do not retry automatically. Caller should
    /// dead-letter and still advance offsets to avoid death loops.
    ClientError(String),
    /// 5xx or network: retry next cycle; do not advance offsets.
    Transient(String),
}

impl std::fmt::Display for UploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClientError(s) => write!(f, "4xx: {}", s),
            Self::Transient(s) => write!(f, "transient: {}", s),
        }
    }
}

pub async fn send_batch(
    client: &reqwest::Client,
    url: &str,
    tool: &str,
    user_email: &str,
    user_name: &str,
    machine_id: &str,
    batch: &[PendingMessage],
) -> Result<u64, UploadError> {
    let payload = ConversationPayload {
        user_email,
        user_name,
        machine_id,
        client_version: env!("CARGO_PKG_VERSION").to_string(),
        tool,
        reported_at: chrono::Utc::now().to_rfc3339(),
        messages: batch.iter().map(|p| &p.message).collect(),
    };

    let resp = client
        .post(url)
        .json(&payload)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| UploadError::Transient(format!("send: {}", e)))?;

    let status = resp.status();
    if status.is_client_error() {
        let body = resp.text().await.unwrap_or_default();
        return Err(UploadError::ClientError(format!("{} {}", status, body)));
    }
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(UploadError::Transient(format!("{} {}", status, body)));
    }
    let parsed: ConversationResponse = resp
        .json()
        .await
        .map_err(|e| UploadError::Transient(format!("parse: {}", e)))?;
    if let Some(err) = parsed.error {
        return Err(UploadError::ClientError(err));
    }
    let _ = parsed.ok; // suppress unused-field warning
    Ok(parsed.received.unwrap_or(0))
}

use crate::conversation::{scanner, state};

fn dead_letter_file() -> Option<PathBuf> {
    let base = dirs::data_dir().or_else(dirs::config_dir)?;
    let dir = base.join("session-viewer");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("conversation-errors.log"))
}

/// 循环诊断日志：每轮 cycle 与每次 batch 的结果都写一行。
/// Windows 上 release build 没有 console，eprintln! 看不见 —— 这个文件是
/// 用户/我们排查上报失败时的唯一可见入口。
/// 路径：`{state_dir}/conversation-cycle.log`。
fn cycle_log_file() -> Option<PathBuf> {
    let base = dirs::data_dir().or_else(dirs::config_dir)?;
    let dir = base.join("session-viewer");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("conversation-cycle.log"))
}

fn log_cycle(line: &str) {
    eprintln!("{}", line);
    let Some(path) = cycle_log_file() else { return };
    use std::io::Write;
    let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) else { return };
    let ts = chrono::Utc::now().to_rfc3339();
    let _ = writeln!(f, "{} {}", ts, line);
}

fn log_dead_letter(batch: &[PendingMessage], err: &str) {
    let Some(path) = dead_letter_file() else { return };
    use std::io::Write;
    let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) else { return };
    let ts = chrono::Utc::now().to_rfc3339();
    let uuids: Vec<&str> = batch.iter().map(|p| p.message.uuid.as_str()).collect();
    let _ = writeln!(f, "{} error={} count={} uuids={:?}", ts, err, batch.len(), uuids);
}

/// Scan all configured tools' sessions and upload pending messages in 10MB batches.
/// Advances per-file offsets only for batches that succeed or 4xx (to avoid
/// death loops). On 5xx/network errors, stops and leaves remaining work for
/// the next cycle.
///
/// `tools` is iterated sequentially; state is loaded once and shared across
/// tools (Claude and Codex file paths never collide, so no partitioning is
/// needed).
pub async fn flush(server_url: &str, tools: &[&str]) -> Result<u64, String> {
    let url = format!("{}/api/conversations", server_url.trim_end_matches('/'));
    log_cycle(&format!("[Conversation] cycle start url={} tools={:?}", url, tools));
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .map_err(|e| {
            let msg = format!("client build: {}", e);
            log_cycle(&format!("[Conversation] FATAL {}", msg));
            msg
        })?;

    let email = crate::report::get_user_email();
    let name = crate::report::get_user_name();
    let machine = crate::report::get_machine_id();

    let mut state_snapshot = state::load();
    let blocklist = crate::blocklist::load();
    let mut total: u64 = 0;
    for &tool in tools {
        let pending = match tool {
            "claude_code" => scanner::scan_all(&state_snapshot),
            "codex" => crate::conversation::codex_scanner::scan_all(&state_snapshot),
            "cursor" => crate::conversation::cursor_scanner::scan_all(&state_snapshot),
            other => {
                log_cycle(&format!("[Conversation] unknown tool '{}', skipping", other));
                continue;
            }
        };
        log_cycle(&format!("[Conversation/{}] scanned {} pending messages", tool, pending.len()));
        if pending.is_empty() {
            continue;
        }

        // 黑名单过滤：命中 cwd 的消息不上报，但仍推进 offset，避免下轮重复扫。
        let total_pending = pending.len();
        let (pending, blocked): (Vec<PendingMessage>, Vec<PendingMessage>) = pending
            .into_iter()
            .partition(|p| !blocklist.is_blocked(&p.message.cwd));
        if !blocked.is_empty() {
            log_cycle(&format!(
                "[Conversation/{}] blocklist filtered {} of {} messages",
                tool,
                blocked.len(),
                total_pending
            ));
            match tool {
                "cursor" => {
                    crate::conversation::cursor_scanner::advance_marks(
                        &mut state_snapshot.cursor_marks,
                        &blocked,
                    );
                    advance_state(&mut state_snapshot, &blocked);
                }
                _ => advance_state(&mut state_snapshot, &blocked),
            }
            state::save(&state_snapshot);
        }
        if pending.is_empty() {
            continue;
        }

        for batch in split_into_batches(pending, MAX_BATCH_BYTES) {
            match send_batch(&client, &url, tool, &email, &name, &machine, &batch).await {
                Ok(n) => {
                    match tool {
                        "cursor" => {
                            crate::conversation::cursor_scanner::advance_marks(&mut state_snapshot.cursor_marks, &batch);
                            advance_state(&mut state_snapshot, &batch);
                        }
                        _ => advance_state(&mut state_snapshot, &batch),
                    }
                    state_snapshot.last_scan_at = Some(chrono::Utc::now().to_rfc3339());
                    state::save(&state_snapshot);
                    total += n;
                    log_cycle(&format!("[Conversation/{}] uploaded {} messages", tool, n));
                }
                Err(UploadError::ClientError(e)) => {
                    log_dead_letter(&batch, &e);
                    match tool {
                        "cursor" => {
                            crate::conversation::cursor_scanner::advance_marks(&mut state_snapshot.cursor_marks, &batch);
                            advance_state(&mut state_snapshot, &batch);
                        }
                        _ => advance_state(&mut state_snapshot, &batch),
                    }
                    state::save(&state_snapshot);
                    log_cycle(&format!("[Conversation/{}] 4xx dead-lettered: {}", tool, e));
                }
                Err(UploadError::Transient(e)) => {
                    log_cycle(&format!(
                        "[Conversation/{}] transient error, will retry next cycle: {}",
                        tool, e
                    ));
                    return Err(e);
                }
            }
        }
    }
    log_cycle(&format!("[Conversation] cycle end total_uploaded={}", total));
    Ok(total)
}

#[cfg(test)]
mod batch_tests {
    use super::*;
    use crate::conversation::RoleTag;

    fn mk(uuid: &str, text_size: usize) -> PendingMessage {
        PendingMessage {
            file: PathBuf::from("/x.jsonl"),
            line_end: 0,
            message: ConversationMessage {
                uuid: uuid.into(),
                session_id: "s".into(),
                parent_uuid: None,
                timestamp: "2026-04-22T00:00:00Z".into(),
                project: "p".into(),
                cwd: "/p".into(),
                git_branch: None,
                model: None,
                role_tag: RoleTag::Followup,
                text: "x".repeat(text_size),
            },
        }
    }

    #[test]
    fn empty_input_yields_no_batches() {
        let batches = split_into_batches(vec![], 1024);
        assert!(batches.is_empty());
    }

    #[test]
    fn everything_fits_in_one_batch() {
        let pending = vec![mk("a", 100), mk("b", 100)];
        let batches = split_into_batches(pending, 10_000);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].len(), 2);
    }

    #[test]
    fn splits_when_size_exceeds_limit() {
        let pending = vec![mk("a", 500), mk("b", 500), mk("c", 500)];
        let batches = split_into_batches(pending, 700);
        assert_eq!(batches.len(), 3);
    }

    #[test]
    fn single_oversized_item_becomes_its_own_batch() {
        let pending = vec![mk("a", 100), mk("b", 10_000)];
        let batches = split_into_batches(pending, 1_000);
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].len(), 1);
        assert_eq!(batches[0][0].message.uuid, "a");
        assert_eq!(batches[1].len(), 1);
        assert_eq!(batches[1][0].message.uuid, "b");
    }

    #[test]
    fn uses_max_batch_bytes_constant() {
        assert_eq!(MAX_BATCH_BYTES, 10 * 1024 * 1024);
    }

    #[test]
    fn max_offsets_by_file_picks_highest_per_file() {
        let msgs = vec![
            PendingMessage {
                file: PathBuf::from("/a.jsonl"),
                line_end: 100,
                message: mk("a", 1).message,
            },
            PendingMessage {
                file: PathBuf::from("/a.jsonl"),
                line_end: 200,
                message: mk("b", 1).message,
            },
            PendingMessage {
                file: PathBuf::from("/b.jsonl"),
                line_end: 50,
                message: mk("c", 1).message,
            },
        ];
        let m = max_offsets_by_file(&msgs);
        assert_eq!(m.get(&PathBuf::from("/a.jsonl")).copied(), Some(200));
        assert_eq!(m.get(&PathBuf::from("/b.jsonl")).copied(), Some(50));
    }

    #[test]
    fn advance_state_updates_offsets() {
        let mut state = ConversationState::default();
        let msgs = vec![
            PendingMessage {
                file: PathBuf::from("/a.jsonl"),
                line_end: 999,
                message: mk("a", 1).message,
            },
        ];
        advance_state(&mut state, &msgs);
        assert_eq!(state.offset_for(&PathBuf::from("/a.jsonl")), 999);
    }

    #[test]
    fn advance_state_skips_synthetic_cursor_paths() {
        let mut state = ConversationState::default();
        let msgs = vec![
            PendingMessage {
                file: PathBuf::from("cursor:comp-abc:1700000000000"),
                line_end: 42,
                message: mk("a", 1).message,
            },
            PendingMessage {
                file: PathBuf::from("/Users/bin/.cursor/projects/x/agent-transcripts/s/s.jsonl"),
                line_end: 999,
                message: mk("b", 1).message,
            },
        ];
        advance_state(&mut state, &msgs);
        // Real path gets file_offsets entry
        assert_eq!(state.offset_for(&PathBuf::from("/Users/bin/.cursor/projects/x/agent-transcripts/s/s.jsonl")), 999);
        // Synthetic "cursor:..." does NOT get an entry
        assert!(!state.file_offsets.contains_key(&PathBuf::from("cursor:comp-abc:1700000000000")));
    }
}

#[cfg(test)]
mod http_tests {
    use super::*;
    use crate::conversation::RoleTag;

    fn mk_msg() -> PendingMessage {
        PendingMessage {
            file: PathBuf::from("/x.jsonl"),
            line_end: 100,
            message: ConversationMessage {
                uuid: "u".into(),
                session_id: "s".into(),
                parent_uuid: None,
                timestamp: "2026-04-22T00:00:00Z".into(),
                project: "p".into(),
                cwd: "/p".into(),
                git_branch: None,
                model: Some("claude-opus-4-6".into()),
                role_tag: RoleTag::First,
                text: "hello".into(),
            },
        }
    }

    #[tokio::test]
    async fn success_returns_received_count() {
        let mut server = mockito::Server::new_async().await;
        let mock = server.mock("POST", "/api/conversations")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"ok":true,"received":1}"#)
            .create_async()
            .await;
        let client = reqwest::Client::new();
        let url = format!("{}/api/conversations", server.url());
        let result = send_batch(&client, &url, "claude_code", "a@b", "a", "m", &[mk_msg()]).await;
        mock.assert_async().await;
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn server_500_is_transient() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server.mock("POST", "/api/conversations")
            .with_status(500)
            .with_body("boom")
            .create_async()
            .await;
        let client = reqwest::Client::new();
        let url = format!("{}/api/conversations", server.url());
        let err = send_batch(&client, &url, "claude_code", "a@b", "a", "m", &[mk_msg()]).await.unwrap_err();
        assert!(matches!(err, UploadError::Transient(_)));
    }

    #[tokio::test]
    async fn server_400_is_client_error() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server.mock("POST", "/api/conversations")
            .with_status(400)
            .with_body("bad")
            .create_async()
            .await;
        let client = reqwest::Client::new();
        let url = format!("{}/api/conversations", server.url());
        let err = send_batch(&client, &url, "claude_code", "a@b", "a", "m", &[mk_msg()]).await.unwrap_err();
        assert!(matches!(err, UploadError::ClientError(_)));
    }
}

