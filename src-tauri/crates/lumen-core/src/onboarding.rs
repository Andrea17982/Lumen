use crate::{models::Provider, provider_cli, signed_in, Account, CliInfo, CoreRef};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub(crate) struct State {
    existing: bool,
    setup: &'static str,
    tour: &'static str,
    revision: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Action {
    SkipSetup,
    FinishSetup,
    SkipTour,
    FinishTour,
}

#[crate::command]
pub(crate) fn onboarding_state(
    app: crate::host::EventHub,
    core: CoreRef<'_>,
    action: Option<Action>,
) -> Result<State, String> {
    let changed = action.is_some();
    let state = core.store.with(|db| {
        if let Some(action) = action {
            db.execute(
                match action {
                    Action::SkipSetup => "UPDATE onboarding SET setup=max(setup,1)",
                    Action::FinishSetup => "UPDATE onboarding SET setup=2",
                    Action::SkipTour => "UPDATE onboarding SET tour=max(tour,1)",
                    Action::FinishTour => "UPDATE onboarding SET tour=2",
                },
                [],
            )?;
        }
        db.query_row(
            "SELECT existing,setup,tour FROM onboarding WHERE id=1",
            [],
            |row| {
                let status =
                    |value: u8| ["pending", "skipped", "completed"][usize::from(value.min(2))];
                let setup: u8 = row.get(1)?;
                let tour: u8 = row.get(2)?;
                Ok(State {
                    existing: row.get(0)?,
                    setup: status(setup),
                    tour: status(tour),
                    revision: setup + tour,
                })
            },
        )
    })?;
    if changed {
        app.emit("lumen:onboarding", &state)?;
    }
    Ok(state)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Agent {
    provider: Provider,
    #[serde(flatten)]
    cli: CliInfo,
    accounts: Vec<CheckedAccount>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckedAccount {
    account: Account,
    signed_in: Option<bool>,
}

#[crate::command]
pub(crate) async fn onboarding_agents(core: CoreRef<'_>) -> Result<Vec<Agent>, String> {
    let accounts = core.store.accounts()?;
    let providers = [
        Provider::Claude,
        Provider::Gpt,
        Provider::Antigravity,
        Provider::Opencode,
        Provider::Cursor,
        Provider::Droid,
        Provider::Ollama,
    ];
    Ok(
        futures_util::future::join_all(providers.into_iter().map(|provider| {
            let accounts = &accounts;
            let core = &core;
            async move {
                let cli = provider_cli(&provider).await;
                let mut checked = Vec::new();
                for account in accounts.iter().filter(|a| a.provider == provider) {
                    checked.push(CheckedAccount {
                        signed_in: if cli.installed {
                            signed_in(core, account).await.ok()
                        } else {
                            None
                        },
                        account: account.clone(),
                    });
                }
                Agent {
                    provider,
                    cli,
                    accounts: checked,
                }
            }
        }))
        .await,
    )
}
