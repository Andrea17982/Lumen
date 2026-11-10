//! Screenshots for prompts and visual review. Captures are taken only when the user asks and are
//! saved inside the project, in `.lumen/screenshots/`, with a sidecar describing label,
//! resolution and source. That folder ignores itself in git, so screen contents are never
//! committed by accident.
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::Manager;

pub const DIR: &str = ".lumen/screenshots";
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif"];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    /// Image path relative to the project, usable as an attachment.
    pub path: String,
    pub label: String,
    pub width: u32,
    pub height: u32,
    /// `Screen`, a window title, or `Imported from <file name>`.
    pub source: String,
    /// Milliseconds since the epoch.
    pub captured: i64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CaptureTarget {
    pub id: u32,
    pub pid: u32,
    pub title: String,
    pub app: String,
}

pub fn is_image(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.to_lowercase().as_str()))
}

fn root_of(folder: &str) -> Result<PathBuf, String> {
    let root = dunce::canonicalize(folder).map_err(|_| "Choose an existing folder")?;
    if root.is_dir() {
        Ok(root)
    } else {
        Err("Choose an existing folder".into())
    }
}

fn store(root: &Path) -> Result<PathBuf, String> {
    let dir = root.join(DIR);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ignore = dir.join(".gitignore");
    if !ignore.exists() {
        std::fs::write(&ignore, "# Lumen screenshots stay local\n*\n")
            .map_err(|e| e.to_string())?;
    }
    Ok(dir)
}

fn sidecar(image: &Path) -> PathBuf {
    image.with_extension("json")
}

/// An image inside the screenshot folder, from a project-relative path.
fn resolve(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let path = dunce::canonicalize(root.join(rel)).map_err(|_| "Screenshot not found")?;
    if !path.starts_with(root.join(DIR)) || !is_image(rel) {
        return Err("Choose a screenshot saved by Lumen".into());
    }
    Ok(path)
}

fn save(
    root: &Path,
    image: &xcap::image::RgbaImage,
    label: &str,
    source: String,
) -> Result<Screenshot, String> {
    let dir = store(root)?;
    let captured = chrono_now();
    let name = format!("shot-{captured}-{}.png", uuid::Uuid::new_v4().simple());
    let file = dir.join(&name);
    image.save(&file).map_err(|e| e.to_string())?;
    let shot = Screenshot {
        path: format!("{DIR}/{name}"),
        label: if label.trim().is_empty() {
            source.clone()
        } else {
            label.trim().to_string()
        },
        width: image.width(),
        height: image.height(),
        source,
        captured,
    };
    write_sidecar(&file, &shot)?;
    Ok(shot)
}

fn write_sidecar(image: &Path, shot: &Screenshot) -> Result<(), String> {
    let text = serde_json::to_string_pretty(shot).map_err(|e| e.to_string())?;
    std::fs::write(sidecar(image), text).map_err(|e| e.to_string())
}

pub(crate) fn save_mcp_image(root: &Path, data: &str, label: &str) -> Result<Screenshot, String> {
    if data.len() > 20 * 1024 * 1024 {
        return Err("The viewport image exceeds 20 MB".into());
    }
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data)
        .map_err(|_| "Invalid viewport image")?;
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().map_err(|e| e.to_string())?;
    save(root, &image.to_rgba8(), label, label.into())
}

fn chrono_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

pub fn list(root: &Path) -> Vec<Screenshot> {
    let Ok(entries) = std::fs::read_dir(root.join(DIR)) else {
        return vec![];
    };
    let mut out: Vec<Screenshot> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| {
            serde_json::from_str::<Screenshot>(&std::fs::read_to_string(e.path()).ok()?).ok()
        })
        .filter(|s| root.join(&s.path).is_file())
        .collect();
    out.sort_by_key(|s| std::cmp::Reverse(s.captured));
    out
}

/// Windows that can be captured, such as a running game. Lumen's own window is left out.
#[tauri::command]
pub async fn capture_targets() -> Result<Vec<CaptureTarget>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let own = std::process::id();
        let mut out: Vec<CaptureTarget> = xcap::Window::all()
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|w| w.pid().is_ok_and(|p| p != own))
            .filter(|w| !w.is_minimized().unwrap_or(true))
            .filter(|w| w.width().unwrap_or(0) > 80 && w.height().unwrap_or(0) > 80)
            .filter_map(|w| {
                let title = w.title().ok()?;
                (!title.trim().is_empty()).then_some(())?;
                Some(CaptureTarget {
                    id: w.id().ok()?,
                    pid: w.pid().ok()?,
                    title,
                    app: w.app_name().unwrap_or_default(),
                })
            })
            .collect();
        out.sort_by_key(|t| t.title.to_lowercase());
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Captures the screen Lumen is on (Lumen hides itself for the moment of the capture) or one
/// window by id, and saves it in the project.
#[tauri::command]
pub async fn capture_screenshot(
    app: tauri::AppHandle,
    folder: String,
    window: Option<u32>,
    label: String,
) -> Result<Screenshot, String> {
    capture_screenshot_inner(app, folder, window, label, true).await
}

