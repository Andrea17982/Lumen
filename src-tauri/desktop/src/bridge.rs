//! The shell's only link to the daemon. It starts the bundled service when none is listening,
//! relays `__events` to the webview, reconnects with backoff and passes commands through
//! `lumen_ipc::Client::call`, so receipts and the single retry stay in tested Rust code.
//! It owns no execution, approvals or protocol decisions.

use serde::Serialize;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};

/// Every daemon event reaches the webview on this one channel, in order, as `{event, payload}`.
pub const CHANNEL: &str = "lumen:event";
/// The daemon sends a heartbeat every 2 s; three missed ones mean the stream is dead.
const SILENCE: Duration = Duration::from_secs(6);
const FIRST_RETRY: Duration = Duration::from_millis(250);
const MAX_RETRY: Duration = Duration::from_secs(4);
/// A spawned service needs time to open its endpoint; do not start a second one meanwhile.
const SPAWN_GRACE: Duration = Duration::from_secs(10);
/// The service's stderr for the latest start, so a failed start can say why.
const SERVICE_LOG: &str = "service.log";

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum Connection {
    Connecting,
    Connected,
    Reconnecting,
    /// Another build of the service owns the data directory.
    Mismatch {
        detail: String,
    },
    /// Nothing will change without the user, for example a missing service binary.
    Failed {
        detail: String,
    },
}

pub struct Bridge {
    client: lumen_ipc::Client,
    daemon: PathBuf,
    root: PathBuf,
    resources: PathBuf,
    state: Mutex<Connection>,
    paused: std::sync::atomic::AtomicBool,
}

impl Bridge {
    pub fn new(root: PathBuf, resources: PathBuf, daemon: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let build = lumen_ipc::binary_digest(&daemon)
            .map_err(|e| format!("Missing Lumen service at {}: {e}", daemon.display()))?;
        Ok(Self {
            client: lumen_ipc::Client {
                endpoint: lumen_ipc::endpoint(&root)?,
                build,
            },
            daemon,
            root,
            resources,
            state: Mutex::new(Connection::Connecting),
            paused: Default::default(),
        })
    }

    pub fn connection(&self) -> Connection {
        self.state
            .lock()
            .map(|s| s.clone())
            .unwrap_or(Connection::Connecting)
    }

