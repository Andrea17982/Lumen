//! Signed updates from the project's GitHub releases. Lumen checks at startup and every few
//! hours, says when a new version exists, and downloads only when the user asks. The package's
//! minisign signature is checked against the public key built into Lumen before anything is
//! installed. Before installing, the service (which would hold its own files open) is stopped and
//! the database is backed up, so "Go back" can reinstall the previous signed version.
use crate::bridge::Bridge;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

const RELEASES: &str = "https://github.com/Andrea17982/Lumen/releases";
const FIRST_CHECK: Duration = Duration::from_secs(20);
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// What the last update recorded, in the data directory: the version to go back to.
const RECORD: &str = "update.json";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Available {
    pub version: String,
    pub notes: Option<String>,
    /// This copy can update itself (the installed app). A portable folder or a Linux package
    /// shows the release page instead.
    pub installable: bool,
    pub page: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    previous: String,
    backup: Option<String>,
    schema: Option<u32>,
    /// A Go back whose installer has started: finished by `settle` at the next start.
    #[serde(default)]
    pending: Option<Pending>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pending {
    version: String,
    /// The earlier version cannot open the current schema: put the backup back.
    restore: bool,
}

#[derive(Default)]
pub struct Updates {
    available: Mutex<Option<Available>>,
    busy: tokio::sync::Mutex<()>,
}

/// Installed with the NSIS setup on Windows, or a macOS app bundle: the forms the updater can
/// replace. The portable ZIP and the Debian package update by hand.
pub fn installable() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    if cfg!(windows) {
        exe.parent()
            .is_some_and(|dir| dir.join("uninstall.exe").is_file())
    } else if cfg!(target_os = "macos") {
        exe.ancestors()
            .any(|p| p.extension().is_some_and(|e| e == "app"))
    } else {
        std::env::var_os("APPIMAGE").is_some()
    }
}

fn page(version: &str) -> String {
    format!("{RELEASES}/tag/v{version}")
}

/// Runs for the life of the app: a quiet check after startup, then every six hours. A found
/// version is told to the page; nothing is downloaded.
pub async fn watch(app: AppHandle) {
    tokio::time::sleep(FIRST_CHECK).await;
    loop {
        if let Ok(Some(found)) = check(&app).await {
            let _ = app.emit(
                crate::bridge::CHANNEL,
                json!({"event": "lumen:update", "payload": found}),
            );
        }
        tokio::time::sleep(EVERY).await;
    }
}

pub async fn check(app: &AppHandle) -> Result<Option<Available>, String> {
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    let found = update.map(|u| Available {
        page: page(&u.version),
        version: u.version,
        notes: u.body,
        installable: installable(),
    });
    *app.state::<Updates>()
        .available
        .lock()
        .map_err(|_| "Update state lock failed")? = found.clone();
    Ok(found)
}

pub fn known(app: &AppHandle) -> Option<Available> {
    app.state::<Updates>().available.lock().ok()?.clone()
}

/// The version the last update replaced, when Lumen can go back to it.
pub fn previous(bridge: &Bridge) -> Option<String> {
    read(bridge.root())
        .filter(|r| r.pending.is_none())
        .map(|r| r.previous)
        .filter(|v| older(v, env!("CARGO_PKG_VERSION")))
}

fn read(root: &Path) -> Option<Record> {
    serde_json::from_slice(&std::fs::read(root.join(RECORD)).ok()?).ok()
}

fn write(root: &Path, record: &Record) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(record).map_err(|e| e.to_string())?;
    std::fs::write(root.join(RECORD), bytes).map_err(|e| e.to_string())
}

