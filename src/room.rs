use futures::stream::{self, Stream, StreamExt};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

use anyhow::{anyhow, Result};
use mediasoup::data_producer::DataProducerId;
use mediasoup::producer::ProducerId;
use mediasoup::router::{Router, RouterOptions};
use mediasoup::rtp_parameters::RtpCodecCapability;
use mediasoup::worker::Worker;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

use crate::session::{Role, Session, SessionId, WeakSession};

pub type RoomId = String;

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
    router: Router,
    producer_available_tx: broadcast::Sender<ProducerId>,
    data_producer_available_tx: broadcast::Sender<DataProducerId>,
}

struct State {
    sessions: HashMap<SessionId, WeakSession>,
}

impl Room {
    pub async fn new(room_id: RoomId, worker: Worker, codecs: Vec<RtpCodecCapability>) -> Self {
        let router = worker
            .create_router(RouterOptions::new(codecs))
            .await
            .unwrap();
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    sessions: HashMap::new(),
                }),
                id: room_id,
                router,
                producer_available_tx: broadcast::channel(16).0,
                data_producer_available_tx: broadcast::channel(16).0,
            }),
        }
    }

    pub fn get_router(&self) -> Router {
        self.shared.router.clone()
    }

    pub fn add_session(&self, session: Session) -> Result<()> {
        let mut state = self.shared.state.lock().unwrap();
        if session.role() == Role::Vulcast
            && state
                .sessions
                .values()
                .any(|session| session.upgrade().unwrap().role() == Role::Vulcast)
        {
            return Err(anyhow!("cannot have more than one Vulcast in a room"));
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

        Ok(())
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
        self.shared.id.clone()
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
