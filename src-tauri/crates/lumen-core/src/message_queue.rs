use crate::{event, models::RunOptions, turns::PreparingJob, Core, CoreRef};
use std::sync::Arc;

#[derive(serde::Serialize)]
pub(crate) struct Queued {
    pub id: String,
    pub text: String,
    pub starting: bool,
}

#[crate::command]
fn queue_message(
    core: CoreRef<'_>,
    id: String,
    text: String,
    options: RunOptions,
) -> Result<(), String> {
    if text.trim().is_empty() || text.len() > 100_000 {
        return Err("Enter a message of up to 100,000 bytes".into());
    }
    if crate::context::pasted_lockfile(&text) {
        return Err("This message contains a complete lockfile".into());
    }
    let options = serde_json::to_string(&options).map_err(|e| e.to_string())?;
    // Serialize with turn completion so that the last message cannot miss the next launch.
    let jobs = core.jobs.lock().map_err(|_| "Jobs lock failed")?;
    if !jobs.get(&id).is_some_and(|job| !*job.borrow()) {
        return Err("The turn already ended. Send this message from the composer.".into());
    }
    core.store
        .queue_message(&id, &crate::models::id(), &text, &options)
}

#[crate::command]
fn queued_messages(core: CoreRef<'_>, id: String) -> Result<Vec<Queued>, String> {
    core.store.queued_messages(&id)
}

#[crate::command]
fn take_queued_message(
    core: CoreRef<'_>,
    id: String,
    message: String,
) -> Result<serde_json::Value, String> {
    let (text, options) = core.store.take_queued_message(&id, &message)?;
    let attachments = serde_json::from_str::<RunOptions>(&options)
        .map(|o| o.attachments)
        .unwrap_or_default();
    Ok(serde_json::json!({"text":text,"attachments":attachments}))
}

pub(crate) fn advance(app: crate::host::EventHub, core: Arc<Core>, id: String) {
    if core.stopping.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    tokio::spawn(async move {
        let Ok(job) = PreparingJob::reserve(&core, &id) else {
            return;
        };
        let (message, text, options) = match core.store.claim_next_message(&id) {
            Ok(Some(row)) => row,
            Ok(None) => return,
            Err(error) => {
                event(
                    &app,
                    &id,
                    "",
                    "error",
                    &format!("Could not read queued messages: {error}"),
                );
                return;
            }
        };
        event(&app, &id, "", "queue-change", "");
        let result = async {
            let options: RunOptions =
                serde_json::from_str(&options).map_err(|e| e.to_string())?;
            if options.access() != core.store.session(&id)?.access_mode {
                return Err("The conversation's access mode changed. Return the queued message to the composer and send it with the current permissions.".into());
            }
            crate::turns::submit(
                app.clone(),
                core.clone(),
                id.clone(),
                text,
                options,
                Some(message.clone()),
                Some(job.receiver()),
            )
            .await
            .map(|_| ())
        }
        .await;
        if let Err(error) = result {
            let release = core.store.release_queued_message(&message);
            let detail = release.err().map_or(error.clone(), |e| {
                format!("{error}. Could not release the queue entry: {e}")
            });
            event(&app, &id, "", "queue-error", &detail);
        } else {
            job.handoff();
            event(&app, &id, "", "queue-change", "");
        }
    });
}

#[cfg(test)]
mod tests {
    use crate::{
        models::{self, Message},
        store::Store,
    };

    #[test]
    fn claims_survive_daemon_reopen_and_moves_to_composer_stay_recoverable() {
        let fixture = crate::test_support::Fixture::new();
        let path = fixture.directory("queue").join("queue.db");
        let db = Store::open(&path).unwrap();
        let session = db.create("project".into()).unwrap().id;
        let first = models::id();
        let second = models::id();
        db.queue_message(&session, &first, "first", "{}").unwrap();
        db.queue_message(&session, &second, "second", "{}").unwrap();
        assert_eq!(db.claim_next_message(&session).unwrap().unwrap().0, first);
        drop(db);

        let db = Store::open(&path).unwrap();
        let queue = db.queued_messages(&session).unwrap();
        assert_eq!(queue.len(), 2);
        assert!(!queue[0].starting);
        assert_eq!(db.take_queued_message(&session, &first).unwrap().0, "first");
        drop(db);

        let db = Store::open(&path).unwrap();
        assert_eq!(db.claim_next_message(&session).unwrap().unwrap().0, second);
        db.insert_message(
            &session,
            &Message {
                id: second,
                role: "user".into(),
                text: "second".into(),
                activity: vec![],
            },
        )
        .unwrap();
        assert!(db.claim_next_message(&session).unwrap().is_none());
        let queue = db.queued_messages(&session).unwrap();
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].id, first);
    }
}
