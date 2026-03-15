use std::process::Command;

/// Resume a Copilot session by opening VS Code in the workspace directory.
/// Since GitHub Copilot runs inside VS Code, we just open VS Code in the workspace.
pub fn resume_session(_session_id: String, work_dir: String) -> Result<(), String> {
    let work_dir = normalize_path(&work_dir);

    if !std::path::Path::new(&work_dir).exists() {
        return Err(format!("Working directory does not exist: {}", work_dir));
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        Command::new("cmd")
            .args(["/c", "code", &work_dir])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Failed to open VS Code: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .args(["-a", "Visual Studio Code", &work_dir])
            .spawn()
            .map_err(|e| format!("Failed to open VS Code: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("code")
            .arg(&work_dir)
            .spawn()
            .map_err(|e| format!("Failed to open VS Code: {}", e))?;
    }

    Ok(())
}

fn normalize_path(path: &str) -> String {
    if cfg!(windows) {
        path.replace('/', "\\")
    } else {
        path.replace('\\', "/")
    }
}
