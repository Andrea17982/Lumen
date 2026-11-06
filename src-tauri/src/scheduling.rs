//! Orchestra admission and assignment. The
//! caller maps accounts, catalogs and quota snapshots into the small types
//! below, starts whatever [`Scheduler::next`] returns, reports each outcome with
//! [`Scheduler::finish`] and calls `next` again whenever capacity, limits or
//! quotas change. Nothing here spawns or retries on its own, so a rate limit can
//! never turn into a retry loop.
use std::collections::{HashMap, HashSet};

/// Matches `PARALLEL` in `orchestra.rs`.
pub const DEFAULT_CONCURRENT: usize = 6;
/// Snapshots older than this still count, but are re-checked with one job first.
pub const FRESH_MS: i64 = 900_000;
const NEUTRAL_PRESSURE: f64 = 1.0;
const LOAD_WEIGHT: f64 = 0.5;

/// Optional caps; `None` means no cap of the user's own.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Limits {
    pub max_concurrent: Option<usize>,
    /// Admitted per run. Finishing a subtask never gives its place back.
    pub max_subtasks: Option<usize>,
    /// Used percentage, keyed by provider id ("claude", "gpt", ...).
    pub quota_thresholds: HashMap<String, f64>,
}

impl Limits {
    pub fn concurrent(&self) -> usize {
        self.max_concurrent.unwrap_or(DEFAULT_CONCURRENT).max(1)
    }
    /// The only way the total grows: admit `extra` more after `admitted`.
    pub fn raise_total(&mut self, admitted: usize, extra: usize) {
        if let Some(m) = self.max_subtasks.as_mut() {
            *m = (*m).max(admitted) + extra;
        }
    }
}

#[derive(Clone, Debug)]
pub struct Model {
    pub id: String,
    /// `router::model_tier`.
    pub tier: i32,
    pub is_default: bool,
}

#[derive(Clone, Debug)]
pub struct SchedAccount {
    pub id: String,
    pub provider: String,
    /// `Provider::fallback_rank`: emergency providers take only trivial work.
    pub rank: u8,
    /// False for text-only providers (Ollama).
    pub can_write: bool,
    /// Loaded catalog; without one the account only keeps its planned work.
    pub models: Option<Vec<Model>>,
}

#[derive(Clone, Debug)]
pub struct Window {
    pub used_percent: f64,
    pub resets_at: i64,
    pub window_minutes: i64,
}

#[derive(Clone, Debug)]
pub struct Usage {
    pub updated_at: i64,
    pub windows: Vec<Window>,
}

