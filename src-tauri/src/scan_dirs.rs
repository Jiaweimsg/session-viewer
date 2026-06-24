//! 额外扫描目录:多个 Claude Code config 目录(多账号场景)的合并扫描配置。
//!
//! 多个 CC 账号各自的 config 在不同的 home 目录(如 `~/.claude-cc-bin`,其下含
//! `projects/`),默认只扫 `~/.claude` 会漏掉它们。这里读取用户在设置页维护的
//! 目录列表,与 `upload-blocklist.json`、`conversation-state.json` 同目录
//! (`{state_dir}/scan-dirs.json`)。
//!
//! 文件格式:
//! ```json
//! { "paths": ["~/.claude-cc-bin", "/Users/x/.claude-work"] }
//! ```
//!
//! 各扫描函数通过 `path_encoder::get_all_projects_dirs()` 在每次扫描时 reload,
//! 编辑文件后下次查询/上报即生效。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanDirs {
    /// 额外的 Claude 目录(绝对路径或 `~/...`)。可填账号根目录(其下含
    /// `projects/`,如 `~/.claude-cc-bin`),也可直接填 `projects` 目录 ——
    /// 扫描时由 `path_encoder::get_all_projects_dirs` 智能识别。
    #[serde(default)]
    pub paths: Vec<String>,
}

fn file() -> Option<PathBuf> {
    let base = dirs::data_dir().or_else(dirs::config_dir)?;
    let dir = base.join("session-viewer");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("scan-dirs.json"))
}

pub fn load() -> ScanDirs {
    let Some(p) = file() else {
        return ScanDirs::default();
    };
    let Ok(content) = std::fs::read_to_string(&p) else {
        return ScanDirs::default();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

pub fn save(dirs: &ScanDirs) -> Result<(), String> {
    let p = file().ok_or_else(|| "no state dir".to_string())?;
    let mut cleaned = ScanDirs::default();
    for raw in &dirs.paths {
        let path = raw.trim();
        if path.is_empty() || cleaned.paths.iter().any(|p| p == path) {
            continue;
        }
        cleaned.paths.push(path.to_string());
    }
    let json = serde_json::to_string_pretty(&cleaned).map_err(|e| e.to_string())?;
    std::fs::write(&p, json).map_err(|e| e.to_string())
}
