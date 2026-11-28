//! One opt-in native continuation after a quota reset. Waiting consumes no inference/work slot.
use crate::{models::*, turns, Core};
use std::sync::Arc;
use tokio::sync::watch;

/// When the account can work again, and whether that time is only Lumen's estimate. Every
/// exhausted window must reset, and the provider's own message counts as one of them.
pub fn reset(core: &Core, account: &str, error: &str) -> (i64, bool) {
    let current = now();
    let windows = crate::read_usage(core)
        .unwrap_or_default()
        .iter()
        .filter(|u| u.account_id == account)
        .flat_map(|u| &u.windows)
        .filter(|w| w.used_percent >= 100.0 && w.resets_at > current)
        .map(|w| w.resets_at)
        .max();
    // Lumen waits a week at most: a later or misread time is shown as an estimate.
    let week = current + 7 * 24 * 3_600_000;
    match windows.max(crate::limits::stated_reset(error, current)) {
        Some(at) if at <= week => (at, false),
        Some(_) => (week, true),
        None => (current + 15 * 60_000, true),
    }
}

#[crate::command]
pub fn resume_at_reset(
    app: crate::host::EventHub,
    core: crate::CoreRef<'_>,
    id: String,
    message: String,
) -> Result<(), String> {
    let core = core.inner().clone();
    let session = core.store.session(&id)?;
    let history = core.store.messages(&id)?;
    let mut output = history
        .last()
        .filter(|m| m.role == "assistant" && m.id == message)
        .cloned()
        .ok_or("Only the latest interrupted reply can be scheduled")?;
    if !output.activity.iter().any(|a| a.kind == "error") {
        return Err("This reply has no interrupted quota error".into());
    }
    let retry = output
        .activity
        .iter()
        .rev()
        .find(|a| a.kind == "quota-reset")
        .ok_or("No quota reset was recorded")?;
    let retry: serde_json::Value =
        serde_json::from_str(&retry.text).map_err(|_| "Invalid quota reset")?;
    let at = retry["at"]
        .as_i64()
        .filter(|at| *at <= now() + 7 * 24 * 3_600_000)
        .ok_or("Invalid reset time")?;
    let account = crate::account(&core, retry["accountId"].as_str().ok_or("Missing account")?)?;
    core.limits.cool(&account.id, at);
    if crate::adapter_contract::contract(&account.provider)
        .native_resume
        .support
        != crate::adapter_contract::Support::Available
    {
        return Err("This adapter cannot safely resume native work automatically".into());
    }
    turns::native_of(&output)
        .filter(|n| n.account == account.id && n.workspace == session.workspace)
        .ok_or("No matching native session was recorded; resume manually")?;
    let request = history
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .ok_or("Missing request")?;
    let mut options: RunOptions = serde_json::from_str(
        &request
            .activity
            .iter()
            .find(|a| a.kind == "run-options")
            .ok_or("This older turn has no saved execution options; resume manually")?
            .text,
    )
    .map_err(|_| "Invalid saved execution options")?;
    if options.access() != session.access_mode {
        return Err(
            "Conversation permissions changed since the interrupted turn; resume manually".into(),
        );
    }
    options.mode = account.provider.id();
    options.account_id = Some(account.id);
    options.model = retry["model"].as_str().map(str::to_owned);
    options.effort = retry["effort"].as_str().unwrap_or("").into();
    options.attachments.clear();
    options.audit_of = None;
    options.resume_only = true;
    let (stop, mut cancelled) = watch::channel(false);
    {
        let mut jobs = core.jobs.lock().map_err(|_| "Jobs lock failed")?;
        if jobs.contains_key(&id) {
            return Err("This conversation already has a running or scheduled turn".into());
        }
        jobs.insert(id.clone(), stop);
    }
    if let Err(e) = core.store.schedule_quota(&id, &message) {
        core.jobs
            .lock()
            .map_err(|_| "Jobs lock failed")?
            .remove(&id);
        return Err(e);
    }
    let activity = Activity {
        kind: "quota-scheduled".into(),
        text: serde_json::json!({"at":at,"state":"waiting"}).to_string(),
    };
    output.activity.push(activity.clone());
    if let Err(error) = core
        .store
        .save_message(&id, &output)
        .and_then(|_| core.store.status(&id, "waiting"))
    {
        let _ = core.store.unschedule_quota(&id);
        if let Ok(mut jobs) = core.jobs.lock() {
            jobs.remove(&id);
        }
        return Err(error);
    }
    crate::event(&app, &id, &message, &activity.kind, &activity.text);
    crate::event(&app, &id, &message, "status", "waiting");
    tokio::spawn(async move {
        let delay = std::time::Duration::from_millis((at - now()).max(0) as u64);
        let resume =
            tokio::select! {biased;_=cancelled.changed()=>false,_=tokio::time::sleep(delay)=>true};
        let unchanged = core.store.session(&id).is_ok_and(|s| {
            s.workspace == session.workspace && s.access_mode == session.access_mode
        }) && core
            .store
            .messages(&id)
            .is_ok_and(|h| h.last().is_some_and(|m| m.id == message));
        if core.stopping.load(std::sync::atomic::Ordering::SeqCst) {
            if let Ok(mut jobs) = core.jobs.lock() {
                jobs.remove(&id);
            }
            return;
        }
        // Consume the durable intention before starting: a crash must never submit it twice.
        if core.store.unschedule_quota(&id).is_err() {
            if let Ok(mut jobs) = core.jobs.lock() {
                jobs.remove(&id);
            }
            let _ = core.store.status(&id, "interrupted");
            return;
        }
        if !resume || !unchanged {
            if let Ok(mut jobs) = core.jobs.lock() {
                jobs.remove(&id);
            }
            let _ = core.store.status(&id, "interrupted");
        }
        let state = if resume && unchanged {
            "resuming"
        } else {
            "cancelled"
        };
        let activity = Activity {
            kind: "quota-scheduled".into(),
            text: serde_json::json!({"at":at,"state":state}).to_string(),
        };
        output.activity.push(activity.clone());
        let _ = core.store.save_message(&id, &output);
        crate::event(&app, &id, &message, &activity.kind, &activity.text);
        if resume && unchanged {
            if let Err(error)=turns::submit(app.clone(),core.clone(),id.clone(),"Prosegui. Continue the interrupted task from the recorded work; preserve existing changes and finish the requested deliverables, including the build if requested.".into(),options,None,Some(cancelled)).await {
                if let Ok(mut jobs) = core.jobs.lock() { jobs.remove(&id); }
                let status = if error == "Cancelled" { "interrupted" } else { "error" };
                turns::note(&app,&id,&mut output,"error",error);
                let _=core.store.save_message(&id,&output);
                let _=core.store.status(&id,status);
                crate::event(&app,&id,&message,"done",status);
            }
        } else {
            crate::event(&app, &id, &message, "done", "interrupted");
        }
    });
    Ok(())
}

