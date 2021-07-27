use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use bimap::BiMap;
use derive_more::Display;
use mediasoup::data_structures::TransportListenIp;
use mediasoup::{rtp_parameters::RtpCodecCapability, worker::Worker};
use thiserror::Error;

use crate::room::{Room, WeakRoom};
use crate::session::{Session, WeakSession};

#[derive(Clone)]
pub struct RelayServer {
    shared: Arc<Shared>,
}

struct Shared {
    state: Mutex<State>,

    transport_listen_ip: TransportListenIp,
    media_codecs: Vec<RtpCodecCapability>,
    worker: Worker,
}

struct State {
    /// 1-1 mapping of foreign session id to respective session token
    registered_sessions: BiMap<ForeignSessionId, SessionToken>,
    /// 1-1 mapping of foreign room id to foreign session id of bound vulcast
    registered_rooms: BiMap<ForeignRoomId, ForeignSessionId>,
    /// mapping of foreign session id to session options
    session_options: HashMap<ForeignSessionId, SessionOptions>,
    /// mapping of foreign session id of vulcast to corresponding room
    rooms: HashMap<ForeignSessionId, WeakRoom>,
    /// mapping of foreign session id to owning session
    sessions: HashMap<ForeignSessionId, Session>,
}

impl RelayServer {
    pub fn new(
        worker: Worker,
        transport_listen_ip: TransportListenIp,
        media_codecs: Vec<RtpCodecCapability>,
    ) -> Self {
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    registered_sessions: BiMap::new(),
                    registered_rooms: BiMap::new(),
                    session_options: HashMap::new(),
                    rooms: HashMap::new(),
                    sessions: HashMap::new(),
                }),
                media_codecs,
                transport_listen_ip,
                worker,
            }),
        }
    }

    /// Register a room with specified FRID, associated to a Vulcast by FSID.
    pub fn register_room(
        &self,
        frid: ForeignRoomId,
        vulcast_fsid: ForeignSessionId,
    ) -> Result<(), RegisterRoomError> {
        let mut state = self.shared.state.lock().unwrap();
        match state.session_options.get(&vulcast_fsid) {
            Some(SessionOptions::Vulcast) => {
                if state.registered_rooms.contains_left(&frid) {
                    Err(RegisterRoomError::NonUniqueId(frid))
                } else if state.registered_rooms.contains_right(&vulcast_fsid) {
                    Err(RegisterRoomError::VulcastInRoom(vulcast_fsid))
                } else {
                    log::trace!("+foreign room {} (vulcast fsid {})", &frid, &vulcast_fsid);
                    state
                        .registered_rooms
                        .insert_no_overwrite(frid, vulcast_fsid)
                        .unwrap();
                    Ok(())
                }
            }
            _ => Err(RegisterRoomError::UnknownSession(vulcast_fsid)),
        }
    }

    /// Unregister a room by FRID. This will also destroy all client sessions in the room (does not include Vulcast).
    pub fn unregister_room(&self, frid: ForeignRoomId) -> Result<(), UnregisterRoomError> {
        let mut state = self.shared.state.lock().unwrap();
        match state.registered_rooms.remove_by_left(&frid) {
            Some(_) => {
                drop(state);
                // nuke all client sessions in this room
                self.get_client_sessions_in_room(&frid)
                    .into_iter()
                    .for_each(|fsid| self.unregister_session(fsid).unwrap());
                log::trace!("-foreign room {}", frid);
                Ok(())
            }
            None => Err(UnregisterRoomError::UnknownRoom(frid)),
        }
    }

    /// Register a session with specified FSID. If the session is a WebClient,
    /// it will be associated to the provided FRID.
    pub fn register_session(
        &self,
        fsid: ForeignSessionId,
        session_options: SessionOptions,
    ) -> Result<SessionToken, RegisterSessionError> {
        let mut state = self.shared.state.lock().unwrap();
        let session_token = SessionToken::new();
        match &session_options {
            SessionOptions::WebClient(frid) | SessionOptions::Host(frid)
                if !state.registered_rooms.contains_left(frid) =>
            {
                Err(RegisterSessionError::UnknownRoom(frid.clone()))
            }
            _ => match state
                .registered_sessions
                .insert_no_overwrite(fsid.clone(), session_token)
            {
                Ok(_) => {
                    log::trace!("+foreign session {} [{:?}]", &fsid, session_options);
                    state.session_options.insert(fsid, session_options.clone());
                    Ok(session_token)
                }
                Err((fsid, _)) => Err(RegisterSessionError::NonUniqueId(fsid)),
            },
        }
    }

    /// Unregister a session by FSID. This will drop the PHY session.
    /// If the session belongs to a Vulcast, this will unregister the PHY room.
    pub fn unregister_session(&self, fsid: ForeignSessionId) -> Result<(), UnregisterSessionError> {
        let mut state = self.shared.state.lock().unwrap();
        // remove registration info
        match state.registered_sessions.remove_by_left(&fsid) {
            Some(_) => {
                let session_options = state.session_options.remove(&fsid).unwrap();
                match session_options {
                    SessionOptions::Vulcast => {
                        // if we are a vulcast in a room, also nuke the room
                        if let Some(frid) = state.registered_rooms.get_by_right(&fsid).cloned() {
                            drop(state);
                            self.unregister_room(frid).unwrap();
                        }
                    }
                    SessionOptions::WebClient(_) | SessionOptions::Host(_) => {
                        drop(state);
                    }
                }
                // nuke any active connections by dropping phy session
                drop(self.take_session(&fsid));
                log::trace!("-foreign session {} [{:?}]", &fsid, session_options);
                Ok(())
            }
            None => Err(UnregisterSessionError::UnknownSession(fsid)),
        }
    }

    /// Get a reference to a PHY session by FSID. You MUST drop this reference
    /// after you are done with it.
    pub fn get_session(&self, fsid: &ForeignSessionId) -> Option<Session> {
        let state = self.shared.state.lock().unwrap();
        state.sessions.get(fsid).cloned()
    }

    /// Take ownership of PHY session by FSID.
    pub fn take_session(&self, fsid: &ForeignSessionId) -> Option<Session> {
        let mut state = self.shared.state.lock().unwrap();
        state.sessions.remove(fsid)
    }

    /// Take ownership of PHY session by session token.
    pub fn take_session_by_token(&self, token: &SessionToken) -> Option<Session> {
        let mut state = self.shared.state.lock().unwrap();
        state
            .registered_sessions
            .get_by_right(token)
            .cloned()
            .and_then(|fsid| state.sessions.remove(&fsid))
    }

    /// Create PHY session from session token, obtained via registration.
    pub fn session_from_token(&self, token: SessionToken) -> Option<WeakSession> {
        let mut state = self.shared.state.lock().unwrap();

        // find fsid corresponding to this session token
        let foreign_session_id = state.registered_sessions.get_by_right(&token)?.clone();
        let session_options = state
            .session_options
            .get(&foreign_session_id)
            .cloned()
            .unwrap();

        // drop existing session if exists
        state.sessions.remove(&foreign_session_id);

        // find vulcast fsid of the room this session should connect to
        let vulcast_fsid = match &session_options {
            SessionOptions::Vulcast => foreign_session_id.clone(),
            SessionOptions::WebClient(frid) | SessionOptions::Host(frid) => {
                state.registered_rooms.get_by_left(frid).cloned().unwrap()
            }
        };

        // find/create the phy room corresponding to the vulcast fsid
        let room = state
            .rooms
            .get(&vulcast_fsid)
            .and_then(|weak_room| weak_room.upgrade())
            .unwrap_or_else(|| {
                Room::new(self.shared.worker.clone(), self.shared.media_codecs.clone())
            });
        state.rooms.insert(vulcast_fsid, room.downgrade()); // may re-insert

        // create and bind session to room
        let session = Session::new(room, session_options, self.shared.transport_listen_ip);

        // store owning session
        state.sessions.insert(foreign_session_id, session.clone());
        Some(session.downgrade())
    }

    /// Get all client sessions in the given room, specified by FRID.
    fn get_client_sessions_in_room(&self, frid: &ForeignRoomId) -> Vec<ForeignSessionId> {
        let state = self.shared.state.lock().unwrap();
        state
            .registered_sessions
            .iter()
            .filter_map(|(fsid, _)| {
                state
                    .session_options
                    .get(&fsid)
                    .filter(|session_options| match session_options {
                        SessionOptions::WebClient(client_frid) => client_frid == frid,
                        _ => false,
                    })
                    .and(Some(fsid))
            })
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Display, Hash)]
pub struct ForeignRoomId(pub String);
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Display, Hash)]
pub struct ForeignSessionId(pub String);

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Display,
    Default,
    Serialize,
    Deserialize,
)]
pub struct SessionToken(pub Uuid);
impl SessionToken {
    pub fn new() -> Self {
        SessionToken(Uuid::new_v4())
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SessionOptions {
    Vulcast,
    WebClient(ForeignRoomId),
    Host(ForeignRoomId),
}

#[derive(Debug, Error, PartialEq, Eq, PartialOrd, Ord)]
pub enum RegisterSessionError {
    #[error("the room `{0}` is not registered")]
    UnknownRoom(ForeignRoomId),
    #[error("the session id `{0}` is already taken")]
    NonUniqueId(ForeignSessionId),
}

#[derive(Debug, Error, PartialEq, Eq, PartialOrd, Ord)]
pub enum UnregisterSessionError {
    #[error("the session `{0}` is not registered")]
    UnknownSession(ForeignSessionId),
}

#[derive(Debug, Error, PartialEq, Eq, PartialOrd, Ord)]
pub enum RegisterRoomError {
    #[error("the session `{0}` is not registered")]
    UnknownSession(ForeignSessionId),
    #[error("the vulcast `{0}` is already in a room")]
    VulcastInRoom(ForeignSessionId),
    #[error("the room id `{0}` is already taken")]
    NonUniqueId(ForeignRoomId),
}

#[derive(Debug, Error, PartialEq, Eq, PartialOrd, Ord)]
pub enum UnregisterRoomError {
    #[error("the room `{0}` is not registered")]
    UnknownRoom(ForeignRoomId),
}
