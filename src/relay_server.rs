use crate::messages::ServerMessage;
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
use mediasoup::worker::{Worker, WorkerSettings};
use mediasoup::worker_manager::WorkerManager;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt;
use std::num::{NonZeroU32, NonZeroU8};
use std::sync::{Arc, Weak};
use tokio::sync::{broadcast, Mutex};

#[derive(Debug, Clone)]
pub struct InvalidSessionError(pub String);
impl fmt::Display for InvalidSessionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
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
            mime_type: MimeTypeVideo::Vp8,
            preferred_payload_type: None,
            clock_rate: NonZeroU32::new(90000).unwrap(),
            parameters: RtpCodecParametersParameters::default(),
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

type RoomId = String;

#[derive(Serialize, Deserialize, Clone)]
pub enum Role {
    Vulcast,
    WebClient,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionToken {
    pub room_id: RoomId,
    pub role: Role,
}

pub struct Room {
    pub host: Option<Arc<Mutex<Session>>>,
    pub sessions: Vec<Arc<Mutex<Session>>>,
    pub router: Router,

    pub room_tx: broadcast::Sender<ServerMessage>,
}

pub struct Session {
    pub client_rtp_capabilities: Option<RtpCapabilities>,
    pub transport: WebRtcTransport,
    pub consumers: HashMap<ConsumerId, Consumer>,
    pub producers: Vec<Producer>,

    pub room: Weak<Mutex<Room>>,
}
impl Session {}

pub struct RelayServer {
    worker_manager: WorkerManager,
    worker: Worker,
    rooms: HashMap<RoomId, Arc<Mutex<Room>>>,
}

impl RelayServer {
    pub async fn new() -> Self {
        let worker_manager = WorkerManager::new();
        let worker = worker_manager
            .create_worker(WorkerSettings::default())
            .await
            .unwrap();
        Self {
            worker_manager: worker_manager,
            worker: worker,
            rooms: HashMap::new(),
        }
    }

    pub async fn get_room(&mut self, room_id: RoomId) -> Arc<Mutex<Room>> {
        match self.rooms.entry(room_id) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => {
                let router = self
                    .worker
                    .create_router(RouterOptions::new(media_codecs()))
                    .await
                    .unwrap();
                let (tx, _) = broadcast::channel(16);
                let room = Room {
                    host: None,
                    sessions: vec![],
                    router: router,
                    room_tx: tx,
                };
                v.insert(Arc::new(Mutex::new(room)))
            }
        }
        .clone()
    }

    pub async fn new_session(
        &mut self,
        token: SessionToken,
    ) -> Result<Arc<Mutex<Session>>, InvalidSessionError> {
        let room_ptr = self.get_room(token.room_id).await;
        let transport_options =
            WebRtcTransportOptions::new(TransportListenIps::new(TransportListenIp {
                ip: "0.0.0.0".parse().unwrap(),
                announced_ip: "192.168.140.136".parse().ok(),
            }));

        let mut room = room_ptr.lock().await;
        let transport = room
            .router
            .create_webrtc_transport(transport_options)
            .await
            .unwrap();
        let session = Arc::new(Mutex::new(Session {
            client_rtp_capabilities: None,
            transport: transport,
            consumers: HashMap::new(),
            producers: vec![],
            room: Arc::downgrade(&room_ptr),
        }));
        match token.role {
            Role::Vulcast if room.host.is_none() => {
                room.host = Some(session.clone());
                Ok(session)
            }
            Role::WebClient => {
                room.sessions.push(session.clone());
                Ok(session)
            }
            _ => Err(InvalidSessionError(
                "Cannot have multiple Vulcasts in one room".to_owned(),
            )),
        }
    }
}
