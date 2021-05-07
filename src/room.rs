use futures::stream::{self, Stream, StreamExt};
use std::collections::HashMap;
use std::num::{NonZeroU32, NonZeroU8};
use std::sync::{Arc, Mutex, Weak};

use mediasoup::producer::ProducerId;
use mediasoup::router::{Router, RouterOptions};
use mediasoup::rtp_parameters::{
    MimeTypeAudio, MimeTypeVideo, RtcpFeedback, RtpCodecCapability, RtpCodecParametersParameters,
};
use mediasoup::worker::Worker;
use thiserror::Error;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

use crate::session::{Role, Session, SessionId};

pub type RoomId = String;
/// Version of Room that weakly owns Shared state, for cycle prevention
pub type WeakRoom = Weak<Shared>;

#[derive(Debug, Clone)]
pub struct Room {
    shared: Arc<Shared>,
}

#[derive(Debug)]
pub struct Shared {
    state: Mutex<State>,

    id: RoomId,
    router: Router,
    producer_available_tx: broadcast::Sender<ProducerId>,
}

#[derive(Debug)]
struct State {
    host: Option<Session>,
    sessions: HashMap<SessionId, Session>,
}

impl Room {
    pub async fn new(room_id: RoomId, worker: Worker) -> Self {
        let router = worker
            .create_router(RouterOptions::new(media_codecs()))
            .await
            .unwrap();
        let (tx, _) = broadcast::channel(16);
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    host: None,
                    sessions: HashMap::new(),
                }),
                id: room_id,
                router: router,
                producer_available_tx: tx,
            }),
        }
    }

    pub fn get_router(&self) -> Router {
        self.shared.router.clone()
    }

    pub fn add_session(&self, session: Session, role: Role) -> Result<(), RoomError> {
        let mut state = self.shared.state.lock().unwrap();
        match role {
            Role::Vulcast => match state.host {
                Some(_) => Err(RoomError::TooManyHosts),
                None => {
                    state.host.replace(session);
                    Ok(())
                }
            },
            Role::WebClient => {
                state.sessions.insert(session.id(), session);
                Ok(())
            }
        }
    }

    pub fn remove_session(&self, session: &Session) {
        let mut state = self.shared.state.lock().unwrap();
        match &state.host {
            Some(host) if host.id() == session.id() => {
                state.host = None;
            }
            _ => {
                state
                    .sessions
                    .remove(&session.id())
                    .expect("session does not exist");
            }
        }
    }

    pub fn notify_new_producer(&self, producer_id: ProducerId) {
        let _ = self.shared.producer_available_tx.send(producer_id);
    }

    /// Get a stream which yields existing and new producers
    pub fn available_producers(&self) -> impl Stream<Item = ProducerId> {
        let state = self.shared.state.lock().unwrap();
        stream::select(
            stream::iter(
                match &state.host {
                    Some(host) => host.get_producers(),
                    None => vec![],
                }
                .into_iter()
                .map(|producer| producer.id()),
            ),
            BroadcastStream::new(self.shared.producer_available_tx.subscribe())
                .map(|x| x.expect("receiver dropped message")),
        )
    }
    pub fn is_empty(&self) -> bool {
        let state = self.shared.state.lock().unwrap();
        state.host.is_none() && state.sessions.is_empty()
    }
    pub fn id(&self) -> RoomId {
        self.shared.id.clone()
    }
}

#[derive(Error, Debug)]
pub enum RoomError {
    #[error("cannot connect more than one Vulcast to a room")]
    TooManyHosts,
}

impl From<WeakRoom> for Room {
    fn from(shared: WeakRoom) -> Self {
        Room {
            shared: shared.upgrade().unwrap(),
        }
    }
}

impl From<Room> for WeakRoom {
    fn from(room: Room) -> Self {
        Arc::downgrade(&room.shared)
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
