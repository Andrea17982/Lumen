//! Explicit static previews of retained Both/Orchestra worktrees. Never runs a build or shell.
use crate::{turns, Core};
use serde::Serialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

#[derive(Serialize)]
pub struct Variant {
    id: String,
    label: String,
    path: String,
}
fn variants(core: &Core, session: &str) -> Result<Vec<Variant>, String> {
    let s = core.store.session(session)?;
    let mut result = vec![Variant {
        id: "project".into(),
        label: "Current project".into(),
        path: s.workspace.clone(),
    }];
    for m in core.store.messages(session)? {
        if let Some(lane) = turns::lane_of(&m) {
            if lane.top == s.workspace && Path::new(&lane.path).is_dir() {
                result.push(Variant {
                    id: m.id.clone(),
                    label: lane.branch,
                    path: lane.path,
                });
            }
        }
        if let Some(state) = crate::orchestra::state_of(&m) {
            if state.workspace == s.workspace {
                for (_, path, branch) in state.lanes {
                    if Path::new(&path).is_dir() {
                        result.push(Variant {
                            id: format!("{}:{branch}", m.id),
                            label: branch,
                            path,
                        });
                    }
                }
            }
        }
    }
    result.sort_by(|a, b| a.id.cmp(&b.id));
    result.dedup_by(|a, b| a.id == b.id);
    Ok(result)
}
#[tauri::command]
pub fn visual_variants(
    core: tauri::State<'_, Arc<Core>>,
    session: String,
) -> Result<Vec<Variant>, String> {
    variants(&core, &session)
}
fn servers() -> &'static Mutex<HashMap<String, tokio::task::JoinHandle<()>>> {
    static SERVERS: OnceLock<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>> = OnceLock::new();
    SERVERS.get_or_init(Default::default)
}
fn asset(root: &Path, target: &str) -> Result<(PathBuf, &'static str), String> {
    let canonical = dunce::canonicalize(root).map_err(|_| "Preview folder is unavailable")?;
    let root = canonical.as_path();
    let url = tauri::Url::parse(&format!("http://localhost{target}"))
        .map_err(|_| "Invalid preview path")?;
    let relative = url.path().trim_start_matches('/');
    let relative = if relative.is_empty() {
        "index.html"
    } else {
        relative
    };
    let (path, _) = crate::media::resolve(root, relative)?;
    if path
        .strip_prefix(root)
        .map_err(|_| "Invalid path")?
        .components()
        .any(|p| p.as_os_str().to_string_lossy().starts_with('.'))
    {
        return Err("Hidden files are not served".into());
    }
    let mime = match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "html" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "gif" => "image/gif",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "ico" => "image/x-icon",
        _ => return Err("Only web preview assets are served".into()),
    };
    Ok((path, mime))
}
#[derive(Serialize)]
pub struct Started {
    id: String,
    url: String,
}
async fn read_headers(input: &mut (impl tokio::io::AsyncRead + Unpin)) -> std::io::Result<String> {
    let mut request = Vec::with_capacity(1024);
    loop {
        let mut chunk = [0; 1024];
        let n = input.read(&mut chunk).await?;
        if n == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        request.extend_from_slice(&chunk[..n]);
        if request.len() > 8192 {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        if request.windows(4).any(|w| w == b"\r\n\r\n") {
            return Ok(String::from_utf8_lossy(&request).into_owned());
        }
    }
}
#[tauri::command]
pub async fn start_variant_preview(
    core: tauri::State<'_, Arc<Core>>,
    session: String,
    variant: String,
    directory: String,
) -> Result<Started, String> {
    let selected = variants(&core, &session)?
        .into_iter()
        .find(|v| v.id == variant)
        .ok_or("This worktree is no longer available")?;
    let base = dunce::canonicalize(selected.path).map_err(|e| e.to_string())?;
    if directory.contains('\\')
        || directory.split('/').any(|p| p.starts_with('.') && p != ".")
        || Path::new(&directory).is_absolute()
    {
        return Err("Choose a relative web output folder, such as dist".into());
    }
    let root = dunce::canonicalize(base.join(&directory))
        .map_err(|_| "Build the selected worktree first, then choose its web output folder")?;
    if !root.starts_with(&base) {
        return Err("The output folder is outside the worktree".into());
    }
    asset(&root, "/index.html")?;
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let mut all = servers().lock().map_err(|_| "Preview lock failed")?;
    if all.len() >= 8 {
        return Err("Close an existing preview first".into());
    }
    let id = uuid::Uuid::new_v4().to_string();
    all.insert(id.clone(),tokio::spawn(async move {
        // Sequential, bounded requests: preview pages cannot create unbounded file readers.
        while let Ok((mut socket,_))=listener.accept().await {
            let root=&root;
            let _=tokio::time::timeout(std::time::Duration::from_secs(3),async {
                let request=read_headers(&mut socket).await?;
                let mut line=request.lines().next().unwrap_or("").split_whitespace();
                let method=line.next().unwrap_or("");let target=line.next().unwrap_or("");
                let (status,mime,body)=if method=="GET" {
                    if let Ok((path,mime))=asset(root,target) {
                        if std::fs::metadata(&path).is_ok_and(|m|m.len()<=8*1024*1024) {
                            ("200 OK",mime,std::fs::read(path).unwrap_or_default())
                        } else {("413 Content Too Large","text/plain",vec![])}
                    } else {("404 Not Found","text/plain",vec![])}
                }else{("405 Method Not Allowed","text/plain",vec![])};
                socket.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",body.len()).as_bytes()).await?;
                socket.write_all(&body).await
            }).await;
        }
    }));
    Ok(Started {
        id,
        url: format!("http://127.0.0.1:{port}/"),
    })
}
#[tauri::command]
pub fn stop_variant_preview(id: String) -> Result<(), String> {
    if let Some(task) = servers()
        .lock()
        .map_err(|_| "Preview lock failed")?
        .remove(&id)
    {
        task.abort();
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn fragmented_http_requests_are_complete_and_bounded() {
        let (mut input, mut output) = tokio::io::duplex(16384);
        let peer = tokio::spawn(async move {
            output.write_all(b"GE").await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            output
                .write_all(b"T / HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .await
                .unwrap();
        });
        let request = read_headers(&mut input).await.unwrap();
        peer.await.unwrap();
        assert!(request.starts_with("GET / HTTP/1.1\r\n"));
        assert!(request.ends_with("\r\n\r\n"));
        let mut oversized = std::io::Cursor::new(vec![b'x'; 10000]);
        assert_eq!(
            read_headers(&mut oversized).await.unwrap_err().kind(),
            std::io::ErrorKind::InvalidData
        );
    }
    #[test]
    fn serves_rendered_output_but_never_hidden_or_foreign_files() {
        let f = crate::test_support::Fixture::new();
        std::fs::write(f.root.join("index.html"), "<h1>Version A</h1>").unwrap();
        std::fs::write(f.root.join(".env"), "secret").unwrap();
        assert_eq!(asset(&f.root, "/").unwrap().1, "text/html");
        assert!(asset(&f.root, "/%2eenv").is_err());
        assert!(asset(&f.root, "/../private.rs").is_err());
        assert!(asset(&f.root, "/%2f%2fserver/share").is_err());
    }
}
