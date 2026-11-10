#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    // Claude starts this same executable as its permission-prompt MCP bridge (see approvals.rs).
    // It must print protocol frames only, so it returns before the window or any logging starts.
    if std::env::args_os()
        .nth(1)
        .is_some_and(|a| a == "--lumen-gatekeeper-mcp")
    {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        let ok = runtime.map(|rt| rt.block_on(lumen_gatekeeper::mcp::proxy_stdio()));
        std::process::exit(match ok {
            Ok(Ok(())) => 0,
            _ => 1,
        });
    }
    // Claude Code and Codex start it as the code-graph MCP server (see graph_mcp.rs).
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if args
        .get(1)
        .is_some_and(|a| a == "--lumen-graph-mcp-context")
    {
        let result = args
            .get(2)
            .zip(std::env::var_os("LUMEN_WORKSPACE_ROOT"))
            .map(|(data, root)| lumen_lib::serve_graph(data.into(), root.into()));
        std::process::exit(if matches!(result, Some(Ok(()))) { 0 } else { 1 });
    }
    if args.get(1).is_some_and(|a| a == "--lumen-graph-mcp") && args.len() >= 4 {
        let ok = lumen_lib::serve_graph(args[2].clone().into(), args[3].clone().into());
        std::process::exit(if ok.is_ok() { 0 } else { 1 });
    }
    lumen_lib::run();
}
