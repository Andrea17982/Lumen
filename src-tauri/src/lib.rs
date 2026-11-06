mod api;
mod catalog;
mod context;
mod gitops;
mod integrations;
mod limits;
mod models;
mod providers;
mod router;
mod settings;
mod store;
mod turns;
mod usage;
use models::*;
use std::{
    collections::{HashMap, VecDeque},
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
    catalogs: Mutex<HashMap<String, (std::time::Instant, Vec<ModelInfo>)>>,
    usage_lock: Mutex<()>,
    limits: limits::Limits,
    settings: Mutex<settings::AppSettings>,
    api: tokio::sync::Mutex<Option<api::Running>>,
    api_log: Mutex<VecDeque<api::ApiLog>>,
}
async fn models_for(
    core: &Core,
    a: &Account,
    refresh: bool,
    cancel: Option<watch::Receiver<bool>>,
) -> Result<Vec<ModelInfo>, String> {
    if !refresh {
        if let Some((at, models)) = core
            .catalogs
            .lock()
            .map_err(|_| "Catalog lock failed")?
            .get(&a.id)
        {
            if at.elapsed().as_secs() < 300 {
                return Ok(models.clone());
            }
        }
    }
    let models = catalog::discover(a, &config_dir(core, a), cancel).await?;
    core.catalogs
        .lock()
        .map_err(|_| "Catalog lock failed")?
        .insert(a.id.clone(), (std::time::Instant::now(), models.clone()));
    Ok(models)
}
#[tauri::command]
async fn list_models(
    core: tauri::State<'_, Arc<Core>>,
    id: String,
    refresh: Option<bool>,
) -> Result<Vec<ModelInfo>, String> {
    let a = account(&core, &id)?;
    models_for(&core, &a, refresh.unwrap_or(false), None).await
}
#[tauri::command]
fn list_workspaces(core: tauri::State<Arc<Core>>) -> Result<Vec<Workspace>, String> {
    core.store.workspaces()
}
#[tauri::command]
fn add_workspace(
    core: tauri::State<Arc<Core>>,
    name: String,
    folder: String,
) -> Result<Workspace, String> {
    let name = name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err("Choose a workspace name of 1–100 characters".into());
    }
    core.store.add_workspace(name.into(), folder)
}
#[tauri::command]
fn set_session_workspace(
    core: tauri::State<Arc<Core>>,
    id: String,
    workspace_id: Option<String>,
    folder: String,
) -> Result<Session, String> {
    let jobs = core.jobs.lock().map_err(|_| "Jobs lock failed")?;
    if jobs.contains_key(&id) && core.store.session(&id)?.workspace != folder {
        return Err("Stop this conversation before changing its folder".into());
    }
    let path = dunce::canonicalize(folder).map_err(|_| "Choose an existing folder")?;
    if !path.is_dir() {
        return Err("Choose a folder".into());
    }
    if let Some(wid) = &workspace_id {
        if !core.store.workspaces()?.iter().any(|w| w.id == *wid) {
            return Err("Workspace not found".into());
        }
    }
    core.store
        .bind_workspace(&id, workspace_id.as_deref(), &path.to_string_lossy())
}
#[tauri::command]
fn rename_account(core: tauri::State<Arc<Core>>, id: String, name: String) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() || name.len() > 80 {
        return Err("Use an account name of 1–80 characters".into());
    }
    account(&core, &id)?;
    core.store.rename_account(&id, name)
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
    let path = dunce::canonicalize(workspace).map_err(|_| "Choose an existing workspace folder")?;
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
fn set_session_access_mode(
    core: tauri::State<Arc<Core>>,
    id: String,
    mode: AccessMode,
) -> Result<Session, String> {
    core.store.set_access_mode(&id, mode)
}
#[tauri::command]
fn reorder_session(
    core: tauri::State<Arc<Core>>,
    id: String,
    before_id: Option<String>,
) -> Result<(), String> {
    core.store.reorder(&id, before_id.as_deref())
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
    gemini: bool,
    antigravity: bool,
    data_dir: String,
}
#[tauri::command]
fn diagnostics(core: tauri::State<Arc<Core>>) -> Diagnostics {
    Diagnostics {
        claude: providers::executable(&Provider::Claude).is_ok(),
        gpt: providers::executable(&Provider::Gpt).is_ok(),
        gemini: providers::executable(&Provider::Gemini).is_ok(),
        antigravity: providers::executable(&Provider::Antigravity).is_ok(),
        data_dir: core.root.to_string_lossy().into(),
    }
}

