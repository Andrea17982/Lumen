#[tokio::main]
async fn main() {
    if lumen_gatekeeper::mcp::proxy_stdio().await.is_err() {
        eprintln!("Lumen Gatekeeper MCP bridge unavailable");
        std::process::exit(1);
    }
}
