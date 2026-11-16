//! Child processes started by Lumen. On Windows every one runs without a console window, so
//! background work never flashes a terminal.
use std::path::Path;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// An async command with no console window.
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> tokio::process::Command {
    tokio::process::Command::from(std_command(program))
}

/// A blocking command with no console window.
pub fn std_command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    #[cfg(unix)]
    if let Some(command) = owned::command(program.as_ref()) {
        return command;
    }
    #[allow(unused_mut)]
    let mut c = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

/// A pipe lifetime, rather than a recorded PID, identifies the daemon to its Unix children.
#[cfg(unix)]
mod owned {
    use std::{
        os::unix::process::CommandExt,
        sync::atomic::{AtomicI32, Ordering},
    };
    static READER: AtomicI32 = AtomicI32::new(-1);
    pub fn enable() -> Result<(), String> {
        unsafe {
            let mut fds = [-1; 2];
            if libc::pipe(fds.as_mut_ptr()) != 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            for fd in fds {
                if libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) < 0 {
                    libc::close(fds[0]);
                    libc::close(fds[1]);
                    return Err(std::io::Error::last_os_error().to_string());
                }
            }
            READER.store(fds[0], Ordering::SeqCst);
            // The write end stays open only in the daemon and closes in the kernel on any exit.
            Ok(())
        }
    }
    pub fn command(program: &std::ffi::OsStr) -> Option<std::process::Command> {
        let fd = READER.load(Ordering::SeqCst);
        if fd < 0 {
            return None;
        }
        let mut command =
            std::process::Command::new(std::env::current_exe().expect("Daemon executable"));
        command
            .arg("--lumen-owned-child")
            .arg(fd.to_string())
            .arg(program)
            .process_group(0);
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        Some(command)
    }
    pub fn run() -> Result<i32, String> {
        let args: Vec<_> = std::env::args_os().collect();
        let fd = args
            .get(2)
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<i32>().ok())
            .filter(|fd| *fd >= 3)
            .ok_or("Missing owner pipe")?;
        let program = args.get(3).ok_or("Missing child executable")?;
        unsafe {
            if libc::getpgrp() != libc::getpid() {
                return Err("Owned child must lead its private process group".into());
            }
            if libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) < 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
        }
        let mut guard =
            std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
        guard
            .arg("--lumen-group-guard")
            .arg(fd.to_string())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        unsafe {
            guard.pre_exec(move || {
                if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        // The guard anchors this group until the wrapper exits or the owner pipe closes.
        let mut guard = guard.spawn().map_err(|e| e.to_string())?;
        use std::io::Read;
        let mut ready = [0; 6];
        guard
            .stdout
            .take()
            .ok_or("Missing guard handshake")?
            .read_exact(&mut ready)
            .map_err(|e| e.to_string())?;
        if &ready != b"READY\n" {
            return Err("Process guard did not start".into());
        }
        let status = std::process::Command::new(program)
            .args(&args[4..])
            .status()
            .map_err(|e| e.to_string())?;
        use std::os::unix::process::ExitStatusExt;
        Ok(status
            .code()
            .unwrap_or_else(|| 128 + status.signal().unwrap_or(1)))
    }
    pub fn guard() -> Result<i32, String> {
        let fd = std::env::args()
            .nth(2)
            .and_then(|s| s.parse::<i32>().ok())
            .filter(|fd| *fd >= 3)
            .ok_or("Missing owner pipe")?;
        unsafe {
            let parent = libc::getppid();
            let group = libc::getpgrp();
            if parent != group || group == libc::getpid() {
                return Err("Guard must belong to its supervising parent's group".into());
            }
            libc::signal(libc::SIGTERM, libc::SIG_IGN);
            libc::signal(libc::SIGHUP, libc::SIG_IGN);
            use std::io::Write;
            std::io::stdout()
                .write_all(b"READY\n")
                .map_err(|e| e.to_string())?;
            let mut poll = libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            };
            loop {
                let result = libc::poll(&mut poll, 1, 10);
                if libc::getppid() != parent || result > 0 {
                    libc::kill(0, libc::SIGKILL);
                    return Ok(1);
                }
                if result < 0
                    && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted
                {
                    return Err(std::io::Error::last_os_error().to_string());
                }
            }
        }
    }
}
#[cfg(unix)]
pub fn own_processes() -> Result<(), String> {
    owned::enable()
}
#[cfg(unix)]
pub fn run_owned_child() -> Result<i32, String> {
    owned::run()
}
#[cfg(unix)]
pub fn run_group_guard() -> Result<i32, String> {
    owned::guard()
}

/// An integration owns its whole subprocess tree, including descendants of an exited root.
pub struct ServiceChild {
    pub stdin: Option<tokio::process::ChildStdin>,
    pub stdout: Option<tokio::process::ChildStdout>,
    pub stderr: Option<tokio::process::ChildStderr>,
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
            stderr: inner.stderr.take(),
            inner,
            #[cfg(unix)]
            group,
            finished: false,
        })
    }
    pub fn id(&mut self) -> Option<u32> {
        self.inner.id()
    }
    pub async fn wait(&mut self) -> Result<std::process::ExitStatus, String> {
        #[cfg(windows)]
        {
            self.inner.wait().await
        }
        #[cfg(not(windows))]
        {
            self.inner.wait().await.map_err(|e| e.to_string())
        }
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
