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
    /// Local wall-clock alignment: `mon@09:00` (weekly) or `@09:00` (daily).
    pub anchor: Option<String>,
}

const DAYS: [(&str, &[&str]); 7] = [
    ("mon", &["mon", "monday", "lun", "lunedi", "lunedì"]),
    ("tue", &["tue", "tuesday", "mar", "martedi", "martedì"]),
    (
        "wed",
        &["wed", "wednesday", "mer", "mercoledi", "mercoledì"],
    ),
    ("thu", &["thu", "thursday", "gio", "giovedi", "giovedì"]),
    ("fri", &["fri", "friday", "ven", "venerdi", "venerdì"]),
    ("sat", &["sat", "saturday", "sab", "sabato"]),
    ("sun", &["sun", "sunday", "dom", "domenica"]),
];

fn day(s: &str) -> Option<&'static str> {
    let s = s.trim().to_lowercase();
    DAYS.iter()
        .find(|(_, names)| names.contains(&s.as_str()))
        .map(|(d, _)| *d)
}

fn time(s: &str) -> Option<chrono::NaiveTime> {
    chrono::NaiveTime::parse_from_str(s.trim(), "%H:%M").ok()
}

/// `every` is an interval (`30m`…`30d`), a weekday (`mon`, `lunedì`) or a time (`09:00`);
/// `at` adds a time to a weekday or to `1d`. Returns the interval and the optional anchor.
pub fn schedule(every: &str, at: Option<&str>) -> Result<(i64, Option<String>), String> {
    let at = at.map(str::trim).filter(|s| !s.is_empty());
    let clock = |s: &str| {
        time(s)
            .map(|t| t.format("%H:%M").to_string())
            .ok_or_else(|| "Use a time like 09:00".to_owned())
    };
    if let Some(d) = day(every) {
        return Ok((
            7 * 86_400_000,
            Some(format!("{d}@{}", clock(at.unwrap_or("09:00"))?)),
        ));
    }
    if time(every).is_some() && at.is_none() {
        return Ok((86_400_000, Some(format!("@{}", clock(every)?))));
    }
    let ms = parse_every(every)?;
    match at {
        None => Ok((ms, None)),
        Some(t) if ms == 86_400_000 => Ok((ms, Some(format!("@{}", clock(t)?)))),
        Some(_) => Err("A time of day needs a daily interval or a weekday".into()),
    }
}

/// First local wall-clock occurrence of `anchor` strictly after `after` (ms), DST-safe.
pub fn next_anchor(anchor: &str, after: i64) -> Option<i64> {
    use chrono::{Datelike, Local, TimeZone};
    let (d, t) = anchor.split_once('@')?;
    let t = time(t)?;
    let from = Local.timestamp_millis_opt(after).single()?.date_naive();
    (0..=8).find_map(|i| {
        let date = from + chrono::Days::new(i);
        if !d.is_empty() && DAYS[date.weekday().num_days_from_monday() as usize].0 != d {
            return None;
        }
        let at = Local
            .from_local_datetime(&date.and_time(t))
            .earliest()?
            .timestamp_millis();
        (at > after).then_some(at)
    })
}

