use clap::{AppSettings, Clap};
use http::Uri;
use mediasoup::rtp_parameters::{MediaKind, MimeTypeVideo};
use mediasoup::rtp_parameters::{
    MimeTypeAudio, RtpCodecParameters, RtpCodecParametersParameters, RtpEncodingParameters,
    RtpParameters,
};
use native_tls::TlsConnector;
use serde::Serialize;
use std::io::Read;
use std::net::TcpStream;
use std::num::{NonZeroU32, NonZeroU8};

use crate::graphql_ws_protocol::GraphQLClient;

mod graphql_ws_protocol;
mod signal_schema;

#[derive(Serialize)]
struct SessionToken {
    token: String,
}

#[derive(Clap)]
#[clap(setting = AppSettings::ColoredHelp)]
pub struct Opts {
    /// Listening address for signal endpoint (domain required).
    #[clap(long, default_value = "wss://localhost:8443")]
    pub signal_addr: String,
    /// Pre-authorized access token.
    #[clap(short, long)]
    pub token: String,
}

fn main() -> Result<(), anyhow::Error> {
    env_logger::init_from_env(
        env_logger::Env::default()
            .filter_or(env_logger::DEFAULT_FILTER_ENV, "ffmpeg_streamer=debug"),
    );
    let opts: Opts = Opts::parse();

    let connector = TlsConnector::builder()
        .danger_accept_invalid_hostnames(true)
        .danger_accept_invalid_certs(true)
        .build()?;

    let uri: Uri = opts.signal_addr.parse()?;
    log::info!("connecting to {}", &uri);

    let host = uri.host().unwrap();
    let port = uri.port_u16().unwrap();
    let stream = TcpStream::connect((host, port))?;
    let tls_stream = connector.connect(host, stream)?;

    let req = tungstenite::handshake::client::Request::builder()
        .uri(uri)
        .header("Sec-WebSocket-Protocol", "graphql-ws")
        .body(())?;
    let (socket, response) = tungstenite::client(req, tls_stream)?;

    log::info!("response http {}:", response.status());
    for (ref header, value) in response.headers() {
        log::debug!("- {}={:?}", header, value);
    }

    let mut client = GraphQLClient::new(
        socket,
        Some(serde_json::to_value(SessionToken { token: opts.token })?),
    );
    let audio_transport_options = client
        .query::<signal_schema::CreateRecvPlainTransport>(
            signal_schema::create_recv_plain_transport::Variables,
        )
        .create_recv_plain_transport;
    let video_transport_options = client
        .query::<signal_schema::CreateRecvPlainTransport>(
            signal_schema::create_recv_plain_transport::Variables,
        )
        .create_recv_plain_transport;

    let audio_transport_id = audio_transport_options.id;
    let video_transport_id = video_transport_options.id;

    log::debug!(
        "audio plain transport options: {:?}",
        audio_transport_options
    );
    log::debug!(
        "video plain transport options: {:?}",
        video_transport_options
    );
    let audio_producer_id = client
        .query::<signal_schema::ProducePlain>(signal_schema::produce_plain::Variables {
            transport_id: audio_transport_id,
            kind: MediaKind::Audio,
            rtp_parameters: RtpParameters {
                codecs: vec![RtpCodecParameters::Audio {
                    mime_type: MimeTypeAudio::Opus,
                    payload_type: 101,
                    clock_rate: NonZeroU32::new(48000).unwrap(),
                    channels: NonZeroU8::new(2).unwrap(),
                    parameters: RtpCodecParametersParameters::from([("sprop-stereo", 1u32.into())]),
                    rtcp_feedback: vec![],
                }],
                encodings: vec![RtpEncodingParameters {
                    ssrc: Some(11111111),
                    ..RtpEncodingParameters::default()
                }],
                ..RtpParameters::default()
            },
        })
        .produce_plain;
    log::debug!("audio producer: {:?}", audio_producer_id);

    let video_producer_id = client
        .query::<signal_schema::ProducePlain>(signal_schema::produce_plain::Variables {
            transport_id: video_transport_id,
            kind: MediaKind::Video,
            rtp_parameters: RtpParameters {
                codecs: vec![RtpCodecParameters::Video {
                    mime_type: MimeTypeVideo::H264,
                    payload_type: 102,
                    clock_rate: NonZeroU32::new(90000).unwrap(),
                    parameters: RtpCodecParametersParameters::default(),
                    rtcp_feedback: vec![],
                }],
                encodings: vec![RtpEncodingParameters {
                    ssrc: Some(22222222),
                    ..RtpEncodingParameters::default()
                }],
                ..RtpParameters::default()
            },
        })
        .produce_plain;
    log::debug!("video producer: {:?}", video_producer_id);

    println!("Press Enter to end session...");
    let _ = std::io::stdin().read(&mut [0u8]).unwrap();
    
    Ok(())
}
