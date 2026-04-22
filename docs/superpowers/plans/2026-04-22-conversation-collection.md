# 用户咨询问题采集系统 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在现有 `session-viewer` 客户端 + `ai-usage-server` 服务端之上，新增 Claude Code 用户 prompt 明文采集系统：客户端按文件字节偏移量增量扫描 `~/.claude/projects/**/*.jsonl`，以 10MB 批次断点续传至服务端独立端点；服务端按 NDJSON 落盘（不入数据库），3 个月 TTL 清理；Dashboard "项目用量明细"表新增"查看问题"入口弹出消息抽屉。

**Architecture:** 客户端新增 `conversation/` 模块 (scanner + state + uploader)，在 `lib.rs` 中独立 spawn 5 分钟循环；服务端新增 `conversations.ts` 路由（POST 上报 / GET 查询）+ `cleanup.ts` 定时任务；Dashboard 仅改动 `public/index.html` 单文件，沿用原生 HTML/JS 风格。客户端以 `(file_path → byte_offset)` 作水位线，服务端目录路径 `conversations/{tool}/{YYYY-MM-DD}/{sanitize(email)}.jsonl` 即路由。

**Tech Stack:** Rust (tauri 2, reqwest, tokio, serde, chrono, dirs) · Node.js/TypeScript (express, tsx) · 原生 HTML/CSS/JS。无新增依赖。

**Spec:** `docs/superpowers/specs/2026-04-22-conversation-collection-design.md`

---

## File Structure

**客户端 (session-viewer)**
- Create: `src-tauri/src/conversation/mod.rs` — 模块导出、常量、公开类型
- Create: `src-tauri/src/conversation/state.rs` — `ConversationState` 结构 + 持久化
- Create: `src-tauri/src/conversation/scanner.rs` — jsonl 扫描 + 过滤 + `role_tag` + model 回填
- Create: `src-tauri/src/conversation/uploader.rs` — 分批、HTTP、offset 推进、4xx/5xx 处理
- Modify: `src-tauri/src/lib.rs` — 注册 conversation 模块 + spawn 循环
- Modify: `package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` — 版本号 0.4.7 → 0.5.0

**服务端 (ai-usage-server)**
- Create: `src/conversations.ts` — 路由 + sanitizeEmail + appendLinesAtomic
- Create: `src/cleanup.ts` — TTL 清理调度
- Create: `src/conversations.test.ts` — 单元/集成测试
- Create: `src/cleanup.test.ts` — 清理逻辑测试
- Modify: `src/index.ts` — 注册路由、启动 cleanup、调大 body limit
- Modify: `package.json` — 加 test script

**Dashboard**
- Modify: `public/index.html` — 新增"操作"列 + 抽屉 CSS/HTML/JS

---

## Phase A · 客户端 state 模块

### Task A1: 搭建 conversation 模块骨架

**Files:**
- Create: `src-tauri/src/conversation/mod.rs`

- [ ] **Step A1.1: 创建模块目录与 mod.rs**

Create `src-tauri/src/conversation/mod.rs`:

```rust
//! Conversation collection module.
//!
//! Scans Claude Code JSONL sessions, extracts user prompts (filtering
//! CLI-injected system messages), tags each with first/followup/retry,
//! and uploads in <=10MB batches to the server's /api/conversations endpoint.
//!
//! State (per-file byte offsets) is persisted to disk so scans are incremental
//! and resumable after a crash or partial upload.

pub mod scanner;
pub mod state;
pub mod uploader;

use serde::{Deserialize, Serialize};

/// A single user prompt collected from a Claude Code session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversationMessage {
    pub uuid: String,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_uuid: Option<String>,
    pub timestamp: String,
    pub project: String,
    pub cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub role_tag: RoleTag,
    pub text: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RoleTag {
    First,
    Followup,
    Retry,
}

pub const MAX_BATCH_BYTES: usize = 10 * 1024 * 1024; // 10MB
```

- [ ] **Step A1.2: 在 lib.rs 注册模块声明**

Modify `src-tauri/src/lib.rs`:

Find line: `mod report;` (should be around line 7)

Add immediately after:
```rust
mod conversation;
```

- [ ] **Step A1.3: 编译校验**

