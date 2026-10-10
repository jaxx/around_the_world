//! Starting and stopping the whole server.

mod common;

use std::time::Duration;

use server::registry::Limits;
use tokio::sync::oneshot;

#[tokio::test]
async fn the_server_stops_when_asked() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = server::http::app(common::registry(Limits::default()), None);
    let (stop, stopped) = oneshot::channel::<()>();
    let server = tokio::spawn(server::run(listener, app, async {
        let _ = stopped.await;
    }));

    tokio::net::TcpStream::connect(addr)
        .await
        .expect("the server accepts connections");
    stop.send(()).unwrap();
    let result = tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .expect("the server stops within 5 s");
    result.unwrap().unwrap();
}