#[derive(Clone, Debug, Default)]
pub struct SubtaskInput {
    pub id: String,
    /// 0 trivial · 1 normal · 2 complex (`tier_of`).
    pub tier: i32,
    pub writes: bool,
    pub depends_on: Vec<String>,
    pub account_id: String,
    pub model: String,
    /// "low" · "medium" · "high".
    pub effort: String,
    /// Chosen in the plan editor: never moved.
    pub pinned: bool,
    /// "done" / "failed" seed finished work on resume; they count as admitted.
    pub status: String,
    /// Attempts already spent before a restart; they count against the budget.
    pub attempts: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Done,
    Failed(String),
    RateLimited(Option<i64>),
    Cancelled,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Start {
    pub id: String,
    pub account_id: String,
    pub model: String,
    pub switched: bool,
    /// First job on an account whose quota is missing or stale.
    pub probe: bool,
    pub attempt: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wait {
    Dependencies,
    Concurrency,
    TotalCap,
    Probe,
    Cooldown,
    Quota,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Pending,
    Running,
    Done,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    Known,
    /// Below the threshold, but from an outdated snapshot.
    Stale,
    Over,
    Unknown,
}

fn fresh(u: &Usage, now: i64) -> bool {
    u.updated_at <= now && now - u.updated_at <= FRESH_MS
}

fn live(u: &Usage, now: i64) -> impl Iterator<Item = &Window> {
    u.windows.iter().filter(move |w| {
        w.resets_at > now
            && w.window_minutes > 0
            && w.used_percent.is_finite()
            && (0.0..=100.0).contains(&w.used_percent)
    })
}

/// Like `router::usage_pressure`, but outdated snapshots still count: their
/// windows keep applying until they reset.
pub fn pressure(u: Option<&Usage>, now: i64) -> Option<f64> {
    live(u?, now)
        .map(|w| {
            if w.used_percent >= 100.0 {
                return 1000.0;
            }
            let time = ((w.resets_at - now) as f64 / (w.window_minutes as f64 * 60_000.0))
                .clamp(0.01, 1.0);
            time / ((100.0 - w.used_percent) / 100.0).max(0.01)
        })
        .reduce(f64::max)
}

fn verdict(u: Option<&Usage>, threshold: Option<f64>, now: i64) -> Verdict {
    let Some(u) = u else {
        return Verdict::Unknown;
    };
    let Some(used) = live(u, now).map(|w| w.used_percent).reduce(f64::max) else {
        return Verdict::Unknown;
    };
    if used >= threshold.unwrap_or(100.0).min(100.0) {
        Verdict::Over
    } else if fresh(u, now) {
        Verdict::Known
    } else {
        Verdict::Stale
    }
}

/// Closest catalog model at or above `floor`; aliases cannot prove a tier.
fn select_at_least(models: &[Model], floor: i32) -> Option<&Model> {
    models
        .iter()
        .filter(|m| m.tier >= floor && m.id != "default" && m.id != "auto")
        .min_by_key(|m| (m.tier - floor, !m.is_default))
}

struct Task {
    input: SubtaskInput,
    state: State,
    admitted: bool,
    attempts: u32,
    errors: u32,
    tried: HashSet<String>,
    floor: i32,
    cost: f64,
    current: Option<(String, String)>,
    error: String,
}

pub struct Options {
    pub max_attempts: u32,
    pub max_errors: u32,
    pub probe_limit: usize,
    pub cooldown_ms: i64,
    pub switch_margin: f64,
    pub read_only: bool,
    pub rebalance: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            max_errors: 2,
            probe_limit: 1,
            cooldown_ms: 15 * 60_000,
            switch_margin: 0.25,
            read_only: false,
            rebalance: true,
        }
    }
}

pub struct Scheduler {
    tasks: Vec<Task>,
    accounts: Vec<SchedAccount>,
    usage: HashMap<String, Usage>,
    cooldown: HashMap<String, i64>,
    confirmed: HashSet<String>,
    pub limits: Limits,
    opts: Options,
}

struct Candidate<'a> {
    account: &'a SchedAccount,
    model: String,
    switched: bool,
}

impl Scheduler {
    pub fn new(
        subtasks: Vec<SubtaskInput>,
        accounts: Vec<SchedAccount>,
        limits: Limits,
        opts: Options,
    ) -> Self {
        let tasks = subtasks
            .into_iter()
            .map(|t| {
                let planned = accounts
                    .iter()
                    .find(|a| a.id == t.account_id)
                    .and_then(|a| a.models.as_ref())
                    .and_then(|m| m.iter().find(|m| m.id == t.model))
                    .map_or(0, |m| m.tier);
                let tier = t.tier.clamp(0, 2);
                let effort = match t.effort.as_str() {
                    "low" => 0.75,
                    "high" => 1.5,
                    _ => 1.0,
                };
                let state = match t.status.as_str() {
                    "done" => State::Done,
                    "failed" => State::Failed,
                    _ => State::Pending,
                };
                Task {
                    floor: tier.max(planned),
                    cost: [1.0, 2.0, 4.0][tier as usize] * effort,
                    admitted: state != State::Pending,
                    state,
                    attempts: t.attempts,
                    errors: 0,
                    tried: HashSet::new(),
                    current: None,
                    error: String::new(),
                    input: t,
                }
            })
            .collect();
        Self {
            tasks,
            accounts,
            usage: HashMap::new(),
            cooldown: HashMap::new(),
            confirmed: HashSet::new(),
            limits,
            opts,
        }
    }

    pub fn set_usage(&mut self, account_id: &str, u: Usage) {
        self.usage.insert(account_id.to_string(), u);
    }

    /// A rate limit seen outside this run (another turn on the same account).
    pub fn cool_down(&mut self, account_id: &str, until: i64) {
        let e = self.cooldown.entry(account_id.to_string()).or_insert(0);
        *e = (*e).max(until);
        self.confirmed.remove(account_id);
    }

