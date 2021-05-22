use futures::stream::{self, Stream, StreamExt};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use uuid::Uuid;

use derive_more::Display;
use mediasoup::data_producer::DataProducerId;
use mediasoup::producer::ProducerId;
use mediasoup::router::{Router, RouterOptions};
use mediasoup::rtp_parameters::RtpCodecCapability;
use mediasoup::worker::Worker;
use tokio::sync::{broadcast, OnceCell};
use tokio_stream::wrappers::BroadcastStream;

use crate::session::{Role, Session, SessionId, WeakSession};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Display, Hash)]
pub struct RoomId(Uuid);
impl RoomId {
    pub fn new() -> Self {
        RoomId(Uuid::new_v4())
    }
}

#[derive(Clone)]
pub struct Room {
    shared: Arc<Shared>,
}

#[derive(Clone)]
pub struct WeakRoom {
    shared: Weak<Shared>,
}

struct Shared {
    state: Mutex<State>,

    id: RoomId,
    worker: Worker,
    codecs: Vec<RtpCodecCapability>,

    router: OnceCell<Router>,
    producer_available_tx: broadcast::Sender<ProducerId>,
    data_producer_available_tx: broadcast::Sender<DataProducerId>,
}

struct State {
    sessions: HashMap<SessionId, WeakSession>,
}

impl Room {
    pub fn new(worker: Worker, codecs: Vec<RtpCodecCapability>) -> Self {
        let id = RoomId::new();
        log::debug!("created new room {}", id);
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    sessions: HashMap::new(),
                }),
                id,
                worker,
                codecs,
                router: OnceCell::new(),
                producer_available_tx: broadcast::channel(16).0,
                data_producer_available_tx: broadcast::channel(16).0,
            }),
        }
    }

    pub async fn get_router(&self) -> Router {
        self.shared
            .router
            .get_or_init(|| async {
                self.shared
                    .worker
                    .create_router(RouterOptions::new(self.shared.codecs.clone()))
                    .await
                    .unwrap()
            })
            .await
            .clone()
    }

    pub fn add_session(&self, session: Session) {
        let mut state = self.shared.state.lock().unwrap();
        if session.role() == Role::Vulcast
            && state
                .sessions
                .values()
                .any(|session| session.upgrade().unwrap().role() == Role::Vulcast)
        {
            panic!("cannot have more than one Vulcast in a room");
        }
        state.sessions.insert(session.id(), session.downgrade());

        let weak_room = self.downgrade();
        session
            .on_closed(move |session_id| {
                if let Some(room) = weak_room.upgrade() {
                    log::debug!("removing session {} from room {}", session_id, room.id());
                    room.remove_session(session_id);
                }
            })
            .detach();
    }

    fn remove_session(&self, session_id: SessionId) {
        let mut state = self.shared.state.lock().unwrap();
        state
            .sessions
            .remove(&session_id)
            .expect("session does not exist");
    }

    pub fn announce_producer(&self, producer_id: ProducerId) {
        let _ = self.shared.producer_available_tx.send(producer_id);
    }
    pub fn announce_data_producer(&self, data_producer_id: DataProducerId) {
        let _ = self
            .shared
            .data_producer_available_tx
            .send(data_producer_id);
    }

    /// Get a stream which yields existing and new producers
    pub fn available_producers(&self) -> impl Stream<Item = ProducerId> {
        let state = self.shared.state.lock().unwrap();
        let producers = state
            .sessions
            .values()
            .flat_map(|session| session.upgrade().unwrap().get_producers())
            .map(|producer| producer.id())
            .collect::<Vec<ProducerId>>();
        stream::select(
            stream::iter(producers),
            BroadcastStream::new(self.shared.producer_available_tx.subscribe())
                .map(|x| x.expect("receiver dropped message")),
        )
    }
    /// Get a stream which yields existing and new data producers
    pub fn available_data_producers(&self) -> impl Stream<Item = DataProducerId> {
        let state = self.shared.state.lock().unwrap();
        let data_producers = state
            .sessions
            .values()
            .flat_map(|session| session.upgrade().unwrap().get_data_producers())
            .map(|data_producer| data_producer.id())
            .collect::<Vec<DataProducerId>>();
        stream::select(
            stream::iter(data_producers),
            BroadcastStream::new(self.shared.data_producer_available_tx.subscribe())
                .map(|x| x.expect("receiver dropped message")),
        )
    }

    pub fn id(&self) -> RoomId {
        self.shared.id
    }
    pub fn downgrade(&self) -> WeakRoom {
        WeakRoom {
            shared: Arc::downgrade(&self.shared),
        }
    }
}

impl WeakRoom {
    pub fn upgrade(&self) -> Option<Room> {
        let shared = self.shared.upgrade()?;
        Some(Room { shared })
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        log::debug!("dropped room {}", self.id)
    }
}
