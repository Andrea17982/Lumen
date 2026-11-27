//! Sign-in without a terminal. Lumen runs the official CLI's own login command with no console
//! window, shows the sign-in link it prints, and passes on the one-time code the user copies from
//! the provider's page. The CLI does the OAuth and keeps the credentials: Lumen never reads,
//! stores or logs them, and the code is written once to the CLI's input and dropped.
//!
//! Claude Code and Codex support this. OpenCode reads its key only from an interactive terminal,
//! and Antigravity signs in through its own app, so they keep the terminal sign-in.
use crate::*;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// The whole sign-in, browser included, must finish within this time.
const LIMIT: Duration = Duration::from_secs(10 * 60);
/// What a CLI prints is kept only up to this size, to find the link and the code prompt.
const OUTPUT_CAP: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignIn {
    /// starting, browser (waiting for the provider's page), code (waiting for the user's code),
    /// checking, done, failed, cancelled.
    pub phase: String,
    /// Whether the provider page gives a code to paste back (Claude Code); Codex completes alone.
    pub needs_code: bool,
    pub has_link: bool,
    pub message: Option<String>,
}

struct Flow {
    state: SignIn,
    url: Option<String>,
    stdin: Option<tokio::process::ChildStdin>,
    cancel: Option<tokio::sync::oneshot::Sender<()>>,
}

#[derive(Default)]
pub(crate) struct SignIns(Mutex<HashMap<String, Flow>>);

impl SignIns {
    fn update(&self, id: &str, f: impl FnOnce(&mut Flow)) {
        if let Ok(mut map) = self.0.lock() {
            if let Some(flow) = map.get_mut(id) {
                f(flow);
            }
        }
    }
}

/// Whether Lumen can run this provider's sign-in without a terminal.
pub(crate) fn supported(provider: &Provider) -> bool {
    matches!(provider, Provider::Claude | Provider::Gpt)
}

/// The first sign-in link in the CLI output, only on the provider's own sign-in hosts.
///
/// While the CLI is still printing, a link at the very end may be cut in the middle of a read,
/// so only links followed by whitespace count until `complete`.
fn link(output: &str, complete: bool) -> Option<String> {
    const HOSTS: [&str; 4] = [
        "claude.com",
        "claude.ai",
        "anthropic.com",
        "auth.openai.com",
    ];
    let finished = if complete {
        output
    } else {
        &output[..output.rfind(char::is_whitespace).unwrap_or(0)]
    };
    finished
        .split(|c: char| c.is_whitespace())
        .filter(|word| word.starts_with("https://"))
        .find_map(|word| {
            let url = word.trim_end_matches(['.', ',', ')', '"', '\'']);
            let host = url
                .strip_prefix("https://")?
                .split(['/', '?', '#'])
                .next()?;
            // Plain DNS names only: no user info, port, backslash, escapes or look-alike characters.
            (host
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
                && HOSTS
                    .iter()
                    .any(|h| host == *h || host.ends_with(&format!(".{h}"))))
            .then(|| url.to_string())
        })
}

/// A code copied from the provider's page: one line of visible characters.
fn valid_code(code: &str) -> Result<&str, String> {
    let code = code.trim();
    if code.is_empty() || code.len() > 2048 || !code.chars().all(|c| c.is_ascii_graphic()) {
        return Err("Paste the code exactly as the sign-in page shows it".into());
    }
    Ok(code)
}

/// The last meaningful line a failed sign-in printed, without any link (they carry one-time state).
fn reason(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .rev()
        .find(|l| {
            !l.is_empty()
                && !l.ends_with('>')
                && !["http", "code=", "state=", "token"]
                    .iter()
                    .any(|secret| l.to_ascii_lowercase().contains(secret))
        })
        .map(|l| l.chars().take(300).collect())
}

