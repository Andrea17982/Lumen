use crate::models::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditIntent {
    #[default]
    Auto,
    Requested,
    Forbidden,
}

/// Model capability and reasoning depth each have four grades, from 0 to 3.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TaskProfile {
    pub tier: i32,
    pub effort: i32,
    pub review: bool,
    pub audit: AuditIntent,
}

pub fn audit_intent(prompt: &str) -> AuditIntent {
    let p = prompt.to_lowercase();
    if [
        "no cross-audit",
        "no cross audit",
        "senza cross-audit",
        "senza cross audit",
        "non fare audit",
        "non usare altri agenti",
        "do not audit",
        "don't audit",
        "no independent review",
    ]
    .iter()
    .any(|s| p.contains(s))
    {
        return AuditIntent::Forbidden;
    }
    if [
        "cross-audit",
        "cross audit",
        "crossaudit",
        "independent review",
        "independent audit",
        "revisione indipendente",
        "audit indipendente",
        "secondo revisore",
    ]
    .iter()
    .any(|s| p.contains(s))
    {
        AuditIntent::Requested
    } else {
        AuditIntent::Auto
    }
}

/// Calibrated against the user's Italian examples. Length raises effort only;
/// a long email should not automatically require a flagship model.
pub(crate) fn deep_research(prompt: &str) -> bool {
    let text = prompt.to_lowercase();
    let has = |terms: &[&str]| terms.iter().any(|term| text.contains(term));
    has(&[
        "deep research",
        "ricerca approfondita",
        "literature review",
        "revisione della letteratura",
    ]) || (has(&[
        "research",
        "ricerc",
        "studia",
        "study",
        "investig",
        "approfond",
    ]) && has(&[
        "online",
        "fonti",
        "sources",
        "literature",
        "letteratura",
        "benchmark",
    ]) && has(&[
        "approfond",
        "in-depth",
        "thorough",
        "confront",
        "compar",
        "sintes",
        "synthesi",
        "trade-off",
        "evidenz",
        "evidence",
    ]))
}

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
        "typescript",
        "javascript",
        "react",
        "gdscript",
        "godot",
        "script",
        "fix",
        "correggi",
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
    let research = deep_research(prompt);
    let visual = visual_work(&p);
    let implementation = has(&[
        "implement",
        "progetta",
        "design",
        "scrivi",
        "write",
        "sviluppa",
        "definisci",
        "proponi",
    ]);
    let narrow = has(&[
        "estrai solo",
        "extract only",
        "senza testo aggiuntivo",
        "no extra text",
        "correggi solo",
    ]) || (has(&["riscrivi", "rewrite"]) && has(&["una sola frase", "one sentence"]));
    let verification = has(&[
        " test",
        "test ",
        "validaz",
        "validat",
        "verific",
        "benchmark",
        "assert",
        "metriche",
        "misur",
    ]);
    let invariants = has(&[
        "atomic",
        "invariant",
        "idempoten",
        "isolament",
        "isolation",
        "transaz",
        "transaction",
        "determinismo",
        "determinism",
        "durabilit",
        "coerenza",
        "consistency",
        "ordinamento",
        "ordering",
        "correttezza",
        "ripetibil",
        "reproducib",
        "linearizzabil",
        "linearizab",
    ]);
    let lifecycle = has(&[
        "streaming",
        "cancellaz",
        "shutdown",
        "concorren",
        "concurrency",
        "asincron",
        "async",
        "thread",
        "goroutine",
        "persisten",
        "salvataggio",
        "lifecycle",
        "retry",
        "rollback",
        "riavvi",
        "restart",
        "replic",
    ]);
    let integration = has(&[
        "migraz",
        "migration",
        "compatibil",
        "distribuit",
        "distributed",
        "deploy",
        "versionat",
        "multi-",
        "più component",
        "across systems",
        "cutover",
        "rollout",
    ]);
    let formal = has(&[
        "dimostra",
        "prove",
        "proof",
        "controesemp",
        "counterexample",
        "equivalen",
        "deriva",
        "derive",
        "dimostrazione",
        "formalizz",
        "formaliz",
    ]);
    let stakes = has(&[
        "produzion",
        "production",
        "perdita di dati",
        "data loss",
        "prestaz",
        "performance",
        "senza ferm",
        "senza interru",
        "deadlock",
        "corruzion",
        "corruption",
    ]);
    let diagnosis = has(&["diagnos", "indag", "investig", "root cause"]);
    let competing_explanations = has(&[
        "ipotesi",
        "hypothes",
        "meccanismi causali",
        "causal mechanism",
        "spiegazioni concorrenti",
        "competing explanations",
        "controprove",
    ]);
    let ambiguity = has(&[
        "conflitt",
        "conflict",
        "trade-off",
        "ambig",
        "incoeren",
        "inconsisten",
    ]);
    let cross_boundary = has(&[
        "migraz",
        "migration",
        "distribuit",
        "distributed",
        "multi-",
        "across systems",
        "client e server",
        "server e client",
        "end-to-end",
    ]);
    let requirements = p.split([',', ';']).count() >= 3;
    let demanding_verification = has(&["matrice di test", "test matrix", "property-based"])
        || (diagnosis && has(&["log", "profiler", "telemetri", "metriche", "traces"]));
    let dimensions = [
        verification,
        invariants,
        lifecycle,
        integration,
        formal,
        stakes,
    ]
    .into_iter()
    .filter(|present| *present)
    .count();
    let frontier = (diagnosis
        && (dimensions >= 3
            || has(&["multi-repo", "non riproducib", "non-reproducib"])
            || (cross_boundary && has(&["rare", "intermittent", "sporadic"]))))
        || (formal
            && has(&[
                "unicità",
                "uniqueness",
                "controesemp",
                "counterexample",
                "non determinist",
                "nondeterminist",
            ]))
        || (integration && invariants && has(&["monorepo", "multi-repo"]))
        || (competing_explanations && (verification || invariants || formal))
        || (formal && invariants && (lifecycle || integration))
        || (has(&["safety", "sicurezza"]) && has(&["liveness", "vivacità"]) && invariants);
    let direct = translation || (narrow && !frontier && dimensions <= 1);
    let tier = if direct {
        0
    } else if frontier {
        3
    } else if research
        || advanced
        || demanding_verification
        || (formal && requirements)
        || dimensions >= 2
        || (debugging
            && has(&[
                "concurrency",
                "concorrenza",
                "intermittent",
                "intermittente",
                "distributed",
                "distribuit",
            ]))
    {
        2
    } else if debugging || implementation || visual || dimensions > 0 {
        1
    } else {
        0
    };
    let audit = if translation {
        AuditIntent::Auto
    } else {
        audit_intent(prompt)
    };
    TaskProfile {
        tier,
        effort: if direct {
            0
        } else if frontier && (formal || (invariants && verification) || competing_explanations) {
            3
        } else if research
            || frontier
            || demanding_verification
            || (formal && requirements)
            || (cross_boundary && requirements)
            || (ambiguity && requirements && (verification || competing_explanations))
            || dimensions >= 3
            || (dimensions >= 2 && (formal || stakes))
        {
            2
        } else {
            i32::from(
                demanding
                    || dimensions >= 2
                    || (requirements && (implementation || debugging || visual || tier >= 1))
                    || (visual && has(&["guidami", "interpret", "verific", "guide me"]))
                    || (dimensions == 1
                        && (verification || lifecycle)
                        && (implementation || debugging))
                    || has(&[
                        "passaggi",
                        "trade-off",
                        "confronta",
                        "compare",
                        "passo per passo",
                        "fammi un esempio",
                        "fammi esempi",
                        "with examples",
                        "perché ",
                        "probabilit",
                    ])
                    || prompt.len() > 1200,
            )
        },
        review: !translation
            && audit != AuditIntent::Forbidden
            && (audit == AuditIntent::Requested
                || (has(&[
                    "security",
                    "sicurezza",
                    "authentication",
                    "autenticazione",
                    "migration",
                    "migrazione",
                ]) && has(&[
                    "implement",
                    "fix",
                    "corregg",
                    "migr",
                    "refactor",
                    "svilupp",
                    "production",
                    "produzione",
                ]))
                || (debugging
                    && has(&[
                        "concurrency",
                        "concorrenza",
                        "intermittent",
                        "intermittente",
                    ]))),
        audit,
    }
}

