use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEntry {
    pub cwd: String,
    pub short_name: String,
    pub session_count: u32,
    pub last_modified: Option<String>,
    pub model_provider: Option<String>,
}
