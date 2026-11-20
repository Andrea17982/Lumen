#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod ownership;
use lumen_core::{host::EventHub, Core};
use lumen_ipc::{write, Reply, Request};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
#[cfg(windows)]
type Peer = tokio::net::windows::named_pipe::NamedPipeServer;
#[cfg(unix)]
type Peer = tokio::net::UnixStream;

struct Service {
    core: Arc<Core>,
    events: EventHub,
    build: String,
    stopping: AtomicBool,
    connections: Arc<std::sync::atomic::AtomicUsize>,
    shutdown: tokio::sync::Notify,
    gate: tokio::sync::RwLock<()>,
    cancel: tokio::sync::watch::Sender<bool>,
}
fn main() {
    #[cfg(unix)]
    if std::env::args_os()
        .nth(1)
        .is_some_and(|a| a == "--lumen-group-guard")
    {
        match lumen_core::run_group_guard() {
            Ok(code) => std::process::exit(code),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
    }
    #[cfg(unix)]
    if std::env::args_os()
        .nth(1)
        .is_some_and(|a| a == "--lumen-owned-child")
    {
        match lumen_core::run_owned_child() {
            Ok(code) => std::process::exit(code),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Service runtime");
    if let Err(error) = runtime.block_on(run()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().collect();
    // Native CLI helpers return before ownership, storage, or any service logging starts.
    if let Some(flag) = args.get(1).and_then(|a| a.to_str()) {
        match flag {
            "--lumen-native-account-mcp" if args.len() == 5 => {
                return lumen_core::serve_native_account(
                    args[2].clone().into(),
                    args[3].clone().into(),
                    args[4] != "full",
                )
                .await
            }
            "--lumen-shared-stdio" if args.len() == 5 => {
                return lumen_core::serve_shared_stdio(
                    args[2].clone().into(),
                    args[3].clone().into(),
                    &args[4].to_string_lossy(),
                )
                .await
            }
            "--lumen-plugin-mcp" if args.len() == 5 => {
                return lumen_core::serve_plugin_mcp(
                    args[2].clone().into(),
                    &args[3].to_string_lossy(),
                    &args[4].to_string_lossy(),
                )
            }
            "--lumen-browser-mcp" => return lumen_core::serve_browser().await,
            "--lumen-code-mcp" => return lumen_core::serve_code().await,
            "--lumen-gatekeeper-mcp" => return lumen_gatekeeper::mcp::proxy_stdio().await,
            "--lumen-graph-mcp-context" if args.len() >= 3 => {
                return lumen_core::serve_graph(
                    args[2].clone().into(),
                    std::env::var_os("LUMEN_WORKSPACE_ROOT")
                        .ok_or("Missing workspace")?
                        .into(),
                )
                .map_err(|e| e.to_string())
            }
            "--lumen-graph-mcp" if args.len() >= 4 => {
                return lumen_core::serve_graph(args[2].clone().into(), args[3].clone().into())
                    .map_err(|e| e.to_string())
            }
            _ => {}
        }
    }
    let option = |key: &str| {
        args.windows(2)
            .find(|a| a[0] == key)
            .map(|a| PathBuf::from(&a[1]))
    };
    let root = option("--data-dir")
        .or_else(|| {
            if cfg!(debug_assertions) {
                std::env::var_os("LUMEN_TEST_DATA_DIR").map(PathBuf::from)
            } else {
                None
            }
        })
        .unwrap_or(
            dirs::data_dir()
                .ok_or("Missing user data directory")?
                .join("io.github.andrea17982.lumen"),
        );
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let endpoint = lumen_ipc::endpoint(&root)?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let build = lumen_ipc::binary_digest(&exe)?;
    let client = lumen_ipc::Client {
        endpoint: endpoint.clone(),
        build: build.clone(),
    };
    if args
        .iter()
        .any(|a| a == "--status" || a == "--shutdown" || a == "--prepare-update")
    {
        let command = if args.iter().any(|a| a == "--shutdown") {
            "__shutdown"
        } else if args.iter().any(|a| a == "--prepare-update") {
            "__prepare_update"
        } else {
            "__status"
        };
        println!("{}", client.call(command, serde_json::json!({})).await?);
        return Ok(());
    }
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join("daemon.lock"))
        .map_err(|e| e.to_string())?;
    lock.try_lock()
        .map_err(|_| "Lumen already owns this data directory".to_string())?;
    ownership::install()?;
    let resources = option("--resources").unwrap_or_else(|| exe.parent().unwrap().to_path_buf());
    let events = EventHub::default();
    let core = lumen_core::open(root.clone(), resources, events.clone())?;
    let service = Arc::new(Service {
        core,
        events,
        build,
        stopping: AtomicBool::new(false),
        connections: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        shutdown: tokio::sync::Notify::new(),
        gate: tokio::sync::RwLock::new(()),
        cancel: tokio::sync::watch::channel(false).0,
    });
    #[cfg(windows)]
    {
        use tokio::net::windows::named_pipe::ServerOptions;
        let mut server = ServerOptions::new()
            .first_pipe_instance(true)
            .reject_remote_clients(true)
            .create(&endpoint)
            .map_err(|e| e.to_string())?;
        loop {
            tokio::select! {_=service.shutdown.notified()=>break,result=server.connect()=>result.map_err(|e|e.to_string())?}
            let connected = server;
            server = ServerOptions::new()
                .reject_remote_clients(true)
                .create(&endpoint)
                .map_err(|e| e.to_string())?;
            spawn_peer(connected, service.clone());
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // The held per-root lock proves the old socket cannot belong to a live service.
        if endpoint.exists() {
            std::fs::remove_file(&endpoint).map_err(|e| e.to_string())?;
        }
        let server = tokio::net::UnixListener::bind(&endpoint).map_err(|e| e.to_string())?;
        std::fs::set_permissions(&endpoint, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
        loop {
            tokio::select! {_=service.shutdown.notified()=>break,peer=server.accept()=>{let(io,_)=peer.map_err(|e|e.to_string())?;if lumen_ipc::verify_unix(&io).is_ok(){spawn_peer(io,service.clone());}}}
        }
        let _ = std::fs::remove_file(endpoint);
    }
    Ok(())
}
fn spawn_peer(io: Peer, service: Arc<Service>) {
    tokio::spawn(async move {
        let _ = serve_peer(io, service).await;
    });
}
async fn serve_peer(mut io: Peer, service: Arc<Service>) -> Result<(), String> {
    let Service {
        core,
        events,
        build,
        stopping,
        connections,
        shutdown,
        gate,
        cancel,
    } = &*service;
    let bytes = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        lumen_ipc::read_packet(&mut io),
    )
    .await
    .map_err(|_| "IPC handshake timed out")??;
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        // Impersonation identifies the last bytes read; parse nothing until that identity passes.
        lumen_ipc::peer::verify_client(io.as_raw_handle())?;
    }
    let request: Request = serde_json::from_slice(&bytes).map_err(|_| "Invalid local IPC JSON")?;
    if request.protocol != lumen_ipc::PROTOCOL || request.build != *build {
        return write(
            &mut io,
            &Reply {
                result: Err(
                    "The running Lumen service uses a different build; stop it before updating"
                        .into(),
                ),
            },
        )
        .await;
    }
    if request.command == "__events" {
        let mut receiver = events.subscribe();
        connections.fetch_add(1, Ordering::SeqCst);
        struct Guard(Arc<std::sync::atomic::AtomicUsize>);
        impl Drop for Guard {
            fn drop(&mut self) {
                self.0.fetch_sub(1, Ordering::SeqCst);
            }
        }
        let _guard = Guard(connections.clone());
        write(
            &mut io,
            &Reply {
                result: Ok(serde_json::json!({"ready":true})),
            },
        )
        .await?;
        let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(2));
        loop {
            let event = tokio::select! {e=receiver.recv()=>match e{Ok(e)=>serde_json::to_value(e).unwrap(),Err(tokio::sync::broadcast::error::RecvError::Lagged(_))=>serde_json::json!({"event":"lumen:resync","payload":{}}),Err(_)=>break},_=heartbeat.tick()=>serde_json::json!({"event":"lumen:heartbeat","payload":{}})};
            write(&mut io, &event).await?;
        }
        return Ok(());
    }
    let stop = matches!(request.command.as_str(), "__shutdown" | "__prepare_update");
    let result = if stop {
        if stopping.swap(true, Ordering::SeqCst) {
            Err("Lumen is already stopping".into())
        } else {
            let exclusive = if request.command == "__prepare_update" {
                gate.try_write().map_err(|_| {
                    "Wait for the active command before installing an update".to_string()
                })
            } else {
                lumen_core::begin_shutdown(core);
                cancel.send_replace(true);
                Ok(gate.write().await)
            };
            match exclusive {
                Err(error) => {
                    stopping.store(false, Ordering::SeqCst);
                    Err(error)
                }
                Ok(_exclusive)
                    if request.command == "__prepare_update"
                        && (!lumen_core::idle(core) || connections.load(Ordering::SeqCst) > 0) =>
                {
                    stopping.store(false, Ordering::SeqCst);
                    Err(
                        "Close every Lumen window and stop active work before installing an update"
                            .into(),
                    )
                }
                Ok(_exclusive) => match lumen_core::shutdown(core).await {
                    Ok(()) => Ok(serde_json::json!({"stopped":true})),
                    Err(error) => {
                        cancel.send_replace(false);
                        stopping.store(false, Ordering::SeqCst);
                        Err(error)
                    }
                },
            }
        }
    } else {
        let _shared = gate.read().await;
        if request.command == "__status" {
            Ok(
                serde_json::json!({"protocol":lumen_ipc::PROTOCOL,"version":env!("CARGO_PKG_VERSION"),"pid":std::process::id(),"idle":lumen_core::idle(core),"interfaces":connections.load(Ordering::SeqCst),"stopping":stopping.load(Ordering::SeqCst)}),
            )
        } else if stopping.load(Ordering::SeqCst) {
            Err("Lumen is stopping; reconnect after it restarts".into())
        } else {
            let mut cancelled = cancel.subscribe();
            let dispatch = async {
                if lumen_ipc::receipted(&request.command) {
                    let id = request.command_id.as_deref().ok_or("Missing command ID")?;
                    let fingerprint = lumen_ipc::digest(
                        &serde_json::to_vec(&(&request.command, &request.args))
                            .map_err(|e| e.to_string())?,
                    );
                    lumen_core::dispatch_receipted(
                        events.clone(),
                        core,
                        &request.command,
                        request.args,
                        id,
                        &fingerprint,
                    )
                    .await
                } else {
                    lumen_core::dispatch(events.clone(), core, &request.command, request.args).await
                }
            };
            tokio::select! {biased;
                _=cancelled.changed()=>Err("Service stopped during the command; review its result before retrying".into()),
                result=dispatch=>result,
            }
        }
    };
    let stopped = stop && result.is_ok();
    let delivered = write(&mut io, &Reply { result }).await;
    if stopped {
        shutdown.notify_one();
    }
    delivered
}