#[crate::command]
pub async fn login_start(core: crate::CoreRef<'_>, id: String) -> Result<SignIn, String> {
    let a = account(&core, &id)?;
    if !supported(&a.provider) {
        return Err("This agent signs in from its own terminal sign-in".into());
    }
    let mut c = providers::command(&a.provider)?;
    providers::isolate(&mut c, &a.provider, &config_dir(&core, &a));
    for key in providers::SIGN_IN_SECRETS {
        c.env_remove(key);
    }
    match a.provider {
        Provider::Claude => c.args(["auth", "login", "--claudeai"]),
        _ => c.arg("login"),
    };
    c.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let state = SignIn {
        phase: "starting".into(),
        needs_code: a.provider == Provider::Claude,
        has_link: false,
        message: None,
    };
    let (cancel, cancelled) = tokio::sync::oneshot::channel();
    {
        let mut map = core.sign_ins.0.lock().map_err(|_| "Sign-in lock failed")?;
        // Signing in again while one is open shows that one instead of starting a second CLI.
        if let Some(open) = map.get(&id).filter(|f| {
            matches!(
                f.state.phase.as_str(),
                "starting" | "browser" | "code" | "checking"
            )
        }) {
            return Ok(open.state.clone());
        }
        let mut child = c.spawn().map_err(|e| e.to_string())?;
        let stdin = child.stdin.take();
        let out = child.stdout.take();
        let err = child.stderr.take();
        map.insert(
            id.clone(),
            Flow {
                state: state.clone(),
                url: None,
                stdin,
                cancel: Some(cancel),
            },
        );
        tokio::spawn(watch(core.inner().clone(), a, child, out, err, cancelled));
    }
    core.catalogs
        .lock()
        .map_err(|_| "Catalog lock failed")?
        .remove(&id);
    core.logins.forget(&id);
    Ok(state)
}

async fn watch(
    core: Arc<Core>,
    a: Account,
    mut child: tokio::process::Child,
    out: Option<tokio::process::ChildStdout>,
    err: Option<tokio::process::ChildStderr>,
    cancelled: tokio::sync::oneshot::Receiver<()>,
) {
    let id = a.id.clone();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(16);
    fn pump(
        mut r: impl tokio::io::AsyncRead + Unpin + Send + 'static,
        tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    ) {
        tokio::spawn(async move {
            let mut buf = [0u8; 2048];
            while let Ok(n) = r.read(&mut buf).await {
                if n == 0 || tx.send(buf[..n].to_vec()).await.is_err() {
                    break;
                }
            }
        });
    }
    if let Some(out) = out {
        pump(out, tx.clone());
    }
    if let Some(err) = err {
        pump(err, tx.clone());
    }
    drop(tx);
    let mut output = String::new();
    let deadline = tokio::time::sleep(LIMIT);
    tokio::pin!(deadline, cancelled);
    let seen = |output: &str, complete: bool| {
        let url = link(output, complete);
        let prompt = output.contains("Paste code");
        core.sign_ins.update(&id, |f| {
            if f.url.is_none() && url.is_some() {
                f.url = url;
                f.state.has_link = true;
            }
            if f.state.phase == "starting" && f.url.is_some() {
                f.state.phase = if prompt { "code" } else { "browser" }.into();
            } else if prompt && f.state.phase == "browser" {
                f.state.phase = "code".into();
            }
        });
    };
    let mut streams = true;
    let outcome = loop {
        tokio::select! {
            chunk = rx.recv(), if streams => match chunk {
                Some(bytes) => {
                    if output.len() < OUTPUT_CAP {
                        output.push_str(&String::from_utf8_lossy(&bytes));
                    }
                    seen(&output, false);
                }
                // Both streams closed; the process may still run, so Stop and the limit still apply.
                None => streams = false,
            },
            status = child.wait() => {
                // What the CLI printed last (often the reason it failed) is read before deciding.
                while let Ok(Some(bytes)) =
                    tokio::time::timeout(Duration::from_millis(200), rx.recv()).await
                {
                    if output.len() < OUTPUT_CAP {
                        output.push_str(&String::from_utf8_lossy(&bytes));
                    }
                }
                seen(&output, true);
                break status.map(|s| s.success()).ok();
            }
            _ = &mut cancelled => {
                providers::terminate(&mut child).await;
                core.sign_ins.update(&id, |f| {
                    f.state.phase = "cancelled".into();
                    f.state.has_link = false;
                    f.url = None;
                    f.stdin = None;
                });
                return;
            }
            _ = &mut deadline => {
                providers::terminate(&mut child).await;
                break None;
            }
        }
    };
    core.sign_ins.update(&id, |f| {
        f.state.phase = "checking".into();
        f.stdin = None;
        f.cancel = None;
    });
    // The CLI's own status is the only proof of a sign-in; its exit code alone is not.
    let checked = crate::signed_in(&core, &a).await;
    let signed_in = checked.as_ref().is_ok_and(|&ok| ok);
    core.catalogs.lock().map(|mut c| c.remove(&id)).ok();
    core.logins.forget(&id);
    core.sign_ins.update(&id, |f| {
        f.url = None;
        f.state.has_link = false;
        if signed_in {
            f.state.phase = "done".into();
            f.state.message = None;
        } else {
            f.state.phase = "failed".into();
            f.state.message = match outcome {
                None => Some("The sign-in did not finish in time.".into()),
                // The CLI ended but its status could not be read: say so instead of no cause.
                Some(_) if checked.is_err() => Some(
                    "Lumen could not check the sign-in. Press Check on the account to try again."
                        .into(),
                ),
                Some(_) => reason(&output),
            };
        }
    });
}