    pub fn running(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| t.state == State::Running)
            .count()
    }

    pub fn admitted(&self) -> usize {
        self.tasks.iter().filter(|t| t.admitted).count()
    }

    fn finished(&self) -> HashSet<String> {
        self.tasks
            .iter()
            .filter(|t| !matches!(t.state, State::Pending | State::Running))
            .map(|t| t.input.id.clone())
            .collect()
    }

    fn threshold(&self, a: &SchedAccount) -> Option<f64> {
        self.limits.quota_thresholds.get(&a.provider).copied()
    }

    fn capable(&self, t: &Task) -> Vec<Candidate<'_>> {
        let mut out = vec![];
        let planned = self.accounts.iter().find(|a| a.id == t.input.account_id);
        let writes = t.input.writes && !self.opts.read_only;
        if let Some(a) = planned.filter(|a| !t.tried.contains(&a.id) && (!writes || a.can_write)) {
            out.push(Candidate {
                account: a,
                model: t.input.model.clone(),
                switched: false,
            });
        }
        if t.input.pinned || !self.opts.rebalance {
            return out;
        }
        let rank = planned
            .map_or(0, |a| a.rank)
            .max(if t.floor == 0 { 3 } else { 0 });
        for a in &self.accounts {
            if a.id == t.input.account_id
                || t.tried.contains(&a.id)
                || a.rank > rank
                || (writes && !a.can_write)
            {
                continue;
            }
            if let Some(m) = a
                .models
                .as_deref()
                .and_then(|m| select_at_least(m, t.floor))
            {
                out.push(Candidate {
                    account: a,
                    model: m.id.clone(),
                    switched: true,
                });
            }
        }
        out
    }

    fn on_account(&self, id: &str) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|t| t.state == State::Running && t.current.as_ref().is_some_and(|c| c.0 == id))
            .collect()
    }

    fn blocked(&self, a: &SchedAccount, now: i64) -> Option<Wait> {
        if self.cooldown.get(&a.id).is_some_and(|&u| u > now) {
            return Some(Wait::Cooldown);
        }
        match verdict(self.usage.get(&a.id), self.threshold(a), now) {
            Verdict::Over => Some(Wait::Quota),
            Verdict::Unknown | Verdict::Stale
                if !self.confirmed.contains(&a.id)
                    && self.on_account(&a.id).len() >= self.opts.probe_limit =>
            {
                Some(Wait::Probe)
            }
            _ => None,
        }
    }

    fn score(&self, a: &SchedAccount, t: &Task, now: i64) -> f64 {
        let p = pressure(self.usage.get(&a.id), now).unwrap_or(NEUTRAL_PRESSURE);
        let load: f64 = self.on_account(&a.id).iter().map(|r| r.cost).sum::<f64>() + t.cost;
        p * (1.0 + LOAD_WEIGHT * load)
    }

    /// Admit and assign everything that may start now, in plan order.
    pub fn next(&mut self, now: i64) -> Vec<Start> {
        let mut finished = self.finished();
        let total = self.limits.max_subtasks.unwrap_or(usize::MAX);
        let mut starts = vec![];
        for i in 0..self.tasks.len() {
            let t = &self.tasks[i];
            if t.state != State::Pending || !t.input.depends_on.iter().all(|d| finished.contains(d))
            {
                continue;
            }
            if self.running() >= self.limits.concurrent() {
                break;
            }
            if !t.admitted && self.admitted() >= total {
                continue;
            }
            let options = self.capable(t);
            if options.is_empty() {
                let t = &mut self.tasks[i];
                t.state = State::Failed;
                if t.error.is_empty() {
                    t.error =
                        "No remaining account can take this subtask at the required model class."
                            .into();
                }
                finished.insert(t.input.id.clone());
                continue;
            }
            let mut best: Option<(f64, &Candidate)> = None;
            let mut kept: Option<(f64, &Candidate)> = None;
            for o in options
                .iter()
                .filter(|o| self.blocked(o.account, now).is_none())
            {
                let s = self.score(o.account, t, now);
                if best.is_none_or(|(b, _)| s < b) {
                    best = Some((s, o));
                }
                if !o.switched {
                    kept = Some((s, o));
                }
            }
            let Some((best_score, best)) = best else {
                continue;
            };
            let pick = match kept {
                Some((s, k)) if s <= best_score * (1.0 + self.opts.switch_margin) => k,
                _ => best,
            };
            let probe = matches!(
                verdict(
                    self.usage.get(&pick.account.id),
                    self.threshold(pick.account),
                    now,
                ),
                Verdict::Unknown | Verdict::Stale
            ) && !self.confirmed.contains(&pick.account.id);
            let (account_id, model, switched) =
                (pick.account.id.clone(), pick.model.clone(), pick.switched);
            let t = &mut self.tasks[i];
            t.state = State::Running;
            t.admitted = true;
            t.attempts += 1;
            t.current = Some((account_id.clone(), model.clone()));
            starts.push(Start {
                id: t.input.id.clone(),
                account_id,
                model,
                switched,
                probe,
                attempt: t.attempts,
            });
        }
        starts
    }

    pub fn finish(&mut self, id: &str, outcome: Outcome, now: i64) {
        let cooldown_ms = self.opts.cooldown_ms;
        let (max_attempts, max_errors) = (self.opts.max_attempts, self.opts.max_errors);
        let Some(t) = self
            .tasks
            .iter_mut()
            .find(|t| t.input.id == id && t.state == State::Running)
        else {
            return;
        };
        let Some((account, _)) = t.current.clone() else {
            return;
        };
        let exhausted = t.attempts >= max_attempts;
        let mut cool = None;
        match outcome {
            Outcome::Done => {
                t.state = State::Done;
                t.error.clear();
                self.confirmed.insert(account.clone());
            }
            Outcome::Cancelled => t.state = State::Cancelled,
            Outcome::RateLimited(until) => {
                cool = Some(until.unwrap_or(now + cooldown_ms));
                t.error = "Rate limited".into();
                t.state = if exhausted {
                    State::Failed
                } else {
                    State::Pending
                };
            }
            Outcome::Failed(e) => {
                t.errors += 1;
                t.tried.insert(account.clone());
                t.error = e;
                t.state = if exhausted || t.errors >= max_errors {
                    State::Failed
                } else {
                    State::Pending
                };
            }
        }
        if t.state == State::Pending {
            t.current = None;
        }
        if let Some(until) = cool {
            self.cool_down(&account, until);
        }
    }

    pub fn cancel_pending(&mut self) {
        for t in self.tasks.iter_mut().filter(|t| t.state == State::Pending) {
            t.state = State::Cancelled;
        }
    }

    pub fn state(&self, id: &str) -> Option<State> {
        self.tasks
            .iter()
            .find(|t| t.input.id == id)
            .map(|t| t.state)
    }

    pub fn error(&self, id: &str) -> Option<&str> {
        self.tasks
            .iter()
            .find(|t| t.input.id == id)
            .map(|t| t.error.as_str())
            .filter(|e| !e.is_empty())
    }

    pub fn complete(&self) -> bool {
        self.tasks
            .iter()
            .all(|t| !matches!(t.state, State::Pending | State::Running))
    }

    /// Nothing runs and nothing can start until limits, quotas or time change.
    #[cfg(test)]
    pub fn suspended(&self) -> bool {
        !self.complete() && self.running() == 0
    }

    /// Why each pending subtask waits, plus the earliest moment something can unblock.
    pub fn waiting(&self, now: i64) -> (Vec<(String, Wait)>, Option<i64>) {
        let finished = self.finished();
        let (running, admitted) = (self.running(), self.admitted());
        let total = self.limits.max_subtasks;
        let mut out = vec![];
        let mut wake: Vec<i64> = vec![];
        for t in self.tasks.iter().filter(|t| t.state == State::Pending) {
            let reason = if !t.input.depends_on.iter().all(|d| finished.contains(d)) {
                Wait::Dependencies
            } else if running >= self.limits.concurrent() {
                Wait::Concurrency
            } else if !t.admitted && total.is_some_and(|m| admitted >= m) {
                Wait::TotalCap
            } else {
                let options = self.capable(t);
                let reasons: Vec<Option<Wait>> = options
                    .iter()
                    .map(|o| self.blocked(o.account, now))
                    .collect();
                if reasons.contains(&None) {
                    continue;
                }
                for o in &options {
                    wake.extend(self.wake_for(o.account, now));
                }
                if reasons.contains(&Some(Wait::Probe)) {
                    Wait::Probe
                } else if reasons.contains(&Some(Wait::Cooldown)) {
                    Wait::Cooldown
                } else {
                    Wait::Quota
                }
            };
            out.push((t.input.id.clone(), reason));
        }
        (out, wake.into_iter().min())
    }

    fn wake_for(&self, a: &SchedAccount, now: i64) -> Vec<i64> {
        let mut out = vec![];
        if let Some(&u) = self.cooldown.get(&a.id).filter(|&&u| u > now) {
            out.push(u);
        }
        if let Some(u) = self.usage.get(&a.id) {
            if verdict(Some(u), self.threshold(a), now) == Verdict::Over {
                out.extend(u.windows.iter().map(|w| w.resets_at).filter(|&r| r > now));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const NOW: i64 = 1_800_000_000_000;

    fn models(ids: &[(&str, i32)]) -> Option<Vec<Model>> {
        Some(
            ids.iter()
                .map(|(id, tier)| Model {
                    id: id.to_string(),
                    tier: *tier,
                    is_default: *tier == 1,
                })
                .collect(),
        )
    }
    fn claude() -> SchedAccount {
        SchedAccount {
            id: "c".into(),
            provider: "claude".into(),
            rank: 0,
            can_write: true,
            models: models(&[
                ("claude-haiku", 0),
                ("claude-sonnet", 1),
                ("claude-opus", 2),
            ]),
        }
    }
    fn gpt() -> SchedAccount {
        SchedAccount {
            id: "g".into(),
            provider: "gpt".into(),
            rank: 0,
            can_write: true,
            models: models(&[("gpt-mini", 0), ("gpt-terra", 1), ("gpt-sol", 2)]),
        }
    }
    fn usage(used: f64, at: i64) -> Usage {
        Usage {
            updated_at: at,
            windows: vec![Window {
                used_percent: used,
                resets_at: NOW + 3_600_000,
                window_minutes: 300,
            }],
        }
    }
    fn tasks(n: usize, pinned: bool) -> Vec<SubtaskInput> {
        (1..=n)
            .map(|i| SubtaskInput {
                id: format!("s{i}"),
                tier: 1,
                writes: true,
                account_id: "c".into(),
                model: "claude-sonnet".into(),
                effort: "medium".into(),
                pinned,
                ..Default::default()
            })
            .collect()
    }
    fn make(list: Vec<SubtaskInput>, limits: Limits, accounts: Vec<SchedAccount>) -> Scheduler {
        let ids: Vec<String> = accounts.iter().map(|a| a.id.clone()).collect();
        let mut s = Scheduler::new(list, accounts, limits, Options::default());
        for id in ids {
            s.set_usage(&id, usage(10.0, NOW));
        }
        s
    }
    fn ids(v: &[Start]) -> Vec<&str> {
        v.iter().map(|s| s.id.as_str()).collect()
    }

    #[test]
    fn concurrency_cap_and_slot_refill() {
        let limits = Limits {
            max_concurrent: Some(2),
            ..Default::default()
        };
        let mut s = make(tasks(4, false), limits, vec![claude(), gpt()]);
        assert_eq!(ids(&s.next(NOW)), ["s1", "s2"]);
        assert!(s.next(NOW).is_empty());
        s.finish("s1", Outcome::Done, NOW);
        assert_eq!(ids(&s.next(NOW)), ["s3"]);
    }

    #[test]
    fn total_cap_needs_an_explicit_raise() {
        let limits = Limits {
            max_subtasks: Some(2),
            ..Default::default()
        };
        let mut s = make(tasks(4, false), limits, vec![claude(), gpt()]);
        assert_eq!(s.next(NOW).len(), 2);
        s.finish("s1", Outcome::Done, NOW);
        s.finish("s2", Outcome::Done, NOW);
        assert!(s.next(NOW).is_empty());
        assert!(s.suspended());
        assert_eq!(s.waiting(NOW).0[0].1, Wait::TotalCap);
        s.limits.raise_total(2, 1);
        assert_eq!(ids(&s.next(NOW)), ["s3"]);
        assert!(s.next(NOW).is_empty());
    }

    #[test]
    fn thresholds_move_work_and_suspend_when_all_are_over() {
        let mut limits = Limits::default();
        limits.quota_thresholds.insert("claude".into(), 80.0);
        limits.quota_thresholds.insert("gpt".into(), 80.0);
        let mut s = make(tasks(2, false), limits, vec![claude(), gpt()]);
        s.set_usage("c", usage(85.0, NOW));
        let started = s.next(NOW);
        assert!(started.iter().all(|x| x.account_id == "g" && x.switched));
        assert_eq!(started[0].model, "gpt-terra");
        let mut s2 = make(tasks(1, false), s.limits.clone(), vec![claude(), gpt()]);
        s2.set_usage("c", usage(90.0, NOW));
        s2.set_usage("g", usage(90.0, NOW));
        assert!(s2.next(NOW).is_empty());
        assert_eq!(
            s2.waiting(NOW),
            (vec![("s1".into(), Wait::Quota)], Some(NOW + 3_600_000))
        );
        // An outdated reading still holds until its window resets.
        assert!(s2.next(NOW + FRESH_MS + 1).is_empty());
        assert!(s2.next(NOW + 3_600_001)[0].probe);
    }

    #[test]
    fn outdated_quota_still_counts_and_is_checked_with_one_job() {
        let mut s = make(tasks(3, true), Limits::default(), vec![claude()]);
        s.set_usage("c", usage(10.0, NOW - FRESH_MS - 1));
        let first = s.next(NOW);
        assert_eq!(ids(&first), ["s1"]);
        assert!(first[0].probe);
        s.finish("s1", Outcome::Done, NOW);
        assert_eq!(ids(&s.next(NOW)), ["s2", "s3"]);
    }

    #[test]
    fn missing_quota_is_probed_one_job_at_a_time() {
        let mut s = Scheduler::new(
            tasks(3, false),
            vec![claude()],
            Limits::default(),
            Options::default(),
        );
        let first = s.next(NOW);
        assert_eq!(ids(&first), ["s1"]);
        assert!(first[0].probe);
        assert_eq!(s.waiting(NOW).0[0].1, Wait::Probe);
        s.finish("s1", Outcome::Done, NOW);
        assert_eq!(ids(&s.next(NOW)), ["s2", "s3"]);
    }

    #[test]
    fn rate_limits_cool_down_and_attempts_are_bounded() {
        let mut s = make(tasks(1, true), Limits::default(), vec![claude()]);
        let mut t = NOW;
        for attempt in 1..=3 {
            assert_eq!(s.next(t)[0].attempt, attempt);
            s.finish("s1", Outcome::RateLimited(Some(t + 1000)), t);
            assert!(s.next(t).is_empty());
            if attempt < 3 {
                assert_eq!(
                    s.waiting(t),
                    (vec![("s1".into(), Wait::Cooldown)], Some(t + 1000))
                );
            }
            t += 1000;
        }
        assert_eq!(s.state("s1"), Some(State::Failed));
        assert!(s.complete());
    }

    #[test]
    fn failures_retry_once_elsewhere_and_never_lower_the_class() {
        let mut list = tasks(1, false);
        list[0].model = "claude-opus".into();
        let mut s = make(list, Limits::default(), vec![claude(), gpt()]);
        s.next(NOW);
        s.finish("s1", Outcome::Failed("boom".into()), NOW);
        let retry = s.next(NOW);
        assert_eq!(
            (retry[0].account_id.as_str(), retry[0].model.as_str()),
            ("g", "gpt-sol")
        );
        s.finish("s1", Outcome::Failed("again".into()), NOW);
        assert!(s.next(NOW).is_empty());
        assert_eq!(s.error("s1"), Some("again"));
    }

    #[test]
    fn balances_predicted_pressure() {
        let mut list = tasks(4, false);
        for t in list.iter_mut() {
            t.tier = 2;
            t.model = "claude-opus".into();
        }
        let mut s = make(list, Limits::default(), vec![claude(), gpt()]);
        let by: Vec<String> = s.next(NOW).into_iter().map(|x| x.account_id).collect();
        assert_eq!(by.iter().filter(|a| *a == "c").count(), 2);
    }

    #[test]
    fn writing_work_stays_off_text_only_providers() {
        let ollama = SchedAccount {
            id: "o".into(),
            provider: "ollama".into(),
            rank: 3,
            can_write: false,
            models: models(&[("llama", 0)]),
        };
        let mut list = tasks(1, false);
        list[0].tier = 0;
        let mut s = make(list, Limits::default(), vec![claude(), ollama]);
        s.set_usage("c", usage(100.0, NOW));
        assert!(s.next(NOW).is_empty());
    }

    #[test]
    fn pinned_writing_work_cannot_use_planned_text_only_provider() {
        let mut account = claude();
        account.can_write = false;
        let mut s = make(tasks(1, true), Limits::default(), vec![account]);
        assert!(s.next(NOW).is_empty());
        assert_eq!(s.state("s1"), Some(State::Failed));
    }
}
