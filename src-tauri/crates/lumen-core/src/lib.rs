pub mod host;
pub use lumen_commands::command;
#[cfg(unix)]
pub use proc::{own_processes, run_group_guard, run_owned_child};
/// A borrowed command context, independent of any desktop runtime.
#[derive(Clone, Copy)]
pub struct CoreRef<'a>(&'a std::sync::Arc<Core>);
impl<'a> CoreRef<'a> {
    pub fn inner(&self) -> &'a std::sync::Arc<Core> {
        self.0
    }
}
impl std::ops::Deref for CoreRef<'_> {
    type Target = std::sync::Arc<Core>;
    fn deref(&self) -> &Self::Target {
        self.0
    }
}
mod activity;
mod adapter_contract;
mod api;
mod api_tools;
mod api_wire;
mod approvals;
mod approvals_codex;
mod assets;
mod browser;
mod capability_catalog;
mod catalog;
mod command_receipts;
pub use command_receipts::dispatch as dispatch_receipted;
mod cursor;
mod opencode;
mod opencode_acp;
mod quota_resume;
pub mod routines;
pub fn serve_plugin_mcp(data: PathBuf, id: &str, server: &str) -> Result<(), String> {
    plugins::serve_mcp(&data, id, server)
}
pub async fn serve_shared_stdio(data: PathBuf, workspace: PathBuf, id: &str) -> Result<(), String> {
    capability_catalog::serve_stdio(data, workspace, id).await
}
mod autonomy;
mod classify;
mod code_mcp;
mod code_mode;
#[cfg(test)]
mod code_mode_bench;
mod codegraph;
mod connectors;
mod context;
mod diagnostics;
mod execution;
mod gitops;
mod graph_mcp;
mod impact;
mod inspector;
mod integrations;
mod interactions;
#[cfg(test)]
mod latency_probe;
mod layers;
mod limits;
mod media;
#[cfg(windows)]
mod metadata_windows;
mod migrations;
mod models;
mod native_mcp;
pub async fn serve_native_account(
    profile: PathBuf,
    workspace: PathBuf,
    read_only: bool,
) -> Result<(), String> {
    native_mcp::serve(profile, workspace, read_only).await
}
#[cfg(test)]
mod compete_corpus;
#[cfg(test)]
mod handoff_corpus;
mod native_claude;
mod ollama;
mod ollama_agent;
mod orchestra;
mod plugins;
mod pr_watch;
mod proc;
mod projecttools;
mod providers;
mod repomap;
mod review;
mod rlm;
mod router;
mod scheduling;
mod screens;
mod settings;
mod shared_http;
mod shared_tools;
mod store;
mod task_worktree;
#[cfg(test)]
mod test_support;
mod turns;
mod usage;
mod validation;
mod wire;
mod workspace;
mod workstate;
use models::*;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::{watch, Semaphore};

