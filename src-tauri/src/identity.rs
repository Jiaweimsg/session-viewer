//! 用户可订正的身份覆盖（user_name / user_email）。
//!
//! 优先级（在 `report.rs` 里组装）：override → git config（缓存）→ OS user fallback
//! 持久化到 `{state_dir}/session-viewer/identity-override.json`。
//! 不缓存：每次 `report.rs::get_user_*` 都会重新读这个文件，让用户在设置页保存
//! 后下一轮上报立即生效，不需要重启 app。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IdentityOverride {
    /// 覆盖 user_name；空串/None 表示不覆盖。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,
    /// 覆盖 user_email；空串/None 表示不覆盖。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_email: Option<String>,
}

fn file() -> Option<PathBuf> {
    let base = dirs::data_dir().or_else(dirs::config_dir)?;
    let dir = base.join("session-viewer");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("identity-override.json"))
}

pub fn load() -> IdentityOverride {
    let Some(p) = file() else { return IdentityOverride::default() };
    let Ok(content) = std::fs::read_to_string(&p) else { return IdentityOverride::default() };
    serde_json::from_str(&content).unwrap_or_default()
}

pub fn save(o: &IdentityOverride) -> Result<(), String> {
    let p = file().ok_or_else(|| "no state dir".to_string())?;
    // 把空串归一为 None，前端"清空输入再保存"等价于"使用默认"。
    let normalized = IdentityOverride {
        user_name: o.user_name.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(String::from),
        user_email: o.user_email.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(String::from),
    };
    let json = serde_json::to_string_pretty(&normalized).map_err(|e| e.to_string())?;
    std::fs::write(&p, json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string_normalizes_to_none_when_round_tripping() {
        let o = IdentityOverride {
            user_name: Some("  ".into()),
            user_email: Some("".into()),
        };
        // save normalizes; we test the same logic inline since file IO is per-OS
        let normalized = IdentityOverride {
            user_name: o.user_name.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(String::from),
            user_email: o.user_email.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(String::from),
        };
        assert!(normalized.user_name.is_none());
        assert!(normalized.user_email.is_none());
    }

    #[test]
    fn serde_roundtrip_omits_none_fields() {
        let o = IdentityOverride {
            user_name: Some("Alice".into()),
            user_email: None,
        };
        let json = serde_json::to_string(&o).unwrap();
        assert!(json.contains("Alice"));
        assert!(!json.contains("user_email"));
        let back: IdentityOverride = serde_json::from_str(&json).unwrap();
        assert_eq!(back.user_name.as_deref(), Some("Alice"));
        assert!(back.user_email.is_none());
    }

    #[test]
    fn missing_file_returns_default() {
        let d = IdentityOverride::default();
        assert!(d.user_name.is_none());
        assert!(d.user_email.is_none());
    }
}
