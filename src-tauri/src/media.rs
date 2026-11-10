//! User-activated response links and project-scoped image previews.
use std::path::{Path, PathBuf};
use tauri_plugin_opener::OpenerExt;

fn decode(text: &str) -> Result<String, String> {
    let mut bytes = vec![];
    let mut input = text.as_bytes().iter().copied();
    while let Some(byte) = input.next() {
        if byte == b'%' {
            let a = input.next().ok_or("Invalid encoded path")? as char;
            let b = input.next().ok_or("Invalid encoded path")? as char;
            bytes.push(
                (a.to_digit(16).ok_or("Invalid encoded path")? * 16
                    + b.to_digit(16).ok_or("Invalid encoded path")?) as u8,
            );
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).map_err(|_| "The path is not valid UTF-8".into())
}

pub(crate) fn resolve(root: &Path, href: &str) -> Result<(PathBuf, u32), String> {
    let root = dunce::canonicalize(root).map_err(|_| "The conversation folder is unavailable")?;
    let pattern = regex::Regex::new(r"^(.*?)(?::([0-9]+)(?::[0-9]+)?|#L([0-9]+))$").unwrap();
    let captures = pattern.captures(href);
    let (raw, line) = captures
        .as_ref()
        .map(|c| {
            (
                c.get(1).unwrap().as_str(),
                c.get(2)
                    .or_else(|| c.get(3))
                    .and_then(|v| v.as_str().parse().ok())
                    .unwrap_or(1),
            )
        })
        .unwrap_or((href, 1));
    let path = if raw.starts_with("file://") {
        tauri::Url::parse(raw)
            .map_err(|_| "Invalid file URL")?
            .to_file_path()
            .map_err(|_| "Invalid file URL")?
    } else {
        let path = decode(raw)?;
        #[cfg(windows)]
        let path =
            if path.as_bytes().first() == Some(&b'/') && path.as_bytes().get(2) == Some(&b':') {
                path[1..].into()
            } else {
                path
            };
        PathBuf::from(path)
    };
    let candidate = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    // Reject foreign drives/UNC paths before touching their filesystem. Canonicalization
    // below still checks symlinks, but must not initiate network access for a response link.
    let mut normalized = PathBuf::new();
    for part in candidate.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    #[cfg(windows)]
    let inside = normalized.to_string_lossy().to_lowercase()
        == root.to_string_lossy().to_lowercase()
        || normalized
            .to_string_lossy()
            .to_lowercase()
            .starts_with(&format!(
                "{}\\",
                root.to_string_lossy().to_lowercase().trim_end_matches('\\')
            ));
    #[cfg(not(windows))]
    let inside = normalized.starts_with(&root);
    if !inside {
        return Err("That file is outside the conversation folder".into());
    }
    let path = dunce::canonicalize(normalized).map_err(|_| "That file is not available")?;
    if !path.starts_with(&root) || !path.is_file() {
        return Err("That file is outside the conversation folder".into());
    }
    Ok((path, line.max(1)))
}

#[tauri::command]
pub fn response_image_data(folder: String, href: String) -> Result<String, String> {
    let (path, _) = resolve(Path::new(&folder), &href)?;
    crate::screens::attachment_image_data(folder, path.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn open_response_link(
    app: tauri::AppHandle,
    folder: Option<String>,
    href: String,
) -> Result<(), String> {
    if href.starts_with("https://") || href.starts_with("http://") {
        let url = tauri::Url::parse(&href).map_err(|_| "Invalid link")?;
        if url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() {
            return Err("Invalid web link".into());
        }
        return app
            .opener()
            .open_url(url.as_str(), None::<&str>)
            .map_err(|e| e.to_string());
    }
    let folder = folder.ok_or("Open a conversation before opening project files")?;
    let (path, line) = resolve(Path::new(&folder), &href)?;
    let relative = path
        .strip_prefix(dunce::canonicalize(&folder).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .into_owned();
    crate::projecttools::open_source_location(app, folder, relative, line)
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn links_preserve_spaces_and_lines_and_cannot_escape_the_project() {
        let fixture = crate::test_support::Fixture::new();
        std::fs::write(fixture.root.join("file name.ts"), "export const value=1;\n").unwrap();
        let (path, line) = resolve(&fixture.root, "file%20name.ts:12:3").unwrap();
        assert_eq!(line, 12);
        assert!(path.ends_with("file name.ts"));
        assert_eq!(resolve(&fixture.root, "file%20name.ts#L9").unwrap().1, 9);
        assert!(resolve(&fixture.root, "../outside.png").is_err());
        assert!(resolve(&fixture.root, "javascript:alert(1)").is_err());
        assert!(resolve(&fixture.root, "file%XX.png").is_err());
    }
}
