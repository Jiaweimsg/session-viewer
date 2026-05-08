//! Cursor conversation scanner.
//!
//! Cursor stores sessions in a SQLite DB (not append-only JSONL), so byte-offset
//! watermarks don't apply. Instead we track per-composer marks of the form
//! `(last_updated_at, bubble_index)`. Composers whose header last_updated_at has
//! not advanced are skipped without reading their bubbles. Bubbles beyond
//! `bubble_index` are emitted as new PendingMessages; if the bubble count has
//! shrunk (user deleted messages), we reset to 0 and re-emit all (which the
//! server will see as duplicates but analysis will dedupe by uuid).
//!
//! PendingMessage carries Cursor marker state by encoding:
//!   file     = PathBuf::from("cursor:{composer_id}:{updated_at_ms}")
//!   line_end = bubble_idx
//! The `advance_marks` function decodes these and updates ConversationState.cursor_marks.

use crate::conversation::codex_scanner::is_system_injection;
use crate::conversation::scanner::{classify_role_tag, PendingMessage};
use crate::conversation::state::{ConversationState, CursorMark};
use crate::conversation::{ConversationMessage, RoleTag};
use crate::cursor::parser::cli_chats::{self, CliSessionData};
use crate::cursor::parser::project_scanner::{
    read_bubbles, read_composer_headers, CursorBubble,
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Compose a stable per-(session, text) uuid. Content-based hashing means user
/// edits to a message yield a new uuid (and a new server row); deletion simply
/// stops the old uuid from being emitted again.
pub fn cursor_uuid(composer_id: &str, text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    let hex = format!("{:x}", h.finalize());
    format!("{}_{}", composer_id, &hex[..16])
}

/// Pack the marker metadata into PendingMessage's file + line_end fields.
pub(crate) fn encode_marker(composer_id: &str, updated_at_ms: u64, bubble_idx: usize) -> (PathBuf, u64) {
    (
        PathBuf::from(format!("cursor:{}:{}", composer_id, updated_at_ms)),
        bubble_idx as u64,
    )
}

/// Parse composer_id + updated_at_ms + bubble_idx back from a PendingMessage.
pub(crate) fn decode_marker(file: &Path, line_end: u64) -> Option<(String, u64, usize)> {
    let s = file.to_str()?;
    let rest = s.strip_prefix("cursor:")?;
    // composer_id is a UUID with ':' never appearing; split from the right on ':'
    let colon = rest.rfind(':')?;
    let composer = &rest[..colon];
    let updated_at: u64 = rest[colon + 1..].parse().ok()?;
    Some((composer.to_string(), updated_at, line_end as usize))
}

/// Walk bubbles preceding `idx` looking for a non-empty model_name (user-side
/// bubbles can lack model; reuse the last observed model from prior bubbles).
fn backfill_model(bubbles: &[CursorBubble], idx: usize) -> Option<String> {
    // self first
    if let Some(name) = bubbles.get(idx).and_then(|b| b.model_name.as_deref()).filter(|s| !s.is_empty()) {
        return Some(name.to_string());
    }
    // look backward through earlier bubbles (same composer's recent model)
    for earlier in bubbles[..idx].iter().rev() {
        if let Some(name) = earlier.model_name.as_deref().filter(|s| !s.is_empty()) {
            return Some(name.to_string());
        }
    }
    None
}

/// Walk all composer headers + Agent transcripts. CLI sessions are scanned by
/// `scan_all_cli` and reported under the separate `cursor_cli` tool.
pub fn scan_all(state: &ConversationState) -> Vec<PendingMessage> {
    let mut out = scan_composers(state);
    out.extend(scan_transcripts(state));
    out
}

/// CLI-only counterpart of `scan_all`: walks `~/.cursor/chats/*/*/store.db`
/// and emits user prompts past the per-DB rowid watermark in
/// `state.file_offsets`. Reported under tool=`cursor_cli`.
pub fn scan_all_cli(state: &ConversationState) -> Vec<PendingMessage> {
    scan_cli_chats(state)
}

fn scan_composers(state: &ConversationState) -> Vec<PendingMessage> {
    let transcript_ids = crate::cursor::parser::agent_transcripts::collect_transcript_session_ids();
    let headers = read_composer_headers();
    let mut out = Vec::new();

    for header in &headers {
        // Skip sessions whose prompts are already covered by the transcripts
        // path — those will be uploaded via scan_transcripts() with a distinct
        // file-offset watermark and real <user_query> extraction.
        if transcript_ids.contains(&header.composer_id) {
            continue;
        }
        let Some(updated_at_ms) = header.last_updated_at else {
            // Can't decide freshness; skip.
            continue;
        };
        let mark = state.cursor_marks.get(&header.composer_id);
        let last_updated_at = mark.map(|m| m.last_updated_at).unwrap_or(0);
        let mut bubble_index = mark.map(|m| m.bubble_index).unwrap_or(0);

        if updated_at_ms <= last_updated_at {
            // No change since last scan.
            continue;
        }

        let bubbles = read_bubbles(&header.composer_id);

        // Reset if bubble count shrank (user deleted messages).
        if bubbles.len() < bubble_index {
            bubble_index = 0;
        }

        let is_fresh_scan = bubble_index == 0;
        let project = header
            .workspace_path
            .as_deref()
            .and_then(|p| {
                let trimmed = p.trim_end_matches(['/', '\\']);
                trimmed.rsplit(['/', '\\']).next()
            })
            .unwrap_or("unknown")
            .to_string();
        let cwd = header.workspace_path.clone().unwrap_or_default();
        let mut first_emitted = false;

        for (i, bubble) in bubbles.iter().enumerate().skip(bubble_index) {
            if bubble.msg_type != 1 {
                continue;
            }
            let Some(text) = bubble.text.as_ref() else { continue };
            if text.trim().is_empty() {
                continue;
            }
            if is_system_injection(text) {
                continue;
            }

            let timestamp = bubble.created_at.clone().unwrap_or_default();
            if timestamp.is_empty() {
                continue; // server needs a date to bucket
            }

            let model = backfill_model(&bubbles, i);
            let is_first_in_window = !first_emitted;
            let role_tag = classify_role_tag(text, is_first_in_window, is_fresh_scan);
            if role_tag == RoleTag::First {
                first_emitted = true;
            }

            let uuid = cursor_uuid(&header.composer_id, text);
            let (file, line_end) = encode_marker(&header.composer_id, updated_at_ms, i);

            out.push(PendingMessage {
                file,
                line_end,
                message: ConversationMessage {
                    uuid,
                    session_id: header.composer_id.clone(),
                    parent_uuid: None,
                    timestamp,
                    project: project.clone(),
                    cwd: cwd.clone(),
                    git_branch: None,
                    model,
                    role_tag,
                    text: text.clone(),
                },
            });
        }
    }

    out
}

fn scan_transcripts(state: &ConversationState) -> Vec<PendingMessage> {
    use crate::cursor::parser::agent_transcripts as at;
    let files = at::scan_all_transcript_files();
    let mut out = Vec::new();
    for path in files {
        let start = state.offset_for(&path);
        let Ok(meta) = std::fs::metadata(&path) else { continue };
        let size = meta.len();
        if start >= size { continue; }
        let Some(t_meta) = at::extract_transcript_meta(&path) else { continue };

        let is_fresh_scan = start == 0;
        let timestamp = crate::cursor::parser::project_scanner::epoch_ms_to_rfc3339(t_meta.file_mtime_ms);
        let project = t_meta.workspace_encoded.clone();
        let cwd = format!("~/.cursor/projects/{}", t_meta.workspace_encoded);

        let Ok(messages) = at::scan_one_transcript(&path, start, size) else { continue };
        let mut first_emitted = false;
        for m in messages {
            let uuid = format!("{}_{}", t_meta.session_id, m.line_start);
            let is_first_in_window = !first_emitted;
            let role_tag = classify_role_tag(&m.text, is_first_in_window, is_fresh_scan);
            if role_tag == RoleTag::First { first_emitted = true; }

            out.push(PendingMessage {
                file: path.clone(), // real path → handled by advance_state
                line_end: m.line_end,
                message: ConversationMessage {
                    uuid,
                    session_id: t_meta.session_id.clone(),
                    parent_uuid: None,
                    timestamp: timestamp.clone(),
                    project: project.clone(),
                    cwd: cwd.clone(),
                    git_branch: None,
                    model: None,
                    role_tag,
                    text: m.text,
                },
            });
        }
    }
    out
}

fn scan_cli_chats(state: &ConversationState) -> Vec<PendingMessage> {
    cli_chats::load_all_sessions()
        .into_iter()
        .flat_map(|session| {
            let start = state.offset_for(&session.db_path);
            pending_from_cli_session(&session, start)
        })
        .collect()
}

pub(crate) fn pending_from_cli_session(
    session: &CliSessionData,
    start_rowid: u64,
) -> Vec<PendingMessage> {
    let prompts = session.user_prompt_rows_after(start_rowid as i64);
    if prompts.is_empty() {
        return Vec::new();
    }

    let timestamp = cli_chats::epoch_ms_to_rfc3339(session.file_mtime_ms);
    if timestamp.is_empty() {
        return Vec::new();
    }

    let is_fresh_scan = start_rowid == 0;
    let cwd = session.cwd();
    let project = crate::shared_models::basename(&cwd);
    let mut first_emitted = false;

    prompts
        .into_iter()
        .map(|(rowid, text)| {
            let is_first_in_window = !first_emitted;
            let role_tag = classify_role_tag(&text, is_first_in_window, is_fresh_scan);
            if role_tag == RoleTag::First {
                first_emitted = true;
            }
            PendingMessage {
                file: session.db_path.clone(),
                line_end: rowid as u64,
                message: ConversationMessage {
                    uuid: cursor_uuid(&session.session_id, &format!("{}:{}", rowid, text)),
                    session_id: session.session_id.clone(),
                    parent_uuid: None,
                    timestamp: timestamp.clone(),
                    project: project.clone(),
                    cwd: cwd.clone(),
                    git_branch: None,
                    model: None,
                    role_tag,
                    text,
                },
            }
        })
        .collect()
}

/// Post-upload: advance state.cursor_marks based on what was in the batch.
pub fn advance_marks(marks: &mut HashMap<String, CursorMark>, batch: &[PendingMessage]) {
    // composer_id -> (max updated_at observed, max bubble_idx observed)
    let mut per: HashMap<String, (u64, usize)> = HashMap::new();
    for m in batch {
        let Some((composer, updated_at, bubble_idx)) = decode_marker(&m.file, m.line_end) else {
            continue;
        };
        let e = per.entry(composer).or_insert((0, 0));
        if updated_at > e.0 {
            e.0 = updated_at;
        }
        if bubble_idx > e.1 {
            e.1 = bubble_idx;
        }
    }
    for (composer, (updated_at, max_idx)) in per {
        let entry = marks.entry(composer).or_default();
        if updated_at > entry.last_updated_at {
            entry.last_updated_at = updated_at;
        }
        // bubble_index is the *next* index to scan.
        let next_idx = max_idx + 1;
        if next_idx > entry.bubble_index {
            entry.bubble_index = next_idx;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cursor_uuid_is_stable() {
        let a = cursor_uuid("session-x", "hello");
        let b = cursor_uuid("session-x", "hello");
        assert_eq!(a, b);
        assert!(a.starts_with("session-x_"));
        assert_eq!(a.len(), "session-x_".len() + 16);
    }

    #[test]
    fn cursor_uuid_differs_on_text() {
        let a = cursor_uuid("s", "hello");
        let b = cursor_uuid("s", "hello!");
        assert_ne!(a, b);
    }

    #[test]
    fn marker_roundtrip() {
        let (file, line_end) = encode_marker("comp-abc", 1700000000123, 42);
        let (composer, updated_at, idx) = decode_marker(&file, line_end).unwrap();
        assert_eq!(composer, "comp-abc");
        assert_eq!(updated_at, 1700000000123);
        assert_eq!(idx, 42);
    }

    #[test]
    fn marker_composer_id_with_dashes_roundtrips() {
        let (file, line_end) = encode_marker("abc-def-123-xyz", 42, 7);
        let (composer, updated_at, idx) = decode_marker(&file, line_end).unwrap();
        assert_eq!(composer, "abc-def-123-xyz");
        assert_eq!(updated_at, 42);
        assert_eq!(idx, 7);
    }

    #[test]
    fn decode_marker_rejects_non_cursor_paths() {
        let p = PathBuf::from("/Users/bin/.claude/foo.jsonl");
        assert!(decode_marker(&p, 100).is_none());
    }

    #[test]
    fn advance_marks_takes_max_per_composer() {
        let mut marks: HashMap<String, CursorMark> = HashMap::new();
        let (f1, l1) = encode_marker("a", 100, 0);
        let (f2, l2) = encode_marker("a", 100, 3);
        let (f3, l3) = encode_marker("a", 200, 1);
        let (f4, l4) = encode_marker("b", 50, 5);

        let mk_pending = |file: PathBuf, line_end: u64| PendingMessage {
            file,
            line_end,
            message: ConversationMessage {
                uuid: "x".into(), session_id: "s".into(), parent_uuid: None,
                timestamp: "".into(), project: "p".into(), cwd: "/".into(),
                git_branch: None, model: None, role_tag: RoleTag::Followup, text: "".into(),
            },
        };

        let batch = vec![
            mk_pending(f1, l1),
            mk_pending(f2, l2),
            mk_pending(f3, l3),
            mk_pending(f4, l4),
        ];
        advance_marks(&mut marks, &batch);

        let a = marks.get("a").unwrap();
        assert_eq!(a.last_updated_at, 200);
        assert_eq!(a.bubble_index, 4); // max idx 3 → next = 4
        let b = marks.get("b").unwrap();
        assert_eq!(b.last_updated_at, 50);
        assert_eq!(b.bubble_index, 6); // max idx 5 → next = 6
    }

    #[test]
    fn advance_marks_never_goes_backward() {
        let mut marks: HashMap<String, CursorMark> = HashMap::new();
        marks.insert("a".into(), CursorMark { last_updated_at: 500, bubble_index: 10 });

        let (f, l) = encode_marker("a", 300, 5);
        let batch = vec![PendingMessage {
            file: f, line_end: l,
            message: ConversationMessage {
                uuid: "x".into(), session_id: "a".into(), parent_uuid: None,
                timestamp: "".into(), project: "p".into(), cwd: "/".into(),
                git_branch: None, model: None, role_tag: RoleTag::Followup, text: "".into(),
            },
        }];
        advance_marks(&mut marks, &batch);

        let a = marks.get("a").unwrap();
        assert_eq!(a.last_updated_at, 500);  // old wins
        assert_eq!(a.bubble_index, 10);       // old wins
    }

    #[test]
    fn backfill_model_uses_self_when_set() {
        let bubbles = vec![CursorBubble {
            msg_type: 1,
            text: Some("hi".into()),
            created_at: None,
            token_count: None,
            model_name: Some("claude-opus".into()),
        }];
        assert_eq!(backfill_model(&bubbles, 0).as_deref(), Some("claude-opus"));
    }

    #[test]
    fn backfill_model_walks_earlier_bubbles_when_missing() {
        let bubbles = vec![
            CursorBubble { msg_type: 2, text: None, created_at: None, token_count: None, model_name: Some("gpt-5".into()) },
            CursorBubble { msg_type: 1, text: Some("x".into()), created_at: None, token_count: None, model_name: None },
        ];
        assert_eq!(backfill_model(&bubbles, 1).as_deref(), Some("gpt-5"));
    }

    #[test]
    fn backfill_model_returns_none_when_no_history() {
        let bubbles = vec![CursorBubble {
            msg_type: 1, text: Some("x".into()), created_at: None, token_count: None, model_name: None,
        }];
        assert_eq!(backfill_model(&bubbles, 0), None);
    }

    #[test]
    fn cli_session_pending_messages_use_db_offsets() {
        let session = CliSessionData {
            meta: cli_chats::CliMeta {
                agent_id: "session-1".into(),
                name: Some("CLI".into()),
                mode: Some("default".into()),
                created_at: Some(1_700_000_000_000),
            },
            project_hash: "project-hash".into(),
            session_id: "session-1".into(),
            db_path: PathBuf::from("/tmp/store.db"),
            workspace_path: Some("/Users/me/project".into()),
            rows: vec![
                cli_chats::CliBlobRow {
                    rowid: 1,
                    value: json!({"role":"user","content":"<user_info>\nWorkspace Path: /Users/me/project\n</user_info>"}),
                },
                cli_chats::CliBlobRow {
                    rowid: 2,
                    value: json!({"role":"user","content":"<user_query>\nfirst\n</user_query>"}),
                },
                cli_chats::CliBlobRow {
                    rowid: 4,
                    value: json!({"role":"user","content":"<user_query>\nsecond\n</user_query>"}),
                },
            ],
            file_mtime_ms: 1_700_000_000_000,
        };

        let pending = pending_from_cli_session(&session, 2);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].line_end, 4);
        assert_eq!(pending[0].message.text, "second");
        assert_eq!(pending[0].message.project, "project");
        assert_eq!(pending[0].message.cwd, "/Users/me/project");
    }
}
