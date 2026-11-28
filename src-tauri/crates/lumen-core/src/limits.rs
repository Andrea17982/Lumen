//! Rate-limit detection, per-account cooldowns and a local 5-hour turn counter.
use serde::Serialize;
use std::{collections::HashMap, sync::Mutex};

pub const RATE_LIMIT: &str = "RATE_LIMIT: ";
const FIVE_HOURS: i64 = 5 * 3_600_000;
const SPEND_LIMIT: &str = "monthly spend limit";

pub fn is_spend_limit(text: &str) -> bool {
    text.to_ascii_lowercase().contains(SPEND_LIMIT)
}

pub fn is_rate_limit(text: &str) -> bool {
    let l = text.to_lowercase();
    [
        "rate limit",
        "rate_limit",
        "ratelimit",
        "429",
        "usage limit",
        SPEND_LIMIT,
        "quota",
        "too many requests",
        "hit your limit",
        "limit reached",
        "out of credits",
        "resource_exhausted",
        "overloaded",
    ]
    .iter()
    .any(|k| l.contains(k))
}

const WEEK: i64 = 7 * 24 * 3_600_000;

/// Best-effort unlock time: the time the provider stated, at most a week away, otherwise 15
/// minutes from now.
pub fn unlock_at(text: &str, now: i64) -> i64 {
    stated_reset(text, now).map_or(now + 15 * 60_000, |at| at.min(now + WEEK))
}

/// The reset time a provider message states, if it states one: a delay ("try again in 2h 5m",
/// "resets in 37 minutes"), a local clock time as Codex ("Try again at 1:18 PM", "Try again at
/// Dec 3rd, 2026 1:18 PM") and Claude Code ("resets 3pm", "resets Dec 3, 3:30pm") write it, or
/// an epoch in seconds after `|`.
pub fn stated_reset(text: &str, now: i64) -> Option<i64> {
    stated_reset_in(text, now, &chrono::Local)
}

fn stated_reset_in<Tz: chrono::TimeZone>(text: &str, now: i64, zone: &Tz) -> Option<i64> {
    let l = text.to_lowercase();
    if let Some(delay) = relative_delay(&l) {
        return Some(now.saturating_add(delay));
    }
    if let Some(at) = l.split('|').skip(1).find_map(|rest| {
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        (digits.len() == 10)
            .then(|| digits.parse::<i64>().ok())
            .flatten()
            .map(|seconds| seconds * 1000)
            .filter(|at| *at > now)
    }) {
        return Some(at);
    }
    ["try again at ", "resets at ", "reset at ", "resets "]
        .iter()
        .flat_map(|prefix| l.match_indices(prefix).map(|(i, p)| &l[i + p.len()..]))
        .find_map(|rest| clock_time(rest, now, zone))
}