const CONTINUE_ELSEWHERE: &str = "Prosegui. The previous agent stopped on a quota limit. Continue the interrupted task from the recorded work and the current files; do not repeat steps that are already done, preserve existing changes and finish the requested deliverables, including the build if requested.";

/// The user's explicit choice after a quota stop: a new turn on the equivalent model Lumen
/// proposed on another account. It reads the conversation and the files as they are; nothing
/// the stopped reply did is replayed.
#[crate::command]
pub async fn continue_elsewhere(
    app: crate::host::EventHub,
    core: crate::CoreRef<'_>,
    id: String,
    message: String,
) -> Result<Plan, String> {
    let core = core.inner().clone();
    let session = core.store.session(&id)?;
    let history = core.store.messages(&id)?;
    let output = history
        .last()
        .filter(|m| m.role == "assistant" && m.id == message)
        .ok_or("Only the latest interrupted reply can continue on another model")?;
    // The proposal belongs to a reply that stopped on a quota and has not been resumed since.
    let stopped = output.activity.iter().any(|a| a.kind == "quota-stop")
        && output.activity.iter().any(|a| a.kind == "error")
        && !output
            .activity
            .iter()
            .rev()
            .find(|a| a.kind == "quota-scheduled")
            .is_some_and(|a| !a.text.contains("\"cancelled\""));
    if !stopped {
        return Err("This reply did not stop on a quota, or it is already resuming".into());
    }
    let proposal = output
        .activity
        .iter()
        .rev()
        .find(|a| a.kind == "quota-alternative")
        .ok_or("No equivalent model was proposed for this reply")?;
    let proposal: serde_json::Value =
        serde_json::from_str(&proposal.text).map_err(|_| "Invalid proposed model")?;
    let account = crate::account(
        &core,
        proposal["accountId"]
            .as_str()
            .ok_or("Missing proposed account")?,
    )?;
    // Proposed when the reply stopped; the account and its model must still qualify now.
    let usage = crate::read_usage(&core)?;
    if !turns::can_take_over(&core, &account, session.access_mode, &usage) {
        return Err(
            "The proposed account cannot take this task now; choose another model in the composer"
                .into(),
        );
    }
    let model = proposal["model"]
        .as_str()
        .ok_or("Missing proposed model")?
        .to_owned();
    if !crate::models_for(&core, &account, false, None)
        .await?
        .iter()
        .any(|m| m.id == model)
    {
        return Err(
            "The proposed model is no longer available; choose another model in the composer"
                .into(),
        );
    }
    let request = history
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .ok_or("Missing request")?;
    let mut options: RunOptions = serde_json::from_str(
        &request
            .activity
            .iter()
            .find(|a| a.kind == "run-options")
            .ok_or("This older turn has no saved execution options; continue manually")?
            .text,
    )
    .map_err(|_| "Invalid saved execution options")?;
    if options.access() != session.access_mode {
        return Err(
            "Conversation permissions changed since the interrupted turn; continue manually".into(),
        );
    }
    options.mode = account.provider.id();
    options.account_id = Some(account.id);
    options.model = Some(model);
    options.effort = proposal["effort"].as_str().unwrap_or("").into();
    options.attachments.clear();
    options.audit_of = None;
    options.resume_only = false;
    turns::submit(
        app,
        core,
        id,
        CONTINUE_ELSEWHERE.into(),
        options,
        None,
        None,
    )
    .await
}

