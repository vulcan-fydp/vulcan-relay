use futures::future;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::num::{NonZeroU32, NonZeroU8};
use uuid::Uuid;

use anyhow::anyhow;
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use clap::Clap;
use mediasoup::data_structures::TransportListenIp;
use mediasoup::rtp_parameters::{
    MimeTypeAudio, MimeTypeVideo, RtcpFeedback, RtpCodecCapability, RtpCodecParametersParameters,
};
use mediasoup::webrtc_transport::{TransportListenIps, WebRtcTransportOptions};
use mediasoup::worker::WorkerSettings;
use mediasoup::worker_manager::WorkerManager;
use warp::http::Response as HttpResponse;
use warp::Filter;

use vulcan_relay::cmdline::Opts;
use vulcan_relay::control_schema::{self, ControlSchema};
use vulcan_relay::relay_server::{RelayServer, SessionToken};
use vulcan_relay::signal_schema::{self};

#[tokio::main]
async fn main() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "vulcan_relay=debug"),
    );

    let opts: Opts = Opts::parse();

    let mut transport_options =
        WebRtcTransportOptions::new(TransportListenIps::new(TransportListenIp {
            ip: opts.rtc_ip.parse().unwrap(),
            announced_ip: opts.rtc_announce_ip.and_then(|x| x.parse().ok()),
        }));
    transport_options.enable_sctp = true; // required for data channel
    let media_codecs = media_codecs();

    let worker_manager = WorkerManager::new();
    let worker = worker_manager
        .create_worker(WorkerSettings::default())
        .await
        .unwrap();
    let relay_server = RelayServer::new(worker, transport_options, media_codecs);

    let signal_schema = signal_schema::schema();
    let control_schema = control_schema::schema(relay_server.clone());

    let graphql_signal_ws = warp::ws()
        .and(warp::filters::cookie::cookie("token"))
        .and(async_graphql_warp::graphql_protocol())
        .map(move |ws: warp::ws::Ws, token: String, protocol| {
            let schema = signal_schema.clone();
            let relay_server = relay_server.clone();

            let reply = ws.on_upgrade(move |websocket| async move {
                async_graphql_warp::graphql_subscription_upgrade_with_data(
                    websocket,
                    protocol,
                    schema,
                    |_value| async move {
                        let mut data = async_graphql::Data::default();
                        let token = SessionToken(Uuid::parse_str(token.as_str())?);
                        let session = relay_server
                            .session_from_token(token)
                            .ok_or_else(|| anyhow!("invalid session token"));
                        data.insert(session);
                        Ok(data)
                    },
                )
                .await;
                // TODO invalidate token
            });
            warp::reply::with_header(
                reply,
                "Sec-WebSocket-Protocol",
                protocol.sec_websocket_protocol(),
            )
        });

    let graphql_control_post = async_graphql_warp::graphql(control_schema.clone()).and_then(
        |(schema, request): (ControlSchema, async_graphql::Request)| async move {
            Ok::<_, Infallible>(async_graphql_warp::Response::from(
                schema.execute(request).await,
            ))
        },
    );

    let graphql_playground = warp::path::end().and(warp::get()).map(|| {
        HttpResponse::builder()
            .header("content-type", "text/html")
            .body(playground_source(GraphQLPlaygroundConfig::new("/")))
    });

    let signal_routes = graphql_signal_ws;
    let control_routes = graphql_playground.or(graphql_control_post);
    future::join(
        warp::serve(signal_routes.with(warp::log("signal-server")))
            .tls()
            .cert_path(opts.cert_path.clone())
            .key_path(opts.key_path.clone())
            .run(opts.signal_addr.parse::<SocketAddr>().unwrap()),
        warp::serve(control_routes.with(warp::log("control-server")))
            .tls()
            .cert_path(opts.cert_path)
            .key_path(opts.key_path)
            .run(opts.control_addr.parse::<SocketAddr>().unwrap()),
    )
    .await;
}

fn media_codecs() -> Vec<RtpCodecCapability> {
    vec![
        RtpCodecCapability::Audio {
            mime_type: MimeTypeAudio::Opus,
            preferred_payload_type: None,
            clock_rate: NonZeroU32::new(48000).unwrap(),
            channels: NonZeroU8::new(2).unwrap(),
            parameters: RtpCodecParametersParameters::from([("useinbandfec", 1u32.into())]),
            rtcp_feedback: vec![RtcpFeedback::TransportCc],
        },
        RtpCodecCapability::Video {
            mime_type: MimeTypeVideo::H264,
            preferred_payload_type: None,
            clock_rate: NonZeroU32::new(90000).unwrap(),
            parameters: RtpCodecParametersParameters::from([
                ("packetization-mode", 0u32.into()),
                ("level-asymmetry-allowed", 1u32.into()),
            ]),
            rtcp_feedback: vec![
                RtcpFeedback::Nack,
                RtcpFeedback::NackPli,
                RtcpFeedback::CcmFir,
                RtcpFeedback::GoogRemb,
                RtcpFeedback::TransportCc,
            ],
        },
    ]
}
