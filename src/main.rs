use futures::future;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::num::{NonZeroU32, NonZeroU8};
use std::sync::{Arc, Mutex};

use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use clap::Clap;
use mediasoup::{
    data_structures::TransportListenIp,
    rtp_parameters::{
        MimeTypeAudio, MimeTypeVideo, RtcpFeedback, RtpCodecCapability,
        RtpCodecParametersParameters,
    },
    worker::WorkerSettings,
    worker_manager::WorkerManager,
};
use warp::{http::Response as HttpResponse, Filter};

use vulcan_relay::built_info;
use vulcan_relay::{
    cmdline::Opts,
    control_schema::{self, ControlSchema},
    relay_server::{RelayServer, SessionToken},
    signal_schema,
};

#[tokio::main]
async fn main() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "vulcan_relay=trace"),
    );

    log::info!(
        "{} {} {} {}",
        built_info::PKG_NAME,
        built_info::PKG_VERSION,
        built_info::TARGET,
        built_info::PROFILE
    );

    let opts: Opts = Opts::parse();

    let transport_listen_ip = TransportListenIp {
        ip: opts.rtc_ip.parse().unwrap(),
        announced_ip: opts.rtc_announce_ip.and_then(|x| x.parse().ok()),
    };
    let media_codecs = media_codecs();

    let worker_manager = WorkerManager::new();
    let worker = worker_manager
        .create_worker(WorkerSettings::default())
        .await
        .unwrap();
    let relay_server = RelayServer::new(worker, transport_listen_ip, media_codecs);

    let signal_schema = signal_schema::schema();
    let control_schema = control_schema::schema(relay_server.clone());

    let graphql_signal_ws = warp::ws()
        .and(warp::filters::cookie::optional("token"))
        .and(async_graphql_warp::graphql_protocol())
        .map(
            move |ws: warp::ws::Ws, cookie_token: Option<String>, protocol| {
                let schema = signal_schema.clone();
                let relay_server = relay_server.clone();

                let reply = ws.on_upgrade(move |websocket| async move {
                    let relay_server_copy = relay_server.clone();
                    // get token from cookie if it exists
                    let cookie_token = cookie_token.and_then(|cookie_token| {
                        serde_json::from_str::<SessionToken>(cookie_token.as_str()).ok()
                    });
                    let selected_token: Arc<Mutex<Option<SessionToken>>> =
                        Arc::new(Mutex::new(None));
                    let selected_token_send = selected_token.clone();
                    async_graphql_warp::graphql_subscription_upgrade_with_data(
                        websocket,
                        protocol,
                        schema,
                        move |value| async move {
                            let mut data = async_graphql::Data::default();
                            // get token from connection params if it exists
                            let param_token = value.get("token").and_then(|param_token| {
                                serde_json::from_value::<SessionToken>(param_token.to_owned()).ok()
                            });
                            let token = param_token.or(cookie_token);
                            if let Some(token) = token {
                                let mut selected_token = selected_token_send.lock().unwrap();
                                selected_token.replace(token);
                                // create session from the selected token
                                if let Some(weak_session) =
                                    relay_server_copy.session_from_token(token)
                                {
                                    data.insert(weak_session);
                                }
                            }
                            Ok(data)
                        },
                    )
                    .await;

                    // drop session on websocket close
                    let mut selected_token = selected_token.lock().unwrap();
                    if let Some(token) = selected_token.take() {
                        drop(relay_server.take_session_by_token(&token))
                    }
                });
                warp::reply::with_header(
                    reply,
                    "Sec-WebSocket-Protocol",
                    protocol.sec_websocket_protocol(),
                )
            },
        );

    let graphql_control_post = async_graphql_warp::graphql(control_schema.clone())
        .and_then(
            |(schema, request): (ControlSchema, async_graphql::Request)| async move {
                Ok::<_, Infallible>(async_graphql_warp::Response::from(
                    schema.execute(request).await,
                ))
            },
        )
        .with(
            warp::cors()
                .allow_any_origin()
                .allow_headers(vec!["content-type"])
                .allow_methods(vec!["POST"]),
        );

    let graphql_playground = warp::path::end().and(warp::get()).map(|| {
        HttpResponse::builder()
            .header("content-type", "text/html")
            .body(playground_source(GraphQLPlaygroundConfig::new("/")))
    });

    let signal_routes = graphql_signal_ws;
    let control_routes = graphql_playground.or(graphql_control_post);

    let signal_addr = opts.signal_addr.parse::<SocketAddr>().unwrap();
    let control_addr = opts.control_addr.parse::<SocketAddr>().unwrap();

    if opts.no_tls {
        log::info!("signal graphql endpoint: ws://{}", signal_addr);
        log::info!("control endpoint: http://{}", control_addr);
        let signal_server = warp::serve(signal_routes.with(warp::log("signal-server")));
        let control_server = warp::serve(control_routes.with(warp::log("control-server")));
        future::join(
            signal_server.run(signal_addr),
            control_server.run(control_addr),
        )
        .await;
    } else {
        log::info!("signal graphql endpoint: wss://{}", signal_addr);
        log::info!("control graphql endpoint: https://{}", control_addr);
        let signal_server = warp::serve(signal_routes.with(warp::log("signal-server")))
            .tls()
            .cert_path(opts.cert_path.clone().unwrap())
            .key_path(opts.key_path.clone().unwrap());
        let control_server = warp::serve(control_routes.with(warp::log("control-server")))
            .tls()
            .cert_path(opts.cert_path.unwrap())
            .key_path(opts.key_path.unwrap());
        future::join(
            signal_server.run(signal_addr),
            control_server.run(control_addr),
        )
        .await;
    };
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
