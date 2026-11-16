use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
pub const PROTOCOL: u32 = 1;
pub const MAX_FRAME: usize = 40 * 1024 * 1024;

pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn binary_digest(path: &Path) -> Result<String, String> {
    std::fs::read(path)
        .map(|b| digest(&b))
        .map_err(|e| e.to_string())
}
pub fn endpoint(root: &Path) -> Result<PathBuf, String> {
    let root = dunce::canonicalize(root).map_err(|e| e.to_string())?;
    let key = digest(root.to_string_lossy().as_bytes());
    #[cfg(windows)]
    {
        Ok(PathBuf::from(format!(r"\\.\pipe\lumen-{}", &key[..32])))
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let dir = std::env::temp_dir().join(format!("lumen-{}", unsafe { libc::geteuid() }));
        match std::fs::create_dir(&dir) {
            Ok(()) => std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.to_string()),
        }
        let meta = std::fs::symlink_metadata(&dir).map_err(|e| e.to_string())?;
        if !meta.is_dir() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
            return Err("Local IPC directory is not private".into());
        }
        Ok(dir.join(format!("{}.sock", &key[..32])))
    }
}
pub async fn write<T: Serialize>(
    io: &mut (impl AsyncWrite + Unpin),
    value: &T,
) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FRAME {
        return Err("Local IPC frame is too large".into());
    }
    io.write_u32(bytes.len() as u32)
        .await
        .map_err(|e| e.to_string())?;
    io.write_all(&bytes).await.map_err(|e| e.to_string())?;
    io.flush().await.map_err(|e| e.to_string())
}
pub async fn read<T: DeserializeOwned>(io: &mut (impl AsyncRead + Unpin)) -> Result<T, String> {
    let bytes = read_packet(io).await?;
    serde_json::from_slice(&bytes).map_err(|_| "Invalid local IPC JSON".into())
}
pub async fn read_packet(io: &mut (impl AsyncRead + Unpin)) -> Result<Vec<u8>, String> {
    let len = io.read_u32().await.map_err(|e| e.to_string())? as usize;
    if len == 0 || len > MAX_FRAME {
        return Err("Invalid local IPC frame length".into());
    }
    let mut bytes = vec![0; len];
    io.read_exact(&mut bytes).await.map_err(|e| e.to_string())?;
    Ok(bytes)
}
#[derive(Serialize, Deserialize)]
pub struct Request {
    pub protocol: u32,
    pub build: String,
    pub command: String,
    pub args: Value,
}
#[derive(Serialize, Deserialize)]
pub struct Reply {
    pub result: Result<Value, String>,
}

#[cfg(windows)]
pub type Stream = tokio::net::windows::named_pipe::NamedPipeClient;
#[cfg(unix)]
pub type Stream = tokio::net::UnixStream;
pub async fn connect(endpoint: &Path) -> Result<Stream, String> {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        let mut attempts = 0;
        let io = loop {
            match tokio::net::windows::named_pipe::ClientOptions::new().open(endpoint) {
                Ok(io) => break io,
                Err(e) if e.raw_os_error() == Some(231) && attempts < 250 => {
                    attempts += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
                Err(e) => return Err(e.to_string()),
            }
        };
        peer::verify_server(io.as_raw_handle())?;
        Ok(io)
    }
    #[cfg(unix)]
    {
        let io = tokio::net::UnixStream::connect(endpoint)
            .await
            .map_err(|e| e.to_string())?;
        verify_unix(&io)?;
        Ok(io)
    }
}
#[cfg(unix)]
pub fn verify_unix(io: &tokio::net::UnixStream) -> Result<(), String> {
    if io.peer_cred().map_err(|e| e.to_string())?.uid() != unsafe { libc::geteuid() } {
        return Err("IPC peer belongs to another user".into());
    }
    Ok(())
}

