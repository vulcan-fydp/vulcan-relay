use mediasoup::consumer::{Consumer, ConsumerId, ConsumerOptions};
use mediasoup::data_structures::{DtlsParameters, IceCandidate, IceParameters, TransportListenIp};
use mediasoup::producer::{Producer, ProducerId, ProducerOptions};
use mediasoup::router::{Router, RouterOptions};
use mediasoup::rtp_parameters::{
    MediaKind, MimeTypeAudio, MimeTypeVideo, RtcpFeedback, RtpCapabilities,
    RtpCapabilitiesFinalized, RtpCodecCapability, RtpCodecParametersParameters, RtpParameters,
};
use mediasoup::transport::{Transport, TransportId};
use mediasoup::webrtc_transport::{
    TransportListenIps, WebRtcTransport, WebRtcTransportOptions, WebRtcTransportRemoteParameters,
};
use mediasoup::worker::WorkerSettings;
use mediasoup::worker_manager::WorkerManager;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TransportOptions {
    id: TransportId,
    dtls_parameters: DtlsParameters,
    ice_candidates: Vec<IceCandidate>,
    ice_parameters: IceParameters,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ServerInitMessage {
    consumer_transport_options: TransportOptions,
    producer_transport_options: TransportOptions,
    router_rtp_capabilities: RtpCapabilitiesFinalized,
}


#[derive(Serialize, Clone)]
#[serde(tag = "action")]
pub enum ServerMessage {
    Init(ServerInitMessage),
    ConnectedProducerTransport,
    #[serde(rename_all = "camelCase")]
    Produced {
        id: ProducerId,
    },
    ConnectedConsumerTransport,
    #[serde(rename_all = "camelCase")]
    Consumed {
        id: ConsumerId,
        producer_id: ProducerId,
        kind: MediaKind,
        rtp_parameters: RtpParameters,
    },
}

#[derive(Deserialize)]
#[serde(tag = "action")]
enum ClientMessage {
    #[serde(rename_all = "camelCase")]
    Init { rtp_capabilities: RtpCapabilities },
    #[serde(rename_all = "camelCase")]
    ConnectProducerTransport { dtls_parameters: DtlsParameters },
    #[serde(rename_all = "camelCase")]
    Produce {
        kind: MediaKind,
        rtp_parameters: RtpParameters,
    },
    #[serde(rename_all = "camelCase")]
    ConnectConsumerTransport { dtls_parameters: DtlsParameters },
    #[serde(rename_all = "camelCase")]
    Consume { producer_id: ProducerId },
    #[serde(rename_all = "camelCase")]
    ConsumerResume { id: ConsumerId },
}
