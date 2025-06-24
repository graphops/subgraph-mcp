// SPDX-License-Identifier: Apache-2.0
pub mod constants;
pub mod error;
pub mod metrics;
pub mod server;
pub mod server_helpers;
pub mod types;
use crate::metrics::METRICS;
use anyhow::Result;
use axum::{
    body::Body,
    extract::State,
    http::{header::CONTENT_TYPE, StatusCode},
    response::{IntoResponse, Response},
};
use clap::Parser;
use prometheus_client::{encoding::text::encode, registry::Registry};
use rmcp::{
    transport::sse_server::{SseServer, SseServerConfig},
    ServiceExt,
};
pub use server::SubgraphServer;
use std::{env, net::SocketAddr, sync::Arc, time::Duration};
use tokio::io;
use tokio_util::sync::CancellationToken;
use tracing::info;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Start the server in SSE mode
    #[arg(long)]
    sse: bool,

    /// Initialize a default configuration file
    #[arg(long, short)]
    init_config: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init()
        .unwrap_or_else(|e| eprintln!("env_logger init failed: {}", e));

    if cli.init_config {
        println!("Configuration initialization logic goes here.");
        return Ok(());
    }

    if cli.sse {
        let shutdown_token = CancellationToken::new();

        let sse_server_handle = tokio::spawn(start_sse_server(shutdown_token.clone()));
        let metrics_server_handle = tokio::spawn(start_metrics_server(shutdown_token.clone()));

        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Ctrl+C (SIGINT) received, initiating graceful shutdown...");
            },
            _ = sigterm.recv() => {
                 info!("SIGTERM received, initiating graceful shutdown...");
            }
        };

        info!("Signalling services to shut down...");
        shutdown_token.cancel();

        let _ = sse_server_handle.await?;
        let _ = metrics_server_handle.await?;

        tokio::time::sleep(Duration::from_secs(1)).await;

        info!("All services shutdown complete.");
        Ok(())
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

async fn start_sse_server(shutdown_token: CancellationToken) -> Result<()> {
    info!("Starting SSE Subgraph MCP Server");
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "8000".to_string());
    let bind_addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid BIND address format '{}:{}': {}", host, port, e))?;

    let sse_path = env::var("SSE_PATH").unwrap_or_else(|_| "/sse".to_string());
    let post_path = env::var("POST_PATH").unwrap_or_else(|_| "/messages".to_string());

    let config = SseServerConfig {
        bind: bind_addr,
        sse_path,
        post_path,
        ct: shutdown_token.clone(),
        sse_keep_alive: Some(Duration::from_secs(30)),
    };

    let sse_server = SseServer::serve_with_config(config).await?;
    info!("SSE Server listening on {}", sse_server.config.bind);

    let service_shutdown_token = sse_server.with_service_directly(SubgraphServer::new);
    info!("Subgraph MCP Service attached to SSE server");

    shutdown_token.cancelled().await;

    info!("SSE Server shutdown signal received. Giving tasks a moment to finish...");
    service_shutdown_token.cancel();
    tokio::time::sleep(Duration::from_secs(1)).await;

    info!("SSE Server shutdown complete.");
    Ok(())
}

async fn metrics_handler(State(registry): State<Arc<Registry>>) -> impl IntoResponse {
    let mut buffer = String::new();
    if let Err(e) = encode(&mut buffer, &registry) {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from(format!("Failed to encode metrics: {}", e)))
            .unwrap();
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(
            CONTENT_TYPE,
            "application/openmetrics-text; version=1.0.0; charset=utf-8",
        )
        .body(Body::from(buffer))
        .unwrap()
}

async fn start_metrics_server(shutdown_token: CancellationToken) -> Result<()> {
    let mut registry = <Registry as Default>::default();
    METRICS.register(&mut registry);
    let registry = Arc::new(registry);

    let host = env::var("METRICS_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("METRICS_PORT").unwrap_or_else(|_| "9091".to_string());
    let bind_addr: SocketAddr = format!("{}:{}", host, port).parse().map_err(|e| {
        anyhow::anyhow!(
            "Invalid METRICS BIND address format '{}:{}': {}",
            host,
            port,
            e
        )
    })?;

    let app = axum::Router::new()
        .route("/metrics", axum::routing::get(metrics_handler))
        .with_state(registry);

    info!("Metrics server listening on {}", bind_addr);
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_token.cancelled().await;
            info!("Metrics server shutting down.");
        })
        .await?;

    Ok(())
}