fn visual_work(lower: &str) -> bool {
    [
        "da uno screenshot",
        "uno screenshot di",
        "screenshot delle",
        "ti mostro",
        "from a screenshot",
        "attached screenshot",
        "screenshot dei passaggi",
    ]
    .iter()
    .any(|cue| lower.contains(cue))
}

/// Tie-break before continuity and quota are applied.
fn provider_for(request: &str, tier: i32) -> Provider {
    let lower = request.to_lowercase();
    let has = |cues: &[&str]| cues.iter().any(|cue| lower.contains(cue));
    if visual_work(&lower) {
        Provider::Gpt
    } else if tier == 0 {
        if has(&[
            "riscrivi",
            "rewrite",
            "traduci",
            "translate",
            "concordanza",
            "tono",
        ]) {
            Provider::Claude
        } else {
            Provider::Gpt
        }
    } else if tier == 1
        || has(&[
            "indag",
            "investig",
            "root cause",
            "riprogett",
            "redesign",
            "architettur",
            "architecture",
        ])
    {
        Provider::Claude
    } else {
        Provider::Gpt
    }
}

pub(crate) fn requested_effort(task: TaskProfile, policy: &AutoPolicy) -> &'static str {
    let adjustment = match policy {
        AutoPolicy::Budget => -1,
        AutoPolicy::Quality => 1,
        AutoPolicy::Default => 0,
    };
    let ceiling = task.effort.clamp(2, 3);
    ["low", "medium", "high", "ultrahigh"][(task.effort + adjustment).clamp(0, ceiling) as usize]
}

