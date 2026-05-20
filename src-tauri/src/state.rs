use lru::LruCache;
use parking_lot::Mutex;
use std::num::NonZeroUsize;

use crate::report::RankingPayload;
use crate::shared_models::DisplayMessage;

/// Cached stats result with timestamp
pub struct CachedStats {
    pub stats_json: serde_json::Value,
    pub token_summary_json: serde_json::Value,
    pub advanced_stats_json: serde_json::Value,
    pub cached_at: std::time::Instant,
}

/// Application state shared across Tauri commands
#[allow(dead_code)]
pub struct AppState {
    /// LRU cache for parsed session messages (key: "encodedName/sessionId")
    pub message_cache: Mutex<LruCache<String, Vec<DisplayMessage>>>,
    /// Stats cache per tool (key: tool name)
    pub stats_cache: Mutex<std::collections::HashMap<String, CachedStats>>,
    /// Most recent ranking payload from the report server. Populated whenever
    /// `send_all_reports` returns a non-None ranking — both from the background
    /// auto-report loop and from the manual `report_usage` Tauri command.
    /// Empty at startup until the first report finishes.
    pub latest_ranking: Mutex<Option<RankingPayload>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            message_cache: Mutex::new(LruCache::new(NonZeroUsize::new(20).unwrap())),
            stats_cache: Mutex::new(std::collections::HashMap::new()),
            latest_ranking: Mutex::new(None),
        }
    }

    /// Invalidate stats cache for a specific tool
    pub fn invalidate_stats(&self, tool: &str) {
        self.stats_cache.lock().remove(tool);
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
