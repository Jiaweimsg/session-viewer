/// 在 Cursor 中打开指定 workspace 目录。Cursor 没有"恢复单个会话"的 CLI
/// 接口，只能打开 workspace 让用户在 IDE 内手动切到目标 chat。
///
/// 平台策略：
/// - macOS: `open -a Cursor "<cwd>"` —— 走 LaunchServices，不要求 cursor CLI
///   在 PATH（绝大部分 mac 用户没主动 Install 'cursor' command）
/// - Windows / Linux: 直接 `cursor "<cwd>"` —— 安装包默认会把 cursor 加进 PATH
pub fn resume_session(_session_id: String, cwd: String) -> Result<(), String> {
    let cwd = cwd.trim();
    if cwd.is_empty() {
        return Err("workspace path is empty".into());
    }

    let mut cmd = build_open_cursor_cmd(cwd);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    cmd.spawn().map(|_| ()).map_err(|e| {
        #[cfg(target_os = "macos")]
        let hint = "请确认 Cursor 已安装到 /Applications";
        #[cfg(not(target_os = "macos"))]
        let hint = "请确认 'cursor' 命令已在 PATH（Cursor → Settings → Install 'cursor' command）";
        format!("无法启动 Cursor：{}。{}", e, hint)
    })
}

#[cfg(target_os = "macos")]
fn build_open_cursor_cmd(cwd: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new("open");
    cmd.args(["-a", "Cursor", cwd]);
    cmd
}

#[cfg(not(target_os = "macos"))]
fn build_open_cursor_cmd(cwd: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new("cursor");
    cmd.arg(cwd);
    cmd
}