async fn capture_screenshot_inner(
    app: tauri::AppHandle,
    folder: String,
    window: Option<u32>,
    label: String,
    restore_focus: bool,
) -> Result<Screenshot, String> {
    let root = root_of(&folder)?;
    let main = app.get_webview_window("main");
    if let Some(id) = window {
        let (image, title) = tauri::async_runtime::spawn_blocking(move || {
            let target = xcap::Window::all()
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|w| w.id().is_ok_and(|i| i == id))
                .ok_or("That window is no longer open")?;
            let title = target.title().unwrap_or_default();
            target
                .capture_image()
                .map(|i| (i, title))
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())??;
        return save(&root, &image, &label, title);
    }
    let was_visible = main
        .as_ref()
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    let point = main
        .as_ref()
        .and_then(|w| w.outer_position().ok())
        .map(|p| (p.x + 40, p.y + 40));
    if was_visible {
        if let Some(w) = &main {
            let _ = w.hide();
        }
    }
    // Give the compositor a moment to remove Lumen from the screen.
    tokio::time::sleep(std::time::Duration::from_millis(350)).await;
    let captured = tauri::async_runtime::spawn_blocking(move || {
        let monitor = match point {
            Some((x, y)) => xcap::Monitor::from_point(x, y).ok(),
            None => None,
        }
        .or_else(|| {
            xcap::Monitor::all()
                .ok()?
                .into_iter()
                .find(|m| m.is_primary().unwrap_or(false))
        })
        .ok_or("No screen to capture")?;
        monitor.capture_image().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string());
    if was_visible {
        if let Some(w) = &main {
            let _ = w.show();
            if restore_focus {
                let _ = w.set_focus();
            }
        }
    }
    save(&root, &captured??, &label, "Screen".into())
}

/// Stores an image pasted from the system clipboard in Lumen's private app-data directory.
#[tauri::command]
pub fn save_pasted_image(
    core: tauri::State<'_, std::sync::Arc<crate::Core>>,
    data: String,
    mime: String,
) -> Result<String, String> {
    if data.len() > 32 * 1024 * 1024 {
        return Err("Clipboard image exceeds the 24 MB limit".into());
    }
    let format = match mime.as_str() {
        "image/png" => ("png", image::ImageFormat::Png),
        "image/jpeg" => ("jpg", image::ImageFormat::Jpeg),
        "image/webp" => ("webp", image::ImageFormat::WebP),
        "image/gif" => ("gif", image::ImageFormat::Gif),
        _ => return Err("Clipboard content is not a supported image".into()),
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|_| "Clipboard image could not be decoded")?;
    if bytes.len() > 24 * 1024 * 1024 || image::guess_format(&bytes).ok() != Some(format.1) {
        return Err("Clipboard image is invalid or exceeds the 24 MB limit".into());
    }
    let dir = core.root.join("attachments");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("clipboard-{}.{}", uuid::Uuid::new_v4(), format.0));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

/// The image as a `data:` URL; the webview CSP allows `data:` images only.
fn data_url(file: &Path, bytes: &[u8]) -> String {
    let mime = match file.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "image/png",
    };
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

#[tauri::command]
pub fn attachment_image_data(folder: String, path: String) -> Result<String, String> {
    let root = root_of(&folder)?;
    let file = crate::context::attachment_path(&root, &path)?;
    if !is_image(&path) {
        return Err("This attachment is not an image".into());
    }
    let bytes = std::fs::read(&file).map_err(|e| e.to_string())?;
    if bytes.len() > 12 * 1024 * 1024 {
        return Err("Image preview is limited to 12 MB".into());
    }
    Ok(data_url(&file, &bytes))
}

/// Copies an existing image (for example a device capture) into the project's screenshots.
#[tauri::command]
pub async fn import_screenshot(
    folder: String,
    file: String,
    label: String,
) -> Result<Screenshot, String> {
    let root = root_of(&folder)?;
    if !is_image(&file) {
        return Err("Choose a PNG, JPEG, WebP or GIF image".into());
    }
    let image = xcap::image::open(&file).map_err(|e| format!("Could not read the image: {e}"))?;
    let name = Path::new(&file)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    save(
        &root,
        &image.to_rgba8(),
        &label,
        format!("Imported from {name}"),
    )
}

#[tauri::command]
pub async fn list_screenshots(folder: String) -> Result<Vec<Screenshot>, String> {
    Ok(list(&root_of(&folder)?))
}

#[tauri::command]
pub async fn rename_screenshot(
    folder: String,
    path: String,
    label: String,
) -> Result<Screenshot, String> {
    let root = root_of(&folder)?;
    let file = resolve(&root, &path)?;
    let mut shot: Screenshot = serde_json::from_str(
        &std::fs::read_to_string(sidecar(&file)).map_err(|_| "Screenshot details not found")?,
    )
    .map_err(|e| e.to_string())?;
    shot.label = label.trim().to_string();
    write_sidecar(&file, &shot)?;
    Ok(shot)
}

#[tauri::command]
pub async fn delete_screenshot(folder: String, path: String) -> Result<(), String> {
    let root = root_of(&folder)?;
    let file = resolve(&root, &path)?;
    let _ = std::fs::remove_file(sidecar(&file));
    std::fs::remove_file(file).map_err(|e| e.to_string())
}

/// The image as a data URL for display; the webview CSP allows `data:` images only.
#[tauri::command]
pub async fn screenshot_data(folder: String, path: String) -> Result<String, String> {
    let root = root_of(&folder)?;
    let file = resolve(&root, &path)?;
    let bytes = std::fs::read(&file).map_err(|e| e.to_string())?;
    Ok(data_url(&file, &bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parallel_browser_captures_never_overwrite_each_other() {
        let f = crate::test_support::Fixture::new();
        let root = f.directory("captures");
        let files = std::thread::scope(|threads| {
            let jobs = (0..16)
                .map(|_| {
                    let root = root.clone();
                    threads.spawn(move || {
                        save(
                            &root,
                            &xcap::image::RgbaImage::new(2, 2),
                            "Moli",
                            "Moli".into(),
                        )
                        .unwrap()
                        .path
                    })
                })
                .collect::<Vec<_>>();
            jobs.into_iter()
                .map(|job| job.join().unwrap())
                .collect::<std::collections::HashSet<_>>()
        });
        assert_eq!(files.len(), 16);
        assert_eq!(list(&root).len(), 16);
        assert!(files.iter().all(|file| root.join(file).is_file()));
    }

    #[test]
    fn saves_lists_and_confines_screenshots_to_their_folder() {
        let root = std::env::temp_dir().join(format!("lumen-shots-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let root = dunce::canonicalize(&root).unwrap();
        let image = xcap::image::RgbaImage::new(360, 640);
        let shot = save(&root, &image, " Portrait ", "Screen".into()).unwrap();
        assert_eq!(
            (shot.width, shot.height, shot.label.as_str()),
            (360, 640, "Portrait")
        );
        assert!(root.join(DIR).join(".gitignore").is_file());
        assert_eq!(list(&root), vec![shot.clone()]);
        std::fs::write(root.join("notes.png"), b"x").unwrap();
        assert!(
            resolve(&root, "notes.png").is_err(),
            "only Lumen's own folder"
        );
        assert!(resolve(&root, &shot.path).is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Real capture of the primary screen and of the first listed window on this machine.
    #[tokio::test]
    #[ignore]
    async fn live_capture_screen_and_window() {
        let root = std::env::temp_dir().join(format!("lumen-live-shots-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let root = dunce::canonicalize(&root).unwrap();
        let monitor = xcap::Monitor::all()
            .unwrap()
            .into_iter()
            .find(|m| m.is_primary().unwrap_or(false))
            .unwrap();
        let shot = save(
            &root,
            &monitor.capture_image().unwrap(),
            "",
            "Screen".into(),
        )
        .unwrap();
        println!("screen {}x{} {}", shot.width, shot.height, shot.path);
        let targets = capture_targets().await.unwrap();
        println!(
            "{} windows, first: {:?}",
            targets.len(),
            targets.first().map(|t| &t.title)
        );
        if let Some(t) = targets.first() {
            let w = xcap::Window::all()
                .unwrap()
                .into_iter()
                .find(|w| w.id().ok() == Some(t.id))
                .unwrap();
            let shot = save(&root, &w.capture_image().unwrap(), "", t.title.clone()).unwrap();
            println!("window {}x{} {}", shot.width, shot.height, shot.label);
        }
        assert_eq!(list(&root).len(), 1 + targets.len().min(1));
        let _ = std::fs::remove_dir_all(&root);
    }
}