/// "1:18 pm", "3pm", "15:30", optionally after a date such as "dec 3rd, 2026" or "dec 3 at".
fn clock_time<Tz: chrono::TimeZone>(rest: &str, now: i64, zone: &Tz) -> Option<i64> {
    use chrono::{Datelike, Duration, NaiveDate, NaiveTime};
    // "1:18 p.m." must keep its meridiem when the sentence is cut at its first dot.
    let rest = rest.replace("a.m.", "am").replace("p.m.", "pm");
    let end = rest
        .find(['.', '(', '·', '∙', '\n', ';', '!'])
        .unwrap_or(rest.len());
    let words: Vec<&str> = rest[..end]
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|w| !w.is_empty())
        .collect();
    let today = zone.timestamp_millis_opt(now).single()?.date_naive();
    const MONTHS: [&str; 12] = [
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ];
    let mut i = 0;
    let mut date = None;
    if let Some(month) = words
        .first()
        // "dec", "sept" or "december": a month name or its abbreviation, not "mark".
        .and_then(|w| MONTHS.iter().position(|m| w.len() >= 3 && m.starts_with(w)))
    {
        let day: u32 = words
            .get(1)?
            .trim_end_matches(|c: char| c.is_ascii_alphabetic())
            .parse()
            .ok()?;
        i = 2;
        let year = match words.get(2).and_then(|w| w.parse::<i32>().ok()) {
            Some(year) if (2000..=2200).contains(&year) => {
                i = 3;
                Some(year)
            }
            _ => None,
        };
        let on = |year| NaiveDate::from_ymd_opt(year, month as u32 + 1, day);
        date = Some(match year {
            Some(year) => on(year)?,
            None => on(today.year())
                .filter(|d| *d >= today)
                .or_else(|| on(today.year() + 1))?,
        });
        if words.get(i) == Some(&"at") {
            i += 1;
        }
    }
    let word = *words.get(i)?;
    let (clock, suffix) = match word.find(|c: char| c.is_ascii_alphabetic()) {
        Some(p) => (&word[..p], &word[p..]),
        None => (word, words.get(i + 1).copied().unwrap_or("")),
    };
    let meridiem = match suffix {
        "am" => Some(false),
        "pm" => Some(true),
        _ => None,
    };
    let (hour, minute) = match clock.split_once(':') {
        Some((h, m)) if m.len() == 2 => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        // A bare number is a time only with am or pm: "resets 3" says nothing.
        None if meridiem.is_some() => (clock.parse::<u32>().ok()?, 0),
        _ => return None,
    };
    let hour = match meridiem {
        Some(pm) if (1..=12).contains(&hour) => hour % 12 + if pm { 12 } else { 0 },
        Some(_) => return None,
        None => hour,
    };
    let time = NaiveTime::from_hms_opt(hour, minute, 0)?;
    // A repeated hour resets at its later occurrence; an hour skipped by the clock change, at
    // the first time that exists after it.
    let at = |day: NaiveDate| {
        let local = day.and_time(time);
        zone.from_local_datetime(&local)
            .latest()
            .or_else(|| {
                zone.from_local_datetime(&(local + Duration::hours(1)))
                    .latest()
            })
            .map(|t| t.timestamp_millis())
    };
    match date {
        Some(day) => at(day).filter(|at| *at > now),
        // A time without a date is the next one: "resets 1am" said at 11pm means tomorrow.
        None => at(today)
            .filter(|at| *at > now)
            .or_else(|| at(today + Duration::days(1))),
    }
}

fn relative_delay(l: &str) -> Option<i64> {
    let relative = ["resets in ", "try again in ", "retry in ", "available in "]
        .iter()
        .find_map(|prefix| l.find(prefix).map(|i| &l[i + prefix.len()..]))?;
    // The delay is one clause: a later sentence can hold other numbers.
    let end = relative
        .find(['·', '∙', '\n', ';', '('])
        .into_iter()
        .chain(relative.find(". "))
        .min()
        .unwrap_or(relative.len());
    let l = &relative[..end];
    let mut total: i64 = 0;
    let words: Vec<&str> = l
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect();
    for (i, w) in words.iter().enumerate() {
        let (num, unit) = match w.find(|c: char| !c.is_ascii_digit()) {
            Some(0) | None => (
                w.parse::<i64>().ok(),
                words.get(i + 1).copied().unwrap_or(""),
            ),
            Some(p) => (w[..p].parse::<i64>().ok(), &w[p..]),
        };
        let Some(n) = num else { continue };
        let ms = if unit.starts_with('w') {
            7 * 24 * 3_600_000
        } else if unit.starts_with('d') {
            24 * 3_600_000
        } else if unit.starts_with('h') {
            3_600_000
        } else if unit.starts_with("m") && !unit.starts_with("ms") {
            60_000
        } else if unit.starts_with('s') {
            1000
        } else {
            continue;
        };
        total = total.saturating_add(n.saturating_mul(ms));
    }
    (total > 0).then_some(total)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    pub account_id: String,
    pub state: String,
    pub until: Option<i64>,
    pub turns: usize,
}

