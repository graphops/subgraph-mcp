// SPDX-License-Identifier: Apache-2.0
pub mod constants;
pub mod error;
pub mod http_utils;
pub mod server;
pub mod server_helpers;
pub mod types;
use anyhow::Result;
use rmcp::{
    transport::sse_server::{SseServer, SseServerConfig},
    ServiceExt,
};
pub use server::SubgraphServer;
use std::{env, net::SocketAddr, time::Duration};
use tokio::io;
use tokio_util::sync::CancellationToken;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init()
        .unwrap_or_else(|e| eprintln!("env_logger init failed: {}", e));

    if args.iter().any(|arg| arg == "--sse") {
        start_sse_server().await
    } else {
        start_stdio_server().await
    }
}

async fn start_stdio_server() -> Result<()> {
    info!("Starting STDIO Subgraph MCP Server");
    let server = SubgraphServer::new();
    let transport = (io::stdin(), io::stdout());
    let running = server.serve(transport).await?;
    running.waiting().await?;
    info!("STDIO Server shutdown complete");
    Ok(())
}

async fn start_sse_server() -> Result<()> {
    info!("Starting SSE Subgraph MCP Server");
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "8000".to_string());
    let bind_addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid BIND address format '{}:{}': {}", host, port, e))?;

    let sse_path = env::var("SSE_PATH").unwrap_or_else(|_| "/sse".to_string());
    let post_path = env::var("POST_PATH").unwrap_or_else(|_| "/messages".to_string());

    let server_shutdown_token = CancellationToken::new();

    let config = SseServerConfig {
        bind: bind_addr,
        sse_path,
        post_path,
        ct: server_shutdown_token.clone(),
        sse_keep_alive: Some(Duration::from_secs(30)),
    };

    let sse_server = SseServer::serve_with_config(config).await?;
    info!("SSE Server listening on {}", sse_server.config.bind);

    let service_shutdown_token = sse_server.with_service(SubgraphServer::new);
    info!("Subgraph MCP Service attached to SSE server");

    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Ctrl+C (SIGINT) received, initiating graceful shutdown...");
        },
        _ = sigterm.recv() => {
             info!("SIGTERM received, initiating graceful shutdown...");
        }
    };

    info!("Signalling service and server to shut down...");
    service_shutdown_token.cancel();
    server_shutdown_token.cancel();

    tokio::time::sleep(Duration::from_secs(1)).await;

    info!("SSE Server shutdown complete.");
    Ok(())
}