#[tauri::command]
async fn account_status(core: tauri::State<'_, Arc<Core>>, id: String) -> Result<bool, String> {
    let a = account(&core, &id)?;
    let mut c = providers::command(&a.provider)?;
    providers::isolate(&mut c, &a.provider, &config_dir(&core, &a));
    c.kill_on_drop(true);
    match a.provider {
        Provider::Claude => {
            c.args(["auth", "status"]);
        }
        Provider::Gpt => {
            c.args(["login", "status"]);
        }
        Provider::Gemini => {
            c.args(["--list-models"]);
        }
        Provider::Antigravity => {
            c.args(["models"]);
        }
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
    core.catalogs
        .lock()
        .map_err(|_| "Catalog lock failed")?
        .remove(&id);
    let exe = providers::executable(&a.provider)?;
    let dir = config_dir(&core, &a);
    #[cfg(windows)]
    {
        fn quote(s: &str) -> String {
            format!("'{}'", s.replace('\'', "''"))
        }
        let action = match a.provider {
            Provider::Claude => "auth login",
            Provider::Gpt => "login",
            Provider::Gemini => "",
            Provider::Antigravity => "",
        };
        let user_profile = if a.provider == Provider::Antigravity {
            format!("$env:USERPROFILE = {}\n", quote(&dir.to_string_lossy()))
        } else {
            String::new()
        };
        let script = format!(
            "$env:{} = {}\n{}{}\n& {} {}\nWrite-Host 'Return to Lumen and check connection.'\n",
            a.provider.env(),
            quote(&dir.to_string_lossy()),
            user_profile,
            [
                "ANTHROPIC_API_KEY",
                "ANTHROPIC_AUTH_TOKEN",
                "CLAUDE_CODE_OAUTH_TOKEN",
                "OPENAI_API_KEY",
                "CODEX_API_KEY",
                "CODEX_ACCESS_TOKEN",
                "GOOGLE_API_KEY",
                "GEMINI_API_KEY"
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
        fn quote(s: &str) -> String {
            format!("'{}'", s.replace('\'', "'\\''"))
        }
        let action = match a.provider {
            Provider::Claude => "auth login",
            Provider::Gpt => "login",
            _ => "",
        };
        let home = if a.provider == Provider::Antigravity {
            format!("export HOME={}\n", quote(&dir.to_string_lossy()))
        } else {
            String::new()
        };
        let script = format!(
            "#!/bin/sh\nexport {}={}\n{home}unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN CLAUDE_CODE_OAUTH_TOKEN OPENAI_API_KEY CODEX_API_KEY CODEX_ACCESS_TOKEN GOOGLE_API_KEY GEMINI_API_KEY\n{} {action}\necho 'Return to Lumen and check connection.'\n",
            a.provider.env(),
            quote(&dir.to_string_lossy()),
            quote(&exe.to_string_lossy()),
        );
        let path = dir.join("login.command");
        std::fs::write(&path, script).map_err(|e| e.to_string())?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .args(["-a", "Terminal"])
                .arg(&path)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        #[cfg(not(target_os = "macos"))]
        {
            let p = path.to_string_lossy().to_string();
            let tries: [(&str, Vec<&str>); 4] = [
                ("x-terminal-emulator", vec!["-e", &p]),
                ("gnome-terminal", vec!["--", &p]),
                ("konsole", vec!["-e", &p]),
                ("xterm", vec!["-e", &p]),
            ];
            if !tries
                .iter()
                .any(|(t, args)| std::process::Command::new(t).args(args).spawn().is_ok())
            {
                return Err(format!(
                    "No terminal found. Run {p} in a terminal to sign in."
                ));
            }
        }
    }
    Ok(())
}
#[tauri::command]
fn cancel_run(core: tauri::State<Arc<Core>>, id: String) -> Result<(), String> {
    if let Some(tx) = core.jobs.lock().map_err(|_| "Jobs lock failed")?.get(&id) {
        let _ = tx.send(true);
    }
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectContext {
    notes: String,
    files: Vec<context::FileEntry>,
    verify: Option<String>,
    git: bool,
}
#[tauri::command]
async fn project_context(folder: String) -> Result<ProjectContext, String> {
    let dir = dunce::canonicalize(&folder).map_err(|_| "Choose an existing folder")?;
    let git = gitops::toplevel(&dir).await.is_some();
    tauri::async_runtime::spawn_blocking(move || ProjectContext {
        notes: context::read_context(&dir),
        files: context::list_files(&dir, 5000),
        verify: context::verify_command(&dir),
        git,
    })
    .await
    .map_err(|e| e.to_string())
}
#[tauri::command]
fn save_project_context(folder: String, notes: String) -> Result<(), String> {
    let dir = dunce::canonicalize(&folder).map_err(|_| "Choose an existing folder")?;
    context::write_context(&dir, &notes)
}
#[tauri::command]
fn get_settings(core: tauri::State<Arc<Core>>) -> Result<settings::AppSettings, String> {
    Ok(core
        .settings
        .lock()
        .map_err(|_| "Settings lock failed")?
        .clone())
}
#[tauri::command]
async fn save_settings(
    core: tauri::State<'_, Arc<Core>>,
    mut value: settings::AppSettings,
) -> Result<settings::AppSettings, String> {
    value.validate()?;
    let mut api = core.api.lock().await;
    let restart = api.as_ref().map(|r| r.port) != Some(value.api_port) || !value.api_enabled;
    if restart {
        if let Some(r) = api.take() {
            let _ = r.stop.send(());
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        }
    }
    if value.api_enabled && api.is_none() {
        *api = Some(api::start(core.inner().clone(), value.api_port).await?);
    }
    value.save(&core.root)?;
    *core.settings.lock().map_err(|_| "Settings lock failed")? = value.clone();
    Ok(value)
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiStatus {
    running: bool,
    url: String,
    log: Vec<api::ApiLog>,
}
#[tauri::command]
async fn api_status(core: tauri::State<'_, Arc<Core>>) -> Result<ApiStatus, String> {
    let port = core.api.lock().await.as_ref().map(|r| r.port);
    Ok(ApiStatus {
        running: port.is_some(),
        url: format!(
            "http://localhost:{}/v1",
            port.unwrap_or_else(|| core.settings.lock().map(|s| s.api_port).unwrap_or(8317))
        ),
        log: core
            .api_log
            .lock()
            .map(|l| l.iter().cloned().collect())
            .unwrap_or_default(),
    })
}
#[tauri::command]
fn provider_health(core: tauri::State<Arc<Core>>) -> Result<Vec<limits::Health>, String> {
    let usage = read_usage(&core)?;
    Ok(core
        .store
        .accounts()?
        .iter()
        .map(|a| {
            let pressure = usage
                .iter()
                .find(|u| u.account_id == a.id)
                .and_then(|u| router::usage_pressure(u, now()));
            core.limits.health(&a.id, pressure, now())
        })
        .collect())
}
#[tauri::command]
fn mcp_health(core: tauri::State<Arc<Core>>) -> Result<serde_json::Value, String> {
    integrations::health(&core.root)
}
#[tauri::command]
fn mcp_toggle(core: tauri::State<Arc<Core>>, name: String, on: bool) -> Result<(), String> {
    integrations::toggle(&core.root, &name, on)
}
#[tauri::command]
fn mcp_add(core: tauri::State<Arc<Core>>, name: String, target: String) -> Result<(), String> {
    integrations::add_server(&core.root, name.trim(), &target)
}
#[tauri::command]
fn mcp_remove(core: tauri::State<Arc<Core>>, name: String) -> Result<(), String> {
    integrations::remove_server(&core.root, &name)
}

/// One read-only CLI turn for the local API. `lumen-auto` falls back across providers.
pub(crate) async fn headless(
    core: &Arc<Core>,
    model: &str,
    prompt: &str,
    mut emit: impl FnMut(String) + Send,
) -> Result<String, String> {
    let accounts = core.store.accounts()?;
    let order: Vec<Provider> = match model {
        "lumen-claude" => vec![Provider::Claude],
        "lumen-chatgpt" | "lumen-gpt" => vec![Provider::Gpt],
        _ => {
            let options = RunOptions {
                mode: "auto".into(),
                account_id: None,
                model: None,
                effort: String::new(),
                write_access: false,
                auto_policy: AutoPolicy::Default,
                balance_usage: false,
                access_mode: None,
                auto_review: false,
                attachments: vec![],
                audit_of: None,
            };
            let first = router::route(prompt, &options, &accounts)?.provider;
            let second = if first == Provider::Claude {
                Provider::Gpt
            } else {
                Provider::Claude
            };
            vec![first, second]
        }
    };
    let workspace = core.root.join("api-workspace");
    std::fs::create_dir_all(&workspace).map_err(|e| e.to_string())?;
    let mut last = "No connected account for this model. Add one in Lumen Settings.".to_string();
    for p in order {
        for a in accounts.iter().filter(|a| a.provider == p) {
            if core.limits.cooling(&a.id, now()) {
                continue;
            }
            let models = models_for(core, a, false, None).await?;
            let selected = router::select_model(&models, prompt, &AutoPolicy::Default)?;
            let effort = resolve_effort(selected, "low");
            let (_tx, rx) = watch::channel(false);
            let mut sent = String::new();
            let result = providers::execute(
                providers::Invocation {
                    account: a,
                    config: &config_dir(core, a),
                    workspace: &workspace,
                    prompt,
                    model: Some(&selected.id),
                    effort: &effort,
                    access: AccessMode::ReadOnly,
                },
                rx,
                |act| match act.kind.as_str() {
                    "delta" => {
                        sent.push_str(&act.text);
                        emit(act.text);
                    }
                    "replace" => {
                        if let Some(rest) = act.text.strip_prefix(sent.as_str()) {
                            if !rest.is_empty() {
                                emit(rest.to_string());
                            }
                        }
                        sent = act.text;
                    }
                    _ => {}
                },
            )
            .await;
            match result {
                Ok(t) => {
                    core.limits.record_turn(&a.id, now());
                    return Ok(t);
                }
                Err(e) if e.starts_with(limits::RATE_LIMIT) => {
                    let msg = e.trim_start_matches(limits::RATE_LIMIT).to_string();
                    core.limits.cool(&a.id, limits::unlock_at(&msg, now()));
                    last = format!("Rate limit: {msg}");
                }
                Err(e) => return Err(e),
            }
        }
    }
    Err(last)
}

/// GUI apps on macOS/Linux don't inherit the login shell PATH (node, claude, codex).
#[cfg(unix)]
fn fix_path() {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    if let Ok(out) = std::process::Command::new(shell)
        .args(["-ilc", "printf '__LUMEN_PATH__%s' \"$PATH\""])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        if let Some(path) = text
            .rsplit("__LUMEN_PATH__")
            .next()
            .filter(|p| !p.trim().is_empty())
        {
            std::env::set_var("PATH", path.trim());
        }
    }
}

pub fn run() {
    #[cfg(unix)]
    fix_path();
    let mut context = tauri::generate_context!();
    // Native tests isolate WebView2 as well as SQLite/account data. A running
    // installed app must never share the test's browser profile or debug port.
    if cfg!(debug_assertions) {
        if let Some(root) = std::env::var_os("LUMEN_TEST_DATA_DIR") {
            for window in &mut context.config_mut().app.windows {
                window.data_directory = Some(PathBuf::from(&root).join("webview"));
            }
        }
    }
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
            let prefs = settings::AppSettings::load(&root);
            let core = Arc::new(Core {
                store: db,
                root,
                jobs: Mutex::new(HashMap::new()),
                slots: Arc::new(Semaphore::new(4)),
                workspace_locks: Mutex::new(HashMap::new()),
                catalogs: Mutex::new(HashMap::new()),
                usage_lock: Mutex::new(()),
                limits: Default::default(),
                settings: Mutex::new(prefs.clone()),
                api: tokio::sync::Mutex::new(None),
                api_log: Mutex::new(VecDeque::new()),
            });
            app.manage(core.clone());
            if prefs.api_enabled {
                tauri::async_runtime::spawn(async move {
                    if let Ok(r) = api::start(core.clone(), prefs.api_port).await {
                        *core.api.lock().await = Some(r);
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_sessions,
            create_session,
            update_session,
            set_session_access_mode,
            reorder_session,
            delete_session,
            get_messages,
            list_accounts,
            add_account,
            remove_account,
            diagnostics,
            account_status,
            login_account,
            turns::send_message,
            turns::resolve_lanes,
            turns::rewind_preview,
            turns::rewind,
            turns::compact_session,
            cancel_run,
            project_context,
            save_project_context,
            get_settings,
            save_settings,
            api_status,
            provider_health,
            mcp_health,
            mcp_toggle,
            mcp_add,
            mcp_remove,
            list_models,
            rename_account,
            list_workspaces,
            add_workspace,
            set_session_workspace,
            get_usage,
            save_usage,
            refresh_usage,
            integration_status,
            install_skill,
            save_mcp
        ])
        .build(context)
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

fn resolve_effort(model: &ModelInfo, requested: &str) -> String {
    let levels = ["minimal", "low", "medium", "high", "xhigh", "max", "ultra"];
    let target = levels.iter().position(|e| *e == requested).unwrap_or(2) as i32;
    model
        .efforts
        .iter()
        .filter_map(|e| {
            levels
                .iter()
                .position(|l| *l == e)
                .map(|i| (e, (i as i32 - target).abs()))
        })
        .min_by_key(|(_, distance)| *distance)
        .map(|(e, _)| e.clone())
        .unwrap_or(model.default_effort.clone())
}
fn read_usage(core: &Core) -> Result<Vec<AccountUsage>, String> {
    let _lock = core.usage_lock.lock().map_err(|_| "Usage lock failed")?;
    read_usage_file(core)
}
fn read_usage_file(core: &Core) -> Result<Vec<AccountUsage>, String> {
    let path = core.root.join("usage.json");
    if !path.exists() {
        return Ok(vec![]);
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn get_usage(core: tauri::State<Arc<Core>>) -> Result<Vec<AccountUsage>, String> {
    read_usage(&core)
}
#[tauri::command]
fn save_usage(core: tauri::State<Arc<Core>>, mut usage: AccountUsage) -> Result<(), String> {
    account(&core, &usage.account_id)?;
    if usage.windows.len() > 10
        || usage.windows.iter().any(|w| {
            !w.used_percent.is_finite()
                || !(0.0..=100.0).contains(&w.used_percent)
                || w.window_minutes <= 0
                || w.resets_at <= now()
        })
    {
        return Err("Enter valid percentages and future reset times".into());
    }
    usage.updated_at = now();
    usage.source = "manual".into();
    write_usage(&core, usage)
}
fn write_usage(core: &Core, usage: AccountUsage) -> Result<(), String> {
    let _lock = core.usage_lock.lock().map_err(|_| "Usage lock failed")?;
    let mut data = read_usage_file(core)?;
    data.retain(|u| u.account_id != usage.account_id);
    data.push(usage);
    std::fs::write(
        core.root.join("usage.json"),
        serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
#[tauri::command]
async fn refresh_usage(
    core: tauri::State<'_, Arc<Core>>,
    id: String,
) -> Result<AccountUsage, String> {
    let a = account(&core, &id)?;
    let snapshot = usage::read(&a, &config_dir(&core, &a)).await?;
    write_usage(&core, snapshot.clone())?;
    Ok(snapshot)
}
#[tauri::command]
fn integration_status(core: tauri::State<Arc<Core>>) -> Result<serde_json::Value, String> {
    integrations::status(&core.root)
}
#[tauri::command]
fn install_skill(core: tauri::State<Arc<Core>>, path: String) -> Result<(), String> {
    integrations::install_skill(&core.root, &PathBuf::from(path))
}
#[tauri::command]
fn save_mcp(core: tauri::State<Arc<Core>>, json: String) -> Result<(), String> {
    integrations::save_mcp(&core.root, &json)
}

/// Usage-aware account choice for Auto (and the Google emergency fallback).
async fn balance(
    core: &Core,
    options: &RunOptions,
    accounts: &[Account],
    plan: &mut Plan,
) -> Result<(), String> {
    if options.balance_usage
        || accounts
            .iter()
            .any(|a| matches!(a.provider, Provider::Gemini | Provider::Antigravity))
    {
        let mut usage = read_usage(core)?;
        for a in accounts.iter().filter(|a| a.provider == Provider::Gpt) {
            let fresh = usage
                .iter()
                .find(|u| u.account_id == a.id)
                .and_then(|u| router::usage_pressure(u, now()))
                .is_some();
            if !fresh
                && (options.balance_usage
                    || accounts
                        .iter()
                        .any(|x| matches!(x.provider, Provider::Gemini | Provider::Antigravity)))
            {
                if let Ok(snapshot) = usage::read(a, &config_dir(core, a)).await {
                    write_usage(core, snapshot.clone())?;
                    usage.retain(|u| u.account_id != a.id);
                    usage.push(snapshot);
                }
            }
        }
        let primary: Vec<_> = accounts
            .iter()
            .filter(|a| matches!(a.provider, Provider::Claude | Provider::Gpt))
            .collect();
        // Google is a last resort only when every configured primary account has fresh exhausted quota.
        let scores: Option<Vec<_>> = primary
            .iter()
            .map(|a| {
                usage
                    .iter()
                    .find(|u| u.account_id == a.id)
                    .and_then(|u| router::usage_pressure(u, now()))
                    .map(|score| (*a, score))
            })
            .collect();
        if let Some(scores) = scores {
            let has_claude = primary.iter().any(|a| a.provider == Provider::Claude);
            let has_gpt = primary.iter().any(|a| a.provider == Provider::Gpt);
            if has_claude
                && has_gpt
                && !scores.is_empty()
                && scores.iter().all(|(_, score)| *score >= 1000.0)
            {
                if let Some(google) = accounts
                    .iter()
                    .find(|a| a.provider == Provider::Antigravity)
                    .or_else(|| accounts.iter().find(|a| a.provider == Provider::Gemini))
                {
                    plan.provider = google.provider.clone();
                    plan.account_id = google.id.clone();
                    plan.strategy = "single".into();
                    plan.partner_account = None;
                    plan.reason =
                        "Claude and GPT quotas confirmed exhausted; emergency Google fallback"
                            .into();
                } else {
                    return Err("Claude and GPT quotas are exhausted. Add a Gemini CLI or Antigravity account for emergency fallback.".into());
                }
            } else if options.balance_usage {
                if let Some((a, _)) = scores
                    .iter()
                    .filter(|(_, score)| *score < 1000.0)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                {
                    plan.provider = a.provider.clone();
                    plan.account_id = a.id.clone();
                    plan.partner_account = if plan.strategy == "single" {
                        None
                    } else {
                        scores
                            .iter()
                            .filter(|(b, score)| b.provider != a.provider && *score < 1000.0)
                            .min_by(|a, b| a.1.total_cmp(&b.1))
                            .map(|(b, _)| b.id.clone())
                    };
                    if plan.partner_account.is_none() {
                        plan.strategy = "single".into();
                    }
                }
            }
        } else {
            plan.reason
                .push_str(" · usage balancing unavailable: missing/stale quota data");
        }
    }
    Ok(())
}
