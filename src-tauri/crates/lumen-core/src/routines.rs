//! Recurring tasks: a saved request that Lumen resends to the same conversation on an interval.
//! The daemon owns the timer, so routines keep running while the GUI is closed.
use crate::{models::*, turns, Core};
use std::sync::Arc;

const MIN_EVERY: i64 = 15 * 60_000;
const MAX_EVERY: i64 = 30 * 24 * 3_600_000;
const TICK: std::time::Duration = std::time::Duration::from_secs(30);

#[derive(serde::Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Routine {
    pub id: String,
    pub session: String,
    pub text: String,
    pub every_ms: i64,
    pub next_at: i64,
    /// Last moment a run may start; `None` repeats until stopped.
    pub until_at: Option<i64>,
}

/// `30m`, `2h`, `1d`, `1w` → milliseconds within the supported range.
pub fn parse_every(s: &str) -> Result<i64, String> {
    let s = s.trim().to_lowercase();
    let (n, unit) = s.split_at(s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len()));
    let n: i64 = n
        .parse()
        .map_err(|_| "Use an interval like 30m, 2h, 1d or 1w")?;
    let ms = n * match unit {
        "m" => 60_000,
        "h" => 3_600_000,
        "d" => 86_400_000,
        "w" => 7 * 86_400_000,
        _ => return Err("Use an interval like 30m, 2h, 1d or 1w".into()),
    };
    if !(MIN_EVERY..=MAX_EVERY).contains(&ms) {
        return Err("Routines run between every 15 minutes and every 30 days".into());
    }
    Ok(ms)
}

#[crate::command]
pub async fn create_routine(
    _app: crate::host::EventHub,
    core: crate::CoreRef<'_>,
    id: String,
    text: String,
    every: String,
    options: RunOptions,
) -> Result<Routine, String> {
    let every_ms = parse_every(&every)?;
    if text.trim().is_empty() {
        return Err("Describe the task to repeat".into());
    }
    core.store.session(&id)?;
    let r = Routine {
        id: crate::models::id(),
        session: id,
        text: text.trim().into(),
        every_ms,
        next_at: now() + every_ms,
        until_at: None,
    };
    let options = serde_json::to_string(&options).map_err(|e| e.to_string())?;
    core.store.save_routine(&r, &options)?;
    Ok(r)
}

#[crate::command]
pub async fn list_routines(
    _app: crate::host::EventHub,
    core: crate::CoreRef<'_>,
    id: String,
) -> Result<Vec<Routine>, String> {
    Ok(core
        .store
        .routines()?
        .into_iter()
        .map(|(r, _)| r)
        .filter(|r| r.session == id)
        .collect())
}

#[crate::command]
pub async fn delete_routine(
    _app: crate::host::EventHub,
    core: crate::CoreRef<'_>,
    routine: String,
) -> Result<(), String> {
    core.store.delete_routine(&routine)
}

/// Instruction appended to every prompt so a plain-language request can schedule a routine.
pub const HINT: &str = "If the user asks for something to be repeated on a schedule (for example every 30 minutes, daily or weekly, optionally until a date), do the first run now and add `<lumen-routine every=\"30m\" until=\"YYYY-MM-DD\">task to repeat, without the schedule</lumen-routine>` to your final reply; `every` uses m, h, d or w (15m to 30d) and `until` is optional. Lumen then repeats the task in this conversation. Never add it unless the user asked for repetition.";

/// Creates at most one routine from a `<lumen-routine>` tag in a successful reply.
pub(crate) fn capture(
    core: &Core,
    session: &str,
    reply: &mut String,
    options: &RunOptions,
) -> Result<Option<Routine>, String> {
    let re = regex::Regex::new(
        r#"(?s)<lumen-routine\s+every="([^"]+)"(?:\s+until="(\d{4}-\d{2}-\d{2})")?\s*>(.*?)</lumen-routine>
?"#,
    )
    .unwrap();
    let Some(c) = re.captures(reply).map(|c| {
        (
            c[1].to_owned(),
            c.get(2).map(|m| m.as_str().to_owned()),
            c[3].trim().to_owned(),
        )
    }) else {
        return Ok(None);
    };
    *reply = re.replace_all(reply, "").trim_end().to_owned();
    let (every, until, text) = c;
    let every_ms = parse_every(&every)?;
    let until_at = until
        .map(|d| {
            chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                .map(|d| {
                    d.and_hms_opt(23, 59, 59)
                        .unwrap()
                        .and_utc()
                        .timestamp_millis()
                })
                .map_err(|_| "Invalid end date".to_owned())
        })
        .transpose()?;
    let start = now();
    if until_at.is_some_and(|u| u <= start) {
        return Err("The routine end date is in the past".into());
    }
    if text.is_empty() || text.chars().count() > 2000 {
        return Err("The routine task is empty or too long".into());
    }
    if core
        .store
        .routines()?
        .iter()
        .any(|(r, _)| r.session == session && r.text == text)
    {
        return Ok(None);
    }
    let r = Routine {
        id: crate::models::id(),
        session: session.into(),
        text,
        every_ms,
        next_at: start + every_ms,
        until_at,
    };
    let mut options = options.clone();
    options.attachments.clear();
    options.audit_of = None;
    core.store.save_routine(
        &r,
        &serde_json::to_string(&options).map_err(|e| e.to_string())?,
    )?;
    Ok(Some(r))
}