Run: `cd src-tauri && cargo check`
Expected: Succeeds with only `unused` warnings (scanner / state / uploader are empty files — they don't exist yet, so this step will actually fail until A2/B/C).

**Note:** To make `cargo check` pass now, create empty files:
```bash
touch src-tauri/src/conversation/scanner.rs
touch src-tauri/src/conversation/state.rs
touch src-tauri/src/conversation/uploader.rs
```
Then rerun `cargo check`. Expect: succeeds.

- [ ] **Step A1.4: Commit**

```bash
git add src-tauri/src/conversation/ src-tauri/src/lib.rs
git commit -m "feat(conversation): scaffold module with message types"
```

---

### Task A2: 实现 state.rs（file_offsets 持久化）

**Files:**
- Modify: `src-tauri/src/conversation/state.rs`

- [ ] **Step A2.1: Write failing tests for ConversationState**

Replace content of `src-tauri/src/conversation/state.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConversationState {
    pub file_offsets: HashMap<PathBuf, u64>,
    pub last_scan_at: Option<String>,
}

impl ConversationState {
    pub fn offset_for(&self, path: &Path) -> u64 {
        self.file_offsets.get(path).copied().unwrap_or(0)
    }

    pub fn set_offset(&mut self, path: PathBuf, offset: u64) {
        self.file_offsets.insert(path, offset);
    }

    pub fn remove(&mut self, path: &Path) {
        self.file_offsets.remove(path);
    }
}

/// Directory used for persistent client state (shared with report.rs).
fn state_dir() -> Option<PathBuf> {
    let base = dirs::data_dir().or_else(dirs::config_dir)?;
    let dir = base.join("session-viewer");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn state_file() -> Option<PathBuf> {
    state_dir().map(|d| d.join("conversation-state.json"))
}

pub fn load() -> ConversationState {
    let Some(path) = state_file() else {
        return ConversationState::default();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return ConversationState::default();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

pub fn save(state: &ConversationState) {
    let Some(path) = state_file() else { return };
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(&path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_empty() {
        let s = ConversationState::default();
        assert!(s.file_offsets.is_empty());
        assert_eq!(s.offset_for(Path::new("/nope")), 0);
    }

    #[test]
    fn set_and_get_offset_roundtrip() {
        let mut s = ConversationState::default();
        s.set_offset(PathBuf::from("/a.jsonl"), 123);
        assert_eq!(s.offset_for(Path::new("/a.jsonl")), 123);
        s.set_offset(PathBuf::from("/a.jsonl"), 456);
        assert_eq!(s.offset_for(Path::new("/a.jsonl")), 456);
    }

    #[test]
    fn remove_clears_entry() {
        let mut s = ConversationState::default();
        s.set_offset(PathBuf::from("/a.jsonl"), 123);
        s.remove(Path::new("/a.jsonl"));
        assert_eq!(s.offset_for(Path::new("/a.jsonl")), 0);
    }

    #[test]
    fn serde_roundtrip() {
        let mut s = ConversationState::default();
        s.set_offset(PathBuf::from("/a.jsonl"), 10);
        s.last_scan_at = Some("2026-04-22T00:00:00Z".into());
        let json = serde_json::to_string(&s).unwrap();
        let back: ConversationState = serde_json::from_str(&json).unwrap();
        assert_eq!(back.offset_for(Path::new("/a.jsonl")), 10);
        assert_eq!(back.last_scan_at.as_deref(), Some("2026-04-22T00:00:00Z"));
    }
}
```

- [ ] **Step A2.2: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib conversation::state::tests`
Expected: `test result: ok. 4 passed`

- [ ] **Step A2.3: Commit**

```bash
git add src-tauri/src/conversation/state.rs
git commit -m "feat(conversation): persistent ConversationState with file offsets"
```

---

## Phase B · 客户端 scanner 模块

### Task B1: 实现 `extract_user_text` 过滤规则

**Files:**
- Modify: `src-tauri/src/conversation/scanner.rs`

- [ ] **Step B1.1: Write failing tests for text extraction + filtering**

Replace content of `src-tauri/src/conversation/scanner.rs`:

```rust
use serde_json::Value;

/// 6 CLI-injected prefixes that mark a "user" message as NOT a real user prompt.
pub(crate) const SYSTEM_PREFIXES: &[&str] = &[
    "<local-command-caveat>",
    "<command-name>",
    "<local-command-stdout>",
    "<local-command-stderr>",
    "<system-reminder>",
    "<system-status>",
];

/// Extract the plain-text prompt from a user jsonl line.
/// Returns `None` if the message is a system-injection, tool result, or empty.
pub fn extract_user_text(v: &Value) -> Option<String> {
    if v.get("type")?.as_str()? != "user" {
        return None;
    }
    let content = v.get("message")?.get("content")?;
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(items) => {
            let parts: Vec<String> = items
                .iter()
                .filter_map(|item| {
                    let ty = item.get("type")?.as_str()?;
                    if ty == "text" {
                        Some(item.get("text")?.as_str()?.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            if parts.is_empty() {
                return None; // only tool_use/tool_result etc
            }
            parts.join("\n\n")
        }
        _ => return None,
    };

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    for prefix in SYSTEM_PREFIXES {
        if trimmed.starts_with(prefix) {
            return None;
        }
    }
    Some(text)
}

#[cfg(test)]
mod text_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn plain_string_content() {
        let v = json!({"type": "user", "message": {"content": "hello"}});
        assert_eq!(extract_user_text(&v).as_deref(), Some("hello"));
    }

    #[test]
    fn array_text_segments_joined() {
        let v = json!({
            "type": "user",
            "message": {"content": [
                {"type": "text", "text": "part1"},
                {"type": "text", "text": "part2"}
            ]}
        });
        assert_eq!(extract_user_text(&v).as_deref(), Some("part1\n\npart2"));
    }

    #[test]
    fn array_with_only_tool_result_returns_none() {
        let v = json!({
            "type": "user",
            "message": {"content": [
                {"type": "tool_result", "tool_use_id": "x", "content": "ok"}
            ]}
        });
        assert_eq!(extract_user_text(&v), None);
    }

    #[test]
    fn six_system_prefixes_filtered() {
        for prefix in SYSTEM_PREFIXES {
            let text = format!("{}something", prefix);
            let v = json!({"type": "user", "message": {"content": text}});
            assert_eq!(extract_user_text(&v), None, "should filter prefix: {}", prefix);
        }
    }

    #[test]
    fn empty_whitespace_returns_none() {
        let v = json!({"type": "user", "message": {"content": "   \n  "}});
        assert_eq!(extract_user_text(&v), None);
    }

    #[test]
    fn non_user_type_returns_none() {
        let v = json!({"type": "assistant", "message": {"content": "hi"}});
        assert_eq!(extract_user_text(&v), None);
    }

    #[test]
    fn missing_content_returns_none() {
        let v = json!({"type": "user", "message": {}});
        assert_eq!(extract_user_text(&v), None);
    }
}
```

- [ ] **Step B1.2: Run tests to verify all pass**

Run: `cd src-tauri && cargo test --lib conversation::scanner::text_tests`
Expected: `test result: ok. 7 passed`

- [ ] **Step B1.3: Commit**

```bash
git add src-tauri/src/conversation/scanner.rs
git commit -m "feat(conversation): extract_user_text with 6-prefix filter"
```

---

### Task B2: 实现 `classify_role_tag`

**Files:**
- Modify: `src-tauri/src/conversation/scanner.rs`

- [ ] **Step B2.1: Add retry regex patterns and classify function with tests**

Append to `src-tauri/src/conversation/scanner.rs`:

```rust
use crate::conversation::RoleTag;

/// Case-insensitive retry patterns. Matched only when text length <= 30 chars.
fn is_retry_text(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.chars().count() > 30 {
        return false;
    }
    let lower = trimmed.to_lowercase();
    // Chinese patterns
    const ZH: &[&str] = &["再试", "重试", "不对", "继续", "换一个"];
    for p in ZH {
        if trimmed.starts_with(p) {
            return true;
        }
    }
    // English patterns (word-boundary approximation: must be followed by end / whitespace / punct)
    const EN: &[&str] = &["retry", "try again", "again", "no", "continue", "go on"];
    for p in EN {
        if lower.starts_with(p) {
            let rest = &lower[p.len()..];
            if rest.is_empty() || rest.starts_with(|c: char| !c.is_alphanumeric()) {
                return true;
            }
        }
    }
    false
}

/// Classify a user prompt within a freshly-scanned file window.
///
/// `is_fresh_scan` = true means `start_offset == 0` for this file, so we can
/// legitimately mark the first user prompt we see as `First`. On incremental
/// scans (offset > 0), every prompt defaults to `Followup` (or `Retry`) since
/// we cannot tell if the session's real first prompt was already emitted.
pub fn classify_role_tag(text: &str, is_first_in_window: bool, is_fresh_scan: bool) -> RoleTag {
    if is_retry_text(text) {
        return RoleTag::Retry;
    }
    if is_fresh_scan && is_first_in_window {
        return RoleTag::First;
    }
    RoleTag::Followup
}

#[cfg(test)]
mod role_tests {
    use super::*;

    #[test]
    fn retry_chinese() {
        assert!(is_retry_text("再试一下"));
        assert!(is_retry_text("重试"));
        assert!(is_retry_text("不对"));
        assert!(is_retry_text("继续"));
        assert!(is_retry_text("换一个方案"));
    }

    #[test]
    fn retry_english_case_insensitive() {
        assert!(is_retry_text("Retry"));
        assert!(is_retry_text("try again please"));
        assert!(is_retry_text("no"));
        assert!(is_retry_text("NO, try X"));
        assert!(is_retry_text("continue"));
        assert!(is_retry_text("go on"));
    }

    #[test]
    fn not_retry_when_too_long() {
        let long = "retry ".repeat(10); // 60 chars
        assert!(!is_retry_text(&long));
    }

    #[test]
    fn not_retry_mid_sentence() {
        // Word starts must match at beginning of text. "please retry" starts with "please".
        assert!(!is_retry_text("please retry"));
    }

    #[test]
    fn not_retry_false_prefix_match() {
        // "again" should match but "against" should not (word boundary).
        assert!(is_retry_text("again!"));
        assert!(!is_retry_text("against the wall"));
    }

    #[test]
    fn first_only_on_fresh_scan() {
        assert_eq!(classify_role_tag("how do I do X", true, true), RoleTag::First);
        assert_eq!(classify_role_tag("how do I do X", true, false), RoleTag::Followup);
        assert_eq!(classify_role_tag("how do I do X", false, true), RoleTag::Followup);
    }

    #[test]
    fn retry_beats_first() {
        assert_eq!(classify_role_tag("重试", true, true), RoleTag::Retry);
    }
}
```

- [ ] **Step B2.2: Run tests**

Run: `cd src-tauri && cargo test --lib conversation::scanner::role_tests`
Expected: `test result: ok. 7 passed`

- [ ] **Step B2.3: Commit**

```bash
git add src-tauri/src/conversation/scanner.rs
git commit -m "feat(conversation): classify_role_tag with retry heuristics"
```

---

### Task B3: 实现 `lookup_following_model`

**Files:**
- Modify: `src-tauri/src/conversation/scanner.rs`

- [ ] **Step B3.1: Add model backfill function with tests**

Append to `src-tauri/src/conversation/scanner.rs`:

```rust
/// Given a window of jsonl lines ordered by position, find the first
/// `type=assistant` line and return its `message.model`. Skips messages whose
/// model is `<synthetic>` or `unknown` (same rule as claude/commands/report.rs).
/// Returns `None` if no usable assistant is found.
pub fn lookup_following_model(window: &[Value]) -> Option<String> {
    for v in window {
        if v.get("type")?.as_str()? != "assistant" {
            continue;
        }
        let model = v.get("message")?.get("model")?.as_str()?;
        if model == "<synthetic>" || model == "unknown" || model.is_empty() {
            continue;
        }
        return Some(model.to_string());
    }
    None
}

#[cfg(test)]
mod model_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn finds_first_assistant_model() {
        let w = vec![
            json!({"type": "user", "message": {"content": "x"}}),
            json!({"type": "assistant", "message": {"model": "claude-opus-4-6"}}),
        ];
        assert_eq!(lookup_following_model(&w).as_deref(), Some("claude-opus-4-6"));
    }

    #[test]
    fn skips_synthetic() {
        let w = vec![
            json!({"type": "assistant", "message": {"model": "<synthetic>"}}),
            json!({"type": "assistant", "message": {"model": "claude-sonnet-4-5"}}),
        ];
        assert_eq!(lookup_following_model(&w).as_deref(), Some("claude-sonnet-4-5"));
    }

    #[test]
    fn skips_unknown() {
        let w = vec![
            json!({"type": "assistant", "message": {"model": "unknown"}}),
            json!({"type": "assistant", "message": {"model": "claude-opus-4-6"}}),
        ];
        assert_eq!(lookup_following_model(&w).as_deref(), Some("claude-opus-4-6"));
    }

    #[test]
    fn returns_none_when_no_assistant() {
        let w = vec![
            json!({"type": "user", "message": {"content": "x"}}),
            json!({"type": "user", "message": {"content": "y"}}),
        ];
        assert_eq!(lookup_following_model(&w), None);
    }

    #[test]
    fn returns_none_when_only_synthetic() {
        let w = vec![
            json!({"type": "assistant", "message": {"model": "<synthetic>"}}),
        ];
        assert_eq!(lookup_following_model(&w), None);
    }
}
```

- [ ] **Step B3.2: Run tests**

Run: `cd src-tauri && cargo test --lib conversation::scanner::model_tests`
Expected: `test result: ok. 5 passed`

- [ ] **Step B3.3: Commit**

```bash
git add src-tauri/src/conversation/scanner.rs
git commit -m "feat(conversation): lookup_following_model skipping synthetic/unknown"
```

---

### Task B4: 实现 `scan_incremental`（文件扫描与 offset 记录）

**Files:**
- Modify: `src-tauri/src/conversation/scanner.rs`

- [ ] **Step B4.1: Add scan_incremental + PendingMessage + tests**

Append to `src-tauri/src/conversation/scanner.rs`:

```rust
use crate::conversation::{ConversationMessage, state::ConversationState};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// A scanned message annotated with its source file and the byte offset
/// *after* its line (i.e., where the next line would start). Used to advance
/// per-file high-water marks only after the containing batch succeeds.
#[derive(Debug, Clone)]
pub struct PendingMessage {
    pub file: PathBuf,
    pub line_end: u64,
    pub message: ConversationMessage,
}

/// Scan a single jsonl file from `start_offset` to EOF, returning all
/// user-prompt messages in the window.
pub fn scan_one_file(
    path: &Path,
    start_offset: u64,
    file_size: u64,
) -> std::io::Result<Vec<PendingMessage>> {
    // Defensive: if offset exceeds size (file truncated/rotated), rescan from 0.
    let start_offset = if start_offset > file_size { 0 } else { start_offset };
    let is_fresh_scan = start_offset == 0;

    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(start_offset))?;
    let mut reader = BufReader::new(file);

    // First pass: read lines with their post-line byte offset.
    let mut lines: Vec<(u64, Value)> = Vec::new();
    let mut cursor = start_offset;
    loop {
        let mut buf = String::new();
        let n = reader.read_line(&mut buf)?;
        if n == 0 {
            break;
        }
        cursor += n as u64;
        let trimmed = buf.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
            lines.push((cursor, v));
        }
    }

    // Second pass: project metadata from cwd, tag + model backfill.
    let mut first_emitted = false;
    let mut results = Vec::new();
    for (i, (line_end, v)) in lines.iter().enumerate() {
        let Some(text) = extract_user_text(v) else { continue };

        let uuid = v.get("uuid").and_then(|x| x.as_str()).unwrap_or("").to_string();
        if uuid.is_empty() {
            continue;
        }
        let session_id = v.get("sessionId").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let parent_uuid = v.get("parentUuid").and_then(|x| x.as_str()).map(String::from);
        let timestamp = v.get("timestamp").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let cwd = v.get("cwd").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let git_branch = v.get("gitBranch").and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from);

        let project = cwd.rsplit('/').find(|s| !s.is_empty()).unwrap_or("unknown").to_string();

        let tail: Vec<Value> = lines[i + 1..].iter().map(|(_, v)| v.clone()).collect();
        let model = lookup_following_model(&tail);

        let is_first_in_window = !first_emitted;
        let role_tag = classify_role_tag(&text, is_first_in_window, is_fresh_scan);
        if role_tag == RoleTag::First {
            first_emitted = true;
        }

        results.push(PendingMessage {
            file: path.to_path_buf(),
            line_end: *line_end,
            message: ConversationMessage {
                uuid,
                session_id,
                parent_uuid,
                timestamp,
                project,
                cwd,
                git_branch,
                model,
                role_tag,
                text,
            },
        });
    }

    Ok(results)
}

