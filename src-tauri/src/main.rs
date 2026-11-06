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
    lumen_lib::run();
}
