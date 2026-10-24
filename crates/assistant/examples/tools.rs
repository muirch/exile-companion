//! cargo run -p exile-assistant --example tools -- <exile-companion-mcp> [tool] ['<json args>']
//! Starts the MCP server the chat uses and calls one tool, without calling Claude.

#[tokio::main(flavor = "current_thread")]
async fn main() {
    use exile_assistant::Tools;
    let mut args = std::env::args().skip(1);
    let exe = args.next().expect("path to exile-companion-mcp");
    let tool = args.next().unwrap_or_else(|| "player_status".into());
    let input = args
        .next()
        .map(|a| serde_json::from_str(&a).expect("tool args as JSON"))
        .unwrap_or_else(|| serde_json::json!({}));
    let server = exile_assistant::Server::start(exe.as_ref(), &[]).await.unwrap();
    println!(
        "{} tools, {} chars of instructions",
        server.tools.len(),
        server.instructions.len()
    );
    let out = server.call(&tool, input).await;
    println!("{tool} error={}:\n{}", out.is_error, out.text);
}