pub(crate) fn effort_grade(effort: &str) -> i32 {
    match effort {
        "medium" => 1,
        "high" => 2,
        "xhigh" | "max" | "ultra" | "ultrahigh" => 3,
        _ => 0,
    }
}

/// Only an explicit instruction at the start pins Auto; mentioning a provider is not a choice.
pub fn requested_provider(prompt: &str) -> Option<&'static str> {
    let text = prompt.trim_start().to_lowercase();
    for (name, provider) in [
        ("antigravity", "antigravity"),
        ("claude", "claude"),
        ("gpt", "gpt"),
        ("codex", "gpt"),
        ("ollama", "ollama"),
        ("opencode", "opencode"),
        ("cursor", "cursor"),
        ("droid", "droid"),
    ] {
        for verb in ["usa ", "use ", "utilizza ", "utilize "] {
            if let Some(rest) = text.strip_prefix(&format!("{verb}{name}")) {
                if rest.is_empty()
                    || rest.starts_with(|c: char| c.is_whitespace() || c == ':' || c == ',')
                {
                    return Some(provider);
                }
            }
        }
    }
    None
}

/// Keyword routing, for callers without a classifier verdict.
#[cfg(test)]
pub fn route(prompt: &str, options: &RunOptions, accounts: &[Account]) -> Result<Plan, String> {
    route_with(profile(prompt), options, accounts, prompt)
}