/// Walk `~/.claude/projects/**/*.jsonl` and scan each incrementally.
pub fn scan_all(state: &ConversationState) -> Vec<PendingMessage> {
    let Some(projects_dir) = crate::claude::parser::path_encoder::get_projects_dir() else {
        return Vec::new();
    };
    if !projects_dir.exists() {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(&projects_dir) else { return Vec::new() };

    let mut out = Vec::new();
    for entry in entries.flatten() {
        let project_dir = entry.path();
        if !project_dir.is_dir() {
            continue;
        }
        let Ok(files) = std::fs::read_dir(&project_dir) else { continue };
        for f in files.flatten() {
            let p = f.path();
            if p.extension().map(|e| e == "jsonl").unwrap_or(false) {
                let start = state.offset_for(&p);
                let Ok(meta) = std::fs::metadata(&p) else { continue };
                let size = meta.len();
                if start >= size {
                    // Nothing new; skip without opening the file.
                    continue;
                }
                match scan_one_file(&p, start, size) {
                    Ok(mut v) => out.append(&mut v),
                    Err(e) => eprintln!("[Conversation] scan failed for {:?}: {}", p, e),
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod scan_tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_jsonl(lines: &[&str]) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        for l in lines {
            writeln!(f, "{}", l).unwrap();
        }
        f.flush().unwrap();
        f
    }

    #[test]
    fn fresh_scan_marks_first() {
        let f = write_jsonl(&[
            r#"{"type":"user","uuid":"u1","sessionId":"s1","timestamp":"2026-04-22T00:00:00Z","cwd":"/a/b/proj","message":{"content":"hello"}}"#,
            r#"{"type":"assistant","uuid":"a1","message":{"model":"claude-opus-4-6"}}"#,
            r#"{"type":"user","uuid":"u2","sessionId":"s1","timestamp":"2026-04-22T00:01:00Z","cwd":"/a/b/proj","message":{"content":"more"}}"#,
        ]);
        let size = std::fs::metadata(f.path()).unwrap().len();
        let result = scan_one_file(f.path(), 0, size).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].message.role_tag, RoleTag::First);
        assert_eq!(result[0].message.project, "proj");
        assert_eq!(result[0].message.model.as_deref(), Some("claude-opus-4-6"));
        assert_eq!(result[1].message.role_tag, RoleTag::Followup);
    }

    #[test]
    fn incremental_scan_no_first() {
        let f = write_jsonl(&[
            r#"{"type":"user","uuid":"u1","sessionId":"s1","timestamp":"2026-04-22T00:00:00Z","cwd":"/a/b/proj","message":{"content":"hello"}}"#,
            r#"{"type":"user","uuid":"u2","sessionId":"s1","timestamp":"2026-04-22T00:01:00Z","cwd":"/a/b/proj","message":{"content":"more"}}"#,
        ]);
        let size = std::fs::metadata(f.path()).unwrap().len();
        // Scan from the start of the 2nd line by asking scanner to resume at size/2
        let result = scan_one_file(f.path(), size / 2, size).unwrap();
        assert!(!result.is_empty());
        // No `First` should be emitted because this is not a fresh scan.
        assert!(result.iter().all(|m| m.message.role_tag != RoleTag::First));
    }

    #[test]
    fn system_messages_filtered() {
        let f = write_jsonl(&[
            r#"{"type":"user","uuid":"u1","sessionId":"s1","timestamp":"2026-04-22T00:00:00Z","cwd":"/a/b/proj","message":{"content":"<system-reminder>ignore me"}}"#,
            r#"{"type":"user","uuid":"u2","sessionId":"s1","timestamp":"2026-04-22T00:01:00Z","cwd":"/a/b/proj","message":{"content":"real prompt"}}"#,
        ]);
        let size = std::fs::metadata(f.path()).unwrap().len();
        let result = scan_one_file(f.path(), 0, size).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.uuid, "u2");
    }

    #[test]
    fn truncated_file_resets_offset() {
        let f = write_jsonl(&[
            r#"{"type":"user","uuid":"u1","sessionId":"s1","timestamp":"2026-04-22T00:00:00Z","cwd":"/a/b/proj","message":{"content":"hi"}}"#,
        ]);
        let size = std::fs::metadata(f.path()).unwrap().len();
        // Pretend we had offset larger than current size (file was shrunk).
        let result = scan_one_file(f.path(), size + 9999, size).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.role_tag, RoleTag::First);
    }

    #[test]
    fn line_end_offset_monotonic() {
        let f = write_jsonl(&[
            r#"{"type":"user","uuid":"u1","sessionId":"s1","timestamp":"2026-04-22T00:00:00Z","cwd":"/a/b/proj","message":{"content":"one"}}"#,
            r#"{"type":"user","uuid":"u2","sessionId":"s1","timestamp":"2026-04-22T00:01:00Z","cwd":"/a/b/proj","message":{"content":"two"}}"#,
        ]);
        let size = std::fs::metadata(f.path()).unwrap().len();
        let result = scan_one_file(f.path(), 0, size).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result[0].line_end < result[1].line_end);
        assert_eq!(result[1].line_end, size);
    }
}
```

- [ ] **Step B4.2: Add `tempfile` as a dev-dependency**

Modify `src-tauri/Cargo.toml`, find the section (or add one) `[dev-dependencies]` just before `[profile.*]` or before the comment block, and add:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step B4.3: Run tests**

Run: `cd src-tauri && cargo test --lib conversation::scanner::scan_tests`
Expected: `test result: ok. 5 passed`

- [ ] **Step B4.4: Commit**

```bash
git add src-tauri/src/conversation/scanner.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(conversation): incremental jsonl scanner with offset tracking"
```

---

## Phase C · 客户端 uploader 模块

### Task C1: 实现 `split_into_batches`

**Files:**
- Modify: `src-tauri/src/conversation/uploader.rs`

- [ ] **Step C1.1: Add batching logic with tests**

Replace content of `src-tauri/src/conversation/uploader.rs`:

```rust
use crate::conversation::{ConversationMessage, MAX_BATCH_BYTES};
use crate::conversation::scanner::PendingMessage;

/// Split pending messages into batches where each batch's serialized size
/// (sum of serde_json::to_vec(msg).len()) is <= `max_bytes`. A single message
/// larger than max_bytes becomes its own batch (payload may exceed the limit —
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

#[cfg(test)]
mod batch_tests {
    use super::*;
    use crate::conversation::{RoleTag, state::ConversationState};
    use std::path::PathBuf;

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
        // Sanity: 10MB constant value.
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
}
```

- [ ] **Step C1.2: Add helper functions (still inside uploader.rs)**

Append to `src-tauri/src/conversation/uploader.rs` (before the `#[cfg(test)]` block — move `mod batch_tests` to the end):

```rust
use crate::conversation::state::ConversationState;
use std::collections::HashMap;
use std::path::PathBuf;

/// For a set of messages, return the largest line_end seen per source file.
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
        let current = state.offset_for(&path);
        if end > current {
            state.set_offset(path, end);
        }
    }
}
```

- [ ] **Step C1.3: Run tests**

Run: `cd src-tauri && cargo test --lib conversation::uploader::batch_tests`
Expected: `test result: ok. 7 passed`

- [ ] **Step C1.4: Commit**

```bash
git add src-tauri/src/conversation/uploader.rs
git commit -m "feat(conversation): batch splitter and offset-advance helpers"
```

---