    fn set(&self, app: &AppHandle, next: Connection) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if *state != next {
            *state = next.clone();
            drop(state);
            let _ = app.emit(
                CHANNEL,
                json!({"event": "lumen:connection", "payload": next}),
            );
        }
    }

    pub async fn call(&self, command: &str, args: Value) -> Result<Value, String> {
        if !allowed(command) {
            return Err(format!("Unknown command {command}"));
        }
        self.client.call(command, args).await
    }

    /// The tray's explicit Quit: the service drains and stops. Never reachable from the page.
    pub async fn stop_service(&self) -> Result<(), String> {
        self.client.call("__shutdown", json!({})).await.map(drop)
    }

    /// Repair: restart the service on this data directory. It is asked to stop first; one that
    /// does not answer (stuck, or another build that refuses this one) is ended, but only if the
    /// process serving the endpoint is a Lumen service of this user. The supervisor then starts
    /// this build's service. Turns that were running are recovered as interrupted, never replayed.
    pub async fn restart_service(&self) -> Result<(), String> {
        // No new service starts until this one is gone, so the PID checked below stays its own.
        self.pause(true);
        let stopped = self.stop_quietly().await;
        self.pause(false);
        stopped
    }

    /// Stops the service on this data directory: asked first, ended if it does not answer.
    pub async fn stop_quietly(&self) -> Result<(), String> {
        const UNKNOWN: &str = "The Lumen service does not answer and cannot be stopped from here. Restart the computer, then open Lumen again.";
        let endpoint = &self.client.endpoint;
        // "Not answering" is not "stopped": a hung service with every pipe instance busy answers
        // no connection either. Only an endpoint that no longer exists counts as stopped.
        let Some(pid) = lumen_ipc::server_pid(endpoint).await else {
            return if lumen_ipc::absent(endpoint).await {
                Ok(())
            } else {
                Err(UNKNOWN.into())
            };
        };
        let _ = tokio::time::timeout(Duration::from_secs(5), self.stop_service()).await;
        for _ in 0..40 {
            if lumen_ipc::absent(endpoint).await {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        // Still the same process serving the pipe, and a Lumen service: only then is it ended.
        match lumen_ipc::server_pid(endpoint).await {
            None if lumen_ipc::absent(endpoint).await => return Ok(()),
            None => return Err(UNKNOWN.into()),
            Some(now) if now == pid && is_service(pid) => end_process(pid)?,
            Some(_) => return Err("Another program answers on Lumen's service address. Restart the computer, then open Lumen again.".into()),
        }
        for _ in 0..40 {
            if lumen_ipc::absent(endpoint).await {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        Err("The Lumen service did not stop. Restart the computer, then open Lumen again.".into())
    }

    /// While an update installs, the supervisor must not start the service it is replacing.
    pub fn pause(&self, paused: bool) {
        self.paused
            .store(paused, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    /// Whether any service listens on this data directory's endpoint.
    pub async fn reachable(&self) -> bool {
        lumen_ipc::connect(&self.client.endpoint).await.is_ok()
    }

    /// Runs for the life of the app: connect, relay, and on any loss reconnect without
    /// replaying anything. The client resynchronises from snapshots when it sees `connected`.
    pub async fn supervise(&self, app: AppHandle, on_connect: impl Fn(&AppHandle)) {
        let mut retry = FIRST_RETRY;
        let mut spawned: Option<(Instant, std::process::Child)> = None;
        let mut failure: Option<String> = None;
        let mut connected_once = false;
        loop {
            match self.subscribe().await {
                Ok(mut io) => {
                    connected_once = true;
                    failure = None;
                    retry = FIRST_RETRY;
                    self.set(&app, Connection::Connected);
                    on_connect(&app);
                    relay(&app, &mut io).await;
                    self.set(&app, Connection::Reconnecting);
                    continue;
                }
                Err(error) if error.contains("different build") => {
                    self.set(&app, Connection::Mismatch { detail: error });
                }
                Err(error) => {
                    // Only start the service when nothing listens; a busy or stopping one is left alone.
                    let absent = lumen_ipc::connect(&self.client.endpoint).await.is_err();
                    // A service that exited at startup (corrupt store, failed migration) says why
                    // instead of leaving the window connecting forever. Lumen keeps retrying.
                    if let Some((_, child)) = spawned.as_mut() {
                        if let Ok(Some(status)) = child.try_wait() {
                            if !status.success() {
                                failure = Some(self.startup_failure(status));
                            }
                        }
                    }
                    if absent
                        && !self.paused.load(std::sync::atomic::Ordering::SeqCst)
                        && spawned
                            .as_ref()
                            .is_none_or(|(at, _)| at.elapsed() > SPAWN_GRACE)
                    {
                        match self.spawn() {
                            Ok(child) => spawned = Some((Instant::now(), child)),
                            Err(spawn) => {
                                self.set(
                                    &app,
                                    Connection::Failed {
                                        detail: format!("{spawn} ({error})"),
                                    },
                                );
                                return;
                            }
                        }
                    }
                    self.set(
                        &app,
                        match &failure {
                            Some(detail) => Connection::Failed {
                                detail: detail.clone(),
                            },
                            None if connected_once => Connection::Reconnecting,
                            None => Connection::Connecting,
                        },
                    );
                }
            }
            tokio::time::sleep(retry).await;
            retry = (retry * 2).min(MAX_RETRY);
        }
    }

    async fn subscribe(&self) -> Result<lumen_ipc::Stream, String> {
        let mut io = self.client.start("__events", json!({})).await?;
        let ready = tokio::time::timeout(SILENCE, lumen_ipc::read::<lumen_ipc::Reply>(&mut io))
            .await
            .map_err(|_| "Lumen service did not answer".to_string())??;
        ready.result?;
        Ok(io)
    }

    fn startup_failure(&self, status: std::process::ExitStatus) -> String {
        let log = std::fs::read_to_string(self.root.join(SERVICE_LOG)).unwrap_or_default();
        let tail: Vec<&str> = log
            .lines()
            .filter(|l| !l.trim().is_empty())
            .rev()
            .take(5)
            .collect();
        // The banner is one line: keep the last few log lines in order, on it.
        let reason = tail.into_iter().rev().collect::<Vec<_>>().join(" · ");
        let code = status
            .code()
            .map_or_else(|| "a signal".to_string(), |c| c.to_string());
        if reason.trim().is_empty() {
            format!("The Lumen service stopped at startup (exit {code}).")
        } else {
            format!("The Lumen service stopped at startup (exit {code}): {reason}")
        }
    }

    /// The service outlives the window: it keeps running turns when the app closes.
    fn spawn(&self) -> Result<std::process::Child, String> {
        // Truncated on each start: it only needs to explain the latest one.
        let stderr = std::fs::File::create(self.root.join(SERVICE_LOG))
            .map(std::process::Stdio::from)
            .unwrap_or_else(|_| std::process::Stdio::null());
        let mut command = std::process::Command::new(&self.daemon);
        command
            .arg("--data-dir")
            .arg(&self.root)
            .arg("--resources")
            .arg(&self.resources)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(stderr);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        command
            .spawn()
            .map_err(|e| format!("Could not start the Lumen service: {e}"))
    }
}

/// Whether `pid` runs the Lumen service executable.
fn is_service(pid: u32) -> bool {
    let mut c = std::process::Command::new(system_tool(if cfg!(windows) {
        "tasklist.exe"
    } else {
        "ps"
    }));
    if cfg!(windows) {
        c.args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"]);
    } else {
        c.args(["-p", &pid.to_string(), "-o", "comm="]);
    }
    hide(&mut c);
    c.output().is_ok_and(|o| {
        String::from_utf8_lossy(&o.stdout)
            .to_ascii_lowercase()
            .contains("lumen-daemon")
    })
}

/// Ends the service and the agent processes it started.
fn end_process(pid: u32) -> Result<(), String> {
    let mut c = if cfg!(windows) {
        let mut c = std::process::Command::new(system_tool("taskkill.exe"));
        c.args(["/PID", &pid.to_string(), "/T", "/F"]);
        c
    } else {
        // The service leads its own process group (see `spawn`).
        let mut c = std::process::Command::new("kill");
        c.args(["-KILL", "--", &format!("-{pid}")]);
        c
    };
    hide(&mut c);
    c.output()
        .map(drop)
        .map_err(|e| format!("Could not stop the Lumen service: {e}"))
}

/// Windows tools from System32, never from the app folder or the search path.
fn system_tool(name: &str) -> std::path::PathBuf {
    match std::env::var_os("SystemRoot") {
        Some(root) if cfg!(windows) => std::path::Path::new(&root).join("System32").join(name),
        _ => name.into(),
    }
}

fn hide(_c: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        _c.creation_flags(0x0800_0000);
    }
}

/// Forwards frames until the stream ends, fails or falls silent. Heartbeats stay here.
async fn relay(app: &AppHandle, io: &mut lumen_ipc::Stream) {
    loop {
        match tokio::time::timeout(SILENCE, lumen_ipc::read::<Value>(io)).await {
            Ok(Ok(frame)) => {
                if frame["event"] == "lumen:heartbeat" {
                    continue;
                }
                if frame["event"].is_string() {
                    let _ = app.emit(CHANNEL, frame);
                }
            }
            Ok(Err(_)) | Err(_) => return,
        }
    }
}

/// The webview may call daemon commands and read `__status`, but never stop or update the
/// service, and never open a second event stream.
fn allowed(command: &str) -> bool {
    !command.starts_with("__") || command == "__status"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webview_cannot_stop_or_update_the_service() {
        assert!(allowed("list_sessions"));
        assert!(allowed("__status"));
        for command in ["__shutdown", "__prepare_update", "__events", "__anything"] {
            assert!(!allowed(command), "{command}");
        }
    }

    #[test]
    fn connection_states_reach_the_client_as_tagged_objects() {
        assert_eq!(
            serde_json::to_value(Connection::Connected).unwrap(),
            json!({"state": "connected"})
        );
        assert_eq!(
            serde_json::to_value(Connection::Mismatch {
                detail: "different build".into()
            })
            .unwrap(),
            json!({"state": "mismatch", "detail": "different build"})
        );
    }
}
