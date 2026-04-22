//! Self-update command (stub — will be wired to tauri-plugin-updater in Phase N).
//!
//! When invoked, triggers the auto-update flow: check server manifest,
//! download signed artifact, verify signature, install, relaunch.
//!
//! Current implementation returns an error since the updater pipeline
//! (signing key, GitHub Actions, release hosting) hasn't been set up.

#[tauri::command]
pub async fn start_self_update() -> Result<(), String> {
    eprintln!("[Updater] start_self_update invoked (Phase N not yet wired)");
    Err("更新流程尚未配置。请稍候或联系管理员。".to_string())
}
