mod server;

use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use server::ForgiaMcp;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> Result<()> {
    // Logs on stderr only — stdout is reserved for JSON-RPC
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("forgia-mcp starting");

    let service = ForgiaMcp::new()
        .serve(stdio())
        .await?;

    service.waiting().await?;
    Ok(())
}
