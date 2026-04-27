//! 上报黑名单：命中的 cwd 不会上报对话内容（用量照常）。
//!
//! 持久化到 `{state_dir}/upload-blocklist.json`，与 `report-high-water.json`、
//! `conversation-state.json` 同目录。每轮 conversation 上报开始时 reload，
//! 用户在设置页修改后下一轮立即生效。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UploadBlocklist {
    /// 绝对路径前缀；命中（精确等于或位于其子目录）的 cwd 会被排除。
    #[serde(default)]
    pub cwd_prefixes: Vec<String>,
}

fn file() -> Option<PathBuf> {
    let base = dirs::data_dir().or_else(dirs::config_dir)?;
    let dir = base.join("session-viewer");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("upload-blocklist.json"))
}

pub fn load() -> UploadBlocklist {
    let Some(p) = file() else { return UploadBlocklist::default() };
    let Ok(content) = std::fs::read_to_string(&p) else { return UploadBlocklist::default() };
    serde_json::from_str(&content).unwrap_or_default()
}

pub fn save(b: &UploadBlocklist) -> Result<(), String> {
    let p = file().ok_or_else(|| "no state dir".to_string())?;
    let json = serde_json::to_string_pretty(b).map_err(|e| e.to_string())?;
    std::fs::write(&p, json).map_err(|e| e.to_string())
}

/// 标准化：trim 空白、去尾部分隔符、把 Windows `\` 统一替换成 `/`
/// 这样无论用户在设置里贴 `C:\foo\bar` 还是 `C:/foo/bar`，与 scanner
/// 抓到的 cwd 都能在统一字符空间里前缀比较。
fn normalize(p: &str) -> String {
    let trimmed = p.trim().replace('\\', "/");
    trimmed.trim_end_matches('/').to_string()
}

impl UploadBlocklist {
    /// `cwd` 命中黑名单 ⇔ 存在某个 prefix 使得 cwd == prefix 或 cwd 位于 prefix 子树。
    pub fn is_blocked(&self, cwd: &str) -> bool {
        let target = normalize(cwd);
        if target.is_empty() {
            return false;
        }
        self.cwd_prefixes.iter().any(|raw| {
            let prefix = normalize(raw);
            if prefix.is_empty() {
                return false;
            }
            target == prefix || target.starts_with(&format!("{}/", prefix))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bl(prefixes: &[&str]) -> UploadBlocklist {
        UploadBlocklist {
            cwd_prefixes: prefixes.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn empty_blocklist_blocks_nothing() {
        assert!(!bl(&[]).is_blocked("/Users/x/work"));
    }

    #[test]
    fn empty_cwd_is_not_blocked() {
        assert!(!bl(&["/Users/x"]).is_blocked(""));
    }

    #[test]
    fn exact_match_is_blocked() {
        assert!(bl(&["/Users/x/secret"]).is_blocked("/Users/x/secret"));
    }

    #[test]
    fn child_directory_is_blocked() {
        assert!(bl(&["/Users/x/secret"]).is_blocked("/Users/x/secret/sub"));
        assert!(bl(&["/Users/x/secret"]).is_blocked("/Users/x/secret/sub/deeper"));
    }

    #[test]
    fn sibling_with_prefix_is_not_blocked() {
        // 防止 /foo 误命中 /foobar
        assert!(!bl(&["/Users/x/foo"]).is_blocked("/Users/x/foobar"));
    }

    #[test]
    fn unrelated_path_is_not_blocked() {
        assert!(!bl(&["/Users/x/secret"]).is_blocked("/Users/x/public"));
    }

    #[test]
    fn trailing_slash_in_prefix_is_normalized() {
        assert!(bl(&["/Users/x/secret/"]).is_blocked("/Users/x/secret/sub"));
        assert!(bl(&["/Users/x/secret/"]).is_blocked("/Users/x/secret"));
    }

    #[test]
    fn whitespace_in_prefix_is_trimmed() {
        assert!(bl(&["  /Users/x/secret  "]).is_blocked("/Users/x/secret"));
    }

    #[test]
    fn blank_prefix_entry_is_ignored() {
        // 防止前端误传空字符串导致全量屏蔽
        assert!(!bl(&[""]).is_blocked("/Users/x/anything"));
        assert!(!bl(&["   "]).is_blocked("/Users/x/anything"));
    }

    #[test]
    fn multiple_prefixes_any_match() {
        let b = bl(&["/Users/x/a", "/Users/x/b"]);
        assert!(b.is_blocked("/Users/x/a/sub"));
        assert!(b.is_blocked("/Users/x/b"));
        assert!(!b.is_blocked("/Users/x/c"));
    }

    #[test]
    fn windows_backslash_path_matches() {
        // Windows 上 cwd 通常是 "C:\Users\x\secret"，黑名单可能存任一形式。
        assert!(bl(&[r"C:\Users\x\secret"]).is_blocked(r"C:\Users\x\secret\sub"));
        assert!(bl(&["C:/Users/x/secret"]).is_blocked(r"C:\Users\x\secret\sub"));
        assert!(bl(&[r"C:\Users\x\secret"]).is_blocked("C:/Users/x/secret/sub"));
    }

    #[test]
    fn windows_drive_letter_exact_match() {
        assert!(bl(&[r"D:\projects\foo"]).is_blocked(r"D:\projects\foo"));
        assert!(!bl(&[r"D:\projects\foo"]).is_blocked(r"D:\projects\foobar"));
    }
}