#[derive(Default)]
pub struct Limits {
    cooldown: Mutex<HashMap<String, i64>>,
    model_cooldown: Mutex<HashMap<(String, String), i64>>,
    turns: Mutex<HashMap<String, Vec<i64>>>,
}
impl Limits {
    pub fn cool_model(&self, account: &str, model: &str, until: i64) {
        if let Ok(mut cooldown) = self.model_cooldown.lock() {
            cooldown.insert((account.into(), model.into()), until);
        }
    }
    pub fn eligible_model(&self, account: &str, model: &str, now: i64) -> bool {
        self.model_cooldown.lock().is_ok_and(|mut cooldown| {
            cooldown.retain(|_, until| *until > now);
            !cooldown.contains_key(&(account.into(), model.into()))
        })
    }
    pub fn record_turn(&self, account: &str, now: i64) {
        if let Ok(mut t) = self.turns.lock() {
            let v = t.entry(account.into()).or_default();
            v.retain(|x| now - x < FIVE_HOURS);
            v.push(now);
        }
    }
    pub fn cool(&self, account: &str, until: i64) {
        if let Ok(mut c) = self.cooldown.lock() {
            let current = c.entry(account.into()).or_default();
            *current = (*current).max(until);
        }
    }
    pub fn cooling(&self, account: &str, now: i64) -> bool {
        self.cooldown
            .lock()
            .map(|c| c.get(account).is_some_and(|u| *u > now))
            .unwrap_or(false)
    }
    pub fn health(&self, account: &str, pressure: Option<f64>, now: i64) -> Health {
        let until = self
            .cooldown
            .lock()
            .ok()
            .and_then(|c| c.get(account).copied())
            .filter(|u| *u > now);
        let turns = self
            .turns
            .lock()
            .map(|t| {
                t.get(account)
                    .map(|v| v.iter().filter(|x| now - **x < FIVE_HOURS).count())
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        let state = if until.is_some() || pressure.is_some_and(|p| p >= 1000.0) {
            "cooldown"
        } else if pressure.is_some_and(|p| p >= 1.6) {
            "near"
        } else {
            "active"
        };
        Health {
            account_id: account.into(),
            state: state.into(),
            until,
            turns,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_and_parses() {
        assert!(is_rate_limit("Error 429: Too Many Requests"));
        assert!(is_rate_limit("You've hit your monthly spend limit"));
        assert!(is_rate_limit(
            "You've hit your usage limit. Try again in 2h 5m."
        ));
        assert!(!is_rate_limit("All tests passed"));
        assert_eq!(
            unlock_at("try again in 2h 5m", 0),
            2 * 3_600_000 + 5 * 60_000
        );
        assert_eq!(unlock_at("resets in 37 minutes", 0), 37 * 60_000);
        assert_eq!(unlock_at("limit reached", 0), 15 * 60_000);
        let l = Limits::default();
        l.cool("a", 100);
        assert!(l.cooling("a", 50));
        assert_eq!(l.health("a", None, 50).state, "cooldown");
        assert_eq!(l.health("b", Some(2.0), 50).state, "near");
    }
    #[test]
    fn quota_window_name_is_not_a_retry_delay() {
        assert_eq!(
            unlock_at("You hit the 5-hour limit. Resets in 30 minutes.", 0),
            30 * 60_000
        );
        assert_eq!(unlock_at("You hit the 5-hour limit.", 0), 15 * 60_000);
        assert_eq!(
            unlock_at("Retry in 999999999999999999 hours", 0),
            7 * 24 * 3_600_000
        );
    }

    /// Local wall-clock time in a fixed +01:00 zone, as milliseconds.
    fn local(zone: &chrono::FixedOffset, y: i32, mo: u32, d: u32, h: u32, mi: u32) -> i64 {
        use chrono::TimeZone;
        zone.with_ymd_and_hms(y, mo, d, h, mi, 0)
            .unwrap()
            .timestamp_millis()
    }

    #[test]
    fn codex_states_its_reset_as_a_local_clock_time() {
        // Codex 0.1xx: "… or try again at 1:18 PM." Lumen read only "try again in" and showed
        // its 15-minute fallback (09:18) for a quota that reset after 13:00.
        let zone = chrono::FixedOffset::east_opt(3600).unwrap();
        let now = local(&zone, 2026, 11, 28, 9, 3);
        let message = "You've hit your usage limit. Visit https://chatgpt.com/codex/settings/usage to purchase more credits or try again at 1:18 PM.";
        assert_eq!(
            stated_reset_in(message, now, &zone),
            Some(local(&zone, 2026, 11, 28, 13, 18))
        );
        assert_eq!(
            stated_reset_in(
                "You've hit your usage limit. Try again at Dec 3rd, 2026 1:18 PM.",
                now,
                &zone
            ),
            Some(local(&zone, 2026, 12, 3, 13, 18))
        );
        assert_eq!(
            stated_reset_in("Try again in 3 days 2 hours.", now, &zone),
            Some(now + 74 * 3_600_000)
        );
        assert_eq!(stated_reset_in("Try again later.", now, &zone), None);
        // Dotted meridiems keep their meaning: not 01:18 at night.
        assert_eq!(
            stated_reset_in("Try again at 1:18 p.m. today.", now, &zone),
            Some(local(&zone, 2026, 11, 28, 13, 18))
        );
        assert_eq!(
            stated_reset_in("Your limit resets mark 3pm", now, &zone),
            None,
            "a word starting with a month abbreviation is not a date"
        );
        assert_eq!(
            stated_reset_in("Try again at December 3, 2026 9:00 AM", now, &zone),
            Some(local(&zone, 2026, 12, 3, 9, 0))
        );
    }

    #[test]
    fn a_reset_more_than_a_week_away_is_waited_for_at_most_a_week() {
        let zone = chrono::FixedOffset::east_opt(3600).unwrap();
        let now = local(&zone, 2026, 11, 28, 9, 3);
        let far = stated_reset_in("Try again at Dec 3rd, 2200 1:18 PM.", now, &zone).unwrap();
        assert!(far > now + WEEK);
        assert_eq!(unlock_at("Try again in 3 weeks", 0), WEEK);
    }

    #[test]
    fn claude_states_its_reset_as_an_hour_or_an_epoch() {
        let zone = chrono::FixedOffset::east_opt(3600).unwrap();
        let now = local(&zone, 2026, 11, 28, 9, 3);
        assert_eq!(
            stated_reset_in(
                "You've hit your limit · resets 3pm (Europe/Rome)",
                now,
                &zone
            ),
            Some(local(&zone, 2026, 11, 28, 15, 0))
        );
        assert_eq!(
            stated_reset_in("5-hour limit reached ∙ resets Dec 1, 2:30pm", now, &zone),
            Some(local(&zone, 2026, 12, 1, 14, 30))
        );
        let late = local(&zone, 2026, 11, 28, 23, 0);
        assert_eq!(
            stated_reset_in("Session limit reached ∙ resets 1am", late, &zone),
            Some(local(&zone, 2026, 11, 29, 1, 0))
        );
        let epoch = now / 1000 + 4 * 3600;
        assert_eq!(
            stated_reset_in(
                &format!("Claude AI usage limit reached|{epoch}"),
                now,
                &zone
            ),
            Some(epoch * 1000)
        );
        // A past epoch, a bare number or a window name is not a reset time.
        assert_eq!(
            stated_reset_in("usage limit reached|1000000000", now, &zone),
            None
        );
        assert_eq!(
            stated_reset_in("Your limit resets 3 times a day", now, &zone),
            None
        );
        assert_eq!(
            stated_reset_in("You hit the 5-hour limit.", now, &zone),
            None
        );
    }
}
