use std::num::{NonZeroU32, NonZeroU8};

use mediasoup::{
    data_structures::{DtlsFingerprint, DtlsParameters, DtlsRole, TransportListenIp},
    rtp_parameters::{
        MediaKind, MimeTypeAudio, MimeTypeVideo, RtcpFeedback, RtcpParameters, RtpCapabilities,
        RtpCodecCapability, RtpCodecParameters, RtpCodecParametersParameters,
        RtpEncodingParameters, RtpEncodingParametersRtx, RtpHeaderExtension,
        RtpHeaderExtensionDirection, RtpHeaderExtensionParameters, RtpHeaderExtensionUri,
        RtpParameters,
    },
    sctp_parameters::SctpStreamParameters,
    worker::WorkerSettings,
    worker_manager::WorkerManager,
};

use vulcan_relay::relay_server::RelayServer;

pub async fn relay_server() -> RelayServer {
    let worker_manager = WorkerManager::new();
    let worker = worker_manager
        .create_worker(WorkerSettings::default())
        .await
        .unwrap();
    RelayServer::new(
        worker,
        TransportListenIp {
            ip: "127.0.0.1".parse().unwrap(),
            announced_ip: None,
        },
        media_codecs(),
    )
}

pub fn media_codecs() -> Vec<RtpCodecCapability> {
    vec![
        RtpCodecCapability::Audio {
            mime_type: MimeTypeAudio::Opus,
            preferred_payload_type: None,
            clock_rate: NonZeroU32::new(48000).unwrap(),
            channels: NonZeroU8::new(2).unwrap(),
            parameters: RtpCodecParametersParameters::default(),
            rtcp_feedback: vec![],
        },
        RtpCodecCapability::Video {
            mime_type: MimeTypeVideo::Vp8,
            preferred_payload_type: None,
            clock_rate: NonZeroU32::new(90000).unwrap(),
            parameters: RtpCodecParametersParameters::default(),
            rtcp_feedback: vec![],
        },
        RtpCodecCapability::Video {
            mime_type: MimeTypeVideo::H264,
            preferred_payload_type: None,
            clock_rate: NonZeroU32::new(90000).unwrap(),
            parameters: RtpCodecParametersParameters::from([
                ("level-asymmetry-allowed", 1u32.into()),
                ("packetization-mode", 1u32.into()),
                ("profile-level-id", "4d0032".into()),
            ]),
            rtcp_feedback: vec![],
        },
    ]
}

pub fn dtls_parameters() -> DtlsParameters {
    DtlsParameters {
        role: DtlsRole::Client,
        fingerprints: vec![DtlsFingerprint::Sha256 {
            value: [
                0x82, 0x5A, 0x68, 0x3D, 0x36, 0xC3, 0x0A, 0xDE, 0xAF, 0xE7, 0x32, 0x43, 0xD2, 0x88,
                0x83, 0x57, 0xAC, 0x2D, 0x65, 0xE5, 0x80, 0xC4, 0xB6, 0xFB, 0xAF, 0x1A, 0xA0, 0x21,
                0x9F, 0x6D, 0x0C, 0xAD,
            ],
        }],
    }
}

pub fn sctp_stream_parameters() -> SctpStreamParameters {
    SctpStreamParameters::new_unordered_with_life_time(12345, 5000)
}

pub fn audio_producer_device_parameters() -> RtpParameters {
    RtpParameters {
        mid: Some("AUDIO".to_string()),
        codecs: vec![RtpCodecParameters::Audio {
            mime_type: MimeTypeAudio::Opus,
            payload_type: 111,
            clock_rate: NonZeroU32::new(48000).unwrap(),
            channels: NonZeroU8::new(2).unwrap(),
            parameters: RtpCodecParametersParameters::from([
                ("useinbandfec", 1u32.into()),
                ("usedtx", 1u32.into()),
                ("foo", "222.222".into()),
                ("bar", "333".into()),
            ]),
            rtcp_feedback: vec![],
        }],
        header_extensions: vec![
            RtpHeaderExtensionParameters {
                uri: RtpHeaderExtensionUri::Mid,
                id: 10,
                encrypt: false,
            },
            RtpHeaderExtensionParameters {
                uri: RtpHeaderExtensionUri::AudioLevel,
                id: 12,
                encrypt: false,
            },
        ],
        encodings: vec![RtpEncodingParameters {
            ssrc: Some(11111111),
            ..RtpEncodingParameters::default()
        }],
        rtcp: RtcpParameters {
            cname: Some("FOOBAR".to_string()),
            ..RtcpParameters::default()
        },
    }
}

