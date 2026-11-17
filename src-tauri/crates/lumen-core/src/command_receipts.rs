//! Rust adaptation of T3 Code's command receipt/dispatch design (MIT).
//! Copyright (c) 2026 T3 Tools Inc. Source: pingdotgg/t3code@375343aaf35133f659435554cd01634bab333a7d,
//! apps/server/src/persistence/OrchestrationCommandReceipts.ts and orchestration-v2/Orchestrator.ts.
//! Lumen changes: per-ID locks, payload binding and fail-closed pending claims rather than an event outbox.
//! Full upstream notice and adaptation inventory: resources/legal/t3code/ and THIRD_PARTY_NOTICES.md.
use crate::{host::EventHub, Core};
use serde_json::Value;
use std::sync::Arc;

pub async fn dispatch(
    app: EventHub,
    core: &Arc<Core>,
    command: &str,
    args: Value,
    id: &str,
    fingerprint: &str,
) -> Result<Value, String> {
    let id = uuid::Uuid::parse_str(id)
        .map_err(|_| "Invalid command ID")?
        .to_string();
    let lock = {
        let mut locks = core
            .command_locks
            .lock()
            .map_err(|_| "Command lock failed")?;
        locks.retain(|_, lock| lock.strong_count() > 0);
        match locks.get(&id).and_then(std::sync::Weak::upgrade) {
            Some(lock) => lock,
            None => {
                let lock = Arc::new(tokio::sync::Mutex::new(()));
                locks.insert(id.clone(), Arc::downgrade(&lock));
                lock
            }
        }
    };
    let _guard = lock.lock().await;
    match core.store.claim_command(&id, fingerprint)? {
        Some(Some(result)) => return serde_json::from_str(&result).map_err(|_| String::from("Invalid command receipt"))?,
        Some(None) => return Err(format!("Command {id} has an uncertain result after interruption; review the conversation before submitting new work. It was not replayed.")),
        None => {}
    }
    // A cancellation or crash after this claim cannot authorize another execution.
    let result = crate::dispatch(app, core, command, args).await;
    let encoded = serde_json::to_string(&result).map_err(|e| e.to_string())?;
    core.store.complete_command(&id, &encoded)?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Fixture;
    use serde_json::json;

    #[tokio::test]
    async fn concurrent_duplicates_replay_results_and_bind_arguments() {
        let fixture = Fixture::new();
        let id = crate::models::id();
        let args = json!({"workspace":fixture.root});
        let call = || {
            dispatch(
                EventHub::default(),
                &fixture.core,
                "create_session",
                args.clone(),
                &id,
                "same",
            )
        };
        let (a, b) = tokio::join!(call(), call());
        assert_eq!(a.unwrap(), b.unwrap());
        assert_eq!(fixture.core.store.sessions("", 0).unwrap().len(), 1);
        assert!(dispatch(
            EventHub::default(),
            &fixture.core,
            "create_session",
            args,
            &id,
            "changed"
        )
        .await
        .unwrap_err()
        .contains("different arguments"));

        let error_id = crate::models::id();
        let bad = || {
            dispatch(
                EventHub::default(),
                &fixture.core,
                "create_session",
                json!({}),
                &error_id,
                "bad",
            )
        };
        assert_eq!(bad().await.unwrap_err(), bad().await.unwrap_err());
        assert!(dispatch(
            EventHub::default(),
            &fixture.core,
            "create_session",
            json!({}),
            "invalid",
            "bad"
        )
        .await
        .unwrap_err()
        .contains("Invalid command ID"));
    }

    #[tokio::test]
    async fn completed_receipts_survive_reopen_and_pending_claims_never_reexecute() {
        let fixture = Fixture::new();
        let path = fixture.root.join("receipts.sqlite");
        let id = crate::models::id();
        let pending = crate::models::id();
        let make_core = || {
            Arc::new(Core::new(
                crate::store::Store::open(&path).unwrap(),
                fixture.root.clone(),
                Default::default(),
                None,
            ))
        };
        let core = make_core();
        let args = json!({"workspace":fixture.root});
        let result = dispatch(
            EventHub::default(),
            &core,
            "create_session",
            args.clone(),
            &id,
            "done",
        )
        .await
        .unwrap();
        assert_eq!(core.store.claim_command(&pending, "pending").unwrap(), None);
        drop(core);
        let core = make_core();
        assert_eq!(
            dispatch(
                EventHub::default(),
                &core,
                "create_session",
                args.clone(),
                &id,
                "done"
            )
            .await
            .unwrap(),
            result
        );
        assert!(dispatch(
            EventHub::default(),
            &core,
            "create_session",
            args,
            &pending,
            "pending"
        )
        .await
        .unwrap_err()
        .contains("not replayed"));
        assert_eq!(core.store.sessions("", 0).unwrap().len(), 1);
        drop(core);
    }
}
