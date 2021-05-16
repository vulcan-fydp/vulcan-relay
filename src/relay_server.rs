use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use mediasoup::rtp_parameters::RtpCodecCapability;
use mediasoup::webrtc_transport::WebRtcTransportOptions;
use mediasoup::worker::{Worker, WorkerSettings};
use mediasoup::worker_manager::WorkerManager;
use tokio::sync::Mutex;

use crate::room::{Room, RoomId, WeakRoom};
use crate::session::{Session, SessionToken};

#[derive(Clone)]
pub struct RelayServer {
    shared: Arc<Shared>,
}

struct Shared {
    state: Mutex<State>, // async mutex

    transport_options: WebRtcTransportOptions,
    media_codecs: Vec<RtpCodecCapability>,
    worker: Worker,
}

struct State {
    rooms: HashMap<RoomId, WeakRoom>,
}

impl RelayServer {
    pub async fn new(
        transport_options: WebRtcTransportOptions,
        media_codecs: Vec<RtpCodecCapability>,
    ) -> Self {
        let worker_manager = WorkerManager::new();
        let worker = worker_manager
            .create_worker(WorkerSettings::default())
            .await
            .unwrap();
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    rooms: HashMap::new(),
                }),
                media_codecs: media_codecs,
                transport_options: transport_options,
                worker: worker,
            }),
        }
    }

    pub async fn get_or_create_room(&self, room_id: RoomId) -> Room {
        let mut state = self.shared.state.lock().await;
        if let Some(room) = state
            .rooms
            .get(&room_id)
            .and_then(|weak_room| weak_room.upgrade())
        {
            return room;
        }

        log::debug!("created new room {}", &room_id);
        let room = Room::new(
            room_id.clone(),
            self.shared.worker.clone(),
            self.shared.media_codecs.clone(),
        )
        .await;
        state.rooms.insert(room_id, room.downgrade());
        room
    }

    pub async fn session_from_token(&self, token: SessionToken) -> Result<Session> {
        let room = self.get_or_create_room(token.room_id).await;
        let session = Session::new(
            room.clone(),
            token.role,
            self.shared.transport_options.clone(),
        )
        .await;

        room.add_session(session.clone())?;

        log::debug!("created new session {}", session.id());
        Ok(session)
    }
}
