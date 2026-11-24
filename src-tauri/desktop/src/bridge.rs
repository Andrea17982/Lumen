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

    /// Runs for the life of the app: connect, relay, and on any loss reconnect without
    /// replaying anything. The client resynchronises from snapshots when it sees `connected`.
    pub async fn supervise(&self, app: AppHandle, on_connect: impl Fn(&AppHandle)) {
        let mut retry = FIRST_RETRY;
        let mut spawned: Option<Instant> = None;
        let mut connected_once = false;
        loop {
            match self.subscribe().await {
                Ok(mut io) => {
                    connected_once = true;
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
                    if absent && spawned.is_none_or(|at| at.elapsed() > SPAWN_GRACE) {
                        spawned = Some(Instant::now());
                        if let Err(spawn) = self.spawn() {
                            self.set(
                                &app,
                                Connection::Failed {
                                    detail: format!("{spawn} ({error})"),
                                },
                            );
                            return;
                        }
                    }
                    self.set(
                        &app,
                        if connected_once {
                            Connection::Reconnecting
                        } else {
                            Connection::Connecting
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

    /// The service outlives the window: it keeps running turns when the app closes.
    fn spawn(&self) -> Result<(), String> {
        let mut command = std::process::Command::new(&self.daemon);
        command
            .arg("--data-dir")
            .arg(&self.root)
            .arg("--resources")
            .arg(&self.resources)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
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
            .map(drop)
            .map_err(|e| format!("Could not start the Lumen service: {e}"))
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
