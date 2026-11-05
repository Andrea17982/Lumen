mod models;
mod providers;
mod router;
mod store;
use models::*;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::{Emitter, Manager};
use tokio::sync::{watch, Semaphore};

pub struct Core {
    store: store::Store,
    root: PathBuf,
    jobs: Mutex<HashMap<String, watch::Sender<bool>>>,
    slots: Arc<Semaphore>,
    workspace_locks: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}
fn account(core: &Core, id: &str) -> Result<Account, String> {
    core.store
        .accounts()?
        .into_iter()
        .find(|a| a.id == id)
        .ok_or("Account not found".into())
}
fn config_dir(core: &Core, a: &Account) -> PathBuf {
    core.root.join("accounts").join(&a.id)
}
fn event(app: &tauri::AppHandle, s: &str, m: &str, kind: &str, text: &str) {
    let _ = app.emit(
        "lumen:run",
        RunEvent {
            session_id: s.into(),
            message_id: m.into(),
            kind: kind.into(),
            text: text.into(),
        },
    );
}

#[tauri::command]
fn list_sessions(
    core: tauri::State<Arc<Core>>,
    search: String,
    offset: u32,
) -> Result<Vec<Session>, String> {
    core.store.sessions(&search, offset)
}
#[tauri::command]
fn create_session(core: tauri::State<Arc<Core>>, workspace: String) -> Result<Session, String> {
    let path =
        std::fs::canonicalize(workspace).map_err(|_| "Choose an existing workspace folder")?;
    if !path.is_dir() {
        return Err("Workspace must be a folder".into());
    }
    core.store.create(path.to_string_lossy().into_owned())
}
#[tauri::command]
fn update_session(
    core: tauri::State<Arc<Core>>,
    id: String,
    title: String,
    pinned: bool,
) -> Result<(), String> {
    let mut s = core.store.session(&id)?;
    s.title = title.trim().chars().take(100).collect();
    if s.title.is_empty() {
        return Err("A title is required".into());
    }
    s.pinned = pinned;
    core.store.update(&s)
}
#[tauri::command]
fn delete_session(core: tauri::State<Arc<Core>>, id: String) -> Result<(), String> {
    if core
        .jobs
        .lock()
        .map_err(|_| "Jobs lock failed")?
        .contains_key(&id)
    {
        return Err("Stop this conversation before deleting it".into());
    }
    core.store.delete(&id)
}
#[tauri::command]
fn get_messages(core: tauri::State<Arc<Core>>, id: String) -> Result<Vec<Message>, String> {
    core.store.messages(&id)
}
#[tauri::command]
fn list_accounts(core: tauri::State<Arc<Core>>) -> Result<Vec<Account>, String> {
    core.store.accounts()
}
#[tauri::command]
fn add_account(
    core: tauri::State<Arc<Core>>,
    name: String,
    provider: Provider,
) -> Result<Account, String> {
    if name.trim().is_empty() || name.len() > 80 {
        return Err("Use an account name of 1–80 characters".into());
    }
    let a = core.store.add_account(name.trim().into(), provider)?;
    let dir = config_dir(&core, &a);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // A file-backed auth store makes CODEX_HOME identity isolation explicit. Lumen never reads it.
    if a.provider == Provider::Gpt {
        std::fs::write(
            dir.join("config.toml"),
            "cli_auth_credentials_store = \"file\"\n",
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(a)
}
#[tauri::command]
fn remove_account(core: tauri::State<Arc<Core>>, id: String) -> Result<(), String> {
    if !core.jobs.lock().map_err(|_| "Jobs lock failed")?.is_empty() {
        return Err("Stop active conversations before removing accounts".into());
    }
    let a = account(&core, &id)?;
    let dir = config_dir(&core, &a);
    // Only UUID-owned directories under the application data root can be removed.
    uuid::Uuid::parse_str(&a.id).map_err(|_| "Invalid account ID")?;
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    core.store.remove_account(&id)
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Diagnostics {
    claude: bool,
    gpt: bool,
    data_dir: String,
}
#[tauri::command]
fn diagnostics(core: tauri::State<Arc<Core>>) -> Diagnostics {
    Diagnostics {
        claude: providers::executable(&Provider::Claude).is_ok(),
        gpt: providers::executable(&Provider::Gpt).is_ok(),
        data_dir: core.root.to_string_lossy().into(),
    }
}

#[tauri::command]
async fn account_status(core: tauri::State<'_, Arc<Core>>, id: String) -> Result<bool, String> {
    let a = account(&core, &id)?;
    let mut c = providers::command(&a.provider)?;
    providers::isolate(&mut c, &a.provider, &config_dir(&core, &a));
    c.kill_on_drop(true);
    if a.provider == Provider::Claude {
        c.args(["auth", "status"]);
    } else {
        c.args(["login", "status"]);
    }
    let out = tokio::time::timeout(std::time::Duration::from_secs(15), c.output())
        .await
        .map_err(|_| "Status check timed out")?
        .map_err(|e| e.to_string())?;
    Ok(out.status.success())
}
#[tauri::command]
fn login_account(core: tauri::State<Arc<Core>>, id: String) -> Result<(), String> {
    let a = account(&core, &id)?;
    let exe = providers::executable(&a.provider)?;
    let dir = config_dir(&core, &a);
    #[cfg(windows)]
    {
        fn quote(s: &str) -> String {
            format!("'{}'", s.replace('\'', "''"))
        }
        let action = if a.provider == Provider::Claude {
            "auth login"
        } else {
            "login"
        };
        let script = format!(
            "$env:{} = {}\n{}\n& {} {}\nWrite-Host 'Return to Lumen and check connection.'\n",
            a.provider.env(),
            quote(&dir.to_string_lossy()),
            [
                "ANTHROPIC_API_KEY",
                "ANTHROPIC_AUTH_TOKEN",
                "CLAUDE_CODE_OAUTH_TOKEN",
                "OPENAI_API_KEY",
                "CODEX_API_KEY",
                "CODEX_ACCESS_TOKEN"
            ]
            .iter()
            .map(|k| format!("Remove-Item Env:{k} -ErrorAction SilentlyContinue"))
            .collect::<Vec<_>>()
            .join("\n"),
            quote(&exe.to_string_lossy()),
            action
        );
        let path = dir.join("login.ps1");
        std::fs::write(&path, script).map_err(|e| e.to_string())?;
        // A visible terminal is deliberate: the user invoked the official login flow.
        use std::os::windows::process::CommandExt;
        std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NoExit",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(path)
            .creation_flags(0x00000010)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(windows))]
    {
        let _ = (exe, dir);
        return Err("Linux: run the official login command with the isolated directory shown in documentation. Terminal launching is Windows-only in v0.1.".into());
    }
    #[allow(unreachable_code)]
    Ok(())
}
#[tauri::command]
fn cancel_run(core: tauri::State<Arc<Core>>, id: String) -> Result<(), String> {
    if let Some(tx) = core.jobs.lock().map_err(|_| "Jobs lock failed")?.get(&id) {
        let _ = tx.send(true);
    }
    Ok(())
}

#[tauri::command]
async fn send_message(
    app: tauri::AppHandle,
    core: tauri::State<'_, Arc<Core>>,
    id: String,
    text: String,
    options: RunOptions,
) -> Result<Plan, String> {
    if text.trim().is_empty() || text.len() > 100_000 {
        return Err("Enter a message of up to 100,000 bytes".into());
    }
    let mut session = core.store.session(&id)?;
    let plan = router::route(&text, &options, &core.store.accounts()?)?;
    let (tx, rx) = watch::channel(false);
    {
        let mut jobs = core.jobs.lock().map_err(|_| "Jobs lock failed")?;
        if jobs.contains_key(&id) {
            return Err("This conversation already has a running turn".into());
        }
        jobs.insert(id.clone(), tx);
    }
    let user = Message {
        id: models::id(),
        role: "user".into(),
        text: text.clone(),
        activity: vec![],
    };
    if let Err(e) = core.store.save_message(&id, &user) {
        core.jobs.lock().unwrap().remove(&id);
        return Err(e);
    }
    if session.title == "New conversation" {
        session.title = text
            .split_whitespace()
            .take(8)
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(64)
            .collect();
    }
    session.status = "waiting".into();
    session.updated_at = now();
    if let Err(e) = core.store.update(&session) {
        core.jobs.lock().unwrap().remove(&id);
        return Err(e);
    }
    let core = core.inner().clone();
    let returned = plan.clone();
    tauri::async_runtime::spawn(async move {
        let mid = models::id();
        let mut output = Message {
            id: mid.clone(),
            role: "assistant".into(),
            text: String::new(),
            activity: vec![Activity {
                kind: "plan".into(),
                text: format!(
                    "{} · {} reasoning · {}",
                    plan.strategy, plan.effort, plan.reason
                ),
            }],
        };
        let _ = core.store.save_message(&id, &output);
        event(&app, &id, &mid, "start", "");
        let mut cancelled = rx.clone();
        let permit = tokio::select! {biased;_=cancelled.changed()=>None,p=core.slots.clone().acquire_owned()=>p.ok()};
        let result = if permit.is_none() {
            Err("Cancelled".into())
        } else {
            session.status = "running".into();
            let _ = core.store.status(&id, "running");
            event(&app, &id, &mid, "status", "running");
            run_plan(&app, &core, &session, &mut output, &plan, &options, rx).await
        };
        drop(permit);
        match result {
            Ok(()) => session.status = "completed".into(),
            Err(e) => {
                session.status = if e == "Cancelled" {
                    "interrupted"
                } else {
                    "error"
                }
                .into();
                output.activity.push(Activity {
                    kind: "error".into(),
                    text: e.clone(),
                });
                event(&app, &id, &mid, "error", &e);
            }
        }
        session.updated_at = now();
        if let Err(e) = core
            .store
            .save_message(&id, &output)
            .and_then(|_| core.store.status(&id, &session.status))
        {
            event(
                &app,
                &id,
                &mid,
                "error",
                &format!("Could not save conversation: {e}"),
            );
        }
        core.jobs.lock().unwrap().remove(&id);
        event(&app, &id, &mid, "done", &session.status);
    });
    Ok(returned)
}

// Stage keeps the app/session/output references explicit across asynchronous worker calls.
#[allow(clippy::too_many_arguments)]
async fn stage(
    app: &tauri::AppHandle,
    core: &Core,
    s: &Session,
    out: &mut Message,
    a: &Account,
    prompt: &str,
    model: Option<&str>,
    effort: &str,
    write: bool,
    cancel: watch::Receiver<bool>,
    visible: bool,
) -> Result<String, String> {
    let config = config_dir(core, a);
    let workspace = PathBuf::from(&s.workspace);
    let mut checkpoint = std::time::Instant::now();
    providers::execute(
        providers::Invocation {
            account: a,
            config: &config,
            workspace: &workspace,
            prompt,
            model,
            effort,
            write,
        },
        cancel,
        |activity| {
            if visible && ["delta", "replace"].contains(&activity.kind.as_str()) {
                if activity.kind == "delta" {
                    out.text.push_str(&activity.text);
                } else {
                    out.text = activity.text.clone();
                }
                event(app, &s.id, &out.id, &activity.kind, &activity.text);
            } else if !["delta", "replace"].contains(&activity.kind.as_str()) {
                event(app, &s.id, &out.id, &activity.kind, &activity.text);
                if out.activity.len() < 500 {
                    out.activity.push(Activity {
                        kind: activity.kind,
                        text: activity.text.chars().take(16000).collect(),
                    });
                }
            }
            if visible && checkpoint.elapsed().as_millis() > 750 {
                let _ = core.store.save_message(&s.id, out);
                checkpoint = std::time::Instant::now();
            }
        },
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn run_plan(
    app: &tauri::AppHandle,
    core: &Core,
    s: &Session,
    out: &mut Message,
    plan: &Plan,
    options: &RunOptions,
    mut cancel: watch::Receiver<bool>,
) -> Result<(), String> {
    let writer = account(core, &plan.account_id)?;
    // Serialize write turns by canonical workspace, even when sessions use different providers.
    let lock = {
        let mut locks = core
            .workspace_locks
            .lock()
            .map_err(|_| "Workspace lock failed")?;
        let key = if cfg!(windows) {
            s.workspace.to_lowercase()
        } else {
            s.workspace.clone()
        };
        locks
            .entry(key)
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    };
    let _guard = if options.write_access {
        Some(
            tokio::select! {biased;_=cancel.changed()=>return Err("Cancelled".into()),g=lock.lock()=>g},
        )
    } else {
        None
    };
    let history = core
        .store
        .messages(&s.id)?
        .into_iter()
        .filter(|m| m.id != out.id)
        .map(|m| format!("{}:\n{}", m.role, m.text))
        .collect::<Vec<_>>()
        .join("\n\n");
    let context: String = history
        .chars()
        .rev()
        .take(100_000)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let prompt=format!("You are working in Lumen. Prior conversation follows as context. Respond to the final user request. {}\n\n{}",if options.write_access{"File edits are enabled in the chosen workspace."}else{"Read-only turn. Do not modify files."},context);
    let primary = if plan.strategy == "parallel" {
        let partner = account(
            core,
            plan.partner_account.as_deref().ok_or("Missing partner")?,
        )?;
        let mut other = Message {
            id: out.id.clone(),
            role: "assistant".into(),
            text: String::new(),
            activity: vec![],
        };
        let (a, b) = tokio::join!(
            stage(
                app,
                core,
                s,
                out,
                &writer,
                &prompt,
                plan.model.as_deref(),
                &plan.effort,
                false,
                cancel.clone(),
                false
            ),
            stage(
                app,
                core,
                s,
                &mut other,
                &partner,
                &prompt,
                None,
                "high",
                false,
                cancel.clone(),
                false
            )
        );
        out.activity.extend(other.activity);
        let a = a?;
        let b = b?;
        out.activity.push(Activity {
            kind: "perspectives".into(),
            text: format!("Perspective A:\n{a}\n\nPerspective B:\n{b}"),
        });
        format!("Synthesize these independent perspectives. Resolve disagreements and answer the request.\nRequest and context:\n{prompt}\n\nA:\n{a}\n\nB:\n{b}")
    } else {
        let draft = stage(
            app,
            core,
            s,
            out,
            &writer,
            &prompt,
            plan.model.as_deref(),
            &plan.effort,
            options.write_access,
            cancel.clone(),
            true,
        )
        .await?;
        if plan.strategy == "single" {
            out.text = draft;
            return Ok(());
        }
        let reviewer = account(
            core,
            plan.partner_account.as_deref().ok_or("Missing reviewer")?,
        )?;
        let review_prompt=format!("Independently review this draft against the request. Read-only: never edit files. Identify concrete mistakes, risks and missing requirements.\n{prompt}\n\nDraft:\n{draft}");
        let mut review_out = Message {
            id: out.id.clone(),
            role: "assistant".into(),
            text: String::new(),
            activity: vec![],
        };
        let review_result = stage(
            app,
            core,
            s,
            &mut review_out,
            &reviewer,
            &review_prompt,
            None,
            "high",
            false,
            cancel.clone(),
            false,
        )
        .await;
        out.activity.extend(review_out.activity);
        let review = review_result?;
        out.activity.push(Activity {
            kind: "review".into(),
            text: review.clone(),
        });
        event(app, &s.id, &out.id, "review", &review);
        format!("Produce the final answer using this independent review. Do not claim to have run checks unless you actually did.\n{prompt}\n\nDraft:\n{draft}\n\nReview:\n{review}")
    };
    out.text.clear();
    event(app, &s.id, &out.id, "replace", "");
    let final_text = stage(
        app,
        core,
        s,
        out,
        &writer,
        &primary,
        plan.model.as_deref(),
        &plan.effort,
        options.write_access,
        cancel,
        true,
    )
    .await?;
    out.text = final_text;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle().clone();
                let core = app.state::<Arc<Core>>().inner().clone();
                let pending = if let Ok(jobs) = core.jobs.lock() {
                    for tx in jobs.values() {
                        let _ = tx.send(true);
                    }
                    !jobs.is_empty()
                } else {
                    false
                };
                if pending {
                    api.prevent_close();
                    tauri::async_runtime::spawn(async move {
                        // Let cancellation reap all provider trees before closing the runtime.
                        for _ in 0..300 {
                            if core.jobs.lock().map(|jobs| jobs.is_empty()).unwrap_or(true) {
                                break;
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                        app.exit(0);
                    });
                }
            }
        })
        .setup(|app| {
            // Development-only isolation for offline native tests. Release always uses app data.
            let root = if cfg!(debug_assertions) {
                std::env::var_os("LUMEN_TEST_DATA_DIR")
                    .map(PathBuf::from)
                    .unwrap_or(app.path().app_data_dir()?)
            } else {
                app.path().app_data_dir()?
            };
            std::fs::create_dir_all(root.join("accounts"))?;
            let db = store::Store::open(&root.join("lumen.db")).map_err(std::io::Error::other)?;
            app.manage(Arc::new(Core {
                store: db,
                root,
                jobs: Mutex::new(HashMap::new()),
                slots: Arc::new(Semaphore::new(4)),
                workspace_locks: Mutex::new(HashMap::new()),
            }));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_sessions,
            create_session,
            update_session,
            delete_session,
            get_messages,
            list_accounts,
            add_account,
            remove_account,
            diagnostics,
            account_status,
            login_account,
            send_message,
            cancel_run
        ])
        .build(tauri::generate_context!())
        .expect("Could not start Lumen")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                let core = app.state::<Arc<Core>>();
                if let Ok(jobs) = core.jobs.lock() {
                    for tx in jobs.values() {
                        let _ = tx.send(true);
                    }
                };
            }
        });
}
