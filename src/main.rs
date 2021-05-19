use std::convert::Infallible;
use std::net::SocketAddr;
use std::num::{NonZeroU32, NonZeroU8};

use clap::Clap;
use mediasoup::data_structures::TransportListenIp;
use mediasoup::rtp_parameters::{
    MimeTypeAudio, MimeTypeVideo, RtcpFeedback, RtpCodecCapability, RtpCodecParametersParameters,
};
use mediasoup::webrtc_transport::{TransportListenIps, WebRtcTransportOptions};
use warp::Filter;

use vulcan_relay::cmdline::{Opts, SubCommand};
use vulcan_relay::relay_server::RelayServer;
use vulcan_relay::session::SessionToken;
use vulcan_relay::signal_schema::SignalSchema;

#[tokio::main]
async fn main() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "vulcan_relay=debug"),
    );

    let opts: Opts = Opts::parse();

    let schema = vulcan_relay::signal_schema::schema();

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

            let media_codecs = vec![
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
            ];

            let relay_server = RelayServer::new(transport_options, media_codecs).await;

            let graphql_post = async_graphql_warp::graphql(schema.clone()).and_then(
                |(schema, request): (SignalSchema, async_graphql::Request)| async move {
                    Ok::<_, Infallible>(async_graphql_warp::Response::from(
                        schema.execute(request).await,
                    ))
                },
            );
            let graphql_ws = warp::ws().and(async_graphql_warp::graphql_protocol()).map(
                move |ws: warp::ws::Ws, protocol| {
                    let schema = schema.clone();
                    let relay_server = relay_server.clone();

                    let reply = ws.on_upgrade(move |websocket| async move {
                        async_graphql_warp::graphql_subscription_upgrade_with_data(
                            websocket,
                            protocol,
                            schema,
                            |value| async move {
                                let mut data = async_graphql::Data::default();
                                if let Ok(token) = serde_json::from_value::<SessionToken>(value) {
                                    let session = relay_server.session_from_token(token).await?;
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
                },
            );
            let routes = graphql_ws.or(graphql_post);
            warp::serve(routes.with(warp::log("warp-server")))
                .tls()
                .cert_path(opts.cert_path)
                .key_path(opts.key_path)
                .run(opts.listen_addr.parse::<SocketAddr>().unwrap())
                .await;
        }
    };
}
