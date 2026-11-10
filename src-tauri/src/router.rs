use crate::models::*;

/// What a request needs. `effort` 0..=2 maps to low/medium/high before the Auto policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TaskProfile {
    pub tier: i32,
    pub effort: i32,
    pub review: bool,
}

/// Calibrated against the user's Italian examples. Length raises effort only;
/// a long email should not automatically require a flagship model.
pub(crate) fn profile(prompt: &str) -> TaskProfile {
    let p = prompt.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| p.contains(w));
    let advanced = has(&[
        "security",
        "sicurezza",
        "authentication",
        "autenticazione",
        "migration",
        "migrazione",
        "architecture",
        "architettura",
    ]);
    let debugging = has(&[
        "bug",
        "debug",
        "css",
        "concurrency",
        "concorrenza",
        "refactor",
        "errore",
        "code",
        "codice",
        "implement",
        "svilupp",
        "costruisci",
    ]);
    let translation = has(&["traduci", "translate", "translation", "traduzione"]);
    let demanding = has(&[
        "intermittent",
        "intermittente",
        "concurrency",
        "concorrenza",
        "distributed",
        "distribuit",
        "production",
        "produzione",
        "zero downtime",
        "complesso",
        "complex",
    ]);
    let tier = if advanced {
        2
    } else if debugging && !translation {
        1
    } else {
        0
    };
    TaskProfile {
        tier,
        effort: i32::from(demanding || prompt.len() > 1200),
        review: advanced && (demanding || prompt.len() > 1800)
            || has(&[
                "validator",
                "validatore",
                "verifier",
                "revisore",
                "independent review",
            ]),
    }
}

fn requested_effort(task: TaskProfile, policy: &AutoPolicy) -> &'static str {
    let adjustment = match policy {
        AutoPolicy::Budget => -1,
        AutoPolicy::Quality => 1,
        AutoPolicy::Default => 0,
    };
    ["low", "medium", "high"][(task.effort + adjustment).clamp(0, 2) as usize]
}

/// Keyword routing, for callers without a classifier verdict.
#[cfg(test)]
pub fn route(prompt: &str, options: &RunOptions, accounts: &[Account]) -> Result<Plan, String> {
    route_with(profile(prompt), options, accounts)
}

