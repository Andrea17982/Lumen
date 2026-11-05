use crate::models::*;

/// Deliberately local and deterministic: routing itself sends no data to a provider.
pub fn route(prompt: &str, options: &RunOptions, accounts: &[Account]) -> Result<Plan, String> {
    if !["auto", "claude", "gpt"].contains(&options.mode.as_str()) {
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
    let p = prompt.to_lowercase();
    let complex = prompt.len() > 1800
        || [
            "architecture",
            "refactor",
            "security",
            "migration",
            "architettura",
            "sicurezza",
        ]
        .iter()
        .any(|k| p.contains(k));
    let ambiguous = [
        "compare",
        "trade-off",
        "alternatives",
        "confronta",
        "alternative",
    ]
    .iter()
    .any(|k| p.contains(k));
    let preferred = if options.mode == "claude" || (options.mode == "auto" && complex) {
        Provider::Claude
    } else {
        Provider::Gpt
    };
    let selected = if options.mode != "auto" {
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
            .or_else(|| accounts.first())
    };
    let a = selected.ok_or(
        "Add an account for this provider in Settings, then complete the official CLI login.",
    )?;
    let partner = accounts.iter().find(|b| b.provider != a.provider);
    let strategy = if options.mode == "auto" && partner.is_some() {
        if ambiguous {
            "parallel"
        } else if complex {
            "review"
        } else {
            "single"
        }
    } else {
        "single"
    };
    Ok(Plan {
        provider: a.provider.clone(),
        account_id: a.id.clone(),
        model: if options.mode == "auto" {
            if a.provider == Provider::Claude {
                Some(if complex { "opus" } else { "sonnet" }.into())
            } else {
                None
            }
        } else {
            options.model.clone().filter(|s| !s.is_empty())
        },
        effort: if options.mode == "auto" {
            if complex {
                "high"
            } else if prompt.len() < 240 {
                "low"
            } else {
                "medium"
            }
            .into()
        } else {
            options.effort.clone()
        },
        strategy: strategy.into(),
        reason: if ambiguous {
            "Independent perspectives, then synthesis"
        } else if complex {
            "Complex task: deep reasoning and an independent review when available"
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
            access_mode: None,
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
            route("security refactor", &opts(), &accounts())
                .unwrap()
                .strategy,
            "review"
        );
        assert_eq!(
            route("compare alternatives", &opts(), &accounts())
                .unwrap()
                .strategy,
            "parallel"
        );
    }
    #[test]
    fn unavailable_and_injection() {
        assert!(route("hi", &opts(), &[]).is_err());
        let mut o = opts();
        o.model = Some("x; calc".into());
        assert!(route("hi", &o, &accounts()).is_err());
    }
}
