//! Rate-limit detection, per-account cooldowns and a local 5-hour turn counter.
use serde::Serialize;
use std::{collections::HashMap, sync::Mutex};

pub const RATE_LIMIT: &str = "RATE_LIMIT: ";
const FIVE_HOURS: i64 = 5 * 3_600_000;

pub fn is_rate_limit(text: &str) -> bool {
    let l = text.to_lowercase();
    [
        "rate limit",
        "rate_limit",
        "ratelimit",
        "429",
        "usage limit",
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

/// Best-effort unlock time from messages such as "try again in 2h 5m" or "resets in 37 minutes".
pub fn unlock_at(text: &str, now: i64) -> i64 {
    let l = text.to_lowercase();
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
        let ms = if unit.starts_with('h') {
            3_600_000
        } else if unit.starts_with("m") && !unit.starts_with("ms") {
            60_000
        } else if unit.starts_with('s') {
            1000
        } else {
            continue;
        };
        total += n * ms;
    }
    now + if total > 0 {
        total.min(7 * 24 * 3_600_000)
    } else {
        15 * 60_000
    }
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
    turns: Mutex<HashMap<String, Vec<i64>>>,
}
impl Limits {
    pub fn record_turn(&self, account: &str, now: i64) {
        if let Ok(mut t) = self.turns.lock() {
            let v = t.entry(account.into()).or_default();
            v.retain(|x| now - x < FIVE_HOURS);
            v.push(now);
        }
    }
    pub fn cool(&self, account: &str, until: i64) {
        if let Ok(mut c) = self.cooldown.lock() {
            c.insert(account.into(), until);
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
}
