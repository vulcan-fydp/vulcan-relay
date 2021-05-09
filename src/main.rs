use async_graphql::Data;
use async_graphql_warp::{graphql_protocol, graphql_subscription_upgrade_with_data};
use tokio::sync::mpsc;
use warp::Filter;

use crate::relay_server::RelayServer;
use crate::session::SessionToken;

mod relay_server;
mod room;
mod session;
mod signal_schema;

#[tokio::main]
async fn main() {
    env_logger::init();

    let relay_server = RelayServer::new().await;
    let schema = signal_schema::schema(&relay_server);
    // println!("{}", &schema.sdl());

    let routes = warp::ws()
        .and(graphql_protocol())
        .map(move |ws: warp::ws::Ws, protocol| {
            let schema = schema.clone();
            let relay_server = relay_server.clone();

            let reply = ws.on_upgrade(move |websocket| async move {
                let (tx, mut rx) = mpsc::channel(1);
                let relay_server_copy = relay_server.clone(); // are you happy, borrow checker

                graphql_subscription_upgrade_with_data(
                    websocket,
                    protocol,
                    schema,
                    |value| async move {
                        let mut data = Data::default();
                        if let Ok(token) = serde_json::from_value::<SessionToken>(value) {
                            let session = relay_server_copy.session_from_token(token).await?;
                            data.insert(session.clone());
                            tx.send(session).await.expect("receiver dropped");
                        }
                        Ok(data)
                    },
                )
                .await;
                if let Some(session) = rx.recv().await {
                    relay_server.end_session(&session).await;
                }
            });
            warp::reply::with_header(
                reply,
                "Sec-WebSocket-Protocol",
                protocol.sec_websocket_protocol(),
            )
        });
    warp::serve(routes.with(warp::log("warp-server")))
        .tls()
        .cert_path("config/cert.pem")
        .key_path("config/key.pem")
        .run(([0, 0, 0, 0], 8443))
        .await;
}