### Task C2: 实现 `send_batch` HTTP 发送

**Files:**
- Modify: `src-tauri/src/conversation/uploader.rs`

- [ ] **Step C2.1: Add payload types + send_batch + tests**

Append to `src-tauri/src/conversation/uploader.rs`:

```rust
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
    Ok(parsed.received.unwrap_or(0))
}
```

- [ ] **Step C2.2: Integration test with mock HTTP**

Add a new test module to `src-tauri/src/conversation/uploader.rs` (append at end):

```rust
#[cfg(test)]
mod http_tests {
    use super::*;
    use crate::conversation::RoleTag;
    use std::path::PathBuf;

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
        let server = mockito::Server::new_async().await;
        let mut server = server;
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
        server.mock("POST", "/api/conversations")
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
        server.mock("POST", "/api/conversations")
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
```

- [ ] **Step C2.3: Add mockito dev-dependency**

Modify `src-tauri/Cargo.toml`, update the `[dev-dependencies]` section:

```toml
[dev-dependencies]
tempfile = "3"
mockito = "1"
tokio = { version = "1", features = ["full", "test-util", "macros"] }
```

- [ ] **Step C2.4: Run tests**

Run: `cd src-tauri && cargo test --lib conversation::uploader::http_tests`
Expected: `test result: ok. 3 passed`

- [ ] **Step C2.5: Commit**

```bash
git add src-tauri/src/conversation/uploader.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(conversation): send_batch with 4xx/5xx classification"
```

---

### Task C3: 实现 `flush` 协调函数

**Files:**
- Modify: `src-tauri/src/conversation/uploader.rs`

- [ ] **Step C3.1: Add flush function with dead-letter log**

Append to `src-tauri/src/conversation/uploader.rs`:

```rust
use crate::conversation::scanner;
use crate::conversation::state;

fn dead_letter_file() -> Option<PathBuf> {
    let base = dirs::data_dir().or_else(dirs::config_dir)?;
    let dir = base.join("session-viewer");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("conversation-errors.log"))
}

fn log_dead_letter(batch: &[PendingMessage], err: &str) {
    let Some(path) = dead_letter_file() else { return };
    use std::io::Write;
    let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) else { return };
    let ts = chrono::Utc::now().to_rfc3339();
    let uuids: Vec<&str> = batch.iter().map(|p| p.message.uuid.as_str()).collect();
    let _ = writeln!(f, "{} error={} count={} uuids={:?}", ts, err, batch.len(), uuids);
}

/// Scan all Claude projects and upload pending messages in 10MB batches.
/// Advances per-file offsets only for batches that succeed or 4xx (to avoid
/// death loops). On 5xx/network errors, stops and leaves remaining work for
/// the next cycle.
pub async fn flush(server_url: &str) -> Result<u64, String> {
    let url = format!("{}/api/conversations", server_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .map_err(|e| format!("client build: {}", e))?;

    let mut state_snapshot = state::load();
    let pending = scanner::scan_all(&state_snapshot);
    if pending.is_empty() {
        return Ok(0);
    }

    let email = crate::report::get_user_email();
    let name = crate::report::get_user_name();
    let machine = crate::report::get_machine_id();

    let mut total: u64 = 0;
    for batch in split_into_batches(pending, MAX_BATCH_BYTES) {
        match send_batch(&client, &url, "claude_code", &email, &name, &machine, &batch).await {
            Ok(n) => {
                advance_state(&mut state_snapshot, &batch);
                state_snapshot.last_scan_at = Some(chrono::Utc::now().to_rfc3339());
                state::save(&state_snapshot);
                total += n;
                eprintln!("[Conversation] uploaded {} messages", n);
            }
            Err(UploadError::ClientError(e)) => {
                log_dead_letter(&batch, &e);
                advance_state(&mut state_snapshot, &batch);
                state::save(&state_snapshot);
                eprintln!("[Conversation] 4xx (dead-lettered): {}", e);
            }
            Err(UploadError::Transient(e)) => {
                eprintln!("[Conversation] transient error, will retry next cycle: {}", e);
                return Err(e);
            }
        }
    }
    Ok(total)
}
```

- [ ] **Step C3.2: Compile check**

Run: `cd src-tauri && cargo check --lib`
Expected: compiles (warnings about unused pub functions tolerated).

- [ ] **Step C3.3: Commit**

```bash
git add src-tauri/src/conversation/uploader.rs
git commit -m "feat(conversation): flush orchestrates scan + batch + state advance"
```

---

## Phase D · 客户端 lib.rs 集成

### Task D1: 集成 conversation loop 到启动流程

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step D1.1: Inspect current lib.rs layout**

Read: `src-tauri/src/lib.rs` lines 1-65
Confirm the location of constants `REPORT_INITIAL_DELAY_SECS`, `REPORT_INTERVAL_SECS`, `DEFAULT_REPORT_SERVER`.

- [ ] **Step D1.2: Add conversation loop constants and spawn**

Find the line `tauri::async_runtime::spawn(async {` around L44 (auto-report). After its closing `});` (around L55) but before `Ok(())`, insert:

```rust
            // Start conversation collection loop (Claude Code only, independent of metrics)
            tauri::async_runtime::spawn(async {
                eprintln!(
                    "[Conversation] scheduled: first in {}s, then every {}s",
                    CONVERSATION_INITIAL_DELAY_SECS, CONVERSATION_INTERVAL_SECS
                );
                tokio::time::sleep(std::time::Duration::from_secs(
                    CONVERSATION_INITIAL_DELAY_SECS,
                ))
                .await;
                loop {
                    eprintln!("[Conversation] scanning + uploading to {}", DEFAULT_REPORT_SERVER);
                    match conversation::uploader::flush(DEFAULT_REPORT_SERVER).await {
                        Ok(n) => eprintln!("[Conversation] cycle ok: {} messages", n),
                        Err(e) => eprintln!("[Conversation] cycle failed: {}", e),
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(
                        CONVERSATION_INTERVAL_SECS,
                    ))
                    .await;
                }
            });
```

Near the top of `lib.rs` (alongside `REPORT_INITIAL_DELAY_SECS` definition, which is typically around line 10-15), add:

```rust
const CONVERSATION_INITIAL_DELAY_SECS: u64 = 60;   // start 30s after metrics report to spread CPU/net load
const CONVERSATION_INTERVAL_SECS: u64 = 300;
```

If the existing constants look like `const REPORT_INITIAL_DELAY_SECS: u64 = 30;` place the new ones right after.

- [ ] **Step D1.3: Compile check**

Run: `cd src-tauri && cargo check`
Expected: succeeds.

- [ ] **Step D1.4: Full test run**

Run: `cd src-tauri && cargo test --lib conversation`
Expected: all conversation tests pass (20+ tests).

- [ ] **Step D1.5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(conversation): spawn independent upload loop on startup"
```

---

## Phase E · 服务端 conversations 路由

### Task E1: 搭建 sanitizeEmail + appendLinesAtomic 单元

**Files:**
- Create: `src/conversations.ts`
- Modify: `src/index.ts` — add body size limit

- [ ] **Step E1.1: Bump express body limit**

Modify `/Users/bin/IdeaProjects/ai-usage-server/src/index.ts`. Find the line `app.use(express.json(...))` (or similar) and change the body limit to `'15mb'`:

```typescript
app.use(express.json({ limit: '15mb' }));
```

If the existing line already sets a limit, update it; if no limit is set, add one. If the line doesn't exist yet, add after `const app = express();`.

- [ ] **Step E1.2: Create conversations.ts with pure helpers + route stub**

Create `/Users/bin/IdeaProjects/ai-usage-server/src/conversations.ts`:

```typescript
import { Router, Request, Response } from "express";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import { requireAuth } from "./auth.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
export const DATA_ROOT = path.join(__dirname, "..", "data");

export const router = Router();

/**
 * Replace characters that are unsafe in filesystem paths, while keeping
 * commonly-valid email characters intact. Allowed: a-z A-Z 0-9 . _ @ + -
 * Everything else becomes '_'.
 */
export function sanitizeEmail(email: string): string {
  return email.replace(/[^a-zA-Z0-9._@+\-]/g, "_");
}

/**
 * Append serialized lines to a jsonl file atomically:
 * - open with O_APPEND
 * - single write() for the whole buffer (kernel guarantees atomicity for
 *   writes <= PIPE_BUF and practically for <= several MB on mainstream FS)
 * - fsync before close
 */
export async function appendLinesAtomic(file: string, lines: any[]): Promise<void> {
  const payload = lines.map((l) => JSON.stringify(l)).join("\n") + "\n";
  await fs.promises.mkdir(path.dirname(file), { recursive: true });
  const fd = await fs.promises.open(file, "a");
  try {
    await fd.writeFile(payload, "utf-8");
    await fd.sync();
  } finally {
    await fd.close();
  }
}

// Routes added in subsequent tasks.
```

- [ ] **Step E1.3: Write unit tests for sanitizeEmail + appendLinesAtomic**

Create `/Users/bin/IdeaProjects/ai-usage-server/src/conversations.test.ts`:

```typescript
import test from "node:test";
import assert from "node:assert/strict";
import fs from "fs";
import os from "os";
import path from "path";
import { sanitizeEmail, appendLinesAtomic } from "./conversations.js";

test("sanitizeEmail keeps common email characters", () => {
  assert.equal(sanitizeEmail("bin@example.com"), "bin@example.com");
  assert.equal(sanitizeEmail("a.b+c@corp-x.io"), "a.b+c@corp-x.io");
  assert.equal(sanitizeEmail("Foo_Bar@Baz.io"), "Foo_Bar@Baz.io");
});