/// Routines whose time has come, with their next time already advanced. Advancing before
/// submitting means a crash skips at most one run instead of repeating it.
pub(crate) fn due(core: &Core, at: i64) -> Vec<(Routine, String)> {
    let mut out = Vec::new();
    for (mut r, options) in core.store.routines().unwrap_or_default() {
        if r.next_at > at {
            continue;
        }
        // Skip missed runs (PC asleep) rather than firing a burst.
        while r.next_at <= at {
            r.next_at += r.every_ms;
        }
        if r.until_at.is_some_and(|u| at > u) {
            let _ = core.store.delete_routine(&r.id);
            continue;
        }
        if core.store.reschedule_routine(&r.id, r.next_at).is_ok() {
            out.push((r, options));
        }
    }
    out
}

pub(crate) fn start(app: crate::host::EventHub, core: &Arc<Core>) {
    let core = core.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(TICK).await;
            if core.stopping.load(std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            for (r, options) in due(&core, now()) {
                let busy = core
                    .jobs
                    .lock()
                    .map_or(true, |j| j.contains_key(&r.session));
                let Ok(options) = serde_json::from_str::<RunOptions>(&options) else {
                    continue;
                };
                if busy {
                    continue;
                }
                let (app, core) = (app.clone(), core.clone());
                tokio::spawn(async move {
                    let _ = turns::submit(app, core, r.session, r.text, options, None, None).await;
                });
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn intervals_are_bounded() {
        assert_eq!(parse_every("30m").unwrap(), 30 * 60_000);
        assert_eq!(parse_every("1W").unwrap(), 7 * 86_400_000);
        assert!(parse_every("5m").is_err());
        assert!(parse_every("31d").is_err());
        assert!(parse_every("daily").is_err());
    }

    #[test]
    fn due_routines_advance_once_and_skip_missed_runs() {
        let f = crate::test_support::Fixture::new();
        let s = f.core.store.create("folder".into()).unwrap();
        let r = Routine {
            id: "r".into(),
            session: s.id.clone(),
            text: "check".into(),
            every_ms: 1000,
            next_at: 1000,
            until_at: Some(9000),
        };
        f.core.store.save_routine(&r, "{}").unwrap();
        assert!(due(&f.core, 999).is_empty());
        let fired = due(&f.core, 5500);
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].0.next_at, 6000);
        assert!(due(&f.core, 5500).is_empty());
        assert!(due(&f.core, 9500).is_empty());
        assert!(f.core.store.routines().unwrap().is_empty());
    }

    #[test]
    fn replies_schedule_one_deduplicated_routine() {
        let f = crate::test_support::Fixture::new();
        let s = f.core.store.create("folder".into()).unwrap();
        let options: RunOptions =
            serde_json::from_value(serde_json::json!({"mode":"auto","effort":"","attachments":[]}))
                .unwrap();
        let tag = "Updated.
<lumen-routine every=\"30m\" until=\"2099-08-30\">Update the dependencies</lumen-routine>";
        let mut reply = tag.to_owned();
        let r = capture(&f.core, &s.id, &mut reply, &options)
            .unwrap()
            .unwrap();
        assert_eq!(reply, "Updated.");
        assert_eq!(
            (r.every_ms, r.text.as_str()),
            (30 * 60_000, "Update the dependencies")
        );
        assert!(r.until_at.unwrap() > now());
        let mut again = tag.to_owned();
        assert!(capture(&f.core, &s.id, &mut again, &options)
            .unwrap()
            .is_none());
        let mut past =
            "<lumen-routine every=\"1d\" until=\"2000-01-01\">x</lumen-routine>".to_owned();
        assert!(capture(&f.core, &s.id, &mut past, &options).is_err());
        let mut plain = "no schedule".to_owned();
        assert!(capture(&f.core, &s.id, &mut plain, &options)
            .unwrap()
            .is_none());
        assert_eq!(f.core.store.routines().unwrap().len(), 1);
    }
}
