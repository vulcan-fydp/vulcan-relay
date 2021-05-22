use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use bimap::BiMap;
use derive_more::Display;
use mediasoup::rtp_parameters::RtpCodecCapability;
use mediasoup::webrtc_transport::WebRtcTransportOptions;
use mediasoup::worker::Worker;
use thiserror::Error;

use crate::room::{Room, WeakRoom};
use crate::session::{Role, Session, WeakSession};

#[derive(Clone)]
pub struct RelayServer {
    shared: Arc<Shared>,
}

struct Shared {
    state: Mutex<State>,

    transport_options: WebRtcTransportOptions,
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
        transport_options: WebRtcTransportOptions,
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
                transport_options,
                worker,
            }),
        }
    }

    pub fn register_room(
        &self,
        room_id: ForeignRoomId,
        vulcast_session_id: ForeignSessionId,
    ) -> Result<(), RegisterRoomError> {
        let mut state = self.shared.state.lock().unwrap();
        match state.session_options.get(&vulcast_session_id) {
            Some(SessionOptions::Vulcast) => {
                match state
                    .registered_rooms
                    .insert_no_overwrite(room_id, vulcast_session_id)
                {
                    Ok(_) => Ok(()),
                    Err((_, vulcast_session_id)) => {
                        Err(RegisterRoomError::VulcastInRoom(vulcast_session_id))
                    }
                }
            }
            _ => Err(RegisterRoomError::UnknownSession(vulcast_session_id)),
        }
    }

    pub fn unregister_room(&self, room_id: ForeignRoomId) -> Result<(), UnregisterRoomError> {
        let mut state = self.shared.state.lock().unwrap();
        match state.registered_rooms.remove_by_left(&room_id) {
            Some(_) => Ok(()),
            None => Err(UnregisterRoomError::UnknownRoom(room_id)),
        }
        // TODO nuke all sessions in this room
    }

    pub fn register_session(
        &self,
        foreign_session_id: ForeignSessionId,
        session_options: SessionOptions,
    ) -> Result<SessionToken, RegisterSessionError> {
        let mut state = self.shared.state.lock().unwrap();
        let session_token = SessionToken::new();
        match &session_options {
            SessionOptions::WebClient(foreign_room_id)
                if !state.registered_rooms.contains_left(foreign_room_id) =>
            {
                Err(RegisterSessionError::UnknownRoom(foreign_room_id.clone()))
            }
            _ => match state
                .registered_sessions
                .insert_no_overwrite(foreign_session_id.clone(), session_token.clone())
            {
                Ok(_) => {
                    state
                        .session_options
                        .insert(foreign_session_id, session_options.clone());
                    Ok(session_token)
                }
                Err((foreign_session_id, _)) => {
                    Err(RegisterSessionError::NonUniqueId(foreign_session_id))
                }
            },
        }
    }

    pub fn unregister_session(
        &self,
        foreign_session_id: ForeignSessionId,
    ) -> Result<(), UnregisterSessionError> {
        let mut state = self.shared.state.lock().unwrap();
        // remove registration info
        match state
            .registered_sessions
            .remove_by_left(&foreign_session_id)
        {
            Some(_) => {
                state.session_options.remove(&foreign_session_id).unwrap();
                // nuke any active connections by dropping phy session
                state.sessions.remove(&foreign_session_id);
                // TODO what if we unregistered a vulcast with other people in the room...
                Ok(())
            }
            None => Err(UnregisterSessionError::UnknownSession(foreign_session_id)),
        }
    }

    pub fn session_from_token(&self, token: SessionToken) -> Option<WeakSession> {
        let mut state = self.shared.state.lock().unwrap();

        // find fsid corresponding to this session token
        let foreign_session_id = state.registered_sessions.get_by_right(&token)?.clone();
        let session_options = state
            .session_options
            .get(&foreign_session_id)
            .cloned()
            .unwrap();

        // find vulcast fsid of the room this session should connect to
        let vulcast_session_id = match &session_options {
            SessionOptions::Vulcast => foreign_session_id.clone(),
            SessionOptions::WebClient(foreign_room_id) => state
                .registered_rooms
                .get_by_left(foreign_room_id)
                .cloned()
                .unwrap(),
        };

        // find/create the phy room corresponding to the vulcast fsid
        let room = state
            .rooms
            .get(&vulcast_session_id)
            .and_then(|weak_room| weak_room.upgrade())
            .unwrap_or_else(|| {
                Room::new(self.shared.worker.clone(), self.shared.media_codecs.clone())
            });
        state
            .rooms
            .insert(vulcast_session_id.clone(), room.downgrade()); // may re-insert

        // create and bind session to room
        let session = Session::new(
            room.clone(),
            Role::from(session_options.clone()),
            self.shared.transport_options.clone(),
        );
        room.add_session(session.clone());

        // store owning session, dropping any existing session
        state.sessions.insert(foreign_session_id, session.clone());
        Some(session.downgrade())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Display, Hash)]
pub struct ForeignRoomId(pub String);
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Display, Hash)]
pub struct ForeignSessionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Display)]
pub struct SessionToken(pub Uuid);
impl SessionToken {
    pub fn new() -> Self {
        SessionToken(Uuid::new_v4())
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SessionOptions {
    Vulcast,
    WebClient(ForeignRoomId),
}
impl From<SessionOptions> for Role {
    fn from(session_options: SessionOptions) -> Self {
        match session_options {
            SessionOptions::Vulcast => Role::Vulcast,
            SessionOptions::WebClient(_) => Role::WebClient,
        }
    }
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
