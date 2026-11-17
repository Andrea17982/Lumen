//! Offline handoff corpus: decisions and constraints of a conversation must survive
//! Handoff & Compact (also twice in a row) and reach every adapter in the exact framing
//! its transport sends: Claude Code, Codex, Antigravity, OpenCode and the three APIs.
//! No CLI, account or network is involved.
use crate::{models::*, test_support::Fixture, turns, CoreRef};

const ADAPTERS: [Provider; 7] = [
    Provider::Claude,
    Provider::Gpt,
    Provider::Antigravity,
    Provider::Opencode,
    Provider::Openaiapi,
    Provider::Anthropicapi,
    Provider::Googleapi,
];

struct Case {
    name: &'static str,
    access: AccessMode,
    /// (role, text); markers are ASCII so JSON framing cannot hide them.
    messages: Vec<(&'static str, String)>,
    /// Text that must reach every adapter after each handoff.
    keep: Vec<&'static str>,
    summary: Option<&'static str>,
}

fn noise(n: usize) -> Vec<(&'static str, String)> {
    (0..n)
        .flat_map(|i| {
            [
                ("user", format!("Small follow-up {i}: rename a label")),
                ("assistant", format!("Renamed label {i}.")),
            ]
        })
        .collect()
}

fn corpus() -> Vec<Case> {
    let mut middle = vec![(
        "user",
        "Implement account storage for the sync service".to_string(),
    )];
    middle.push((
        "user",
        "Keep credentials out of the database and use only UUIDs for accounts (KEEP_UUID_ONLY)"
            .into(),
    ));
    middle.extend(noise(25));
    middle.push(("user", "Continue with the remaining work".into()));
    vec![
        Case {
            name: "english constraint and decision",
            access: AccessMode::FileEdits,
            messages: vec![
                ("user", "Add CSV export to the reports page".into()),
                (
                    "user",
                    "Never add a new dependency for this; use the standard library (NO_NEW_DEPS)"
                        .into(),
                ),
                ("assistant", "I will write the CSV by hand.".into()),
                (
                    "user",
                    "Decision: dates are always ISO 8601 in UTC (ISO_UTC_DATES)".into(),
                ),
            ],
            keep: vec!["NO_NEW_DEPS", "ISO_UTC_DATES"],
            summary: None,
        },
        Case {
            name: "italian constraint",
            access: AccessMode::Ask,
            messages: vec![
                ("user", "Implementa la coda dei messaggi".into()),
                (
                    "user",
                    "Mantieni Shift+F10 e sospendi la coda sugli errori (KEEP_SHIFT_F10)".into(),
                ),
                ("assistant", "Everything is done and all tests pass".into()),
            ],
            keep: vec!["KEEP_SHIFT_F10"],
            summary: None,
        },
        Case {
            name: "constraint buried in a long conversation",
            access: AccessMode::ReadOnly,
            messages: middle,
            keep: vec!["KEEP_UUID_ONLY"],
            summary: None,
        },
        Case {
            name: "user-edited handoff",
            access: AccessMode::Autopilot,
            messages: vec![("user", "Port the parser to Rust".into())],
            keep: vec!["EDITED_KEEP_ERRORS_TYPED"],
            summary: Some("Goal: port the parser. Constraint: errors stay typed enums (EDITED_KEEP_ERRORS_TYPED)."),
        },
    ]
}

/// Every adapter's framed user turn for the session's next prompt.
async fn framed(f: &Fixture, session: &Session) -> Vec<(Provider, String)> {
    let options = Fixture::options(session.access_mode);
    let (prompt, _) = turns::build_prompt(&f.core, session, &[], &options)
        .await
        .unwrap();
    ADAPTERS
        .iter()
        .map(|p| {
            (
                p.clone(),
                crate::providers::wire_prompt(p, &prompt).to_string(),
            )
        })
        .collect()
}

#[tokio::test]
async fn decisions_and_constraints_survive_compaction_and_handoff_to_every_adapter() {
    for case in corpus() {
        let f = Fixture::new();
        let workspace = f.directory("project");
        let store = &f.core.store;
        let mut session = store.create(workspace.to_string_lossy().into()).unwrap();
        session = store.set_access_mode(&session.id, case.access).unwrap();
        for (role, text) in &case.messages {
            store
                .save_message(
                    &session.id,
                    &Message {
                        id: id(),
                        role: (*role).into(),
                        text: text.clone(),
                        activity: vec![],
                    },
                )
                .unwrap();
        }
        let core = CoreRef(&f.core);
        let first = turns::compact_session(core, session.id.clone(), case.summary.map(Into::into))
            .await
            .unwrap();
        // A handoff of a handoff: nothing must fade on the second compaction.
        let second = turns::compact_session(core, first.id.clone(), None)
            .await
            .unwrap();
        for (hop, next) in [("first", &first), ("second", &second)] {
            assert_eq!(
                next.access_mode, case.access,
                "{}: {hop} handoff changed permissions",
                case.name
            );
            let package = &store.messages(&next.id).unwrap()[0].text;
            assert!(
                package.contains("private memory and native session are not carried over"),
                "{}: {hop} handoff claims to transfer private memory",
                case.name
            );
            for (provider, wire) in framed(&f, next).await {
                for marker in &case.keep {
                    assert!(
                        wire.contains(marker),
                        "{}: {marker} lost after the {hop} handoff to {provider:?}",
                        case.name
                    );
                }
            }
        }
    }
}

#[test]
fn every_adapter_frames_the_prompt_text_verbatim() {
    let text = "line one\nKEEP \"quoted\" & <tags> à";
    for p in ADAPTERS {
        let wire = crate::providers::wire_prompt(&p, text);
        let found = |v: &serde_json::Value| -> bool {
            fn walk(v: &serde_json::Value, text: &str) -> bool {
                match v {
                    serde_json::Value::String(s) => s == text,
                    serde_json::Value::Array(a) => a.iter().any(|x| walk(x, text)),
                    serde_json::Value::Object(o) => o.values().any(|x| walk(x, text)),
                    _ => false,
                }
            }
            walk(v, text)
        };
        assert!(
            found(&wire),
            "{p:?} does not carry the prompt verbatim: {wire}"
        );
    }
}