/// Plans a turn for a rated task (`crate::classify`, or the keyword profile).
pub fn route_with(
    task: TaskProfile,
    options: &RunOptions,
    accounts: &[Account],
    request: &str,
) -> Result<Plan, String> {
    let auto = options.mode == "auto";
    let preferred = if auto {
        provider_for(request, task.tier)
    } else {
        Provider::from_id(&options.mode).ok_or("Unknown provider mode")?
    };
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
    let selected = if !auto {
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
            .or_else(|| accounts.first())
    };
    let a = selected.ok_or(
        "Add an account for this provider in Settings, then complete the official CLI login.",
    )?;
    let partner = review_partner(accounts, a);
    let review_required = options.auto_review || (auto && task.audit == AuditIntent::Requested);
    let review_suggested =
        auto && (task.review || (options.auto_policy == AutoPolicy::Quality && task.tier >= 2));
    let review_allowed = task.audit != AuditIntent::Forbidden;
    if review_allowed && review_required && partner.is_none() {
        return Err(
            "Independent review requires another available Claude or ChatGPT account.".into(),
        );
    }
    let strategy = if review_allowed && partner.is_some() && (review_required || review_suggested) {
        "review"
    } else {
        "single"
    };
    let mut plan = Plan {
        provider: a.provider.clone(),
        account_id: a.id.clone(),
        model: if auto {
            None
        } else {
            options.model.clone().filter(|s| !s.is_empty())
        },
        effort: if auto {
            requested_effort(task, &options.auto_policy).into()
        } else {
            options.effort.clone()
        },
        strategy: strategy.into(),
        reason: if !auto && strategy == "single" {
            "Native session with the selected model and reasoning"
        } else if !auto {
            "Answer, then cross-audit by another account"
        } else if task.tier == 3 {
            "Exceptionally difficult task: highest model capability with proportional reasoning"
        } else if task.tier == 2 {
            "Demanding task: flagship model with proportional reasoning"
        } else if task.tier == 1 {
            "Bounded task: balanced model with proportional reasoning"
        } else {
            "Direct response with a smaller reasoning budget"
        }
        .into(),
        partner_account: if strategy != "single" {
            partner.map(|a| a.id.clone())
        } else {
            None
        },
    };
    if review_allowed && review_suggested && partner.is_none() {
        plan.reason
            .push_str(" · independent review unavailable: no other configured primary account");
    }
    Ok(plan)
}

pub(crate) enum RlmTarget {
    Local {
        http: reqwest::Client,
        base: reqwest::Url,
        model: String,
        native: bool,
        thinking: bool,
    },
    Api {
        account: Account,
        model: String,
    },
}

impl RlmTarget {
    pub(crate) fn provider(&self) -> Provider {
        match self {
            Self::Local { .. } => Provider::Ollama,
            Self::Api { account, .. } => account.provider.clone(),
        }
    }

    pub(crate) fn structured(&self) -> bool {
        matches!(self, Self::Local { native: false, .. })
    }
}

pub(crate) async fn rlm_target(
    data: &std::path::Path,
    endpoint: &str,
    current_local: Option<&str>,
    accounts: &[Account],
) -> Result<RlmTarget, String> {
    let local = async {
        let (http, base) = crate::ollama::local_http_at(endpoint)?;
        let model = if let Some(model) = current_local {
            model.to_owned()
        } else {
            let models = crate::ollama::list_at(&http, &base).await?;
            let pinned = crate::settings::AppSettings::load(data).ollama_model;
            models
                .iter()
                .find(|m| m.name == pinned)
                .or_else(|| models.iter().min_by_key(|m| m.bytes))
                .ok_or("No installed local text model")?
                .name
                .clone()
        };
        crate::ollama_agent::local_model(&model, &serde_json::Value::Null)?;
        let show = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let raw = crate::ollama_agent::post(
                &http,
                &base,
                "api/show",
                &serde_json::json!({"model":model}),
            )
            .await?;
            crate::rlm::bounded_json(raw).await
        })
        .await
        .map_err(|_| "Local model discovery timed out")??;
        crate::ollama_agent::local_model(&model, &show)?;
        let capability = |name: &str| {
            show["capabilities"]
                .as_array()
                .is_some_and(|list| list.iter().any(|c| c == name))
        };
        Ok(RlmTarget::Local {
            http,
            base,
            model,
            native: capability("tools"),
            thinking: capability("thinking"),
        })
    }
    .await;
    if local.is_ok() || current_local.is_some() {
        // A local parent stays local, even on discovery or generation failure.
        return local;
    }
    accounts
        .iter()
        .filter_map(|a| crate::api::rlm_price(data, a).map(|(model, rate)| (a, model, rate)))
        .min_by(|left, right| {
            left.2
                .total_cmp(&right.2)
                .then_with(|| left.0.id.cmp(&right.0.id))
        })
        .map(|(account, model, _)| RlmTarget::Api {
            account: account.clone(),
            model,
        })
        .ok_or_else(|| "llm() has no local model or API account opted into Auto".into())
}