#[derive(Clone)]
pub struct Client {
    pub endpoint: PathBuf,
    pub build: String,
}
impl Client {
    pub async fn start(&self, command: &str, args: Value) -> Result<Stream, String> {
        let mut io = connect(&self.endpoint).await?;
        write(
            &mut io,
            &Request {
                protocol: PROTOCOL,
                build: self.build.clone(),
                command: command.into(),
                args,
            },
        )
        .await?;
        Ok(io)
    }
    pub async fn call(&self, command: &str, args: Value) -> Result<Value, String> {
        let mut io = self.start(command, args).await?;
        read::<Reply>(&mut io).await?.result
    }
}

#[cfg(windows)]
pub mod peer {
    use windows::Win32::Security::RevertToSelf;
    use windows::Win32::{
        Foundation::{CloseHandle, HANDLE},
        Security::{EqualSid, GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER},
        System::{
            Pipes::{GetNamedPipeServerProcessId, ImpersonateNamedPipeClient},
            Threading::{
                GetCurrentProcess, GetCurrentThread, OpenProcess, OpenProcessToken,
                OpenThreadToken, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
    };
    struct Token(HANDLE);
    impl Drop for Token {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    fn user(token: HANDLE) -> Result<Vec<usize>, String> {
        unsafe {
            let mut n = 0;
            let _ = GetTokenInformation(token, TokenUser, None, 0, &mut n);
            if n == 0 {
                return Err("Cannot read IPC identity".into());
            }
            let mut data = vec![0usize; (n as usize).div_ceil(std::mem::size_of::<usize>())];
            GetTokenInformation(token, TokenUser, Some(data.as_mut_ptr().cast()), n, &mut n)
                .map_err(|e| e.to_string())?;
            Ok(data)
        }
    }
    fn same(token: HANDLE) -> Result<(), String> {
        unsafe {
            let mut own = HANDLE::default();
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut own)
                .map_err(|e| e.to_string())?;
            let own = Token(own);
            let a = user(own.0)?;
            let b = user(token)?;
            let a = &*(a.as_ptr().cast::<TOKEN_USER>());
            let b = &*(b.as_ptr().cast::<TOKEN_USER>());
            if EqualSid(a.User.Sid, b.User.Sid).is_err() {
                return Err("IPC peer belongs to another user".into());
            }
            Ok(())
        }
    }
    pub fn verify_server(raw: *mut std::ffi::c_void) -> Result<(), String> {
        unsafe {
            let mut pid = 0;
            GetNamedPipeServerProcessId(HANDLE(raw), &mut pid).map_err(|e| e.to_string())?;
            let process = Token(
                OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
                    .map_err(|e| e.to_string())?,
            );
            let mut token = HANDLE::default();
            OpenProcessToken(process.0, TOKEN_QUERY, &mut token).map_err(|e| e.to_string())?;
            let token = Token(token);
            same(token.0)
        }
    }
    pub fn verify_client(raw: *mut std::ffi::c_void) -> Result<(), String> {
        unsafe {
            ImpersonateNamedPipeClient(HANDLE(raw)).map_err(|e| e.to_string())?;
            let mut token = HANDLE::default();
            let opened = OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, true, &mut token);
            // No await or return is allowed while the worker impersonates the client.
            if RevertToSelf().is_err() {
                std::process::abort();
            }
            opened.map_err(|e| e.to_string())?;
            let token = Token(token);
            same(token.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn framing_rejects_oversize_before_allocation_and_invalid_json() {
        let (mut a, mut b) = tokio::io::duplex(128);
        a.write_u32((MAX_FRAME + 1) as u32).await.unwrap();
        assert!(read::<Value>(&mut b).await.unwrap_err().contains("length"));
        a.write_u32(1).await.unwrap();
        a.write_all(b"{").await.unwrap();
        assert!(read::<Value>(&mut b).await.is_err());
        write(&mut a, &serde_json::json!({"ok":true}))
            .await
            .unwrap();
        assert_eq!(read::<Value>(&mut b).await.unwrap()["ok"], true);
    }
}
