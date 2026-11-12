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
            if success {
                // An MCP helper receives EOF before finish. Let it close its own
                // native runtime (which can own a separate process group).
                let _ = tokio::time::timeout(std::time::Duration::from_secs(2), self.inner.wait())
                    .await;
            }
            crate::providers::terminate(&mut self.inner).await;
            #[cfg(unix)]
            if let Some(group) = self.group {
                // child.id() is already None after an exited/reaped root. Its
                // saved group still owns any surviving or stubborn descendants.
                unsafe {
                    libc::kill(-(group as i32), libc::SIGKILL);
                }
            }
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use tokio::io::{AsyncBufReadExt, BufReader};

    #[tokio::test]
    async fn finishing_an_exited_service_stops_its_remaining_process_group() {
        let mut command = command("sh");
        command
            .args(["-c", "trap '' TERM; sleep 60 & echo $!"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        let mut service = ServiceChild::spawn(command).unwrap();
        let mut lines = BufReader::new(service.stdout.take().unwrap()).lines();
        let descendant = lines
            .next_line()
            .await
            .unwrap()
            .unwrap()
            .trim()
            .parse::<i32>()
            .unwrap();
        service.inner.wait().await.unwrap();
        assert!(
            service.inner.id().is_none(),
            "Root must be reaped before exercising cleanup"
        );
        service.finish(true).await.unwrap();
        let mut stopped = false;
        for _ in 0..50 {
            let status = super::command("ps")
                .args(["-o", "stat=", "-p", &descendant.to_string()])
                .output()
                .await
                .unwrap();
            let state = String::from_utf8_lossy(&status.stdout);
            // An orphan may remain as an OS-owned zombie until reaped; it can
            // no longer execute or hold the service's files and connections.
            if state.trim().is_empty() || state.trim().starts_with('Z') {
                stopped = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        if !stopped {
            unsafe {
                libc::kill(descendant, libc::SIGKILL);
            }
        }
        assert!(stopped, "Exited service left a running descendant");
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