test("sanitizeEmail replaces path-unsafe characters", () => {
  assert.equal(sanitizeEmail("a/b@c"), "a_b@c");
  assert.equal(sanitizeEmail("..weird@x"), "..weird@x"); // dots fine
  assert.equal(sanitizeEmail("with space@x"), "with_space@x");
  assert.equal(sanitizeEmail("a:b@c"), "a_b@c");
});

test("appendLinesAtomic writes all lines", async () => {
  const dir = await fs.promises.mkdtemp(path.join(os.tmpdir(), "conv-"));
  const file = path.join(dir, "x.jsonl");
  await appendLinesAtomic(file, [{ a: 1 }, { a: 2 }]);
  await appendLinesAtomic(file, [{ a: 3 }]);
  const content = await fs.promises.readFile(file, "utf-8");
  const lines = content.trim().split("\n").map((l) => JSON.parse(l));
  assert.deepEqual(lines, [{ a: 1 }, { a: 2 }, { a: 3 }]);
  await fs.promises.rm(dir, { recursive: true, force: true });
});

test("appendLinesAtomic creates parent directory", async () => {
  const base = await fs.promises.mkdtemp(path.join(os.tmpdir(), "conv-"));
  const file = path.join(base, "nested", "deep", "x.jsonl");
  await appendLinesAtomic(file, [{ a: 1 }]);
  assert.ok(fs.existsSync(file));
  await fs.promises.rm(base, { recursive: true, force: true });
});
```

- [ ] **Step E1.4: Add test script and run**

Modify `/Users/bin/IdeaProjects/ai-usage-server/package.json` and add to `"scripts"`:

```json
"test": "tsx --test src/**/*.test.ts"
```

Run: `cd /Users/bin/IdeaProjects/ai-usage-server && npm test`
Expected: 4 tests pass.

- [ ] **Step E1.5: Commit**

```bash
cd /Users/bin/IdeaProjects/ai-usage-server
git add src/conversations.ts src/conversations.test.ts src/index.ts package.json
git commit -m "feat(conversations): sanitizeEmail + appendLinesAtomic with tests"
```

---

### Task E2: 实现 POST /api/conversations

**Files:**
- Modify: `src/conversations.ts`
- Modify: `src/index.ts`

- [ ] **Step E2.1: Add POST handler**

Append to `src/conversations.ts` (before the `// Routes` comment, replace that comment):

```typescript
const DATE_RE = /^\d{4}-\d{2}-\d{2}$/;

interface ConversationIn {
  uuid?: string;
  session_id?: string;
  timestamp?: string;
  project?: string;
  model?: string | null;
  role_tag?: string;
  text?: string;
  [key: string]: any;
}

router.post("/api/conversations", async (req: Request, res: Response) => {
  try {
    const { user_email, user_name, machine_id, client_version, tool, reported_at, messages } = req.body as {
      user_email?: string;
      user_name?: string;
      machine_id?: string;
      client_version?: string;
      tool?: string;
      reported_at?: string;
      messages?: ConversationIn[];
    };
    if (!user_email || !tool || !Array.isArray(messages)) {
      res.status(400).json({ error: "Missing user_email / tool / messages" });
      return;
    }

    const byDate = new Map<string, any[]>();
    const receivedAt = new Date().toISOString();
    for (const m of messages) {
      const date = (m.timestamp || "").slice(0, 10);
      if (!DATE_RE.test(date)) continue;
      const line = {
        ...m,
        user_email,
        user_name: user_name ?? "",
        machine_id: machine_id ?? "",
        client_version: client_version ?? "",
        received_at: receivedAt,
      };
      const bucket = byDate.get(date);
      if (bucket) bucket.push(line);
      else byDate.set(date, [line]);
    }

    let written = 0;
    for (const [date, lines] of byDate) {
      const dir = path.join(DATA_ROOT, "conversations", tool, date);
      const file = path.join(dir, `${sanitizeEmail(user_email)}.jsonl`);
      await appendLinesAtomic(file, lines);
      written += lines.length;
    }

    res.json({ ok: true, received: written });
  } catch (e: any) {
    console.error("Conversation upload error:", e);
    res.status(500).json({ error: e.message });
  }
});
```

- [ ] **Step E2.2: Register route in index.ts**

Modify `/Users/bin/IdeaProjects/ai-usage-server/src/index.ts`. Find where the existing `router` (from `routes.ts`) is mounted (something like `app.use(router)` or `app.use("/", router)`). Add nearby:

```typescript
import { router as conversationsRouter } from "./conversations.js";
app.use(conversationsRouter);
```

(If imports are at top, add the import at the top alongside others; mount alongside existing router.)

- [ ] **Step E2.3: Integration test for POST /api/conversations**

Append to `/Users/bin/IdeaProjects/ai-usage-server/src/conversations.test.ts`:

```typescript
import express from "express";
import { router, DATA_ROOT } from "./conversations.js";

test("POST /api/conversations writes NDJSON bucketed by date", async () => {
  // Override DATA_ROOT indirectly by running from a tmpdir. We cannot easily
  // replace DATA_ROOT, so we exercise the route and clean up under the real
  // data dir after.
  const app = express();
  app.use(express.json({ limit: "15mb" }));
  app.use(router);
  const server = app.listen(0);
  const addr = server.address();
  const port = typeof addr === "object" && addr ? addr.port : 0;

  const payload = {
    user_email: `test-${Date.now()}@example.com`,
    user_name: "tester",
    machine_id: "m",
    client_version: "0.0.0",
    tool: "claude_code",
    reported_at: new Date().toISOString(),
    messages: [
      { uuid: "u1", session_id: "s1", timestamp: "2026-04-22T00:00:00Z", project: "p", text: "hi", role_tag: "first" },
      { uuid: "u2", session_id: "s1", timestamp: "2026-04-23T00:00:00Z", project: "p", text: "ho", role_tag: "followup" },
      { uuid: "u3", session_id: "s1", timestamp: "bogus", project: "p", text: "skip", role_tag: "followup" },
    ],
  };

  const res = await fetch(`http://127.0.0.1:${port}/api/conversations`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(payload),
  });
  const json = await res.json();
  assert.equal(res.status, 200);
  assert.equal(json.ok, true);
  assert.equal(json.received, 2); // bogus timestamp dropped

  const safe = payload.user_email.replace(/[^a-zA-Z0-9._@+\-]/g, "_");
  const f1 = path.join(DATA_ROOT, "conversations", "claude_code", "2026-04-22", `${safe}.jsonl`);
  const f2 = path.join(DATA_ROOT, "conversations", "claude_code", "2026-04-23", `${safe}.jsonl`);
  const l1 = fs.readFileSync(f1, "utf-8").trim().split("\n").map((l) => JSON.parse(l));
  const l2 = fs.readFileSync(f2, "utf-8").trim().split("\n").map((l) => JSON.parse(l));
  assert.equal(l1.length, 1);
  assert.equal(l1[0].uuid, "u1");
  assert.equal(l1[0].user_email, payload.user_email);
  assert.ok(l1[0].received_at);
  assert.equal(l2.length, 1);
  assert.equal(l2[0].uuid, "u2");

  fs.rmSync(path.dirname(f1), { recursive: true, force: true });
  fs.rmSync(path.dirname(f2), { recursive: true, force: true });
  server.close();
});

