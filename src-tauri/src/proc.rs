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

/// An integration owns its whole subprocess tree, including descendants of an exited root.
pub struct ServiceChild {
    pub stdin: Option<tokio::process::ChildStdin>,
    pub stdout: Option<tokio::process::ChildStdout>,
    #[cfg(windows)]
    inner: crate::metadata_windows::MetadataChild,
    #[cfg(not(windows))]
    inner: tokio::process::Child,
    #[cfg(unix)]
    group: Option<u32>,
    finished: bool,
}
impl ServiceChild {
    pub fn spawn(mut command: tokio::process::Command) -> std::io::Result<Self> {
        #[cfg(unix)]
        command.process_group(0);
        command.kill_on_drop(true);
        #[cfg(windows)]
        let mut inner = crate::metadata_windows::MetadataChild::spawn(command)?;
        #[cfg(not(windows))]
        let mut inner = command.spawn()?;
        #[cfg(unix)]
        let group = inner.id();
        Ok(Self {
            stdin: inner.stdin.take(),
            stdout: inner.stdout.take(),
            inner,
            #[cfg(unix)]
            group,
            finished: false,
        })
    }
    pub async fn finish(&mut self, success: bool) -> Result<(), String> {
        #[cfg(windows)]
        {
            self.inner.finish(success, None).await?;
        }
        #[cfg(not(windows))]
        {
            let _ = success;
            crate::providers::terminate(&mut self.inner).await;
        }
        self.finished = true;
        Ok(())
    }
}
impl Drop for ServiceChild {
    fn drop(&mut self) {
        #[cfg(unix)]
        if !self.finished {
            if let Some(group) = self.group {
                unsafe {
                    libc::kill(-(group as i32), libc::SIGKILL);
                }
            }
        }
        // Windows KillOnDrop closes the owned Job Object even when cleanup is aborted.
    }
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
