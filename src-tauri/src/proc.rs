//! Child processes started by Lumen. On Windows every one runs without a console window, so
//! background work never flashes a terminal.
use std::path::Path;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// An async command with no console window.
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> tokio::process::Command {
    #[allow(unused_mut)]
    let mut c = tokio::process::Command::new(program);
    #[cfg(windows)]
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

/// A blocking command with no console window.
pub fn std_command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    #[allow(unused_mut)]
    let mut c = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

/// `cmd` through the platform shell (`cmd.exe /C` or `sh -c`), in `cwd`, with no stdin.
pub fn shell(cmd: &str, cwd: &Path) -> tokio::process::Command {
    #[cfg(windows)]
    let mut c = {
        let mut c = command("cmd.exe");
        c.args(["/D", "/S", "/C", cmd]);
        c
    };
    #[cfg(not(windows))]
    let mut c = {
        let mut c = command("sh");
        c.args(["-c", cmd]);
        c
    };
    c.current_dir(cwd).stdin(std::process::Stdio::null());
    c
}

/// Force-stops a process and its children (Windows `taskkill /T /F`).
#[cfg(windows)]
pub async fn taskkill(pid: u32) {
    let _ = command("taskkill.exe")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .output()
        .await;
}