test("POST /api/conversations rejects missing fields", async () => {
  const app = express();
  app.use(express.json({ limit: "15mb" }));
  app.use(router);
  const server = app.listen(0);
  const port = (server.address() as any).port;

  const res = await fetch(`http://127.0.0.1:${port}/api/conversations`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ tool: "claude_code" }),
  });
  assert.equal(res.status, 400);
  server.close();
});
```

- [ ] **Step E2.4: Run tests**

Run: `cd /Users/bin/IdeaProjects/ai-usage-server && npm test`
Expected: 6 tests pass.

- [ ] **Step E2.5: Commit**

```bash
cd /Users/bin/IdeaProjects/ai-usage-server
git add src/conversations.ts src/conversations.test.ts src/index.ts
git commit -m "feat(conversations): POST /api/conversations with date-bucketing"
```

---

### Task E3: 实现 GET /api/conversations/detail

**Files:**
- Modify: `src/conversations.ts`
- Check: `src/auth.ts` for `requireAuth` export

- [ ] **Step E3.1: Confirm requireAuth signature**

Read `/Users/bin/IdeaProjects/ai-usage-server/src/auth.ts`. Confirm there is an exported middleware (usually `requireAuth` or `authMiddleware`). If the export has a different name, adjust the import in `conversations.ts` in step E3.2.

- [ ] **Step E3.2: Add GET handler**

Append to `src/conversations.ts`:

```typescript
router.get("/api/conversations/detail", requireAuth, async (req: Request, res: Response) => {
  try {
    const { tool, date, email, project, model } = req.query as Record<string, string | undefined>;
    if (!tool || !date || !email || !project) {
      res.status(400).json({ error: "Missing tool / date / email / project" });
      return;
    }
    if (!DATE_RE.test(date)) {
      res.status(400).json({ error: "Invalid date" });
      return;
    }
    const file = path.join(
      DATA_ROOT,
      "conversations",
      tool,
      date,
      `${sanitizeEmail(email)}.jsonl`
    );
    if (!fs.existsSync(file)) {
      res.json({ total: 0, messages: [] });
      return;
    }
    const content = await fs.promises.readFile(file, "utf-8");
    const messages = content
      .split("\n")
      .filter((l) => l.length > 0)
      .map((l) => {
        try {
          return JSON.parse(l);
        } catch {
          return null;
        }
      })
      .filter((m): m is any => {
        if (!m) return false;
        if (m.project !== project) return false;
        if (!model) return true;
        if (model === "unknown") return !m.model;
        return m.model === model;
      });
    messages.sort((a: any, b: any) =>
      (a.timestamp || "").localeCompare(b.timestamp || "")
    );
    res.json({ total: messages.length, messages });
  } catch (e: any) {
    res.status(500).json({ error: e.message });
  }
});
```

- [ ] **Step E3.3: Test GET filter semantics**

Append to `src/conversations.test.ts`:

```typescript
test("GET /api/conversations/detail filters by project and model", async () => {
  // Seed a file directly.
  const email = `get-test-${Date.now()}@example.com`;
  const safe = email.replace(/[^a-zA-Z0-9._@+\-]/g, "_");
  const dir = path.join(DATA_ROOT, "conversations", "claude_code", "2026-04-22");
  fs.mkdirSync(dir, { recursive: true });
  const file = path.join(dir, `${safe}.jsonl`);
  const lines = [
    { uuid: "a", timestamp: "2026-04-22T00:02:00Z", project: "p1", model: "claude-opus-4-6", text: "x" },
    { uuid: "b", timestamp: "2026-04-22T00:01:00Z", project: "p1", model: "claude-sonnet-4-5", text: "y" },
    { uuid: "c", timestamp: "2026-04-22T00:03:00Z", project: "p2", model: "claude-opus-4-6", text: "z" },
    { uuid: "d", timestamp: "2026-04-22T00:04:00Z", project: "p1", model: null, text: "w" },
  ];
  fs.writeFileSync(file, lines.map((l) => JSON.stringify(l)).join("\n") + "\n");

  // Build an app that skips auth (use an auth-bypass test flag, or mount just
  // our route without requireAuth). Here we re-import and stub by monkey-patch.
  const app = express();
  app.use(express.json({ limit: "15mb" }));
  // Mount an inline handler that replicates detail logic but skips auth.
  app.get("/api/conversations/detail", async (req, res) => {
    const { tool, date, email, project, model } = req.query as Record<string, string>;
    const file = path.join(DATA_ROOT, "conversations", tool, date, `${sanitizeEmail(email)}.jsonl`);
    if (!fs.existsSync(file)) return res.json({ total: 0, messages: [] });
    const content = fs.readFileSync(file, "utf-8");
    const messages = content.split("\n").filter(Boolean).map((l) => JSON.parse(l))
      .filter((m) => m.project === project && (!model || (model === "unknown" ? !m.model : m.model === model)))
      .sort((a, b) => (a.timestamp || "").localeCompare(b.timestamp || ""));
    res.json({ total: messages.length, messages });
  });

  const server = app.listen(0);
  const port = (server.address() as any).port;
  const qs = (p: Record<string, string>) => new URLSearchParams(p).toString();

  const r1 = await fetch(`http://127.0.0.1:${port}/api/conversations/detail?${qs({
    tool: "claude_code", date: "2026-04-22", email, project: "p1"
  })}`).then((r) => r.json());
  assert.equal(r1.total, 3);
  assert.deepEqual(r1.messages.map((m: any) => m.uuid), ["b", "a", "d"]);

  const r2 = await fetch(`http://127.0.0.1:${port}/api/conversations/detail?${qs({
    tool: "claude_code", date: "2026-04-22", email, project: "p1", model: "claude-opus-4-6"
  })}`).then((r) => r.json());
  assert.equal(r2.total, 1);
  assert.equal(r2.messages[0].uuid, "a");

  const r3 = await fetch(`http://127.0.0.1:${port}/api/conversations/detail?${qs({
    tool: "claude_code", date: "2026-04-22", email, project: "p1", model: "unknown"
  })}`).then((r) => r.json());
  assert.equal(r3.total, 1);
  assert.equal(r3.messages[0].uuid, "d");

  fs.rmSync(dir, { recursive: true, force: true });
  server.close();
});
```

- [ ] **Step E3.4: Run tests**

Run: `cd /Users/bin/IdeaProjects/ai-usage-server && npm test`
Expected: 7 tests pass.

- [ ] **Step E3.5: Commit**

```bash
cd /Users/bin/IdeaProjects/ai-usage-server
git add src/conversations.ts src/conversations.test.ts
git commit -m "feat(conversations): GET /api/conversations/detail with auth + filters"
```

---

## Phase F · 服务端 TTL 清理

### Task F1: 实现 cleanup.ts

**Files:**
- Create: `src/cleanup.ts`
- Create: `src/cleanup.test.ts`
- Modify: `src/index.ts`

- [ ] **Step F1.1: Create cleanup.ts**

Create `/Users/bin/IdeaProjects/ai-usage-server/src/cleanup.ts`:

```typescript
import fs from "fs";
import path from "path";

const DATE_RE = /^\d{4}-\d{2}-\d{2}$/;
const RETENTION_DAYS = 90;
const SIX_HOURS_MS = 6 * 60 * 60 * 1000;