#[crate::command]
pub fn login_state(core: crate::CoreRef<'_>, id: String) -> Result<Option<SignIn>, String> {
    let map = core.sign_ins.0.lock().map_err(|_| "Sign-in lock failed")?;
    Ok(map.get(&id).map(|f| f.state.clone()))
}

/// Opens the provider's sign-in page again if the CLI could not open the browser itself.
/// The shell opens the link this returns; the page only names the account.
#[crate::command]
pub fn open_login_url(core: crate::CoreRef<'_>, id: String) -> Result<String, String> {
    let map = core.sign_ins.0.lock().map_err(|_| "Sign-in lock failed")?;
    map.get(&id)
        .and_then(|f| f.url.clone())
        .ok_or_else(|| "The sign-in link is no longer available. Start the sign-in again.".into())
}

#[crate::command]
pub async fn login_code(core: crate::CoreRef<'_>, id: String, code: String) -> Result<(), String> {
    let code = valid_code(&code)?.to_string();
    let stdin = {
        let mut map = core.sign_ins.0.lock().map_err(|_| "Sign-in lock failed")?;
        let flow = map
            .get_mut(&id)
            .filter(|f| f.state.phase == "code" || f.state.phase == "browser")
            .ok_or("This sign-in is no longer waiting for a code")?;
        flow.state.phase = "checking".into();
        flow.stdin.take()
    };
    let mut stdin = stdin.ok_or("This sign-in is no longer waiting for a code")?;
    let written = async {
        stdin.write_all(code.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await
    }
    .await;
    written
        .map_err(|_| "The sign-in ended before the code was received. Start it again.".to_string())
}

#[crate::command]
pub fn login_cancel(core: crate::CoreRef<'_>, id: String) -> Result<(), String> {
    let mut map = core.sign_ins.0.lock().map_err(|_| "Sign-in lock failed")?;
    if let Some(cancel) = map.get_mut(&id).and_then(|f| f.cancel.take()) {
        let _ = cancel.send(());
    } else {
        map.remove(&id);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_only_the_providers_sign_in_link() {
        let claude = "Opening browser to sign in…\nIf the browser didn't open, visit: https://claude.com/cai/oauth/authorize?code=true&state=x\nPaste code here if prompted > ";
        assert_eq!(
            link(claude, false).as_deref(),
            Some("https://claude.com/cai/oauth/authorize?code=true&state=x")
        );
        let codex = "Starting local login server on http://localhost:1455.\n\nhttps://auth.openai.com/oauth/authorize?response_type=code&state=y\n";
        assert!(link(codex, false)
            .unwrap()
            .starts_with("https://auth.openai.com/"));
        // A read that ends inside the link waits for the rest of it.
        let cut = "visit: https://claude.com/cai/oauth/authorize?code=tr";
        assert_eq!(link(cut, false), None);
        assert!(link(cut, true).is_some());
        for hostile in [
            "visit https://evil.example/claude.com",
            "visit https://claude.com.evil.example/x",
            "visit https://user@claude.com/x",
            "visit https://claude.com:8443/x",
            "visit http://claude.com/x",
            "visit https://evil.com\\.claude.com/x",
            "visit https://evil%2ecom.claude.com/x",
            "visit https://CLAUDE.com.evil.example/x",
        ] {
            assert_eq!(link(&format!("{hostile} "), true), None, "{hostile}");
        }
    }

    #[test]
    fn accepts_one_line_codes_only() {
        assert_eq!(valid_code("  abc#DEF-123_x  ").unwrap(), "abc#DEF-123_x");
        assert!(valid_code("").is_err());
        assert!(valid_code("two\nlines").is_err());
        assert!(valid_code("with space").is_err());
        assert!(valid_code(&"x".repeat(3000)).is_err());
    }

    #[test]
    fn a_failure_reason_never_carries_the_link() {
        let out = "Opening browser\nvisit: https://claude.com/x?state=s\nPaste code here if prompted > \nOAuth error: invalid code\n";
        assert_eq!(reason(out).as_deref(), Some("OAuth error: invalid code"));
        assert_eq!(reason("https://claude.com/x\n"), None);
        assert_eq!(
            reason("callback http://localhost:1455/auth/callback?code=abc\n"),
            None
        );
    }
}