/// Plans a turn for a rated task (`crate::classify`, or the keyword profile).
pub fn route_with(
    task: TaskProfile,
    options: &RunOptions,
    accounts: &[Account],
) -> Result<Plan, String> {
    if !["auto", "claude", "gpt", "both"].contains(&options.mode.as_str()) {
        return Err("Unknown provider mode".into());
    }
    if options.effort.len() > 32 || !options.effort.chars().all(|c| c.is_ascii_lowercase()) {
        return Err("Unknown effort".into());
    }
    if let Some(m) = &options.model {
        if m.len() > 100
            || !m
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-._:/[]".contains(c))
        {
            return Err("Invalid model identifier".into());
        }
    }
    let preferred = match options.mode.as_str() {
        "claude" | "both" => Provider::Claude,
        "gpt" => Provider::Gpt,
        _ if task.tier == 2 => Provider::Claude,
        _ => Provider::Gpt,
    };
    let selected = if options.mode == "both" {
        accounts.iter().find(|a| a.provider == Provider::Claude)
    } else if options.mode != "auto" {
        options
            .account_id
            .as_ref()
            .and_then(|id| {
                accounts
                    .iter()
                    .find(|a| a.id == *id && a.provider == preferred)
            })
            .or_else(|| {
                if options.account_id.is_none() {
                    accounts.iter().find(|a| a.provider == preferred)
                } else {
                    None
                }
            })
    } else {
        accounts
            .iter()
            .find(|a| a.provider == preferred)
            .or_else(|| {
                accounts
                    .iter()
                    .find(|a| matches!(a.provider, Provider::Claude | Provider::Gpt))
            })
    };
    let a = selected.ok_or(
        "Add an account for this provider in Settings, then complete the official CLI login.",
    )?;
    let partner = accounts.iter().find(|b| {
        b.provider != a.provider && matches!(b.provider, Provider::Claude | Provider::Gpt)
    });
    if options.mode == "both" && partner.is_none() {
        return Err(
            "Both needs a Claude and a ChatGPT account. Add the missing one in Settings.".into(),
        );
    }
    let strategy = if options.mode == "both" {
        "broadcast"
    } else if options.mode == "auto" && partner.is_some() {
        if (task.review && options.auto_policy != AutoPolicy::Budget)
            || (options.auto_policy == AutoPolicy::Quality && task.tier > 0)
            || options.auto_review
        {
            "review"
        } else {
            "single"
        }
    } else if options.auto_review && partner.is_some() {
        "review"
    } else {
        "single"
    };
    let automatic = options.mode == "auto" || options.mode == "both";
    Ok(Plan {
        provider: a.provider.clone(),
        account_id: a.id.clone(),
        model: if automatic {
            None
        } else {
            options.model.clone().filter(|s| !s.is_empty())
        },
        effort: if automatic {
            requested_effort(task, &options.auto_policy).into()
        } else {
            options.effort.clone()
        },
        strategy: strategy.into(),
        reason: if !automatic && strategy == "single" {
            "Native session with the selected model and reasoning"
        } else if options.mode == "both" {
            "Same prompt to Claude and ChatGPT, side by side"
        } else if strategy == "review" && options.mode != "auto" {
            "Answer, then cross-audit by the other model"
        } else if task.tier == 2 {
            "Migration, architecture or security: flagship model with proportional reasoning"
        } else if task.tier == 1 {
            "Coding or debugging: Sonnet/Terra class with proportional reasoning"
        } else {
            "Direct response with a smaller reasoning budget"
        }
        .into(),
        partner_account: if strategy != "single" {
            partner.map(|a| a.id.clone())
        } else {
            None
        },
    })
}
/// Model class: 0 Haiku/Luna, 1 Sonnet/Terra (and unknown), 2 Opus/Sol.
pub fn model_tier(m: &ModelInfo) -> i32 {
    let name = format!("{} {}", m.id, m.name).to_lowercase();
    if ["luna", "mini", "nano", "haiku"]
        .iter()
        .any(|k| name.contains(k))
    {
        0
    } else if ["astra", "opus", "sol", "pro"]
        .iter()
        .any(|k| name.contains(k))
    {
        2
    } else {
        1
    }
}
/// Catalog aliases ("default", "auto") hide which model actually runs.
fn is_alias(m: &ModelInfo) -> bool {
    matches!(m.id.as_str(), "default" | "auto")
}
/// Closest catalog model to a class; named models beat aliases, then catalog defaults break ties.
pub fn select_tier(models: &[ModelInfo], target: i32) -> Option<&ModelInfo> {
    // Terra is also acceptable for small tasks, but prefer Luna when present.
    models
        .iter()
        .min_by_key(|m| ((model_tier(m) - target).abs(), is_alias(m), !m.is_default))
}
/// Use catalog IDs, never manufacture a model name unavailable to the account.
pub fn select_model<'a>(
    models: &'a [ModelInfo],
    prompt: &str,
    _policy: &AutoPolicy,
) -> Result<&'a ModelInfo, String> {
    // Budget lowers effort, preserving the capability needed by the task.
    select_tier(models, profile(prompt).tier).ok_or("No provider model available".into())
}
pub fn task_tier(prompt: &str) -> i32 {
    profile(prompt).tier
}
/// Orchestra takes over only for complex work: architecture/security/migration,
/// or coding that is demanding or long. Simple prompts keep the normal path.
pub fn complex(prompt: &str) -> bool {
    let t = profile(prompt);
    let checklist_items = prompt
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            line.starts_with("- ")
                || line.starts_with("* ")
                || line.starts_with("• ")
                || (line.len() >= 3
                    && line.as_bytes()[0].is_ascii_digit()
                    && matches!(line.as_bytes()[1], b'.' | b')')
                    && line.as_bytes()[2].is_ascii_whitespace())
        })
        .count();
    let explicit_parallel = [
        "multiple tasks",
        "several tasks",
        "many tasks",
        "parallel tasks",
        "multiple features",
        "several features",
        "multiple tasks",
        "several tasks",
        "many concurrent tasks",
        "molte task",
        "più task",
        "contemporaneamente",
        "contemporanee",
        "in parallelo",
        "più attività",
        "molte attività",
    ]
    .iter()
    .any(|phrase| prompt.to_lowercase().contains(phrase));
    checklist_items >= 3
        || (explicit_parallel && (checklist_items >= 2 || prompt.len() > 400))
        || t.tier == 2
        || (t.tier == 1 && (t.effort > 0 || prompt.len() > 600))
}
/// Quota pressure per time remaining. The tightest active window is the constraint.
/// Snapshots expire after 15 minutes; exhausted windows never receive reset bonuses.
pub fn usage_pressure(usage: &AccountUsage, at: i64) -> Option<f64> {
    if at - usage.updated_at > 900_000 || usage.updated_at > at {
        return None;
    }
    usage
        .windows
        .iter()
        .filter(|w| {
            w.resets_at > at
                && w.window_minutes > 0
                && w.used_percent.is_finite()
                && (0.0..=100.0).contains(&w.used_percent)
        })
        .map(|w| {
            if w.used_percent >= 100.0 {
                return 1000.0;
            }
            let remaining_time =
                ((w.resets_at - at) as f64 / (w.window_minutes as f64 * 60_000.0)).clamp(0.01, 1.0);
            remaining_time / ((100.0 - w.used_percent) / 100.0).max(0.01)
        })
        .reduce(f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn opts() -> RunOptions {
        RunOptions {
            mode: "auto".into(),
            account_id: None,
            model: None,
            effort: "medium".into(),
            write_access: false,
            auto_policy: AutoPolicy::Default,
            balance_usage: false,
            access_mode: None,
            auto_review: false,
            attachments: vec![],
            audit_of: None,
        }
    }
    fn accounts() -> Vec<Account> {
        vec![
            Account {
                id: "c".into(),
                name: "Claude".into(),
                provider: Provider::Claude,
            },
            Account {
                id: "g".into(),
                name: "GPT".into(),
                provider: Provider::Gpt,
            },
        ]
    }
    #[test]
    fn routing() {
        assert_eq!(
            route("hello", &opts(), &accounts()).unwrap().provider,
            Provider::Gpt
        );
        assert_eq!(
            route("security refactor in production", &opts(), &accounts())
                .unwrap()
                .strategy,
            "review"
        );
        assert_eq!(
            route("compare alternatives", &opts(), &accounts())
                .unwrap()
                .strategy,
            "single"
        );
    }

    #[test]
    fn explicit_gpt_architecture_never_requires_or_switches_to_claude() {
        let mut options = opts();
        options.mode = "gpt".into();
        options.account_id = Some("g".into());
        options.model = Some("selected-model".into());
        let accounts = accounts()
            .into_iter()
            .filter(|a| a.provider == Provider::Gpt)
            .collect::<Vec<_>>();
        let result = route(
            "Implement the architecture and security migration",
            &options,
            &accounts,
        )
        .unwrap();
        assert_eq!(result.provider, Provider::Gpt);
        assert_eq!(result.account_id, "g");
        assert_eq!(result.model.as_deref(), Some("selected-model"));
        assert_eq!(result.effort, "medium");
    }
    #[test]
    fn unavailable_and_injection() {
        assert!(route("hi", &opts(), &[]).is_err());
        let mut o = opts();
        o.model = Some("x; calc".into());
        assert!(route("hi", &o, &accounts()).is_err());
    }
    fn catalog(names: &[&str]) -> Vec<ModelInfo> {
        names
            .iter()
            .enumerate()
            .map(|(i, name)| ModelInfo {
                id: name.to_string(),
                name: name.to_string(),
                description: String::new(),
                efforts: vec!["low".into(), "medium".into(), "high".into()],
                default_effort: "medium".into(),
                is_default: i == 0,
            })
            .collect()
    }
    #[test]
    fn user_calibration_for_both_catalogs() {
        let gpt = catalog(&["gpt-6.1-sol", "gpt-6.1-terra", "gpt-6.1-luna"]);
        let claude = catalog(&["opus-5.5", "sonnet-5.5", "haiku-5.5"]);
        for (prompt, tier, effort) in [
            ("Test", 2, "low"),
            ("Traduci questa email", 2, "low"),
            ("Correggi un errore CSS", 1, "low"),
            ("Trova un bug intermittente di concorrenza", 1, "medium"),
            ("Progetta una migrazione del database", 0, "low"),
            ("Controlla la sicurezza dell'autenticazione", 0, "low"),
        ] {
            assert_eq!(
                select_model(&gpt, prompt, &AutoPolicy::Default).unwrap().id,
                gpt[tier].id,
                "{prompt}"
            );
            assert_eq!(
                select_model(&claude, prompt, &AutoPolicy::Default)
                    .unwrap()
                    .id,
                claude[tier].id,
                "{prompt}"
            );
            assert_eq!(
                route(prompt, &opts(), &accounts()).unwrap().effort,
                effort,
                "{prompt}"
            );
        }
        assert_eq!(
            select_model(&gpt[..2], "Test", &AutoPolicy::Default)
                .unwrap()
                .id,
            "gpt-6.1-terra"
        );
        assert!(select_model(&[], "Test", &AutoPolicy::Default).is_err());
    }
    #[test]
    fn policy_changes_one_effort_step_and_review_is_proportional() {
        let prompt = "Trova un bug intermittente di concorrenza";
        let mut o = opts();
        o.auto_policy = AutoPolicy::Budget;
        assert_eq!(route(prompt, &o, &accounts()).unwrap().effort, "low");
        o.auto_policy = AutoPolicy::Quality;
        let quality = route(prompt, &o, &accounts()).unwrap();
        assert_eq!(quality.effort, "high");
        assert_eq!(quality.strategy, "review");
        assert_eq!(route("Test", &o, &accounts()).unwrap().strategy, "single");
        o.mode = "gpt".into();
        o.model = Some("gpt-6.1-luna".into());
        o.effort = "low".into();
        let manual = route(prompt, &o, &accounts()).unwrap();
        assert_eq!(manual.model, o.model);
        assert_eq!(manual.effort, "low");
        assert_eq!(manual.strategy, "single");
    }
    #[test]
    fn balancing_respects_resets_exhaustion_and_freshness() {
        let at = 1_800_000_000_000;
        let usage = |used, minutes, remaining: i64| AccountUsage {
            account_id: "g".into(),
            updated_at: at,
            source: "test".into(),
            windows: vec![UsageWindow {
                group: None,
                used_percent: used,
                window_minutes: minutes,
                resets_at: at + remaining * 60_000,
            }],
        };
        let near_reset = usage(90.0, 10080, 120);
        let other = usage(40.0, 10080, 8000);
        assert!(usage_pressure(&near_reset, at).unwrap() < usage_pressure(&other, at).unwrap());
        let mut constrained = near_reset.clone();
        constrained.windows.push(UsageWindow {
            group: None,
            used_percent: 100.0,
            window_minutes: 300,
            resets_at: at + 60_000,
        });
        assert_eq!(usage_pressure(&constrained, at), Some(1000.0));
        assert_eq!(usage_pressure(&near_reset, at + 900_001), None);
        assert_eq!(usage_pressure(&near_reset, at - 1), None);
        assert_eq!(usage_pressure(&usage(20.0, 300, 0), at), None);
    }
    #[test]
    fn named_models_beat_catalog_aliases() {
        let m = |id: &str, name: &str, default: bool| ModelInfo {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            efforts: vec![],
            default_effort: String::new(),
            is_default: default,
        };
        let models = vec![
            m("default", "Default (recommended)", true),
            m("sonnet", "Sonnet 5.5", false),
            m("opus", "Opus 5.5", false),
        ];
        assert_eq!(select_tier(&models, 1).unwrap().id, "sonnet");
        assert_eq!(select_tier(&models, 2).unwrap().id, "opus");
        assert_eq!(select_tier(&models[..1], 1).unwrap().id, "default");
    }
}