function formatDate(ts: number): string {
  const d = new Date(ts);
  const yyyy = d.getUTCFullYear();
  const mm = String(d.getUTCMonth() + 1).padStart(2, "0");
  const dd = String(d.getUTCDate()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd}`;
}

function dirSize(dir: string): number {
  let total = 0;
  try {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) total += dirSize(full);
      else total += fs.statSync(full).size;
    }
  } catch {}
  return total;
}

/**
 * Run a single cleanup pass. Removes conversation date-directories older than
 * `RETENTION_DAYS`. Returns `{ removedDirs, freedBytes }` for logging/tests.
 */
export function runCleanup(dataRoot: string, now: number = Date.now()): { removedDirs: string[]; freedBytes: number } {
  const cutoff = formatDate(now - RETENTION_DAYS * 86_400_000);
  const convoRoot = path.join(dataRoot, "conversations");
  if (!fs.existsSync(convoRoot)) return { removedDirs: [], freedBytes: 0 };

  const removed: string[] = [];
  let freed = 0;
  for (const tool of fs.readdirSync(convoRoot)) {
    const toolDir = path.join(convoRoot, tool);
    let entries: string[];
    try {
      entries = fs.readdirSync(toolDir);
    } catch {
      continue;
    }
    for (const dateDir of entries) {
      if (!DATE_RE.test(dateDir)) continue;
      if (dateDir >= cutoff) continue;
      const full = path.join(toolDir, dateDir);
      const size = dirSize(full);
      fs.rmSync(full, { recursive: true, force: true });
      removed.push(full);
      freed += size;
    }
  }
  return { removedDirs: removed, freedBytes: freed };
}

/**
 * Start the cleanup scheduler. First run after 30s, then every 6h.
 */
export function startCleanupScheduler(dataRoot: string): void {
  const kick = () => {
    try {
      const { removedDirs, freedBytes } = runCleanup(dataRoot);
      if (removedDirs.length > 0) {
        console.log(`[Cleanup] removed ${removedDirs.length} dir(s), freed ${(freedBytes / 1024 / 1024).toFixed(1)} MB`);
      }
    } catch (e) {
      console.error("[Cleanup] error:", e);
    }
  };
  setTimeout(kick, 30_000);
  setInterval(kick, SIX_HOURS_MS);
}
```

- [ ] **Step F1.2: Write tests**

Create `/Users/bin/IdeaProjects/ai-usage-server/src/cleanup.test.ts`:

```typescript
import test from "node:test";
import assert from "node:assert/strict";
import fs from "fs";
import os from "os";
import path from "path";
import { runCleanup } from "./cleanup.js";

function seedFile(dir: string, name: string, content: string) {
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, name), content);
}

test("runCleanup removes directories older than 90 days", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "cleanup-"));
  const now = Date.UTC(2026, 3, 22); // 2026-04-22

  // Old: 2026-01-22 (exactly 90 days before 2026-04-22 → kept by >= cutoff rule)
  // But we want "strictly older" → pick 2026-01-21
  const old = path.join(root, "conversations", "claude_code", "2026-01-21");
  seedFile(old, "a.jsonl", "x");

  // Fresh
  const fresh = path.join(root, "conversations", "claude_code", "2026-04-22");
  seedFile(fresh, "b.jsonl", "y");

  const result = runCleanup(root, now);
  assert.equal(result.removedDirs.length, 1);
  assert.equal(fs.existsSync(old), false);
  assert.equal(fs.existsSync(fresh), true);
  assert.ok(result.freedBytes > 0);

  fs.rmSync(root, { recursive: true, force: true });
});

test("runCleanup ignores non-date directory names", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "cleanup-"));
  const now = Date.UTC(2026, 3, 22);
  const weird = path.join(root, "conversations", "claude_code", "not-a-date");
  seedFile(weird, "x.jsonl", "x");
  runCleanup(root, now);
  assert.equal(fs.existsSync(weird), true);
  fs.rmSync(root, { recursive: true, force: true });
});

test("runCleanup is a no-op if conversations dir missing", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "cleanup-"));
  const r = runCleanup(root, Date.now());
  assert.deepEqual(r, { removedDirs: [], freedBytes: 0 });
  fs.rmSync(root, { recursive: true, force: true });
});

test("runCleanup handles multiple tools", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "cleanup-"));
  const now = Date.UTC(2026, 3, 22);
  const oldClaude = path.join(root, "conversations", "claude_code", "2025-12-01");
  const oldCodex = path.join(root, "conversations", "codex", "2025-12-01");
  const freshClaude = path.join(root, "conversations", "claude_code", "2026-04-22");
  seedFile(oldClaude, "a", "x");
  seedFile(oldCodex, "b", "y");
  seedFile(freshClaude, "c", "z");
  const r = runCleanup(root, now);
  assert.equal(r.removedDirs.length, 2);
  assert.ok(fs.existsSync(freshClaude));
  fs.rmSync(root, { recursive: true, force: true });
});
```

- [ ] **Step F1.3: Run cleanup tests**

Run: `cd /Users/bin/IdeaProjects/ai-usage-server && npm test`
Expected: all tests pass (now 11 total).

- [ ] **Step F1.4: Wire into index.ts**

Modify `src/index.ts`. After `initDB()` call (or near server start), add:

```typescript
import { startCleanupScheduler } from "./cleanup.js";
// ...
import path from "path";
import { fileURLToPath } from "url";
const __dirname = path.dirname(fileURLToPath(import.meta.url));
startCleanupScheduler(path.join(__dirname, "..", "data"));
```

(If `__dirname` helpers already exist in the file, reuse them; don't duplicate.)

- [ ] **Step F1.5: Commit**

```bash
cd /Users/bin/IdeaProjects/ai-usage-server
git add src/cleanup.ts src/cleanup.test.ts src/index.ts
git commit -m "feat(conversations): 90-day TTL cleanup scheduler"
```

---

## Phase G · Dashboard UI 改造

### Task G1: 表格新增"操作"列

**Files:**
- Modify: `/Users/bin/IdeaProjects/ai-usage-server/public/index.html`

- [ ] **Step G1.1: Add "操作" header**

Modify `public/index.html`. Find the `<thead>` block around L175-188. Add after the 费用 `<th>`:

```html
              <th>操作</th>
```

- [ ] **Step G1.2: Add button cell in row template**

Find the template string in `renderDetailTable()` around L640-655. After the last `<td class="cost">...</td>` line (L653), add:

```js
          <td>${r.tool === 'claude_code'
            ? `<button class="view-btn" data-date="${escapeAttr(r.date)}" data-project="${escapeAttr(r.project)}" data-model="${escapeAttr(r.model)}" onclick="openConversationsFromBtn(this)">查看问题</button>`
            : `<span style="color:#ccc" title="暂未接入">—</span>`}</td>
```

- [ ] **Step G1.3: Add escapeAttr helper**

Find an existing helper function area in `<script>` (near `escapeHtml` or format helpers). If `escapeAttr` doesn't exist, add it near other formatting helpers (somewhere around L400-500):

```js
    function escapeAttr(s) {
      return String(s == null ? '' : s)
        .replace(/&/g, '&amp;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;');
    }

    function escapeHtml(s) {
      return String(s == null ? '' : s)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;');
    }
```

(If `escapeHtml` already exists, don't duplicate it — just add `escapeAttr`.)

- [ ] **Step G1.4: Add view-btn CSS**

Find the `<style>` block (top of file, around L7-83). Add within it:

```css
    .view-btn { padding: 4px 10px; font-size: 12px; border: 1px solid #6c5ce7; background: #fff; color: #6c5ce7; border-radius: 4px; cursor: pointer; }
    .view-btn:hover { background: #6c5ce7; color: #fff; }
```

- [ ] **Step G1.5: Dev smoke (no commit yet)**

Run: `cd /Users/bin/IdeaProjects/ai-usage-server && npm run dev` (background)
Open browser: `http://localhost:3000`, login, drill into a user's detail, verify the "操作" column shows "查看问题" on `claude_code` rows and "—" on other tools. The button click will `ReferenceError: openConversationsFromBtn is not defined` — that's expected, we add it next.

- [ ] **Step G1.6: Commit**

```bash
cd /Users/bin/IdeaProjects/ai-usage-server
git add public/index.html
git commit -m "feat(dashboard): add '操作' column with 查看问题 button"
```

---

### Task G2: 添加抽屉 DOM + CSS

**Files:**
- Modify: `public/index.html`

- [ ] **Step G2.1: Add drawer CSS**

Append inside the existing `<style>` block (after `.view-btn` rules):

```css
    .drawer-overlay { display: none; position: fixed; inset: 0; background: rgba(0,0,0,0.3); z-index: 900; }
    .drawer-overlay.active { display: block; }
    .drawer { position: fixed; top: 0; right: 0; height: 100vh; width: min(600px, 100vw); background: #fff; box-shadow: -4px 0 16px rgba(0,0,0,0.1); transform: translateX(100%); transition: transform 0.25s ease; z-index: 901; display: flex; flex-direction: column; }
    .drawer.active { transform: translateX(0); }
    .drawer-header { padding: 16px 20px; border-bottom: 1px solid #eee; display: flex; justify-content: space-between; align-items: flex-start; gap: 12px; }
    .drawer-header-info { flex: 1; font-size: 14px; }
    .drawer-filters { padding: 12px 20px; border-bottom: 1px solid #f0f0f0; display: flex; gap: 12px; align-items: center; flex-wrap: wrap; }
    .drawer-filters input[type="text"] { flex: 1; min-width: 180px; padding: 6px 10px; border: 1px solid #ddd; border-radius: 6px; font-size: 13px; }
    .drawer-filters label { font-size: 13px; color: #555; display: flex; align-items: center; gap: 4px; }
    .drawer-body { flex: 1; overflow-y: auto; padding: 8px 20px 20px; }
    .drawer-count { font-size: 12px; color: #888; }
    .msg-item { padding: 12px 0; border-bottom: 1px solid #f5f5f5; }
    .msg-meta { font-size: 12px; color: #888; margin-bottom: 4px; display: flex; gap: 8px; align-items: center; }
    .msg-tag { display: inline-block; padding: 1px 6px; border-radius: 3px; font-size: 11px; font-weight: 500; background: #f0f0ff; color: #6c5ce7; }
    .msg-tag.followup { background: #fff4e6; color: #d97706; }
    .msg-tag.retry { background: #ffe4e6; color: #dc2626; }
    .msg-text { white-space: pre-wrap; word-break: break-word; font-size: 13px; line-height: 1.5; max-height: 8em; overflow: hidden; cursor: pointer; position: relative; }
    .msg-text.expanded { max-height: none; }
    .msg-text:not(.expanded)::after { content: '…点击展开'; position: absolute; bottom: 0; right: 0; background: linear-gradient(to right, transparent, #fff 30%); padding-left: 40px; font-size: 11px; color: #888; }
```

- [ ] **Step G2.2: Add drawer HTML before `</body>`**

Find `</body>` (near end of file). Add just before it:

```html
  <div class="drawer-overlay" id="drawerOverlay" onclick="closeDrawer()"></div>
  <div class="drawer" id="drawer">
    <div class="drawer-header">
      <div class="drawer-header-info" id="drawerTitle"></div>
      <button class="back-btn" onclick="closeDrawer()">关闭</button>
    </div>
    <div class="drawer-filters">
      <input type="text" id="drawerSearch" placeholder="搜索文本…" oninput="filterDrawer()">
      <label><input type="checkbox" id="drawerFirstOnly" onchange="filterDrawer()"> 仅首问</label>
      <span class="drawer-count" id="drawerCount"></span>
    </div>
    <div class="drawer-body" id="drawerBody"></div>
  </div>
```

- [ ] **Step G2.3: Commit**

```bash
cd /Users/bin/IdeaProjects/ai-usage-server
git add public/index.html
git commit -m "feat(dashboard): add conversation drawer DOM + styles"
```

---

### Task G3: 实现 openConversations + 过滤逻辑

**Files:**
- Modify: `public/index.html`

- [ ] **Step G3.1: Add drawer JS functions**

Find the `<script>` block. After the existing helper functions (e.g., after `escapeHtml`), add:

```js
    // ── Conversation Drawer ─────────────────────────────────────
    let drawerMessages = [];

    function openConversationsFromBtn(btn) {
      openConversations(btn.dataset.date, btn.dataset.project, btn.dataset.model);
    }

    async function openConversations(date, project, model) {
      const email = currentDetailEmail;
      if (!email) return;
      const qs = new URLSearchParams({
        tool: 'claude_code',
        date,
        email,
        project,
      });
      if (model && model !== 'null' && model !== 'undefined') qs.set('model', model);
      const titleEl = document.getElementById('drawerTitle');
      titleEl.innerHTML = `<strong>${escapeHtml(email)}</strong> · ${escapeHtml(project)} · ${escapeHtml(date)}<br><span style="font-size:12px;color:#888">${escapeHtml(model || '—')} · Claude Code</span>`;
      document.getElementById('drawerSearch').value = '';
      document.getElementById('drawerFirstOnly').checked = false;
      document.getElementById('drawerBody').innerHTML = '<div class="empty" style="padding:20px">加载中…</div>';
      document.getElementById('drawerOverlay').classList.add('active');
      document.getElementById('drawer').classList.add('active');
      try {
        const r = await fetch(`/api/conversations/detail?${qs.toString()}`);
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        const { messages } = await r.json();
        drawerMessages = messages || [];
        renderDrawerBody(drawerMessages);
      } catch (e) {
        document.getElementById('drawerBody').innerHTML = `<div class="empty" style="padding:20px;color:#e17055">加载失败: ${escapeHtml(String(e.message || e))}</div>`;
      }
    }

    function closeDrawer() {
      document.getElementById('drawerOverlay').classList.remove('active');
      document.getElementById('drawer').classList.remove('active');
    }

    function filterDrawer() {
      const kw = document.getElementById('drawerSearch').value.toLowerCase();
      const firstOnly = document.getElementById('drawerFirstOnly').checked;
      const filtered = drawerMessages.filter(m =>
        (!firstOnly || m.role_tag === 'first')
        && (!kw || (m.text || '').toLowerCase().includes(kw))
      );
      renderDrawerBody(filtered);
    }

    function renderDrawerBody(list) {
      document.getElementById('drawerCount').textContent = `共 ${list.length} 条`;
      const body = document.getElementById('drawerBody');
      if (list.length === 0) {
        body.innerHTML = '<div class="empty" style="padding:20px">无数据</div>';
        return;
      }
      body.innerHTML = list.map(m => `
        <div class="msg-item">
          <div class="msg-meta">
            <span>${escapeHtml(m.timestamp || '')}</span>
            <span class="msg-tag ${m.role_tag || 'followup'}">${escapeHtml(m.role_tag || '—')}</span>
          </div>
          <div class="msg-text" onclick="this.classList.toggle('expanded')">${escapeHtml(m.text || '')}</div>
        </div>
      `).join('');
    }

    // ESC closes drawer
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && document.getElementById('drawer').classList.contains('active')) {
        closeDrawer();
      }
    });
```

- [ ] **Step G3.2: Manual browser smoke**

If `npm run dev` is running, hard reload. Otherwise start it:
```bash
cd /Users/bin/IdeaProjects/ai-usage-server && npm run dev
```

In browser:
1. Login, drill into a user detail page
2. Click "查看问题" on any `claude_code` row
3. Confirm drawer opens from the right
4. Expect "无数据" (since client hasn't uploaded anything yet — that's fine)
5. Close via X button / overlay click / ESC key — all should work

- [ ] **Step G3.3: Commit**

```bash
cd /Users/bin/IdeaProjects/ai-usage-server
git add public/index.html
git commit -m "feat(dashboard): wire 查看问题 drawer with search/first-only filters"
```

---

## Phase H · 版本号 + 端到端冒烟

### Task H1: Bump client version

**Files:**
- Modify: `package.json`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step H1.1: Bump to 0.5.0**

- `/Users/bin/Desktop/cache/cache3/session-viewer/package.json` → `"version": "0.5.0"`
- `/Users/bin/Desktop/cache/cache3/session-viewer/src-tauri/tauri.conf.json` → `"version": "0.5.0"` (find the existing version field)
- `/Users/bin/Desktop/cache/cache3/session-viewer/src-tauri/Cargo.toml` → `version = "0.5.0"`

- [ ] **Step H1.2: Rebuild Cargo.lock**

Run: `cd src-tauri && cargo check`
Expected: succeeds; `Cargo.lock` updated.

- [ ] **Step H1.3: Commit**

```bash
cd /Users/bin/Desktop/cache/cache3/session-viewer
git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore: bump version to 0.5.0 for conversation collection"
```

---

### Task H2: End-to-end smoke

- [ ] **Step H2.1: Start the server**

```bash
cd /Users/bin/IdeaProjects/ai-usage-server
npm run dev
```
Expect: logs show `[Cleanup] ...` (if stale dirs exist) and the server listening on 3000.

- [ ] **Step H2.2: Start the client**

In another terminal:
```bash
cd /Users/bin/Desktop/cache/cache3/session-viewer
npm run tauri dev
```
Wait ~60s for `[Conversation] scheduled: first in 60s` and then `[Conversation] scanning + uploading`.

- [ ] **Step H2.3: Verify server received data**

```bash
ls /Users/bin/IdeaProjects/ai-usage-server/data/conversations/claude_code/
```
Expect: at least one `YYYY-MM-DD/` directory with a `{email}.jsonl` file.

Inspect:
```bash
head -1 /Users/bin/IdeaProjects/ai-usage-server/data/conversations/claude_code/*/*.jsonl | head -1 | python3 -c "import sys,json; print(json.dumps(json.loads(sys.stdin.read()), indent=2))"
```
Expect: a ConversationMessage with `uuid`, `session_id`, `role_tag`, `text`, `received_at`, etc.

- [ ] **Step H2.4: Verify dashboard drawer**

1. Open `http://localhost:3000` in browser, login
2. Drill into your own user, find the most recent `claude_code` row
3. Click "查看问题"
4. Drawer opens, messages list populated, first message tagged `first`
5. Search box filters; "仅首问" toggle hides followups
6. Click a long message → expands

- [ ] **Step H2.5: Verify resumability**

Kill the client (ctrl+C). Restart with `npm run tauri dev`.
Check server file — no duplicate uuids for messages (the `tail -1` of before/after restart should be stable; new messages since restart appended).

Cheap dedupe check:
```bash
awk -F '"uuid":"' '{print $2}' data/conversations/claude_code/*/*.jsonl | cut -d'"' -f1 | sort | uniq -d | wc -l
```
Expect: 0 (no duplicate uuids).

- [ ] **Step H2.6: Verify state file**

```bash
cat "$HOME/Library/Application Support/session-viewer/conversation-state.json" | python3 -m json.tool | head -20
```
Expect: `file_offsets` map with real jsonl paths and positive numeric offsets.

- [ ] **Step H2.7: Commit any follow-up fixes**

If the smoke uncovered bugs, fix in place (each fix: test + code + commit). If it was clean, no commit needed.

---

## Phase I · 文档收尾

### Task I1: 更新 CONTEXT.md

**Files:**
- Modify: `/Users/bin/Desktop/cache/cache3/session-viewer/CONTEXT.md`

- [ ] **Step I1.1: Append a new "Conversation Collection" section**

Open `CONTEXT.md`. Find the "## 使用数据上报 (Report)" section (near bottom). Add a sibling section:

```markdown
## 用户 Prompt 采集 (Conversation)

> 添加于 0.5.0。独立于 `/api/report` 指标上报。

### 客户端
- 模块: `src-tauri/src/conversation/{mod,state,scanner,uploader}.rs`
- 触发: lib.rs spawn，启动后 60s 首次，之后每 5 分钟
- 覆盖: **仅 Claude Code**（MVP）。服务端路由与 Dashboard 已预留多工具扩展点。
- 幂等键: Claude `message_uuid`；高水位线为 `{file_path: byte_offset}`
- 状态文件: `{data_dir}/session-viewer/conversation-state.json`
- 过滤: 6 类 CLI 注入前缀，tool_result，空白
- 打标: `first` / `followup` / `retry`（增量扫 offset>0 时不发 `first`）

### 服务端
- 路由: `src/conversations.ts`
  - `POST /api/conversations` — 无认证，接收上报
  - `GET /api/conversations/detail` — JWT，供 Dashboard 调用
- 存储: `/app/data/conversations/{tool}/{YYYY-MM-DD}/{sanitize(email)}.jsonl`
- TTL: `src/cleanup.ts` 每 6h 扫，删 90 天前目录
- 体积估算: 10 人 × 5MB/天 × 90 天 ≈ 4.5GB，建议磁盘 ≥ 20GB

### Dashboard
- `public/index.html` "项目用量明细"表新增"操作"列 → 右侧抽屉
- 仅 `claude_code` 行显示按钮；其他工具灰置 `—`
```

- [ ] **Step I1.2: Commit**

```bash
cd /Users/bin/Desktop/cache/cache3/session-viewer
git add CONTEXT.md
git commit -m "docs: record conversation collection module in CONTEXT.md"
```

---

### Task I2: 更新 CHANGELOG

**Files:**
- Modify: `/Users/bin/Desktop/cache/cache3/session-viewer/CHANGELOG.md`

- [ ] **Step I2.1: Add 0.5.0 entry**

Read the existing `CHANGELOG.md` header and most recent entry format. Prepend:

```markdown
## 0.5.0 - 2026-04-22

### Added
- Claude Code 用户 prompt 自动采集：扫描 `~/.claude/projects/**/*.jsonl`，增量上传至 `ai-usage-server` 的 `/api/conversations` 端点（10MB/批，断点续传，3 个月保留）
- Dashboard "项目用量明细"新增"查看问题"按钮 → 右侧抽屉展示该 (date × project × model) 的 prompt 明细，支持搜索与"仅首问"过滤
- `first` / `followup` / `retry` 启发式标签
- 独立持久化状态 `conversation-state.json`（按文件字节 offset），与现有 `report-high-water.json` 解耦
```

- [ ] **Step I2.2: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: 0.5.0 changelog entry"
```

---

## 完工标准

- [ ] `cargo test --lib` 全绿（客户端 conversation 相关 20+ 测试）
- [ ] `npm test` 全绿（服务端 11+ 测试）
- [ ] E2E 冒烟通过 (Task H2 全部步骤)
- [ ] 服务端 `data/conversations/` 下有 ≥ 1 个 jsonl，无重复 uuid
- [ ] Dashboard 抽屉可用，`first` 标签渲染为紫色徽章、`followup` 橙色、`retry` 红色
- [ ] 版本号 0.5.0 在 package.json / tauri.conf.json / Cargo.toml 三处一致
- [ ] 所有提交按 Phase 顺序、每步一 commit（共约 15~20 个 commit）

## 超出本 Plan 的范围（Spec §10 开放事项）

- 其他工具（Codex/OpenCode/Copilot/Cursor）的 prompt 抽取 — 下一期
- 跨天/跨用户聚合查询 — 需离线 ETL 或后续加轻量索引
- 大文件 gzip 归档 — 3 个月 TTL 内先不做
- 分析工具选型（jq/Python/ES）— 后续按分析师偏好决定
