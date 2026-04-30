/// 在 Cursor 中打开指定 workspace 目录。Cursor 没有"恢复单个会话"的 CLI
/// 接口，只能打开 workspace 让用户在 IDE 内手动切到目标 chat。
///
/// 平台策略：
/// - macOS: `open -a Cursor "<cwd>"` —— 走 LaunchServices，不要求 cursor CLI
///   在 PATH（绝大部分 mac 用户没主动 Install 'cursor' command）
/// - Windows: `cmd /D /C cursor "<cwd>"` —— 让 cmd 解析 `cursor.cmd` / `cursor.bat`
///   这类 PATHEXT shim；直接 `Command::new("cursor")` 可能找不到 `.cmd`
/// - Linux: 直接 `cursor "<cwd>"`
pub fn resume_session(_session_id: String, cwd: String) -> Result<(), String> {
    let cwd = normalize_cursor_workspace_path(cwd.trim());
    if cwd.is_empty() {
        return Err("workspace path is empty".into());
    }
    if !std::path::Path::new(&cwd).exists() {
        return Err(format!("workspace path does not exist: {}", cwd));
    }

    #[cfg(target_os = "windows")]
    ensure_cursor_command_available()?;

    let mut cmd = build_open_cursor_cmd(&cwd);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    cmd.spawn().map(|_| ()).map_err(|e| {
        let hint = cursor_launch_hint();
        format!("无法启动 Cursor：{}。{}", e, hint)
    })
}

#[cfg(target_os = "macos")]
fn cursor_launch_hint() -> &'static str {
    "请确认 Cursor 已安装到 /Applications"
}

#[cfg(not(target_os = "macos"))]
fn cursor_launch_hint() -> &'static str {
    "请确认 'cursor' 命令已在 PATH（Cursor → Settings → Install 'cursor' command）"
}

#[cfg(target_os = "windows")]
fn ensure_cursor_command_available() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let output = std::process::Command::new("cmd")
        .args(["/D", "/C", "where", "cursor"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("无法启动 Cursor：{}。{}", e, cursor_launch_hint()))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "无法启动 Cursor：program not found。{}",
            cursor_launch_hint()
        ))
    }
}

#[cfg(target_os = "macos")]
fn build_open_cursor_cmd(cwd: &str) -> std::process::Command {
    command_from_spec(build_open_cursor_cmd_spec(cwd, "macos"))
}

#[cfg(target_os = "windows")]
fn build_open_cursor_cmd(cwd: &str) -> std::process::Command {
    command_from_spec(build_open_cursor_cmd_spec(cwd, "windows"))
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn build_open_cursor_cmd(cwd: &str) -> std::process::Command {
    command_from_spec(build_open_cursor_cmd_spec(cwd, "linux"))
}

struct OpenCursorCmdSpec {
    program: String,
    args: Vec<String>,
}

fn build_open_cursor_cmd_spec(cwd: &str, target_os: &str) -> OpenCursorCmdSpec {
    match target_os {
        "macos" => OpenCursorCmdSpec {
            program: "open".to_string(),
            args: vec!["-a".to_string(), "Cursor".to_string(), cwd.to_string()],
        },
        "windows" => OpenCursorCmdSpec {
            program: "cmd".to_string(),
            args: vec![
                "/D".to_string(),
                "/C".to_string(),
                "cursor".to_string(),
                cwd.to_string(),
            ],
        },
        _ => OpenCursorCmdSpec {
            program: "cursor".to_string(),
            args: vec![cwd.to_string()],
        },
    }
}

fn normalize_cursor_workspace_path(path: &str) -> String {
    let decoded = percent_decode(path);
    if cfg!(windows) {
        let without_uri_drive_slash = if decoded.len() >= 3
            && decoded.as_bytes()[0] == b'/'
            && decoded.as_bytes()[2] == b':'
            && decoded.as_bytes()[1].is_ascii_alphabetic()
        {
            &decoded[1..]
        } else {
            decoded.as_str()
        };
        without_uri_drive_slash.replace('/', "\\")
    } else {
        decoded
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                out.push(hex);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn command_from_spec(spec: OpenCursorCmdSpec) -> std::process::Command {
    let mut cmd = std::process::Command::new(spec.program);
    cmd.args(spec.args);
    cmd
}

#[cfg(test)]
mod tests {
    #[test]
    fn windows_uses_cmd_shell_to_resolve_cursor_shims() {
        let spec = super::build_open_cursor_cmd_spec("C:\\Users\\me\\project", "windows");

        assert_eq!(spec.program, "cmd");
        assert_eq!(
            spec.args,
            vec!["/D", "/C", "cursor", "C:\\Users\\me\\project"]
        );
    }

    #[test]
    fn linux_uses_cursor_directly() {
        let spec = super::build_open_cursor_cmd_spec("/home/me/project", "linux");

        assert_eq!(spec.program, "cursor");
        assert_eq!(spec.args, vec!["/home/me/project"]);
    }

    #[cfg(windows)]
    #[test]
    fn windows_normalizes_uri_drive_path_to_local_path() {
        let path = super::normalize_cursor_workspace_path("/D%3A/Code/github/session-viewer");

        assert_eq!(path, "D:\\Code\\github\\session-viewer");
    }

    #[cfg(windows)]
    #[test]
    fn windows_normalizes_plain_drive_path_slashes() {
        let path = super::normalize_cursor_workspace_path("D:/Code/github/session-viewer");

        assert_eq!(path, "D:\\Code\\github\\session-viewer");
    }
}
