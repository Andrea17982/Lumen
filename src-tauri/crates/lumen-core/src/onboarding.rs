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
    let state = core.store.with(|db| apply(db, action))?;
    // The state is already saved: a failed broadcast must not report the write as failed.
    // Other windows read the state again when they reconnect.
    if changed {
        if let Err(e) = app.emit("lumen:onboarding", &state) {
            eprintln!("lumen: onboarding event not sent: {e}");
        }
    }
    Ok(state)
}

/// Steps only move forward: pending (0), skipped (1), completed (2).
fn apply(db: &rusqlite::Connection, action: Option<Action>) -> rusqlite::Result<State> {
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
        "SELECT existing,min(max(setup,0),2),min(max(tour,0),2) FROM onboarding WHERE id=1",
        [],
        |row| {
            let status = |value: u8| ["pending", "skipped", "completed"][usize::from(value)];
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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn state(db: &Connection, action: Option<Action>) -> (bool, &'static str, &'static str, u8) {
        let s = apply(db, action).unwrap();
        (s.existing, s.setup, s.tour, s.revision)
    }

    #[test]
    fn a_new_database_starts_the_welcome_and_steps_only_move_forward() {
        let mut db = Connection::open_in_memory().unwrap();
        crate::migrations::run(&mut db).unwrap();
        assert_eq!(state(&db, None), (false, "pending", "pending", 0));
        assert_eq!(
            state(&db, Some(Action::SkipSetup)),
            (false, "skipped", "pending", 1)
        );
        assert_eq!(
            state(&db, Some(Action::FinishSetup)),
            (false, "completed", "pending", 2)
        );
        // Skipping after finishing keeps it finished; the revision never goes back.
        assert_eq!(
            state(&db, Some(Action::SkipSetup)),
            (false, "completed", "pending", 2)
        );
        assert_eq!(
            state(&db, Some(Action::FinishTour)),
            (false, "completed", "completed", 4)
        );
        // Reopening the database does not reset the state or the first-run flag.
        crate::migrations::run(&mut db).unwrap();
        assert_eq!(state(&db, None), (false, "completed", "completed", 4));
    }

    #[test]
    fn a_database_from_0_15_is_an_existing_install() {
        let mut db = Connection::open_in_memory().unwrap();
        crate::migrations::run(&mut db).unwrap();
        // What 0.15 leaves behind: the current schema without the onboarding table.
        db.execute_batch("DROP TABLE onboarding").unwrap();
        crate::migrations::run(&mut db).unwrap();
        assert_eq!(state(&db, None), (true, "pending", "pending", 0));
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        let mut db = Connection::open_in_memory().unwrap();
        crate::migrations::run(&mut db).unwrap();
        db.execute_batch("UPDATE onboarding SET setup=200, tour=-3")
            .unwrap();
        assert_eq!(state(&db, None), (false, "completed", "pending", 2));
    }
}
