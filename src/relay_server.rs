use std::collections::HashMap;
use std::error;
use std::sync::{Arc, Mutex};

use mediasoup::worker::{Worker, WorkerSettings};
use mediasoup::worker_manager::WorkerManager;

use crate::room::{Room, RoomId};
use crate::session::{Session, SessionToken};

type Result<T> = std::result::Result<T, Box<dyn error::Error>>;

#[derive(Debug, Clone)]
pub struct RelayServer {
    shared: Arc<Shared>,
}

#[derive(Debug)]
struct Shared {
    state: Mutex<State>,

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
        // https://github.com/rust-lang/rust/issues/57478
        {
            let mut state = self.shared.state.lock().unwrap();
            if let Some(room) = state.rooms.get_mut(&room_id) {
                return room.clone();
            }
        }
        let room = Room::new(room_id.clone(), self.shared.worker.clone()).await;
        log::info!("created new room {}", &room_id);
        let mut state = self.shared.state.lock().unwrap();
        state.rooms.insert(room_id, room.clone());
        room
    }

    pub fn remove_room(&self, room: &Room) {
        let mut state = self.shared.state.lock().unwrap();
        state.rooms.remove(&room.id()).expect("room does not exist");
        log::info!("ended empty room {}", room.id());
    }

    pub async fn new_session(&self, token: SessionToken) -> Result<Session> {
        let room = self.get_or_create_room(token.room_id).await;
        let session = Session::new(room.clone()).await;
        log::info!("created new session {}", session.id());
        room.add_session(session.clone(), token.role)?;
        Ok(session)
    }

    pub fn end_session(&self, session: &Session) {
        let room = session.get_room();
        room.remove_session(session);
        if room.is_empty() {
            self.remove_room(&room);
        }
        log::info!("ended session {}", session.id());
    }
}