pub struct Core {
    stopping: std::sync::atomic::AtomicBool,
    store: Arc<store::Store>,
    root: PathBuf,
    jobs: Mutex<HashMap<String, watch::Sender<bool>>>,
    slots: Arc<Semaphore>,
    /// At most eight CLI inference processes at once, Orchestra subtasks included.
    inference: Arc<Semaphore>,
    workspace_locks: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    command_locks: Mutex<HashMap<String, std::sync::Weak<tokio::sync::Mutex<()>>>>,
    catalogs: Mutex<HashMap<String, (std::time::Instant, Vec<ModelInfo>)>>,
    usage_lock: Mutex<()>,
    limits: limits::Limits,
    settings: Mutex<settings::AppSettings>,
    /// Approval journal for Ask-first turns; None when it could not be opened.
    approvals: Option<lumen_gatekeeper::Gatekeeper>,
    /// Extra subtasks the user admitted beyond the total cap, per running Orchestra message.
    orchestra_raises: Mutex<HashMap<String, usize>>,
}
impl Core {
    fn new(
        store: store::Store,
        root: PathBuf,
        prefs: settings::AppSettings,
        approvals: Option<lumen_gatekeeper::Gatekeeper>,
    ) -> Self {
        Self {
            stopping: std::sync::atomic::AtomicBool::new(false),
            store: Arc::new(store),
            root,
            approvals,
            jobs: Mutex::new(HashMap::new()),
            slots: Arc::new(Semaphore::new(4)),
            inference: Arc::new(Semaphore::new(8)),
            workspace_locks: Mutex::new(HashMap::new()),
            command_locks: Mutex::new(HashMap::new()),
            catalogs: Mutex::new(HashMap::new()),
            usage_lock: Mutex::new(()),
            orchestra_raises: Mutex::new(HashMap::new()),
            limits: Default::default(),
            settings: Mutex::new(prefs),
        }
    }
    /// Signal every running job to stop; true when some were running.
    pub(crate) fn cancel_all(&self) -> bool {
        self.jobs
            .lock()
            .map(|jobs| {
                for tx in jobs.values() {
                    let _ = tx.send(true);
                }
                !jobs.is_empty()
            })
            .unwrap_or(false)
    }
    pub(crate) fn jobs_idle(&self) -> bool {
        self.jobs.lock().map(|jobs| jobs.is_empty()).unwrap_or(true)
    }
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
    let models = if a.provider == Provider::Ollama {
        let pinned = core
            .settings
            .lock()
            .map(|s| s.ollama_model.clone())
            .unwrap_or_default();
        ollama::catalog(&pinned).await?
    } else {
        catalog::discover(a, &config_dir(core, a), cancel).await?
    };
    core.catalogs
        .lock()
        .map_err(|_| "Catalog lock failed")?
        .insert(a.id.clone(), (std::time::Instant::now(), models.clone()));
    Ok(models)
}
async fn models_in(
    core: &Core,
    a: &Account,
    refresh: bool,
    cancel: Option<watch::Receiver<bool>>,
    workspace: &str,
) -> Result<Vec<ModelInfo>, String> {
    // OpenCode project configuration can add or disable models independently of the native profile.
    if a.provider == Provider::Opencode && !workspace.is_empty() {
        return opencode::models(Some(std::path::Path::new(workspace)), cancel).await;
    }
    models_for(core, a, refresh, cancel).await
}
#[crate::command]
async fn list_models(
    core: crate::CoreRef<'_>,
    id: String,
    refresh: Option<bool>,
    workspace: Option<String>,
) -> Result<Vec<ModelInfo>, String> {
    let a = account(&core, &id)?;
    models_in(
        &core,
        &a,
        refresh.unwrap_or(false),
        None,
        workspace.as_deref().unwrap_or(""),
    )
    .await
}
#[crate::command]
fn list_workspaces(core: crate::CoreRef<'_>) -> Result<Vec<Workspace>, String> {
    core.store.workspaces()
}
#[crate::command]
fn add_workspace(
    core: crate::CoreRef<'_>,
    name: String,
    folder: String,
) -> Result<Workspace, String> {
    let name = name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err("Choose a workspace name of 1–100 characters".into());
    }
    core.store.add_workspace(name.into(), folder)
}
#[crate::command]
fn rename_workspace(core: crate::CoreRef<'_>, id: String, name: String) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err("Choose a project name of 1–100 characters".into());
    }
    core.store.rename_workspace(&id, name)
}
#[crate::command]
fn delete_workspace(core: crate::CoreRef<'_>, id: String) -> Result<(), String> {
    let jobs = core.jobs.lock().map_err(|_| "Jobs lock failed")?;
    let sessions = core.store.workspace_sessions(&id)?;
    if sessions.iter().any(|id| jobs.contains_key(id)) {
        return Err("Stop the running conversations in this project before deleting it".into());
    }
    for session in &sessions {
        workstate::remove_snapshot(&core, session)?;
    }
    core.store.delete_workspace(&id)
}
#[crate::command]
fn set_session_workspace(
    core: crate::CoreRef<'_>,
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
#[crate::command]
fn rename_account(core: crate::CoreRef<'_>, id: String, name: String) -> Result<(), String> {
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
fn event(app: &crate::host::EventHub, s: &str, m: &str, kind: &str, text: &str) {
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

#[crate::command]
fn list_sessions(
    core: crate::CoreRef<'_>,
    search: String,
    offset: u32,
) -> Result<Vec<Session>, String> {
    core.store.sessions(&search, offset)
}
#[crate::command]
fn create_session(core: crate::CoreRef<'_>, workspace: String) -> Result<Session, String> {
    let path = dunce::canonicalize(workspace).map_err(|_| "Choose an existing workspace folder")?;
    if !path.is_dir() {
        return Err("Workspace must be a folder".into());
    }
    core.store.create(path.to_string_lossy().into_owned())
}
#[crate::command]
fn update_session(
    core: crate::CoreRef<'_>,
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
#[crate::command]
fn set_session_access_mode(
    core: crate::CoreRef<'_>,
    id: String,
    mode: AccessMode,
) -> Result<Session, String> {
    core.store.set_access_mode(&id, mode)
}
#[crate::command]
fn reorder_session(
    core: crate::CoreRef<'_>,
    id: String,
    before_id: Option<String>,
) -> Result<(), String> {
    core.store.reorder(&id, before_id.as_deref())
}
#[crate::command]
fn delete_session(core: crate::CoreRef<'_>, id: String) -> Result<(), String> {
    if core
        .jobs
        .lock()
        .map_err(|_| "Jobs lock failed")?
        .contains_key(&id)
    {
        return Err("Stop this conversation before deleting it".into());
    }
    workstate::remove_snapshot(&core, &id)?;
    core.store.delete(&id)
}
#[crate::command]
fn get_messages(core: crate::CoreRef<'_>, id: String) -> Result<Vec<serde_json::Value>, String> {
    let mut messages = core.store.message_snapshots(&id)?;
    wire::project_messages(&mut messages);
    Ok(messages)
}
/// Full text of a tool output or diff that `get_messages` or a live event sent bounded.
#[crate::command]
fn activity_detail(
    core: crate::CoreRef<'_>,
    id: String,
    message: String,
    digest: String,
) -> Result<String, String> {
    wire::detail(&core.store.message_snapshots(&id)?, &message, &digest)
        .ok_or("This output is no longer available; reopen the conversation".into())
}
#[crate::command]
fn list_accounts(core: crate::CoreRef<'_>) -> Result<Vec<Account>, String> {
    core.store.accounts()
}
#[crate::command]
fn add_account(
    core: crate::CoreRef<'_>,
    name: String,
    provider: Provider,
) -> Result<Account, String> {
    if name.trim().is_empty() || name.len() > 80 {
        return Err("Use an account name of 1–80 characters".into());
    }
    if matches!(provider, Provider::Opencode | Provider::Cursor)
        && core
            .store
            .accounts()?
            .iter()
            .any(|a| a.provider == provider)
    {
        return Err(
            "This adapter uses one existing native profile; it cannot create isolated identities yet."
                .into(),
        );
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
#[crate::command]
fn remove_account(core: crate::CoreRef<'_>, id: String) -> Result<(), String> {
    if !core.jobs.lock().map_err(|_| "Jobs lock failed")?.is_empty() {
        return Err("Stop active conversations before removing accounts".into());
    }
    let a = account(&core, &id)?;
    let dir = config_dir(&core, &a);
    if a.provider.is_api() {
        crate::api::forget_key(&a.id)?;
    }
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
    antigravity: bool,
    ollama: bool,
    data_dir: String,
}
#[crate::command]
fn diagnostics(core: crate::CoreRef<'_>) -> Diagnostics {
    Diagnostics {
        claude: providers::executable(&Provider::Claude).is_ok(),
        gpt: providers::executable(&Provider::Gpt).is_ok(),
        antigravity: providers::executable(&Provider::Antigravity).is_ok(),
        ollama: providers::executable(&Provider::Ollama).is_ok(),
        data_dir: core.root.to_string_lossy().into(),
    }
}

#[crate::command]
async fn account_status(core: crate::CoreRef<'_>, id: String) -> Result<bool, String> {
    let a = account(&core, &id)?;
    if a.provider.is_api() {
        return crate::api::has_key(&a.id);
    }
    if a.provider == Provider::Ollama {
        return Ok(ollama::list().await.map(|m| !m.is_empty()).unwrap_or(false));
    }
    if a.provider == Provider::Cursor {
        return Ok(!crate::cursor::models().await?.is_empty());
    }
    if a.provider == Provider::Opencode {
        return Ok(!crate::opencode::models(None, None).await?.is_empty());
    }
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

        Provider::Antigravity => {
            c.args(["models"]);
        }
        Provider::Ollama
        | Provider::Opencode
        | Provider::Cursor
        | Provider::Openaiapi
        | Provider::Anthropicapi
        | Provider::Googleapi => unreachable!(),
    }
    let out = tokio::time::timeout(std::time::Duration::from_secs(15), c.output())
        .await
        .map_err(|_| "Status check timed out")?
        .map_err(|e| e.to_string())?;
    Ok(out.status.success())
}
#[crate::command]
fn login_account(core: crate::CoreRef<'_>, id: String) -> Result<PathBuf, String> {
    let a = account(&core, &id)?;
    if a.provider == Provider::Ollama {
        return Err("Ollama runs locally and has no sign-in. Install it from ollama.com.".into());
    }
    core.catalogs
        .lock()
        .map_err(|_| "Catalog lock failed")?
        .remove(&id);
    if a.provider.is_api() {
        return Err("Use API settings to save a key in the OS keychain".into());
    }
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
            Provider::Antigravity | Provider::Ollama => "",
            Provider::Opencode => "auth login",
            Provider::Cursor => "login",
            Provider::Openaiapi | Provider::Anthropicapi | Provider::Googleapi => {
                return Err("Use API settings to save a key in the OS keychain".into())
            }
        };
        let user_profile = if a.provider == Provider::Antigravity {
            format!("$env:USERPROFILE = {}\n", quote(&dir.to_string_lossy()))
        } else {
            String::new()
        };
        let script = format!(
            "$env:{} = {}\n{}{}\n& {} {}\nWrite-Host 'Return to Lumen and check connection.'\n",
            if matches!(a.provider, Provider::Opencode | Provider::Cursor) {
                "LUMEN_NATIVE_PROFILE"
            } else {
                a.provider.env()
            },
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
                "GEMINI_API_KEY",
                "CURSOR_API_KEY"
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
        Ok(path)
    }
    #[cfg(not(windows))]
    {
        fn quote(s: &str) -> String {
            format!("'{}'", s.replace('\'', "'\\''"))
        }
        let action = match a.provider {
            Provider::Claude => "auth login",
            Provider::Gpt => "login",
            Provider::Opencode => "auth login",
            Provider::Cursor => "login",
            Provider::Openaiapi | Provider::Anthropicapi | Provider::Googleapi => {
                return Err("Use API settings to save a key in the OS keychain".into())
            }
            _ => "",
        };
        let home = if a.provider == Provider::Antigravity {
            format!("export HOME={}\n", quote(&dir.to_string_lossy()))
        } else {
            String::new()
        };
        let script = format!(
            "#!/bin/sh\nexport {}={}\n{home}unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN CLAUDE_CODE_OAUTH_TOKEN OPENAI_API_KEY CODEX_API_KEY CODEX_ACCESS_TOKEN GOOGLE_API_KEY GEMINI_API_KEY CURSOR_API_KEY\n{} {action}\necho 'Return to Lumen and check connection.'\n",
            if matches!(a.provider, Provider::Opencode | Provider::Cursor) { "LUMEN_NATIVE_PROFILE" } else { a.provider.env() },
            quote(&dir.to_string_lossy()),
            quote(&exe.to_string_lossy()),
        );
        let path = dir.join("login.command");
        std::fs::write(&path, script).map_err(|e| e.to_string())?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
        Ok(path)
    }
}

#[crate::command]
fn cancel_run(core: crate::CoreRef<'_>, id: String) -> Result<(), String> {
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
#[crate::command]
async fn project_context(folder: String) -> Result<ProjectContext, String> {
    let dir = dunce::canonicalize(&folder).map_err(|_| "Choose an existing folder")?;
    let git = gitops::toplevel(&dir).await.is_some();
    tokio::task::spawn_blocking(move || ProjectContext {
        notes: context::read_context(&dir),
        files: context::list_files(&dir, 5000),
        verify: context::verify_command(&dir),
        git,
    })
    .await
    .map_err(|e| e.to_string())
}
#[crate::command]
fn save_project_context(folder: String, notes: String) -> Result<(), String> {
    let dir = dunce::canonicalize(&folder).map_err(|_| "Choose an existing folder")?;
    context::write_context(&dir, &notes)
}
#[crate::command]
fn get_settings(core: crate::CoreRef<'_>) -> Result<settings::AppSettings, String> {
    Ok(core
        .settings
        .lock()
        .map_err(|_| "Settings lock failed")?
        .clone())
}
#[crate::command]
fn acknowledge_tray_hint(core: crate::CoreRef<'_>) -> Result<(), String> {
    let mut prefs = core.settings.lock().map_err(|_| "Settings lock failed")?;
    prefs.tray_hint_shown = true;
    prefs.save(&core.root)
}
#[crate::command]
async fn save_settings(
    core: crate::CoreRef<'_>,
    mut value: settings::AppSettings,
) -> Result<settings::AppSettings, String> {
    value.validate()?;
    value.tray_hint_shown |= core
        .settings
        .lock()
        .map(|s| s.tray_hint_shown)
        .unwrap_or(false);
    host::autostart(&core.root, value.open_at_login)?;
    value.save(&core.root)?;
    *core.settings.lock().map_err(|_| "Settings lock failed")? = value.clone();
    Ok(value)
}

#[crate::command]
async fn reset_settings(core: crate::CoreRef<'_>) -> Result<settings::AppSettings, String> {
    let value = settings::AppSettings::default();
    host::autostart(&core.root, false)?;
    value.save(&core.root)?;
    *core.settings.lock().map_err(|_| "Settings lock failed")? = value.clone();
    Ok(value)
}
#[crate::command]
fn provider_health(core: crate::CoreRef<'_>) -> Result<Vec<limits::Health>, String> {
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
#[crate::command]
fn mcp_health(core: crate::CoreRef<'_>) -> Result<serde_json::Value, String> {
    integrations::health(&core.root)
}
#[crate::command]
fn mcp_toggle(core: crate::CoreRef<'_>, name: String, on: bool) -> Result<(), String> {
    integrations::toggle(&core.root, &name, on)
}
#[crate::command]
fn mcp_add(core: crate::CoreRef<'_>, name: String, target: String) -> Result<(), String> {
    integrations::add_server(&core.root, name.trim(), &target)
}
#[crate::command]
fn mcp_remove(core: crate::CoreRef<'_>, name: String) -> Result<(), String> {
    integrations::remove_server(&core.root, &name)
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

/// Forwards Gatekeeper changes to the window. A lagged listener asks the UI to reload its snapshot.
fn relay_approvals(app: crate::host::EventHub, keeper: lumen_gatekeeper::Gatekeeper) {
    let mut rx = keeper.subscribe();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let _ = app.emit("lumen:gatekeeper", event);
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let _ = app.emit("lumen:gatekeeper", serde_json::json!({"resync": true}));
                }
                Err(_) => break,
            }
        }
    });
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ApprovalSnapshot {
    requests: Vec<lumen_gatekeeper::ApprovalRequest>,
    sessions: Vec<lumen_gatekeeper::SessionView>,
}

/// Approvals and live provider sessions belonging to one chat.
#[crate::command]
fn gatekeeper_snapshot(core: crate::CoreRef<'_>, chat: String) -> Result<ApprovalSnapshot, String> {
    let keeper = core
        .approvals
        .as_ref()
        .ok_or("unavailable: The approval journal could not be opened.")?;
    Ok(ApprovalSnapshot {
        requests: keeper
            .snapshot()?
            .into_iter()
            .filter(|r| r.scope.session_id == chat)
            .collect(),
        sessions: keeper
            .sessions()?
            .into_iter()
            .filter(|s| s.scope.session_id == chat)
            .collect(),
    })
}

/// The renderer sends only the request ID, scope and action. The broker keeps the immutable request.
#[crate::command]
fn gatekeeper_decide(
    core: crate::CoreRef<'_>,
    decision: lumen_gatekeeper::Decision,
) -> Result<(), String> {
    let keeper = core
        .approvals
        .as_ref()
        .ok_or("unavailable: The approval journal could not be opened.")?;
    core.store.session(&decision.scope.session_id)?;
    keeper.decide(decision)
}

/// Entry for `lumen --lumen-graph-mcp <data> <root>`: the code graph as a stdio MCP server.
pub async fn serve_browser() -> Result<(), String> {
    browser::proxy_stdio().await
}

pub async fn serve_code() -> Result<(), String> {
    browser::proxy_code_stdio().await
}
pub fn serve_graph(data: std::path::PathBuf, root: std::path::PathBuf) -> std::io::Result<()> {
    graph_mcp::serve(data, root)
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
#[crate::command]
fn get_usage(core: crate::CoreRef<'_>) -> Result<Vec<AccountUsage>, String> {
    read_usage(&core)
}
fn write_usage(core: &Core, mut usage: AccountUsage) -> Result<AccountUsage, String> {
    let _lock = core.usage_lock.lock().map_err(|_| "Usage lock failed")?;
    let mut data = read_usage_file(core)?;
    if usage.source == "claude-cli" {
        if let Some(previous) = data
            .iter()
            .find(|u| u.account_id == usage.account_id && u.source == usage.source)
        {
            usage::merge_claude_windows(&mut usage, previous);
        }
    }
    data.retain(|u| u.account_id != usage.account_id);
    data.push(usage.clone());
    std::fs::write(
        core.root.join("usage.json"),
        serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(usage)
}
#[crate::command]
async fn refresh_usage(core: crate::CoreRef<'_>, id: String) -> Result<AccountUsage, String> {
    let a = account(&core, &id)?;
    let snapshot = usage::read(&a, &config_dir(&core, &a)).await?;
    write_usage(&core, snapshot)
}
fn record_claude_usage(core: &Core, app: &crate::host::EventHub, account_id: &str, raw: &str) {
    if let Ok(value) = serde_json::from_str(raw) {
        if let Some(snapshot) = usage::claude_event(account_id, &value) {
            if let Ok(snapshot) = write_usage(core, snapshot) {
                let _ = app.emit("lumen:usage", snapshot);
            }
        }
    }
}
#[crate::command]
fn integration_status(core: crate::CoreRef<'_>) -> Result<serde_json::Value, String> {
    let mut state = integrations::status(&core.root)?;
    let mut catalog = capability_catalog::discover(&core.root, None);
    for account in core.store.accounts()? {
        for entry in &mut catalog.entries {
            entry.source = entry
                .source
                .replace(&format!("Lumen account {}", account.id), &account.name);
        }
    }
    state["discovered"] = serde_json::to_value(catalog.entries)
        .map_err(|_| "Could not read integration inventory")?;
    state["warnings"] = serde_json::json!(catalog.warnings);
    Ok(state)
}
#[crate::command]
fn toggle_discovered_integration(
    core: crate::CoreRef<'_>,
    id: String,
    on: bool,
) -> Result<(), String> {
    capability_catalog::toggle(&core.root, &id, on)
}
#[crate::command]
async fn ollama_status(core: crate::CoreRef<'_>) -> Result<ollama::OllamaStatus, String> {
    let pinned = core
        .settings
        .lock()
        .map(|s| s.ollama_model.clone())
        .unwrap_or_default();
    Ok(ollama::status(&pinned).await)
}
#[crate::command]
fn save_recipe(core: crate::CoreRef<'_>, id: String, message: String) -> Result<String, String> {
    let history = core.store.messages(&id)?;
    let at = history
        .iter()
        .position(|m| m.id == message && m.role == "assistant")
        .ok_or("Reply not found")?;
    let request = history[..at]
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .ok_or("This reply has no request to save")?;
    let name = integrations::save_recipe(&core.root, &request.text, &history[at].text)?;
    let mut reply = history[at].clone();
    reply.activity.push(crate::models::Activity {
        kind: "recipe-saved".into(),
        text: name.clone(),
    });
    core.store.save_message(&id, &reply)?;
    Ok(name)
}
#[crate::command]
fn remember_global(
    core: crate::CoreRef<'_>,
    id: String,
    message: String,
    fact: String,
) -> Result<(), String> {
    let mut reply = core
        .store
        .messages(&id)?
        .into_iter()
        .find(|m| m.id == message)
        .ok_or("Reply not found")?;
    let proposed = reply
        .activity
        .iter()
        .any(|a| a.kind == "memory-proposed" && a.text.lines().any(|l| l == fact));
    if !proposed {
        return Err("Only a fact an assistant proposed can be saved".into());
    }
    let title = core.store.session(&id)?.title;
    integrations::remember_global(&core.root, &fact, crate::models::now() / 1000, &title)?;
    reply.activity.push(crate::models::Activity {
        kind: "memory-global".into(),
        text: fact,
    });
    core.store.save_message(&id, &reply)
}
/// Saves to LUMEN.md a project fact that was held because its reply read untrusted content.
#[crate::command]
fn remember_project(
    core: crate::CoreRef<'_>,
    id: String,
    message: String,
    fact: String,
) -> Result<(), String> {
    let mut reply = core
        .store
        .messages(&id)?
        .into_iter()
        .find(|m| m.id == message)
        .ok_or("Reply not found")?;
    let held = reply
        .activity
        .iter()
        .any(|a| a.kind == "memory-held" && a.text.lines().any(|l| l == fact));
    if !held {
        return Err("Only a fact an assistant proposed can be saved".into());
    }
    let s = core.store.session(&id)?;
    let saved = context::save_memory(
        std::path::Path::new(&s.workspace),
        vec![fact],
        crate::models::now() / 1000,
        &s.title,
    );
    reply.activity.push(crate::models::Activity {
        kind: "memory-saved".into(),
        text: saved.join("\n"),
    });
    core.store.save_message(&id, &reply)
}
/// The conversation's audit log: classified actions and Lumen's decisions, rollbacks included.
#[crate::command]
fn audit_log(core: crate::CoreRef<'_>, id: String) -> Result<Vec<autonomy::Entry>, String> {
    let mut log = autonomy::audit(&core.store.messages(&id)?);
    log.extend(autonomy::logged(&core.root, &id));
    Ok(log)
}
#[crate::command]
fn install_skill(core: crate::CoreRef<'_>, path: String) -> Result<(), String> {
    integrations::install_skill(&core.root, &PathBuf::from(path))
}
#[crate::command]
fn plugins_list(core: crate::CoreRef<'_>) -> Vec<serde_json::Value> {
    plugins::list(&core.root)
}
#[crate::command]
async fn plugin_inspect(
    core: crate::CoreRef<'_>,
    source: String,
) -> Result<serde_json::Value, String> {
    plugins::inspect(&core.root, &source).await
}
#[crate::command]
fn plugin_install(core: crate::CoreRef<'_>, token: String) -> Result<serde_json::Value, String> {
    plugins::install(&core.root, &token)
}
#[crate::command]
fn plugin_set_enabled(core: crate::CoreRef<'_>, id: String, on: bool) -> Result<(), String> {
    plugins::set_enabled(&core.root, &id, on)
}
#[crate::command]
fn plugin_remove(core: crate::CoreRef<'_>, id: String) -> Result<(), String> {
    plugins::remove(&core.root, &id)
}
#[crate::command]
async fn plugin_reload(core: crate::CoreRef<'_>, id: String) -> Result<serde_json::Value, String> {
    plugins::reload(&core.root, &id).await
}
#[crate::command]
fn plugin_contributions(core: crate::CoreRef<'_>) -> serde_json::Value {
    plugins::contributions(&core.root)
}
#[crate::command]
fn plugin_panel(core: crate::CoreRef<'_>, id: String, panel: String) -> Result<String, String> {
    plugins::panel_html(&core.root, &id, &panel)
}
#[crate::command]
async fn plugin_panel_fetch(
    core: crate::CoreRef<'_>,
    id: String,
    url: String,
) -> Result<serde_json::Value, String> {
    plugins::panel_fetch(&core.root, &id, &url).await
}
#[crate::command]
fn plugin_set_secret(
    core: crate::CoreRef<'_>,
    id: String,
    name: String,
    value: String,
) -> Result<(), String> {
    plugins::set_secret(&core.root, &id, &name, &value)
}
#[crate::command]
fn save_mcp(core: crate::CoreRef<'_>, json: String) -> Result<(), String> {
    integrations::save_mcp(&core.root, &json)
}

/// Usage-aware account choice for Auto (and the Google emergency fallback).
async fn balance(
    core: &Core,
    options: &RunOptions,
    accounts: &[Account],
    plan: &mut Plan,
) -> Result<bool, String> {
    let mut selected_by_usage = false;
    if options.balance_usage || accounts.iter().any(|a| a.provider.fallback_rank() > 0) {
        let mut usage = read_usage(core)?;
        for a in accounts.iter().filter(|a| a.provider == Provider::Gpt) {
            let fresh = usage
                .iter()
                .find(|u| u.account_id == a.id)
                .and_then(|u| router::usage_pressure(u, now()))
                .is_some();
            if !fresh
                && (options.balance_usage
                    || accounts.iter().any(|x| x.provider.fallback_rank() > 0))
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
            if !scores.is_empty() && scores.iter().all(|(_, score)| *score >= 1000.0) {
                if let Some(google) = accounts
                    .iter()
                    .filter(|a| {
                        a.provider.fallback_rank() > 0
                            && !core.limits.cooling(&a.id, now())
                            && !usage
                                .iter()
                                .find(|u| u.account_id == a.id)
                                .and_then(|u| router::usage_pressure(u, now()))
                                .is_some_and(|p| p >= 1000.0)
                    })
                    .min_by_key(|a| a.provider.fallback_rank())
                {
                    plan.provider = google.provider.clone();
                    selected_by_usage = true;
                    plan.account_id = google.id.clone();
                    plan.strategy = "single".into();
                    plan.partner_account = None;
                    plan.reason = format!(
                        "Claude and GPT quotas confirmed exhausted; emergency fallback to {}",
                        google.provider.binary()
                    );
                } else {
                    return Err("Claude and GPT quotas are exhausted. Add an Antigravity or Ollama account for emergency fallback.".into());
                }
            } else if options.balance_usage {
                if let Some((a, _)) = scores
                    .iter()
                    .filter(|(_, score)| *score < 1000.0)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                {
                    plan.provider = a.provider.clone();
                    selected_by_usage = true;
                    plan.account_id = a.id.clone();
                    plan.partner_account = if plan.strategy == "single" {
                        None
                    } else {
                        scores
                            .iter()
                            .filter(|(b, score)| b.id != a.id && *score < 1000.0)
                            .min_by(|left, right| {
                                (left.0.provider == a.provider)
                                    .cmp(&(right.0.provider == a.provider))
                                    .then(left.1.total_cmp(&right.1))
                            })
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
    Ok(selected_by_usage)
}

pub async fn dispatch(
    app: host::EventHub,
    core: &Arc<Core>,
    command: &str,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    match command {
        "acknowledge_tray_hint" => __ipc_acknowledge_tray_hint(app, core, args).await,
        "inspect_json" => inspector::__ipc_inspect_json(app, core, args).await,
        "inspect_sqlite" => inspector::__ipc_inspect_sqlite(app, core, args).await,
        "file_change_diff" => review::__ipc_file_change_diff(app, core, args).await,
        "count_tokens" => context::__ipc_count_tokens(app, core, args).await,
        "pending_questions" => interactions::__ipc_pending_questions(app, core, args).await,
        "answer_question" => interactions::__ipc_answer_question(app, core, args).await,
        "open_connector_url" => interactions::__ipc_open_connector_url(app, core, args).await,
        "toggle_discovered_integration" => {
            __ipc_toggle_discovered_integration(app, core, args).await
        }
        "list_sessions" => __ipc_list_sessions(app, core, args).await,
        "create_session" => __ipc_create_session(app, core, args).await,
        "update_session" => __ipc_update_session(app, core, args).await,
        "set_session_access_mode" => __ipc_set_session_access_mode(app, core, args).await,
        "reorder_session" => __ipc_reorder_session(app, core, args).await,
        "delete_session" => __ipc_delete_session(app, core, args).await,
        "get_messages" => __ipc_get_messages(app, core, args).await,
        "activity_detail" => __ipc_activity_detail(app, core, args).await,
        "list_accounts" => __ipc_list_accounts(app, core, args).await,
        "adapter_capabilities" => {
            adapter_contract::__ipc_adapter_capabilities(app, core, args).await
        }
        "get_api_settings" => api::__ipc_get_api_settings(app, core, args).await,
        "save_api_settings" => api::__ipc_save_api_settings(app, core, args).await,
        "save_api_key" => api::__ipc_save_api_key(app, core, args).await,
        "create_routine" => routines::__ipc_create_routine(app, core, args).await,
        "list_routines" => routines::__ipc_list_routines(app, core, args).await,
        "delete_routine" => routines::__ipc_delete_routine(app, core, args).await,
        "resume_at_reset" => quota_resume::__ipc_resume_at_reset(app, core, args).await,
        "add_account" => __ipc_add_account(app, core, args).await,
        "remove_account" => __ipc_remove_account(app, core, args).await,
        "diagnostics" => __ipc_diagnostics(app, core, args).await,
        "account_status" => __ipc_account_status(app, core, args).await,
        "login_account" => __ipc_login_account(app, core, args).await,
        "send_message" => turns::__ipc_send_message(app, core, args).await,
        "rewind_preview" => turns::__ipc_rewind_preview(app, core, args).await,
        "rewind" => turns::__ipc_rewind(app, core, args).await,
        "compact_session" => turns::__ipc_compact_session(app, core, args).await,
        "prompt_estimate" => turns::__ipc_prompt_estimate(app, core, args).await,
        "session_work_state" => workstate::__ipc_session_work_state(app, core, args).await,
        "gatekeeper_snapshot" => __ipc_gatekeeper_snapshot(app, core, args).await,
        "gatekeeper_decide" => __ipc_gatekeeper_decide(app, core, args).await,
        "cancel_run" => __ipc_cancel_run(app, core, args).await,
        "project_context" => __ipc_project_context(app, core, args).await,
        "project_tools" => projecttools::__ipc_project_tools(app, core, args).await,
        "project_checks" => projecttools::__ipc_project_checks(app, core, args).await,
        "change_layers" => layers::__ipc_change_layers(app, core, args).await,
        "repo_map_search" => repomap::__ipc_repo_map_search(app, core, args).await,
        "repo_map_references" => repomap::__ipc_repo_map_references(app, core, args).await,
        "capture_targets" => screens::__ipc_capture_targets(app, core, args).await,
        "capture_screenshot" => screens::__ipc_capture_screenshot(app, core, args).await,
        "save_pasted_image" => screens::__ipc_save_pasted_image(app, core, args).await,
        "attachment_image_data" => screens::__ipc_attachment_image_data(app, core, args).await,
        "response_image_data" => media::__ipc_response_image_data(app, core, args).await,
        "open_response_link" => media::__ipc_open_response_link(app, core, args).await,
        "list_screenshots" => screens::__ipc_list_screenshots(app, core, args).await,
        "run_project_check" => projecttools::__ipc_run_project_check(app, core, args).await,
        "open_source_location" => projecttools::__ipc_open_source_location(app, core, args).await,
        "save_project_context" => __ipc_save_project_context(app, core, args).await,
        "get_settings" => __ipc_get_settings(app, core, args).await,
        "save_settings" => __ipc_save_settings(app, core, args).await,
        "reset_settings" => __ipc_reset_settings(app, core, args).await,
        "ollama_status" => __ipc_ollama_status(app, core, args).await,
        "orchestra_answer" => orchestra::__ipc_orchestra_answer(app, core, args).await,
        "orchestra_approve" => orchestra::__ipc_orchestra_approve(app, core, args).await,
        "orchestra_cancel" => orchestra::__ipc_orchestra_cancel(app, core, args).await,
        "orchestra_lanes" => orchestra::__ipc_orchestra_lanes(app, core, args).await,
        "orchestra_raise" => orchestra::__ipc_orchestra_raise(app, core, args).await,
        "orchestra_resume" => orchestra::__ipc_orchestra_resume(app, core, args).await,
        "orchestra_init_git" => orchestra::__ipc_orchestra_init_git(app, core, args).await,
        "provider_health" => __ipc_provider_health(app, core, args).await,
        "mcp_health" => __ipc_mcp_health(app, core, args).await,
        "mcp_toggle" => __ipc_mcp_toggle(app, core, args).await,
        "mcp_add" => __ipc_mcp_add(app, core, args).await,
        "mcp_remove" => __ipc_mcp_remove(app, core, args).await,
        "list_models" => __ipc_list_models(app, core, args).await,
        "rename_account" => __ipc_rename_account(app, core, args).await,
        "list_workspaces" => __ipc_list_workspaces(app, core, args).await,
        "add_workspace" => __ipc_add_workspace(app, core, args).await,
        "rename_workspace" => __ipc_rename_workspace(app, core, args).await,
        "delete_workspace" => __ipc_delete_workspace(app, core, args).await,
        "set_session_workspace" => __ipc_set_session_workspace(app, core, args).await,
        "task_worktree_status" => task_worktree::__ipc_task_worktree_status(app, core, args).await,
        "publish_task" => task_worktree::__ipc_publish_task(app, core, args).await,
        "apply_task" => task_worktree::__ipc_apply_task(app, core, args).await,
        "discard_task" => task_worktree::__ipc_discard_task(app, core, args).await,
        "stop_watching_task" => task_worktree::__ipc_stop_watching_task(app, core, args).await,
        "get_usage" => __ipc_get_usage(app, core, args).await,
        "refresh_usage" => __ipc_refresh_usage(app, core, args).await,
        "integration_status" => __ipc_integration_status(app, core, args).await,
        "save_recipe" => __ipc_save_recipe(app, core, args).await,
        "remember_global" => __ipc_remember_global(app, core, args).await,
        "remember_project" => __ipc_remember_project(app, core, args).await,
        "audit_log" => __ipc_audit_log(app, core, args).await,
        "install_skill" => __ipc_install_skill(app, core, args).await,
        "plugins_list" => __ipc_plugins_list(app, core, args).await,
        "plugin_inspect" => __ipc_plugin_inspect(app, core, args).await,
        "plugin_install" => __ipc_plugin_install(app, core, args).await,
        "plugin_set_enabled" => __ipc_plugin_set_enabled(app, core, args).await,
        "plugin_remove" => __ipc_plugin_remove(app, core, args).await,
        "plugin_reload" => __ipc_plugin_reload(app, core, args).await,
        "plugin_contributions" => __ipc_plugin_contributions(app, core, args).await,
        "plugin_panel" => __ipc_plugin_panel(app, core, args).await,
        "plugin_panel_fetch" => __ipc_plugin_panel_fetch(app, core, args).await,
        "plugin_set_secret" => __ipc_plugin_set_secret(app, core, args).await,
        "save_mcp" => __ipc_save_mcp(app, core, args).await,
        _ => Err("Unknown core command".into()),
    }
}

/// Opens the existing data directory exactly once, in the service process.
pub fn open(root: PathBuf, resources: PathBuf, app: host::EventHub) -> Result<Arc<Core>, String> {
    #[cfg(unix)]
    fix_path();
    std::fs::create_dir_all(root.join("accounts")).map_err(|e| e.to_string())?;
    let db = store::Store::open(&root.join("lumen.db"))?;
    let prefs = settings::AppSettings::load(&root);
    let journal = lumen_gatekeeper::Gatekeeper::open(
        root.join("gatekeeper.sqlite"),
        std::time::Duration::from_secs(300),
    )
    .ok();
    let core = Arc::new(Core::new(db, root, prefs, journal));
    app.attach(core.store.clone());
    host::resources(resources.clone());
    browser::resources(resources);
    host::autostart(
        &core.root,
        core.settings
            .lock()
            .map_err(|_| "Settings lock failed")?
            .open_at_login,
    )?;
    if let Some(keeper) = core.approvals.clone() {
        relay_approvals(app.clone(), keeper);
    }
    orchestra::recover(&core);
    routines::start(app.clone(), &core);
    pr_watch::start(app.clone(), &core);
    quota_resume::restore(app, &core);
    Ok(core)
}
pub fn begin_shutdown(core: &Core) {
    core.stopping
        .store(true, std::sync::atomic::Ordering::SeqCst);
    core.cancel_all();
}
pub async fn shutdown(core: &Core) -> Result<(), String> {
    core.stopping
        .store(true, std::sync::atomic::Ordering::SeqCst);
    core.cancel_all();
    for _ in 0..300 {
        if core.jobs_idle() {
            browser::shutdown();
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    core.stopping
        .store(false, std::sync::atomic::Ordering::SeqCst);
    Err("Some work has not stopped yet".into())
}
pub fn idle(core: &Core) -> bool {
    core.jobs_idle()
}