fn semver(v: &str) -> Option<(u64, u64, u64)> {
    let mut parts = v.split('.').map(|p| p.parse::<u64>().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

/// `a` is a plain release version older than `b`.
fn older(a: &str, b: &str) -> bool {
    matches!((semver(a), semver(b)), (Some(a), Some(b)) if a < b)
}

/// Downloads and installs the newest release. On Windows the installer takes over and Lumen
/// exits; elsewhere Lumen restarts into the new version.
pub async fn install(app: &AppHandle, bridge: &Arc<Bridge>) -> Result<(), String> {
    let updates = app.state::<Updates>();
    let _busy = updates
        .busy
        .try_lock()
        .map_err(|_| "An update is already being installed")?;
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Lumen is up to date")?;
    run(app, bridge, update, None).await
}

/// Reinstalls the version the last update replaced, from its own signed release. The database
/// is not touched now: if that version cannot open the current schema, the backup taken before
/// the update is put back when it first starts (see `settle`).
pub async fn go_back(app: &AppHandle, bridge: &Arc<Bridge>) -> Result<(), String> {
    let updates = app.state::<Updates>();
    let _busy = updates
        .busy
        .try_lock()
        .map_err(|_| "An update is already being installed")?;
    let record = read(bridge.root())
        .filter(|r| r.pending.is_none())
        .ok_or("There is no earlier version to go back to")?;
    if !older(&record.previous, env!("CARGO_PKG_VERSION")) {
        return Err("There is no earlier version to go back to".into());
    }
    let target = record.previous.clone();
    let manifest = tauri::Url::parse(&format!("{RELEASES}/download/v{target}/latest.json"))
        .map_err(|e| e.to_string())?;
    let wanted = target.clone();
    let update = app
        .updater_builder()
        .endpoints(vec![manifest])
        .map_err(|e| e.to_string())?
        .version_comparator(move |_, remote| remote.version.to_string() == wanted)
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("The earlier version is not available for reinstalling")?;
    run(app, bridge, update, Some(record)).await
}

async fn idle(bridge: &Bridge) -> Result<(), String> {
    let status = bridge.call("__status", json!({})).await?;
    if status["idle"] != Value::Bool(true) {
        return Err("Wait for running work to finish, then update.".into());
    }
    Ok(())
}

async fn run(
    app: &AppHandle,
    bridge: &Arc<Bridge>,
    update: tauri_plugin_updater::Update,
    back: Option<Record>,
) -> Result<(), String> {
    if !installable() {
        return Err(
            "This copy of Lumen cannot update itself. Download the new version from the release page."
                .into(),
        );
    }
    idle(bridge).await?;
    // Verified against the built-in public key before it is returned.
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|e| format!("The update could not be downloaded or verified: {e}"))?;
    // The download can take minutes: work started meanwhile is not stopped.
    idle(bridge).await?;
    let root = bridge.root();
    let saved = bridge
        .call("backup_database", json!({"beforeUpdate": back.is_none()}))
        .await?;
    let schema = saved["schema"].as_u64().map(|s| s as u32);
    let record = match &back {
        None => Record {
            previous: env!("CARGO_PKG_VERSION").into(),
            backup: saved["path"].as_str().map(str::to_string),
            schema,
            pending: None,
        },
        Some(record) => Record {
            pending: Some(Pending {
                version: record.previous.clone(),
                restore: record.schema.is_some() && record.schema != schema,
            }),
            ..record.clone()
        },
    };
    write(root, &record)?;
    bridge.pause(true);
    let result = async {
        bridge.stop_quietly().await?;
        update.install(bytes).map_err(|e| e.to_string())
    }
    .await;
    match result {
        Ok(()) => app.restart(),
        Err(error) => {
            // Nothing was replaced: the supervisor starts the service again.
            if let Some(record) = back {
                let _ = write(root, &record);
            }
            bridge.pause(false);
            Err(error)
        }
    }
}

/// At startup, before the service: finishes a Go back. In the version it went back to, the
/// pre-update backup replaces a database that version cannot open; in any other version (the
/// installer did not run), the pending step is dropped and nothing changes.
pub fn settle(root: &Path) -> Result<(), String> {
    let Some(record) = read(root) else {
        return Ok(());
    };
    let Some(pending) = &record.pending else {
        return Ok(());
    };
    if pending.version != env!("CARGO_PKG_VERSION") {
        return write(
            root,
            &Record {
                pending: None,
                ..record.clone()
            },
        );
    }
    if pending.restore {
        restore(root, &record)?;
    }
    // Back on the earlier version: there is nothing further to go back to.
    std::fs::remove_file(root.join(RECORD)).map_err(|e| e.to_string())
}

/// Puts the pre-update backup in place of the database, keeping the newer one (with its
/// write-ahead log) in `backups/kept`. Any failure leaves the current database in place.
fn restore(root: &Path, record: &Record) -> Result<(), String> {
    let backups = root.join("backups");
    let backup = PathBuf::from(
        record
            .backup
            .as_deref()
            .ok_or("The update left no backup")?,
    );
    let backup = canonical(&backup)?;
    if !backup.starts_with(canonical(&backups)?) || !backup.is_file() {
        return Err(format!("The backup {} is not usable", backup.display()));
    }
    let db = root.join("lumen.db");
    let staged = root.join("lumen.db.restoring");
    std::fs::copy(&backup, &staged).map_err(|e| e.to_string())?;
    let kept = backups.join("kept");
    std::fs::create_dir_all(&kept).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let moved: Vec<(PathBuf, PathBuf)> = ["", "-wal", "-shm"]
        .iter()
        .map(|side| {
            (
                root.join(format!("lumen.db{side}")),
                kept.join(format!("lumen-newer-{stamp}.db{side}")),
            )
        })
        .filter(|(from, _)| from.exists())
        .collect();
    let mut done = vec![];
    for (from, to) in &moved {
        if let Err(e) = std::fs::rename(from, to) {
            for (from, to) in done.iter().rev() {
                let _ = std::fs::rename(to, from);
            }
            let _ = std::fs::remove_file(&staged);
            return Err(e.to_string());
        }
        done.push((from.clone(), to.clone()));
    }
    if let Err(e) = std::fs::rename(&staged, &db) {
        for (from, to) in done.iter().rev() {
            let _ = std::fs::rename(to, from);
        }
        return Err(e.to_string());
    }
    Ok(())
}

/// Both sides of a containment check are canonicalised the same way (Windows adds `\\?\`).
fn canonical(path: &Path) -> Result<PathBuf, String> {
    std::fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_release_versions() {
        assert!(older("0.15.0", "0.15.1"));
        assert!(older("0.9.9", "0.15.0"));
        assert!(!older("0.15.1", "0.15.0"));
        assert!(!older("0.15.0", "0.15.0"));
        assert!(!older("0.15.0/../x", "0.16.0"));
        assert!(!older("0.15", "0.16.0"));
    }

    fn root() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lumen-settle-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("backups/update")).unwrap();
        dir
    }

    #[test]
    fn going_back_restores_the_backup_only_in_the_target_version() {
        let dir = root();
        let backup = dir.join("backups/update/lumen-1.db");
        std::fs::write(&backup, "old").unwrap();
        std::fs::write(dir.join("lumen.db"), "new").unwrap();
        std::fs::write(dir.join("lumen.db-wal"), "log").unwrap();
        let record = |version: &str| Record {
            previous: version.into(),
            backup: Some(backup.to_string_lossy().into()),
            schema: Some(8),
            pending: Some(Pending {
                version: version.into(),
                restore: true,
            }),
        };
        // The installer did not run: this version keeps its data and forgets the step.
        write(&dir, &record("0.0.1")).unwrap();
        settle(&dir).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("lumen.db")).unwrap(),
            "new"
        );
        assert!(read(&dir).unwrap().pending.is_none());
        // In the version it went back to: the backup is in place, the newer data kept.
        write(&dir, &record(env!("CARGO_PKG_VERSION"))).unwrap();
        settle(&dir).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("lumen.db")).unwrap(),
            "old"
        );
        assert!(!dir.join("lumen.db-wal").exists());
        let kept: Vec<_> = std::fs::read_dir(dir.join("backups/kept"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(kept.len(), 2, "{kept:?}");
        assert!(read(&dir).is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_backup_outside_the_backups_folder_is_refused() {
        let dir = root();
        std::fs::write(dir.join("elsewhere.db"), "x").unwrap();
        std::fs::write(dir.join("lumen.db"), "new").unwrap();
        let record = Record {
            previous: "0.0.1".into(),
            backup: Some(dir.join("elsewhere.db").to_string_lossy().into()),
            schema: Some(1),
            pending: None,
        };
        assert!(restore(&dir, &record).is_err());
        assert_eq!(
            std::fs::read_to_string(dir.join("lumen.db")).unwrap(),
            "new"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
