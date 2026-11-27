use serde::Serialize;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
static RESOURCES: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
pub(crate) fn resources(path: std::path::PathBuf) {
    let _ = RESOURCES.set(path);
}
pub(crate) fn autostart(root: &std::path::Path, enabled: bool) -> Result<(), String> {
    let expected = dirs::data_dir()
        .ok_or("Missing user data directory")?
        .join("io.github.andrea17982.lumen");
    if dunce::canonicalize(root).ok() != dunce::canonicalize(expected).ok() {
        return Ok(());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut builder = auto_launch::AutoLaunchBuilder::new();
    builder
        .set_app_name("lumen")
        .set_app_path(&exe.to_string_lossy());
    let resources = RESOURCES.get().ok_or("Missing application resources")?;
    builder.set_args(&["--autostart", "--resources", &resources.to_string_lossy()]);
    #[cfg(windows)]
    builder.set_windows_enable_mode(auto_launch::WindowsEnableMode::CurrentUser);
    let launcher = builder.build().map_err(|e| e.to_string())?;
    // Rewrite the same opt-in login entry so an older GUI launch becomes a daemon launch.
    if enabled {
        launcher.enable()
    } else if launcher.is_enabled().unwrap_or(false) {
        launcher.disable()
    } else {
        return Ok(());
    }
    .map_err(|e| format!("Could not update the login item: {e}"))
}

#[derive(Clone, Debug, Serialize)]
pub struct Event {
    pub event: String,
    pub payload: Value,
}

#[derive(Clone)]
pub struct EventHub {
    events: broadcast::Sender<Event>,
    store: Arc<Mutex<Option<Arc<crate::store::Store>>>>,
}
impl Default for EventHub {
    fn default() -> Self {
        Self {
            events: broadcast::channel(4096).0,
            store: Default::default(),
        }
    }
}
impl EventHub {
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }
    pub(crate) fn attach(&self, store: Arc<crate::store::Store>) {
        *self.store.lock().expect("event store") = Some(store);
    }
    pub fn emit(&self, event: &str, payload: impl Serialize) -> Result<(), String> {
        let mut payload = serde_json::to_value(payload).map_err(|e| e.to_string())?;
        // The same cap as the saved message, so the live digest matches the stored text. Cut
        // before taking the store lock: the text can be up to a few MiB.
        if event == "lumen:run" {
            if let Some(text) = payload["text"]
                .as_str()
                .filter(|t| t.len() > crate::wire::KEPT)
            {
                let mut text = text.to_string();
                crate::wire::keep(payload["kind"].as_str().unwrap_or(""), &mut text);
                payload["text"] = serde_json::json!(text);
            }
        }
        // Persist before publishing: reconnects and a killed service can recover the same text.
        let store = self.store.lock().map_err(|_| "Event store lock failed")?;
        let mut unsaved = None;
        if event == "lumen:run" {
            if let Some(store) = store.as_ref() {
                unsaved = store.append_event(&mut payload).err();
            }
            crate::wire::project_activity(&mut payload);
        }
        let _ = self.events.send(Event {
            event: event.into(),
            payload,
        });
        // A store error must not swallow the event (a lost `done` leaves a turn looking alive):
        // it is still published, and clients read the snapshots again to fill any gap.
        if let Some(error) = unsaved {
            eprintln!("Lumen: an event was not saved: {error}");
            let _ = self.events.send(Event {
                event: "lumen:resync".into(),
                payload: serde_json::json!({}),
            });
            return Err(error);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_output_over_the_store_cap_matches_what_the_store_serves() {
        let store = Arc::new(crate::store::Store::open(std::path::Path::new(":memory:")).unwrap());
        let session = store.create("/tmp".into()).unwrap();
        let hub = EventHub::default();
        hub.attach(store.clone());
        let mut events = hub.subscribe();
        let output = "fixture line\n".repeat(120_000); // about 1.5 MiB
        hub.emit(
            "lumen:run",
            serde_json::json!({"sessionId": session.id, "messageId": "m", "kind": "tool", "text": output}),
        )
        .unwrap();
        let live = events.try_recv().unwrap().payload;
        assert!(live["text"].as_str().unwrap().len() <= crate::wire::LIMIT + 100);
        let digest = live["detail"]["digest"].as_str().unwrap();
        let saved = store.message_snapshots(&session.id).unwrap();
        let whole =
            crate::wire::detail(&saved, "m", digest).expect("the live digest finds the saved text");
        assert!(whole.len() <= crate::wire::KEPT && whole.ends_with("bytes in total]"));
        assert!(output.starts_with(whole.split("\n…").next().unwrap()));
    }
}