pub(crate) fn restore(app: crate::host::EventHub, core: &Arc<Core>) {
    let schedules = core.store.quota_schedules().unwrap_or_else(|error| {
        eprintln!("Lumen: scheduled continuations could not be read: {error}");
        Vec::new()
    });
    for (id, message) in schedules {
        if let Err(error) = resume_at_reset(
            app.clone(),
            crate::CoreRef(core),
            id.clone(),
            message.clone(),
        ) {
            let _ = core.store.unschedule_quota(&id);
            crate::event(
                &app,
                &id,
                &message,
                "error",
                &format!("Scheduled continuation needs manual review: {error}"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_exhausted_windows_must_reset_before_resumption() {
        let f = crate::test_support::Fixture::new();
        let time = now();
        let usage = serde_json::json!([{"accountId":"a","updatedAt":time,"source":"manual","windows":[{"usedPercent":100,"resetsAt":time+1000,"windowMinutes":300},{"usedPercent":100,"resetsAt":time+5000,"windowMinutes":10080},{"usedPercent":20,"resetsAt":time+9000,"windowMinutes":60}]}]);
        std::fs::write(f.root.join("usage.json"), usage.to_string()).unwrap();
        assert_eq!(
            reset(&f.core, "a", "Resets in 1 second"),
            (time + 5000, false)
        );
        // The provider's stated time wins over an earlier window; nothing stated is an estimate.
        assert!(reset(&f.core, "a", "Resets in 1 hour").0 >= time + 3_600_000);
        assert!(reset(&f.core, "b", "Try again later.").1);
        // A stated time past a week is waited for a week at most, as an estimate.
        let (at, estimated) = reset(&f.core, "b", "Try again at Dec 3rd, 2200 1:18 PM.");
        assert!(estimated && at <= now() + 7 * 24 * 3_600_000);
    }

    #[tokio::test]
    async fn restore_cancels_a_schedule_if_conversation_permissions_changed() {
        let f = crate::test_support::Fixture::new();
        let session = f
            .core
            .store
            .create(f.root.to_string_lossy().into())
            .unwrap();
        let account = f
            .core
            .store
            .add_account("Codex fixture".into(), Provider::Gpt)
            .unwrap();
        let request = Message {
            id: id(),
            role: "user".into(),
            text: "Make a change".into(),
            activity: vec![Activity {
                kind: "run-options".into(),
                text: serde_json::to_string(&crate::test_support::Fixture::options(
                    AccessMode::Autopilot,
                ))
                .unwrap(),
            }],
        };
        let reply = Message {
            id: id(),
            role: "assistant".into(),
            text: "Partial work".into(),
            activity: vec![
                Activity { kind: "error".into(), text: "Rate limit".into() },
                Activity {
                    kind: "quota-reset".into(),
                    text: serde_json::json!({"at":now()+60_000,"accountId":account.id,"model":"test","effort":"low"}).to_string(),
                },
                Activity {
                    kind: "native".into(),
                    text: serde_json::json!({"account":account.id,"workspace":session.workspace,"session":"native-session"}).to_string(),
                },
            ],
        };
        f.core.store.insert_message(&session.id, &request).unwrap();
        f.core.store.save_message(&session.id, &reply).unwrap();
        f.core.store.schedule_quota(&session.id, &reply.id).unwrap();
        restore(crate::host::EventHub::default(), &f.core);
        assert!(f.core.store.quota_schedules().unwrap().is_empty());
        assert!(f.core.jobs.lock().unwrap().is_empty());
        assert!(f.core.limits.cooling(&account.id, now()));
        assert_eq!(
            f.core
                .store
                .messages(&session.id)
                .unwrap()
                .last()
                .unwrap()
                .text,
            "Partial work"
        );
    }
}
