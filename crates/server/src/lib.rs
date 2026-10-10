//! Game server for «Reis ümber maailma». Room logic in [`room::state`] is
//! pure (time and dice passed in); [`room::actor`] runs each room as a
//! tokio task, [`registry`] finds rooms by code and [`http`] puts it all on
//! the network.

pub mod client_ip;
pub mod config;
pub mod http;
pub mod registry;
pub mod room;
mod ws;

use std::future::Future;
use std::net::SocketAddr;

use axum::Router;
use tokio::net::TcpListener;

/// Serves `app` until `shutdown` completes, then lets open requests finish.
pub async fn run(
    listener: TcpListener,
    app: Router,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await
}