fn first_run(anchor: Option<&str>, every_ms: i64, start: i64) -> Result<i64, String> {
    match anchor {
        Some(a) => next_anchor(a, start).ok_or_else(|| "Invalid schedule".to_owned()),
        None => Ok(start + every_ms),
    }
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
    at: Option<String>,
    options: RunOptions,
) -> Result<Routine, String> {
    let (every_ms, anchor) = schedule(&every, at.as_deref())?;
    if text.trim().is_empty() {
        return Err("Describe the task to repeat".into());
    }
    core.store.session(&id)?;
    let r = Routine {
        id: crate::models::id(),
        session: id,
        text: text.trim().into(),
        every_ms,
        next_at: first_run(anchor.as_deref(), every_ms, now())?,
        until_at: None,
        anchor,
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
pub const HINT: &str = "If the user asks for something to be repeated on a schedule (for example every 30 minutes, every day at 9, every Monday, optionally until a date), do the first run now and add `<lumen-routine every=\"30m\" until=\"YYYY-MM-DD\">task to repeat, without the schedule</lumen-routine>` to your final reply. `every` is an interval with m, h, d or w (15m to 30d), a weekday such as `mon`, or a time such as `09:00` for daily; add `at=\"HH:MM\"` (local time) to a weekday or `1d`. `until` is optional. Lumen then repeats the task in this conversation. Never add it unless the user asked for repetition.";

/// Creates at most one routine from a `<lumen-routine>` tag in a successful reply.
pub(crate) fn capture(
    core: &Core,
    session: &str,
    reply: &mut String,
    options: &RunOptions,
) -> Result<Option<Routine>, String> {
    let re = regex::Regex::new(r#"(?s)<lumen-routine\s([^>]*)>(.*?)</lumen-routine>\n?"#).unwrap();
    let attr = |attrs: &str, name: &str| {
        regex::Regex::new(&format!(r#"\b{name}="([^"]*)""#))
            .unwrap()
            .captures(attrs)
            .map(|c| c[1].to_owned())
    };
    let Some(c) = re.captures(reply).map(|c| {
        (
            attr(&c[1], "every"),
            attr(&c[1], "at"),
            attr(&c[1], "until"),
            c[2].trim().to_owned(),
        )
    }) else {
        return Ok(None);
    };
    *reply = re.replace_all(reply, "").trim_end().to_owned();
    let (every, at, until, text) = c;
    let (every_ms, anchor) = schedule(&every.ok_or("The routine has no schedule")?, at.as_deref())?;
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
        next_at: first_run(anchor.as_deref(), every_ms, start)?,
        until_at,
        anchor,
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
        r.next_at = match r.anchor.as_deref().and_then(|a| next_anchor(a, at)) {
            Some(next) => next,
            None => {
                while r.next_at <= at {
                    r.next_at += r.every_ms;
                }
                r.next_at
            }
        };
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
    fn weekday_and_time_anchors_follow_local_clock() {
        use chrono::{Datelike, Local, TimeZone, Timelike};
        assert_eq!(
            schedule("lunedì", Some("9:30")).unwrap(),
            (7 * 86_400_000, Some("mon@09:30".into()))
        );
        assert_eq!(
            schedule("08:00", None).unwrap(),
            (86_400_000, Some("@08:00".into()))
        );
        assert_eq!(
            schedule("1d", Some("07:15")).unwrap().1.as_deref(),
            Some("@07:15")
        );
        assert!(schedule("2h", Some("07:15")).is_err());
        assert!(schedule("mon", Some("25:00")).is_err());
        let start = now();
        let next = Local
            .timestamp_millis_opt(next_anchor("mon@09:30", start).unwrap())
            .unwrap();
        assert_eq!(
            (next.weekday(), next.hour(), next.minute()),
            (chrono::Weekday::Mon, 9, 30)
        );
        assert!(next.timestamp_millis() > start);
        assert!(next.timestamp_millis() - start <= 7 * 86_400_000);
        let d = Local
            .timestamp_millis_opt(next_anchor("@09:30", next.timestamp_millis()).unwrap())
            .unwrap();
        assert_eq!((d.weekday(), d.hour()), (chrono::Weekday::Tue, 9));
    }

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
            anchor: None,
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
        let mut weekly =
            r#"<lumen-routine every="mon" at="09:00">Summarize TODOs</lumen-routine>"#.to_owned();
        let w = capture(&f.core, &s.id, &mut weekly, &options)
            .unwrap()
            .unwrap();
        assert_eq!(w.anchor.as_deref(), Some("mon@09:00"));
        assert_eq!(f.core.store.routines().unwrap().len(), 2);
    }
}
