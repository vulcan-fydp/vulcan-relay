use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use mediasoup::worker::{Worker, WorkerSettings};
use mediasoup::worker_manager::WorkerManager;
use tokio::sync::Mutex;

use crate::room::{Room, RoomId};
use crate::session::{Session, SessionToken};

#[derive(Debug, Clone)]
pub struct RelayServer {
    shared: Arc<Shared>,
}

#[derive(Debug)]
struct Shared {
    state: Mutex<State>, // async mutex

    worker_manager: WorkerManager,
    worker: Worker,
}

#[derive(Debug)]
struct State {
    rooms: HashMap<RoomId, Room>,
}

impl RelayServer {
    pub async fn new() -> Self {
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
                worker: worker,
                worker_manager: worker_manager,
            }),
        }
    }

    pub async fn get_or_create_room(&self, room_id: RoomId) -> Room {
        let mut state = self.shared.state.lock().await;
        let room = state.rooms.entry(room_id.clone()).or_insert({
            log::info!("created new room {}", &room_id);
            Room::new(room_id, self.shared.worker.clone()).await // long critical section
        });
        room.clone()
    }

    pub async fn session_from_token(&self, token: SessionToken) -> Result<Session> {
        let room = self.get_or_create_room(token.room_id).await;
        let session = Session::new(room.clone()).await;
        log::info!("created new session {}", session.id());
        room.add_session(session.clone(), token.role)?;
        Ok(session)
    }

    pub async fn end_session(&self, session: &Session) {
        let mut state = self.shared.state.lock().await;
        let room = session.get_room();
        room.remove_session(session);
        log::info!("ended session {}", session.id());
        if room.is_empty() {
            state.rooms.remove(&room.id()).expect("room does not exist");
            log::info!("ended room {}", room.id());
        }
    }
}
