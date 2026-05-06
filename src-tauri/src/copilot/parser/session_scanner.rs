use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::copilot::models::session::CopilotSession;

/// Path to the Copilot CLI session-state directory
pub fn get_session_state_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".copilot").join("session-state"))
}

/// Normalize a path so that macOS symlink variants are unified.
/// On macOS /var is a symlink to /private/var; canonicalize to /private/var/...
fn normalize_path(path: &str) -> String {
    // Try to resolve the real path via the filesystem first
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return canonical.to_string_lossy().into_owned();
    }
    // Fallback: rewrite known macOS symlink prefix
    if path.starts_with("/var/") {
        return format!("/private{}", path);
    }
    path.to_string()
}

/// Parse a flat YAML file (key: value lines only) into key-value pairs
fn parse_flat_yaml(content: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for line in content.lines() {
        if let Some(colon) = line.find(':') {
            let key = line[..colon].trim().to_string();
            let value = line[colon + 1..].trim().to_string();
            if !key.is_empty() && !value.is_empty() {
                map.insert(key, value);
            }
        }
    }
    map
}

/// Count user.message lines in events.jsonl (fast scan, no full parse)
fn count_messages(events_path: &Path) -> usize {
    let file = match fs::File::open(events_path) {
        Ok(f) => f,
        Err(_) => return 0,
    };
    let reader = BufReader::new(file);
    reader
        .lines()
        .map_while(Result::ok)
        .filter(|l| l.contains("\"type\":\"user.message\"") || l.contains("\"type\":\"assistant.message\""))
        .count()
}

/// Extract the first user message content from events.jsonl
fn extract_first_prompt(events_path: &Path) -> Option<String> {
    let file = fs::File::open(events_path).ok()?;
    let reader = BufReader::new(file);
    for line in reader.lines().map_while(Result::ok) {
        if line.contains("\"type\":\"user.message\"") {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                let content = v["data"]["content"].as_str()?;
                let preview: String = content.chars().take(200).collect();
                return Some(preview);
            }
        }
    }
    None
}

/// Scan a single session directory and return a CopilotSession
pub fn scan_session_dir(dir: &Path) -> Option<CopilotSession> {
    let yaml_path = dir.join("workspace.yaml");
    let content = fs::read_to_string(&yaml_path).ok()?;
    let yaml = parse_flat_yaml(&content);

    let session_id = yaml.get("id")?.clone();
    let cwd = normalize_path(yaml.get("cwd")?);
    let git_root = yaml.get("git_root").map(|s| normalize_path(s));

    let events_path = dir.join("events.jsonl");
    let message_count = if events_path.exists() {
        count_messages(&events_path)
    } else {
        0
    };
    let first_prompt = if events_path.exists() {
        extract_first_prompt(&events_path)
    } else {
        None
    };

    Some(CopilotSession {
        session_id,
        cwd,
        git_root,
        branch: yaml.get("branch").cloned(),
        summary: yaml.get("summary").cloned(),
        created_at: yaml.get("created_at").cloned().unwrap_or_default(),
        updated_at: yaml.get("updated_at").cloned(),
        message_count,
        first_prompt,
    })
}

/// Scan all session directories and return sessions for a specific cwd
pub fn scan_sessions_for_cwd(target_cwd: &str) -> Vec<CopilotSession> {
    let Some(state_dir) = get_session_state_dir() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(&state_dir) else {
        return Vec::new();
    };

    let mut sessions: Vec<CopilotSession> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| scan_session_dir(&e.path()))
        .filter(|s| s.cwd == target_cwd)
        .collect();

    sessions.sort_by(|a, b| {
        let ta = a.updated_at.as_deref().unwrap_or(&a.created_at);
        let tb = b.updated_at.as_deref().unwrap_or(&b.created_at);
        tb.cmp(ta)
    });
    sessions
}

/// Scan all session directories and return all sessions
pub fn scan_all_sessions() -> Vec<CopilotSession> {
    let Some(state_dir) = get_session_state_dir() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(&state_dir) else {
        return Vec::new();
    };

    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| scan_session_dir(&e.path()))
        .collect()
}
