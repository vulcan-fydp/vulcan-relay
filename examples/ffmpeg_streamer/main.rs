use serde::Serialize;
use std::io::Read;
use std::num::{NonZeroU32, NonZeroU8};
use std::process::Command;
use std::sync::Arc;

use clap::{AppSettings, Clap};
use futures::StreamExt;
use http::Uri;
use mediasoup::rtp_parameters::{
    MediaKind, MimeTypeAudio, MimeTypeVideo, RtcpFeedback, RtpCodecParameters,
    RtpCodecParametersParameters, RtpEncodingParameters, RtpParameters,
};
use tokio::net::TcpStream;
use tokio_tungstenite::Connector;

use graphql_ws::GraphQLWebSocket;

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
    /// Disable TLS.
    #[clap(long)]
    pub no_tls: bool,
    /// File to stream, copied verbatim without re-encoding.
    #[clap(long)]
    pub file: String,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init_from_env(
        env_logger::Env::default()
            .filter_or(env_logger::DEFAULT_FILTER_ENV, "ffmpeg_streamer=debug"),
    );
    let opts: Opts = Opts::parse();

    struct PromiscuousServerVerifier;
    impl rustls::ServerCertVerifier for PromiscuousServerVerifier {
        fn verify_server_cert(
            &self,
            _roots: &rustls::RootCertStore,
            _presented_certs: &[rustls::Certificate],
            _dns_name: webpki::DNSNameRef,
            _ocsp_response: &[u8],
        ) -> Result<rustls::ServerCertVerified, rustls::TLSError> {
            // here be dragons
            Ok(rustls::ServerCertVerified::assertion())
        }
    }
    let mut client_config = rustls::ClientConfig::default();
    client_config
        .dangerous()
        .set_certificate_verifier(Arc::new(PromiscuousServerVerifier));

    let uri: Uri = opts.signal_addr.parse()?;
    log::info!("connecting to {}", &uri);

    let host = uri.host().unwrap();
    let port = uri.port_u16().unwrap();
    let stream = TcpStream::connect((host, port)).await?;

    let req = http::Request::builder()
        .uri(uri)
        .header("Sec-WebSocket-Protocol", "graphql-ws")
        .body(())?;
    let (socket, response) = tokio_tungstenite::client_async_tls_with_config(
        req,
        stream,
        None,
        Some(if opts.no_tls {
            Connector::Plain
        } else {
            Connector::Rustls(Arc::new(client_config))
        }),
    )
    .await?;

    log::info!("response http {}:", response.status());
    for (ref header, value) in response.headers() {
        log::debug!("- {}={:?}", header, value);
    }

    let client = GraphQLWebSocket::new(
        socket,
        Some(serde_json::to_value(SessionToken { token: opts.token })?),
    );
    let audio_transport_options = client
        .query_unchecked::<signal_schema::CreatePlainTransport>(
            signal_schema::create_plain_transport::Variables,
        )
        .await
        .create_plain_transport;
    let video_transport_options = client
        .query_unchecked::<signal_schema::CreatePlainTransport>(
            signal_schema::create_plain_transport::Variables,
        )
        .await
        .create_plain_transport;

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
        .query_unchecked::<signal_schema::ProducePlain>(signal_schema::produce_plain::Variables {
            transport_id: audio_transport_id,
            kind: MediaKind::Audio,
            rtp_parameters: RtpParameters {
                codecs: vec![RtpCodecParameters::Audio {
                    mime_type: MimeTypeAudio::Opus,
                    payload_type: 101,
                    clock_rate: NonZeroU32::new(48000).unwrap(),
                    channels: NonZeroU8::new(2).unwrap(),
                    parameters: RtpCodecParametersParameters::from([("sprop-stereo", 1u32.into())]),
                    rtcp_feedback: vec![RtcpFeedback::TransportCc],
                }],
                encodings: vec![RtpEncodingParameters {
                    ssrc: Some(11111111),
                    ..RtpEncodingParameters::default()
                }],
                ..RtpParameters::default()
            },
        })
        .await
        .produce_plain;
    log::debug!("audio producer: {:?}", audio_producer_id);

    let video_producer_id = client
        .query_unchecked::<signal_schema::ProducePlain>(signal_schema::produce_plain::Variables {
            transport_id: video_transport_id,
            kind: MediaKind::Video,
            rtp_parameters: RtpParameters {
                codecs: vec![RtpCodecParameters::Video {
                    mime_type: MimeTypeVideo::Vp8,
                    payload_type: 102,
                    clock_rate: NonZeroU32::new(90000).unwrap(),
                    parameters: RtpCodecParametersParameters::default(),
                    rtcp_feedback: vec![
                        RtcpFeedback::Nack,
                        RtcpFeedback::NackPli,
                        RtcpFeedback::CcmFir,
                        RtcpFeedback::GoogRemb,
                        RtcpFeedback::TransportCc,
                    ],
                }],
                encodings: vec![RtpEncodingParameters {
                    ssrc: Some(22222222),
                    ..RtpEncodingParameters::default()
                }],
                ..RtpParameters::default()
            },
        })
        .await
        .produce_plain;
    log::debug!("video producer: {:?}", video_producer_id);

    let data_producer_available = client.subscribe::<signal_schema::DataProducerAvailable>(
        signal_schema::data_producer_available::Variables,
    );
    let mut data_producer_available_stream = data_producer_available.execute();
    tokio::spawn(async move {
        while let Some(Ok(response)) = data_producer_available_stream.next().await {
            log::debug!(
                "data producer available: {}",
                response.data.unwrap().data_producer_available
            )
        }
    });

    // pause in case we want to ffmpeg manually
    println!("Press any key to continue...");
    let _ = std::io::stdin().read(&mut [0u8]).unwrap();

    let mut ffmpeg = Command::new("sh")
        .args(&[
            "-c",
            &format!(
                r#"gst-launch-1.0  \
                rtpbin name=rtpbin  \
                filesrc location="{0}"  \
                ! qtdemux name=demux  \
                demux.video_0  \
                ! queue  \
                ! decodebin  \
                ! videoconvert  \
                ! vp8enc target-bitrate=3000000 end-usage=cbr deadline=1 threads=8 cpu-used=-5  \
                ! queue  \
                ! rtpvp8pay pt=102 ssrc=22222222 picture-id-mode=1  \
                ! rtpbin.send_rtp_sink_0  \
                rtpbin.send_rtp_src_0 ! udpsink host={1} port={2} bind-port=50000  \
                rtpbin.send_rtcp_src_0 ! udpsink host={1} port={2} bind-port=50000 sync=false async=false  \
                demux.audio_0  \
                ! queue  \
                ! decodebin  \
                ! audioresample  \
                ! audioconvert  \
                ! opusenc bitrate=64000 inband-fec=true  \
                ! queue  \
                ! rtpopuspay pt=101 ssrc=11111111  \
                ! rtpbin.send_rtp_sink_1  \
                rtpbin.send_rtp_src_1 ! udpsink host={3} port={4} bind-port=50001  \
                rtpbin.send_rtcp_src_1 ! udpsink host={3} port={4} bind-port=50001 sync=false async=false
                "#,
                opts.file,
                video_transport_options.tuple.local_ip(),
                video_transport_options.tuple.local_port(),
                audio_transport_options.tuple.local_ip(),
                audio_transport_options.tuple.local_port(),
            ),
        ])
        .spawn()?;

    ffmpeg.wait()?;

    Ok(())
}
