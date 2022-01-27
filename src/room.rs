use futures::{
    future,
    stream::{self, Stream, StreamExt},
};
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

use crate::session::{Session, SessionId, WeakSession};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Display, Hash, Default)]
pub struct RoomId(Uuid);
impl RoomId {
    pub fn new() -> Self {
        RoomId(Uuid::new_v4())
    }
}

#[derive(Debug, Clone)]
pub struct Room {
    shared: Arc<Shared>,
}

#[derive(Debug, Clone)]
pub struct WeakRoom {
    shared: Weak<Shared>,
}

#[derive(Debug)]
struct Shared {
    state: Mutex<State>,

    id: RoomId,
    worker: Worker,
    codecs: Vec<RtpCodecCapability>,

    router: OnceCell<Router>,
    channel_tx: broadcast::Sender<Message>,
}

#[derive(Debug)]
struct State {
    sessions: HashMap<SessionId, WeakSession>,
}

#[derive(Debug, Clone)]
pub enum Message {
    ProducerAvailable(ProducerId),
    DataProducerAvailable(DataProducerId),
}

impl Room {
    pub fn new(worker: Worker, codecs: Vec<RtpCodecCapability>) -> Self {
        let id = RoomId::new();
        log::trace!("+room {}", id);
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    sessions: HashMap::new(),
                }),
                id,
                worker,
                codecs,
                router: OnceCell::new(),
                channel_tx: broadcast::channel(16).0,
            }),
        }
    }

    /// Get the Mediasoup Router associated with this room.
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

    /// Add a session to this room.
    pub fn add_session(&self, session: Session) {
        let mut state = self.shared.state.lock().unwrap();
        let session_id = session.id();
        state.sessions.insert(session_id, session.downgrade());
        log::trace!("<-> session {} (room {})", session.id(), self.id());
    }

    /// Remove a session from this room.
    pub fn remove_session(&self, session_id: SessionId) {
        let mut state = self.shared.state.lock().unwrap();
        state.sessions.remove(&session_id).unwrap();
        log::trace!("</> session {} (room {})", session_id, self.id());
    }

    /// Announce a new producer to all sessions in this room.
    pub fn announce_producer(&self, producer_id: ProducerId) {
        let _ = self
            .shared
            .channel_tx
            .send(Message::ProducerAvailable(producer_id));
    }
    /// Announce a new data producer to all sessions in this room.
    pub fn announce_data_producer(&self, data_producer_id: DataProducerId) {
        let _ = self
            .shared
            .channel_tx
            .send(Message::DataProducerAvailable(data_producer_id));
    }

    /// Get a stream which yields existing and new producers.
    pub fn available_producers(&self) -> impl Stream<Item = ProducerId> {
        let producers = self
            .active_sessions() // ignore dropped sessions
            .into_iter()
            .flat_map(|session| session.get_producers())
            .filter(|producer| !producer.closed()) // ignore closed producers
            .map(|producer| producer.id())
            .collect::<Vec<ProducerId>>();
        stream::select(
            stream::iter(producers),
            self.channel_stream().filter_map(|x| async move {
                match x {
                    Message::ProducerAvailable(producer_id) => Some(producer_id),
                    _ => None,
                }
            }),
        )
    }
    /// Get a stream which yields existing and new data producers.
    pub fn available_data_producers(&self) -> impl Stream<Item = DataProducerId> {
        let data_producers = self
            .active_sessions() // ignore dropped sessions
            .into_iter()
            .flat_map(|session| session.get_data_producers())
            .filter(|data_producer| !data_producer.closed()) // ignore closed data producers
            .map(|data_producer| data_producer.id())
            .collect::<Vec<DataProducerId>>();
        stream::select(
            stream::iter(data_producers),
            self.channel_stream().filter_map(|x| async move {
                match x {
                    Message::DataProducerAvailable(data_producer_id) => Some(data_producer_id),
                    _ => None,
                }
            }),
        )
    }

    fn active_sessions(&self) -> Vec<Session> {
        let state = self.shared.state.lock().unwrap();
        state
            .sessions
            .values()
            .filter_map(|weak_session| weak_session.upgrade())
            .collect()
    }
    fn channel_stream(&self) -> impl Stream<Item = Message> {
        BroadcastStream::new(self.shared.channel_tx.subscribe())
            .take_while(|x| future::ready(x.is_ok()))
            .map(|x| x.unwrap())
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
        log::trace!("-room {}", self.id)
    }
}
