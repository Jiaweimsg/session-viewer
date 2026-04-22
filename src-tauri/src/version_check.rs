//! Server-driven minimum-version enforcement.
//!
//! On startup the client fetches `GET {server}/api/config`. The response's
//! `min_client_version` is compared against this build's `CARGO_PKG_VERSION`.
//! If the running client is too old:
//!   - the shared `upload_blocked` flag flips to `true`, stopping both the
//!     metrics and conversation upload loops on their next iteration
//!   - a Tauri event `force-update` is emitted carrying
//!     `{current, min_required}`, which the frontend overlay listens for
//!
//! Fail-open: any network / parse error leaves `upload_blocked = false` —
//! we never block the client on transient server problems.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Deserialize)]
struct ServerConfig {
    #[serde(default)]
    min_client_version: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ForceUpdatePayload {
    pub current: String,
    pub min_required: String,
}

/// Return true iff `current` >= `required` under simple dotted semver.
/// Missing components are treated as 0. Non-numeric segments silently become 0.
pub fn version_at_least(current: &str, required: &str) -> bool {
    let parse = |s: &str| -> Vec<u32> {
        s.split('.')
            .map(|p| p.parse::<u32>().unwrap_or(0))
            .collect()
    };
    let a = parse(current);
    let b = parse(required);
    let len = a.len().max(b.len());
    for i in 0..len {
        let av = a.get(i).copied().unwrap_or(0);
        let bv = b.get(i).copied().unwrap_or(0);
        if av > bv {
            return true;
        }
        if av < bv {
            return false;
        }
    }
    true // equal
}

/// One-shot version check. Non-blocking on all errors (returns Ok(false)).
/// Returns Ok(true) if the client is up-to-date (or the server doesn't set a
/// minimum), Ok(false) if the client is too old, or Err on anything else — but
/// callers should treat Err the same as Ok(true) (fail-open).
async fn fetch_and_compare(server_url: &str, current: &str) -> Result<(bool, String), String> {
    let url = format!("{}/api/config", server_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .map_err(|e| format!("client build: {}", e))?;
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("request: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let cfg: ServerConfig = resp
        .json()
        .await
        .map_err(|e| format!("parse: {}", e))?;
    let min = cfg.min_client_version.trim().to_string();
    if min.is_empty() {
        // Server disables enforcement.
        return Ok((true, min));
    }
    Ok((version_at_least(current, &min), min))
}

/// Entry point: kick off a version check, flipping `upload_blocked` and
/// emitting a Tauri event if the client is too old. Fail-open otherwise.
/// Edge-triggered: the `force-update` event fires only when we transition
/// from unblocked → blocked, and `force-update-cleared` on the reverse.
pub async fn enforce_min_version(
    server_url: &str,
    app: AppHandle,
    upload_blocked: Arc<AtomicBool>,
) {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let was_blocked = upload_blocked.load(Ordering::SeqCst);
    match fetch_and_compare(server_url, &current).await {
        Ok((true, _)) => {
            upload_blocked.store(false, Ordering::SeqCst);
            if was_blocked {
                eprintln!(
                    "[VersionCheck] client v{} now at or above min; unblocking",
                    current
                );
                let _ = app.emit("force-update-cleared", ());
            } else {
                eprintln!("[VersionCheck] OK ({})", current);
            }
        }
        Ok((false, min_required)) => {
            upload_blocked.store(true, Ordering::SeqCst);
            let payload = ForceUpdatePayload {
                current: current.clone(),
                min_required: min_required.clone(),
            };
            if !was_blocked {
                eprintln!(
                    "[VersionCheck] client v{} < required v{}; blocking uploads",
                    current, min_required
                );
                if let Err(e) = app.emit("force-update", payload) {
                    eprintln!("[VersionCheck] emit failed: {}", e);
                }
            }
        }
        Err(e) => {
            // Fail-open: don't flip the flag on transient failures.
            eprintln!("[VersionCheck] skipped (fail-open): {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_equal_is_at_least() {
        assert!(version_at_least("0.5.2", "0.5.2"));
    }

    #[test]
    fn newer_is_at_least() {
        assert!(version_at_least("0.5.3", "0.5.2"));
        assert!(version_at_least("1.0.0", "0.9.99"));
        assert!(version_at_least("0.6.0", "0.5.999"));
    }

    #[test]
    fn older_is_not_at_least() {
        assert!(!version_at_least("0.5.1", "0.5.2"));
        assert!(!version_at_least("0.4.999", "0.5.0"));
    }

    #[test]
    fn missing_components_default_to_zero() {
        assert!(version_at_least("0.5", "0.5.0"));
        assert!(version_at_least("1", "0.99.99"));
        assert!(!version_at_least("0.5", "0.5.1"));
    }

    #[test]
    fn garbage_segments_treated_as_zero() {
        // "0.5.abc" → [0, 5, 0], compared to [0, 5, 0] → equal → at_least
        assert!(version_at_least("0.5.abc", "0.5.0"));
        assert!(!version_at_least("0.5.abc", "0.5.1"));
    }
}
