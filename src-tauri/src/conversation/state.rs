use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CursorMark {
    #[serde(default)]
    pub last_updated_at: u64,  // ms since epoch
    #[serde(default)]
    pub bubble_index: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConversationState {
    pub file_offsets: HashMap<PathBuf, u64>,
    pub last_scan_at: Option<String>,
    #[serde(default)]
    pub cursor_marks: HashMap<String, CursorMark>,  // composer_id -> mark
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

    #[test]
    fn serde_roundtrip_includes_cursor_marks() {
        let mut s = ConversationState::default();
        s.cursor_marks.insert("abc".into(), CursorMark { last_updated_at: 1700000000000, bubble_index: 5 });
        let json = serde_json::to_string(&s).unwrap();
        let back: ConversationState = serde_json::from_str(&json).unwrap();
        let m = back.cursor_marks.get("abc").unwrap();
        assert_eq!(m.last_updated_at, 1700000000000);
        assert_eq!(m.bubble_index, 5);
    }

    #[test]
    fn old_state_without_cursor_marks_still_loads() {
        let old_json = r#"{"file_offsets":{},"last_scan_at":null}"#;
        let s: ConversationState = serde_json::from_str(old_json).unwrap();
        assert!(s.cursor_marks.is_empty());
    }
}
