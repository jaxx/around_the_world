//! Starts the game server: `PORT` (default 8080), optional `STATIC_DIR`
//! with the built browser client, log level from `RUST_LOG` (default info).

use std::sync::Arc;

use rand::rngs::StdRng;
use server::config::Config;
use server::registry::{Limits, Registry};
use server::room::dice::RandomDice;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = match Config::from_env(|name| std::env::var(name).ok()) {
        Ok(config) => config,
        Err(problem) => {
            tracing::error!("{problem}");
            std::process::exit(2);
        }
    };
    if let Some(dir) = &config.static_dir
        && !dir.join("index.html").is_file()
    {
        tracing::warn!(dir = %dir.display(), "STATIC_DIR has no index.html");
    }

    let registry = Registry::new(
        Limits::default(),
        Arc::new(|| Box::new(RandomDice(rand::make_rng::<StdRng>()))),
    );
    let app = server::http::app(registry, config.static_dir.as_deref(), config.proxy_hops);
    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", config.port)).await {
        Ok(listener) => listener,
        Err(problem) => {
            tracing::error!(port = config.port, "cannot listen: {problem}");
            std::process::exit(1);
        }
    };
    tracing::info!(
        addr = %listener.local_addr().expect("bound"),
        static_dir = ?config.static_dir,
        proxy_hops = config.proxy_hops,
        "listening"
    );

    if let Err(problem) = server::run(listener, app, shutdown_signal()).await {
        tracing::error!("server failed: {problem}");
        std::process::exit(1);
    }
    tracing::info!("stopped");
}

/// Ctrl+C, or SIGTERM where there is one (as container hosts send).
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Ctrl+C can be handled");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("SIGTERM can be handled")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
    tracing::info!("shutting down");
}