pub fn review_partner<'a>(accounts: &'a [Account], writer: &Account) -> Option<&'a Account> {
    accounts
        .iter()
        .filter(|a| a.id != writer.id && matches!(a.provider, Provider::Claude | Provider::Gpt))
        .min_by_key(|a| a.provider == writer.provider)
}
/// Model class: fast, balanced, flagship, frontier. Unknown named models are balanced.
pub fn model_tier(m: &ModelInfo) -> i32 {
    let name = format!("{} {}", m.id, m.name).to_lowercase();
    let belongs_to = |families: &[&str]| {
        name.split(|c: char| !c.is_alphanumeric())
            .any(|word| families.contains(&word))
    };
    if belongs_to(&["astra", "fable", "mythos"]) {
        3
    } else if belongs_to(&["luna", "mini", "nano", "haiku", "flash"]) {
        0
    } else if belongs_to(&["opus", "sol", "pro"]) {
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
    models.iter().min_by_key(|m| {
        let tier = model_tier(m);
        (
            (tier - target).abs(),
            tier < target,
            is_alias(m),
            !m.is_default,
        )
    })
}
/// Use catalog IDs, never manufacture a model name unavailable to the account.
#[cfg(test)]
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
/// The keyword fallback identifies parallel deliverables or implementation across distinct
/// areas. Difficulty or length alone does not justify Orchestra's task-graph overhead.
pub fn complex(prompt: &str) -> bool {
    let p = prompt.to_lowercase();
    if [
        "traduci",
        "translate",
        "translation",
        "traduzione",
        "spiega",
        "explain",
        "without changing",
        "senza toccare",
        "non modificare",
    ]
    .iter()
    .any(|s| p.contains(s))
    {
        return false;
    }
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
    let words: Vec<_> = p.split(|c: char| !c.is_alphanumeric()).collect();
    let area = |names: &[&str]| names.iter().any(|name| words.contains(name));
    let areas = usize::from(area(&["api", "backend", "server"]))
        + usize::from(area(&["ui", "frontend", "interfaccia", "client"]))
        + usize::from(area(&["database", "storage", "persistenza"]));
    let implementation = ["implement", "svilupp", "costruisci", "build", "crea "]
        .iter()
        .any(|s| p.contains(s));
    (explicit_parallel && (checklist_items >= 2 || prompt.len() > 400))
        || (implementation && areas >= 2)
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
            (usage.source != "antigravity-cli"
                || w.group.as_deref().is_some_and(|g| {
                    let g = g.to_lowercase();
                    g.contains("gemini") || g.contains("google")
                }))
                && w.resets_at > at
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
    #[test]
    fn retired_antigravity_quotas_do_not_exhaust_google() {
        let usage = crate::models::AccountUsage {
            account_id: "a".into(),
            source: "antigravity-cli".into(),
            updated_at: 0,
            windows: vec![
                crate::models::UsageWindow {
                    group: Some("Claude and GPT models".into()),
                    used_percent: 100.0,
                    resets_at: 100000,
                    window_minutes: 300,
                },
                crate::models::UsageWindow {
                    group: Some("Gemini Models".into()),
                    used_percent: 1.0,
                    resets_at: 100000,
                    window_minutes: 300,
                },
            ],
        };
        assert!(super::usage_pressure(&usage, 0).unwrap() < 1000.0);
    }

    #[test]
    fn explicit_provider_instruction_pins_auto_without_matching_mentions() {
        assert_eq!(
            super::requested_provider(
                "Usa antigravity per questo prompt: che falle di vulnerabilita ha Lumen?"
            ),
            Some("antigravity")
        );
        assert_eq!(
            super::requested_provider("Use GPT: review this"),
            Some("gpt")
        );
        assert_eq!(super::requested_provider("What is Antigravity?"), None);
        assert_eq!(super::requested_provider("Use ClaudeCodeExploit"), None);
        assert_eq!(super::requested_provider("usa droid: fix"), Some("droid"));
    }

    #[test]
    fn every_provider_is_a_known_mode() {
        let mut o = opts();
        for provider in crate::adapter_contract::adapter_capabilities().adapters {
            o.mode = serde_json::to_value(&provider.provider)
                .unwrap()
                .as_str()
                .unwrap()
                .into();
            let accounts = [Account {
                id: "a".into(),
                name: "A".into(),
                provider: provider.provider.clone(),
            }];
            if let Err(e) = route("Test", &o, &accounts) {
                assert_ne!(e, "Unknown provider mode", "mode {}", o.mode);
            }
        }
    }

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
            resume_only: false,
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
    fn quality_audit_intent_and_account_topology_are_explicit() {
        let mut options = opts();
        options.auto_policy = AutoPolicy::Budget;
        assert_eq!(
            route("Fai un cross-audit di questo fix", &options, &accounts())
                .unwrap()
                .strategy,
            "review"
        );
        options.auto_policy = AutoPolicy::Quality;
        assert_eq!(
            route("Correggi CSS", &options, &accounts())
                .unwrap()
                .strategy,
            "single"
        );
        assert_eq!(
            route(
                "Fix security without changing API; no cross-audit",
                &options,
                &accounts()
            )
            .unwrap()
            .strategy,
            "single"
        );
        let mut same = accounts();
        same[1].provider = Provider::Claude;
        let plan = route("Fai un cross-audit di questo fix", &options, &same).unwrap();
        assert_ne!(plan.account_id, plan.partner_account.unwrap());
        options.mode = "auto".into();
        assert!(route("Fai un cross-audit di questo fix", &options, &same[..1]).is_err());
    }
    #[test]
    fn quality_calibration_corpus() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/quality/auto-cases.json"
        ))
        .unwrap();
        let mut rows = vec![];
        for case in cases.as_array().unwrap() {
            let request = case["request"].as_str().unwrap();
            let task = profile(request);
            let mut options = opts();
            options.auto_policy = AutoPolicy::Quality;
            let plan = route_with(task, &options, &accounts(), request).unwrap();
            let actual = serde_json::json!({"tier":task.tier,"review":task.review,"complex":complex(request),"qualityStrategy":plan.strategy});
            let ok = ["tier", "review", "complex", "qualityStrategy"]
                .iter()
                .all(|key| actual[*key] == case[*key]);
            rows.push(serde_json::json!({"id":case["id"],"actual":actual,"passed":ok}));
        }
        println!(
            "QUALITY_CALIBRATION {}",
            serde_json::to_string(&rows).unwrap()
        );
        assert!(
            rows.iter().all(|r| r["passed"] == true),
            "Routing corpus mismatches; see QUALITY_CALIBRATION"
        );
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
    #[test]
    fn every_explicit_provider_preserves_the_selected_model() {
        for (mode, provider) in [
            ("antigravity", Provider::Antigravity),
            ("ollama", Provider::Ollama),
        ] {
            let mut options = opts();
            options.mode = mode.into();
            options.model = Some("chosen-model:8b".into());
            options.account_id = Some("selected".into());
            let accounts = vec![Account {
                id: "selected".into(),
                name: "test".into(),
                provider: provider.clone(),
            }];
            let routed = route("Implement architecture", &options, &accounts).unwrap();
            assert_eq!(routed.provider, provider);
            assert_eq!(routed.model.as_deref(), Some("chosen-model:8b"));
        }
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

    fn score_routes(corpus: &str) -> [usize; 5] {
        let gpt = catalog(&["luna", "terra", "sol", "astra"]);
        let claude = catalog(&["haiku", "sonnet", "opus", "fable"]);
        let options = opts();
        let accounts = accounts();
        let rank = |family: &str| match family {
            "luna" | "haiku" => 0,
            "terra" | "sonnet" => 1,
            "sol" | "opus" => 2,
            "astra" | "fable" => 3,
            _ => panic!("Unknown model family {family}"),
        };
        let mut totals = [0; 5];
        let mut tier_confusion = [[0; 4]; 4];
        let mut effort_confusion = [[0; 4]; 4];
        for line in corpus.lines() {
            let row: serde_json::Value = serde_json::from_str(line).unwrap();
            let expected = row["model"]
                .as_str()
                .unwrap()
                .split_whitespace()
                .nth(1)
                .unwrap()
                .to_ascii_lowercase();
            let expected_effort = row["effort"].as_str().unwrap();
            let request = row["question"].as_str().unwrap();
            let task = profile(request);
            let plan = route_with(task, &options, &accounts, request).unwrap();
            let models = if plan.provider == Provider::Claude {
                &claude
            } else {
                &gpt
            };
            let actual = &select_tier(models, task.tier).unwrap().id;
            let effort = plan.effort;
            let tier_matches = rank(actual) == rank(&expected);
            let effort_matches = effort == expected_effort;
            tier_confusion[rank(&expected)][rank(actual)] += 1;
            effort_confusion[effort_grade(expected_effort) as usize]
                [effort_grade(&effort) as usize] += 1;
            totals[0] += 1;
            totals[1] += usize::from(tier_matches);
            totals[2] += usize::from(effort_matches);
            totals[3] += usize::from(actual == &expected && effort_matches);
            totals[4] += usize::from(tier_matches && effort_matches);
        }
        println!("cases/tier/effort/exact pair/tier+effort: {totals:?}");
        println!("tier matrix: {tier_confusion:?}; effort matrix: {effort_confusion:?}");
        totals
    }

    #[test]
    #[ignore = "requires the owner's private calibration corpus"]
    fn annotated_router_corpus() {
        let path = std::env::var_os("LUMEN_ROUTER_CALIBRATION")
            .expect("set LUMEN_ROUTER_CALIBRATION to the annotated JSONL corpus");
        let corpus = std::fs::read_to_string(path).expect("read the annotated JSONL corpus");
        let totals = score_routes(&corpus);
        assert_eq!(totals[0], 100);
        assert!(
            totals[1] >= 52 && totals[2] >= 42 && totals[3] >= 11,
            "{totals:?}"
        );
    }

    #[test]
    fn independent_router_validation() {
        let totals = score_routes(include_str!(
            "../../../../tests/fixtures/quality/model-router-validation.jsonl"
        ));
        assert_eq!(totals[0], 32);
        assert!(totals[1] >= 15 && totals[4] >= 8, "{totals:?}");
    }

    #[test]
    fn frontier_capability_and_effort_use_the_catalog() {
        let mut models = catalog(&["gpt-6.2-sol", "gpt-6.2-astra"]);
        models[1].efforts.extend(["xhigh".into(), "max".into()]);
        let frontier = select_tier(&models, 3).unwrap();
        assert_eq!(frontier.id, "gpt-6.2-astra");
        assert_eq!(crate::resolve_effort(frontier, "ultrahigh"), "max");
        assert_eq!(crate::resolve_effort(frontier, "xhigh"), "xhigh");
        assert_eq!(crate::resolve_effort(&models[0], "ultrahigh"), "high");
        let task = TaskProfile {
            tier: 3,
            effort: 2,
            review: false,
            audit: AuditIntent::Auto,
        };
        assert_eq!(requested_effort(task, &AutoPolicy::Quality), "high");
        let mut max = task;
        max.effort = 3;
        assert_eq!(requested_effort(max, &AutoPolicy::Default), "ultrahigh");
        assert_eq!(requested_effort(max, &AutoPolicy::Budget), "high");
    }

    #[test]
    fn user_calibration_for_both_catalogs() {
        let gpt = catalog(&["gpt-6.1-sol", "gpt-6.1-terra", "gpt-6.1-luna"]);
        let claude = catalog(&["opus-5.5", "sonnet-5.5", "haiku-5.5"]);
        for (prompt, tier, effort) in [
            ("Test", 2, "low"),
            ("Traduci questa email", 2, "low"),
            ("Correggi un errore CSS", 1, "low"),
            ("Trova un bug intermittente di concorrenza", 0, "medium"),
            ("Progetta una migrazione del database", 0, "low"),
            ("Controlla la sicurezza dell'autenticazione", 0, "low"),
            ("Studia come funziona ora la modalità Auto e fammi ogni domanda dopo aver approfondito online la questione", 0, "high"),
            ("Research primary sources and synthesize the trade-offs between routing approaches", 0, "high"),
            ("Cerca online il numero della versione corrente", 2, "low"),
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