pub fn video_producer_device_parameters() -> RtpParameters {
    RtpParameters {
        mid: Some("VIDEO".to_string()),
        codecs: vec![
            RtpCodecParameters::Video {
                mime_type: MimeTypeVideo::H264,
                payload_type: 112,
                clock_rate: NonZeroU32::new(90000).unwrap(),
                parameters: RtpCodecParametersParameters::from([
                    ("packetization-mode", 1u32.into()),
                    ("profile-level-id", "4d0032".into()),
                ]),
                rtcp_feedback: vec![
                    RtcpFeedback::Nack,
                    RtcpFeedback::NackPli,
                    RtcpFeedback::GoogRemb,
                ],
            },
            RtpCodecParameters::Video {
                mime_type: MimeTypeVideo::Rtx,
                payload_type: 113,
                clock_rate: NonZeroU32::new(90000).unwrap(),
                parameters: RtpCodecParametersParameters::from([("apt", 112u32.into())]),
                rtcp_feedback: vec![],
            },
        ],
        header_extensions: vec![
            RtpHeaderExtensionParameters {
                uri: RtpHeaderExtensionUri::Mid,
                id: 10,
                encrypt: false,
            },
            RtpHeaderExtensionParameters {
                uri: RtpHeaderExtensionUri::VideoOrientation,
                id: 13,
                encrypt: false,
            },
        ],
        encodings: vec![
            RtpEncodingParameters {
                ssrc: Some(22222222),
                rtx: Some(RtpEncodingParametersRtx { ssrc: 22222223 }),
                ..RtpEncodingParameters::default()
            },
            RtpEncodingParameters {
                ssrc: Some(22222224),
                rtx: Some(RtpEncodingParametersRtx { ssrc: 22222225 }),
                ..RtpEncodingParameters::default()
            },
            RtpEncodingParameters {
                ssrc: Some(22222226),
                rtx: Some(RtpEncodingParametersRtx { ssrc: 22222227 }),
                ..RtpEncodingParameters::default()
            },
            RtpEncodingParameters {
                ssrc: Some(22222228),
                rtx: Some(RtpEncodingParametersRtx { ssrc: 22222229 }),
                ..RtpEncodingParameters::default()
            },
        ],
        rtcp: RtcpParameters {
            cname: Some("FOOBAR".to_string()),
            ..RtcpParameters::default()
        },
    }
}

pub fn consumer_device_capabilities() -> RtpCapabilities {
    RtpCapabilities {
        codecs: vec![
            RtpCodecCapability::Audio {
                mime_type: MimeTypeAudio::Opus,
                preferred_payload_type: Some(100),
                clock_rate: NonZeroU32::new(48000).unwrap(),
                channels: NonZeroU8::new(2).unwrap(),
                parameters: RtpCodecParametersParameters::default(),
                rtcp_feedback: vec![],
            },
            RtpCodecCapability::Video {
                mime_type: MimeTypeVideo::H264,
                preferred_payload_type: Some(101),
                clock_rate: NonZeroU32::new(90000).unwrap(),
                parameters: RtpCodecParametersParameters::from([
                    ("level-asymmetry-allowed", 1u32.into()),
                    ("packetization-mode", 1u32.into()),
                    ("profile-level-id", "4d0032".into()),
                ]),
                rtcp_feedback: vec![
                    RtcpFeedback::Nack,
                    RtcpFeedback::NackPli,
                    RtcpFeedback::CcmFir,
                    RtcpFeedback::GoogRemb,
                ],
            },
            RtpCodecCapability::Video {
                mime_type: MimeTypeVideo::Rtx,
                preferred_payload_type: Some(102),
                clock_rate: NonZeroU32::new(90000).unwrap(),
                parameters: RtpCodecParametersParameters::from([("apt", 101u32.into())]),
                rtcp_feedback: vec![],
            },
        ],
        header_extensions: vec![
            RtpHeaderExtension {
                kind: Some(MediaKind::Audio),
                uri: RtpHeaderExtensionUri::Mid,
                preferred_id: 1,
                preferred_encrypt: false,
                direction: RtpHeaderExtensionDirection::default(),
            },
            RtpHeaderExtension {
                kind: Some(MediaKind::Video),
                uri: RtpHeaderExtensionUri::Mid,
                preferred_id: 1,
                preferred_encrypt: false,
                direction: RtpHeaderExtensionDirection::default(),
            },
            RtpHeaderExtension {
                kind: Some(MediaKind::Video),
                uri: RtpHeaderExtensionUri::RtpStreamId,
                preferred_id: 2,
                preferred_encrypt: false,
                direction: RtpHeaderExtensionDirection::default(),
            },
            RtpHeaderExtension {
                kind: Some(MediaKind::Audio),
                uri: RtpHeaderExtensionUri::AbsSendTime,
                preferred_id: 4,
                preferred_encrypt: false,
                direction: RtpHeaderExtensionDirection::default(),
            },
            RtpHeaderExtension {
                kind: Some(MediaKind::Video),
                uri: RtpHeaderExtensionUri::AbsSendTime,
                preferred_id: 4,
                preferred_encrypt: false,
                direction: RtpHeaderExtensionDirection::default(),
            },
            RtpHeaderExtension {
                kind: Some(MediaKind::Audio),
                uri: RtpHeaderExtensionUri::AudioLevel,
                preferred_id: 10,
                preferred_encrypt: false,
                direction: RtpHeaderExtensionDirection::default(),
            },
            RtpHeaderExtension {
                kind: Some(MediaKind::Video),
                uri: RtpHeaderExtensionUri::VideoOrientation,
                preferred_id: 11,
                preferred_encrypt: false,
                direction: RtpHeaderExtensionDirection::default(),
            },
            RtpHeaderExtension {
                kind: Some(MediaKind::Video),
                uri: RtpHeaderExtensionUri::TimeOffset,
                preferred_id: 12,
                preferred_encrypt: false,
                direction: RtpHeaderExtensionDirection::default(),
            },
        ],
        fec_mechanisms: vec![],
    }
}
