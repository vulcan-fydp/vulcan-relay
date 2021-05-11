use std::net::SocketAddr;

use async_graphql::Data;
use async_graphql_warp::{graphql_protocol, graphql_subscription_upgrade_with_data};
use clap::Clap;
use warp::Filter;

use crate::cmdline::{Opts, SubCommand};
use crate::relay_server::RelayServer;
use crate::session::SessionToken;
use mediasoup::data_structures::TransportListenIp;
use mediasoup::webrtc_transport::TransportListenIps;
use mediasoup::webrtc_transport::WebRtcTransportOptions;

mod cmdline;
mod relay_server;
mod room;
mod session;
mod signal_schema;

#[tokio::main]
async fn main() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "vulcan_relay=debug"));

    let opts: Opts = Opts::parse();

    let schema = signal_schema::schema();

    match opts.subcmd {
        SubCommand::Schema => {
            println!("{}", &schema.sdl());
        }
        SubCommand::Run(opts) => {
            let mut transport_options =
                WebRtcTransportOptions::new(TransportListenIps::new(TransportListenIp {
                    ip: opts.rtc_ip.parse().unwrap(),
                    announced_ip: opts.rtc_announce_ip.and_then(|x| x.parse().ok()),
                }));
            transport_options.enable_sctp = true; // required for data channel
            let relay_server = RelayServer::new(transport_options).await;

            let routes =
                warp::ws()
                    .and(graphql_protocol())
                    .map(move |ws: warp::ws::Ws, protocol| {
                        let schema = schema.clone();
                        let relay_server = relay_server.clone();

                        let reply = ws.on_upgrade(move |websocket| async move {
                            graphql_subscription_upgrade_with_data(
                                websocket,
                                protocol,
                                schema,
                                |value| async move {
                                    let mut data = Data::default();
                                    if let Ok(token) = serde_json::from_value::<SessionToken>(value)
                                    {
                                        let session =
                                            relay_server.session_from_token(token).await?;
                                        data.insert(session.clone());
                                    }
                                    Ok(data)
                                },
                            )
                            .await;
                        });
                        warp::reply::with_header(
                            reply,
                            "Sec-WebSocket-Protocol",
                            protocol.sec_websocket_protocol(),
                        )
                    });
            warp::serve(routes.with(warp::log("warp-server")))
                .tls()
                .cert_path(opts.cert_path)
                .key_path(opts.key_path)
                .run(opts.listen_addr.parse::<SocketAddr>().unwrap())
                .await;
        }
    };
}
