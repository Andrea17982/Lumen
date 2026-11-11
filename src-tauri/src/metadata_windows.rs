//! Metadata-only process lifetime. Assignment happens while suspended, before descendants
//! can escape supervision; kill-on-close covers task abortion as well as ordinary cleanup.
use process_wrap::tokio::{ChildWrapper, CommandWrap, CreationFlags, JobObject, KillOnDrop};
use tokio::{
    process::{ChildStderr, ChildStdin, ChildStdout, Command},
    sync::watch,
};
use windows::Win32::System::Threading::CREATE_NO_WINDOW;

pub struct MetadataChild {
    pub stdin: Option<ChildStdin>,
    pub stdout: Option<ChildStdout>,
    pub stderr: Option<ChildStderr>,
    child: Box<dyn ChildWrapper>,
}

pub struct Cleanup {
    pub graceful: bool,
    pub cancelled: bool,
}
impl MetadataChild {
    fn root(&mut self) -> Result<&mut tokio::process::Child, String> {
        // SAFETY: private access is used only to wait/observe root exit. We never move or
        // replace the native child, steal its handles, or bypass the job for termination.
        unsafe { self.child.try_inner_child_mut() }
            .ok_or("Metadata process handle unavailable".into())
    }
    pub fn spawn(command: Command) -> std::io::Result<Self> {
        let mut command = CommandWrap::from(command);
        command
            .wrap(CreationFlags(CREATE_NO_WINDOW))
            .wrap(KillOnDrop)
            .wrap(JobObject);
        let mut child = command.spawn()?;
        Ok(Self {
            stdin: child.stdin().take(),
            stdout: child.stdout().take(),
            stderr: child.stderr().take(),
            child,
        })
    }
    pub async fn finish(
        &mut self,
        success: bool,
        mut cancel: Option<watch::Receiver<bool>>,
    ) -> Result<Cleanup, String> {
        let mut cancelled = cancel.as_ref().is_some_and(|rx| *rx.borrow());
        let mut graceful = false;
        if success && !cancelled {
            // Wait only for the root here: waiting on the whole job before forcing it can
            // deadlock on an orphaned descendant. The first full job wait happens below.
            let root = self.root()?;
            let cancellation = async {
                if let Some(rx) = &mut cancel {
                    let _ = rx.changed().await;
                } else {
                    std::future::pending::<()>().await;
                }
            };
            tokio::select! {
                biased;
                _ = cancellation => { cancelled = true; }
                status = root.wait() => { graceful = status.is_ok(); }
                _ = tokio::time::sleep(std::time::Duration::from_millis(80)) => {}
            }
        }
        // Native job termination also catches descendants after the root has exited.
        self.child
            .start_kill()
            .map_err(|e| format!("Cannot stop metadata process group: {e}"))?;
        self.child
            .wait()
            .await
            .map_err(|e| format!("Cannot reap metadata process group: {e}"))?;
        cancelled |= cancel.as_ref().is_some_and(|rx| *rx.borrow());
        Ok(Cleanup {
            graceful,
            cancelled,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncBufReadExt, BufReader};

    async fn fixture(with_descendant: bool, ignore_eof: bool) -> (MetadataChild, Vec<u32>) {
        let spawn = if with_descendant {
            "$p=Start-Process powershell.exe -ArgumentList @('-NoProfile','-NonInteractive','-Command','Start-Sleep -Seconds 60') -WindowStyle Hidden -PassThru; $ids=\"$PID $($p.Id)\";"
        } else {
            "$ids=\"$PID\";"
        };
        let tail = if ignore_eof {
            "Start-Sleep -Seconds 60"
        } else {
            "[Console]::In.ReadToEnd() | Out-Null"
        };
        let script =
            format!("{spawn} [Console]::Out.WriteLine($ids); [Console]::Out.Flush(); {tail}");
        let mut command = crate::proc::command("powershell.exe");
        command
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        let mut child = MetadataChild::spawn(command).unwrap();
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let line = tokio::time::timeout(std::time::Duration::from_secs(10), lines.next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let ids = line
            .split_whitespace()
            .map(|p| p.parse::<u32>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(ids.len(), if with_descendant { 2 } else { 1 });
        (child, ids)
    }
    async fn gone(ids: &[u32]) {
        let ids = ids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
        let script = format!("if(Get-Process -Id {ids} -ErrorAction SilentlyContinue){{exit 1}}");
        for _ in 0..10 {
            let status = crate::proc::command("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .status()
                .await
                .unwrap();
            if status.success() {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        }
        panic!("Owned metadata processes did not exit");
    }
    #[tokio::test]
    async fn metadata_eof_exits_without_a_taskkill_process() {
        let (mut child, ids) = fixture(false, false).await;
        drop(child.stdin.take());
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            child.root().unwrap().wait(),
        )
        .await
        .unwrap()
        .unwrap();
        let result = child.finish(true, None).await.unwrap();
        assert!(result.graceful && !result.cancelled);
        gone(&ids).await;
    }
    #[tokio::test]
    async fn metadata_stubborn_tree_is_forced_and_reaped() {
        let (mut child, ids) = fixture(true, true).await;
        drop(child.stdin.take());
        let result = child.finish(true, None).await.unwrap();
        assert!(!result.graceful && !result.cancelled);
        gone(&ids).await;
    }
    #[tokio::test]
    async fn metadata_child_is_cleaned_after_the_parent_exits() {
        let (mut child, ids) = fixture(true, false).await;
        drop(child.stdin.take());
        // Establish parent exit before group cleanup, leaving the descendant in its job.
        child.root().unwrap().wait().await.unwrap();
        child.finish(true, None).await.unwrap();
        gone(&ids).await;
    }
    #[tokio::test]
    async fn metadata_cancellation_during_grace_stops_the_tree() {
        let (mut child, ids) = fixture(true, true).await;
        drop(child.stdin.take());
        let (tx, rx) = watch::channel(false);
        let mut cleanup = Box::pin(child.finish(true, Some(rx)));
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(cleanup.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        tx.send(true).unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), cleanup)
            .await
            .unwrap()
            .unwrap();
        assert!(result.cancelled && !result.graceful);
        gone(&ids).await;
    }
    #[tokio::test]
    async fn metadata_abort_kills_only_its_own_job() {
        let (child, ids) = fixture(true, true).await;
        let (mut other, other_ids) = fixture(false, true).await;
        let task = tokio::spawn(async move {
            let _guard = child;
            std::future::pending::<()>().await;
        });
        task.abort();
        let _ = task.await;
        gone(&ids).await;
        assert!(other.root().unwrap().try_wait().unwrap().is_none());
        other.finish(false, None).await.unwrap();
        gone(&other_ids).await;
    }
    #[tokio::test]
    async fn catalog_query_cancellation_and_abort_leave_no_descendants() {
        for abort in [false, true] {
            let file = std::env::temp_dir()
                .join(format!("lumen-catalog-pids-{}.txt", uuid::Uuid::new_v4()));
            let quoted = file.to_string_lossy().replace('\'', "''");
            let script = format!("$p=Start-Process powershell.exe -ArgumentList @('-NoProfile','-NonInteractive','-Command','Start-Sleep -Seconds 60') -WindowStyle Hidden -PassThru; Set-Content -LiteralPath '{quoted}' -Value \"$PID $($p.Id)\"; [Console]::In.ReadLine() | Out-Null; Start-Sleep -Seconds 60");
            let mut command = crate::proc::command("powershell.exe");
            command
                .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .kill_on_drop(true);
            let (tx, rx) = watch::channel(false);
            let task = tokio::spawn(async move {
                crate::catalog::native_query(
                    command,
                    &crate::models::Provider::Claude,
                    Some(rx),
                    |_| {},
                )
                .await
            });
            let text = tokio::time::timeout(std::time::Duration::from_secs(10), async {
                loop {
                    if let Ok(text) = std::fs::read_to_string(&file) {
                        if text.split_whitespace().count() == 2 {
                            break text;
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
            })
            .await
            .unwrap();
            let ids = text
                .split_whitespace()
                .map(|p| p.parse().unwrap())
                .collect::<Vec<u32>>();
            if abort {
                task.abort();
                let _ = task.await;
            } else {
                tx.send(true).unwrap();
                assert_eq!(
                    tokio::time::timeout(std::time::Duration::from_secs(5), task)
                        .await
                        .unwrap()
                        .unwrap()
                        .unwrap_err(),
                    "Cancelled"
                );
            }
            gone(&ids).await;
            std::fs::remove_file(file).unwrap();
        }
    }
}
