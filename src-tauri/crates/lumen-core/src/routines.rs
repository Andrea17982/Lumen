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
        };
        f.core.store.save_routine(&r, "{}").unwrap();
        assert!(due(&f.core, 999).is_empty());
        let fired = due(&f.core, 5500);
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].0.next_at, 6000);
        assert!(due(&f.core, 5500).is_empty());
        f.core.store.delete_routine("r").unwrap();
        assert!(f.core.store.routines().unwrap().is_empty());
    }
}
