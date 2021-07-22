use anyhow::anyhow;
use async_graphql::{Context, EmptySubscription, Object, Schema, SimpleObject, Union, ID};

use crate::built_info;
use crate::relay_server::{
    ForeignRoomId, ForeignSessionId, RegisterRoomError, RegisterSessionError, RelayServer,
    SessionOptions, UnregisterRoomError, UnregisterSessionError,
};

#[derive(Default)]
pub struct QueryRoot;
#[Object]
impl QueryRoot {
    /// Get the version and build info of this relay instance.
    async fn version(&self, _ctx: &Context<'_>) -> String {
        format!(
            "{}_{}_{}_{}",
            built_info::PKG_NAME,
            built_info::PKG_VERSION,
            built_info::TARGET,
            built_info::PROFILE
        )
    }

    /// Get various statistics for a session.
    async fn stats(&self, ctx: &Context<'_>, session_id: ID) -> Result<String, anyhow::Error> {
        let relay_server = ctx.data_unchecked::<RelayServer>();
        let session = relay_server
            .get_session(&ForeignSessionId::from(session_id))
            .ok_or_else(|| anyhow!("unknown fsid"))?;
        Ok(serde_json::to_string(&session.get_stats().await?)?)
    }
}

#[derive(Default)]
pub struct MutationRoot;
#[Object]
impl MutationRoot {
    /// Register a room tied to a specific Vulcast, identified by its session ID.
    /// This will fail if the specified Vulcast is already tied to an existing room.
    async fn register_room(
        &self,
        ctx: &Context<'_>,
        room_id: ID,
        vulcast_session_id: ID,
    ) -> RegisterRoomResult {
        let relay_server = ctx.data_unchecked::<RelayServer>();
        match relay_server.register_room(
            ForeignRoomId::from(room_id.clone()),
            ForeignSessionId::from(vulcast_session_id),
        ) {
            Ok(_) => RegisterRoomResult::Ok(Room { id: room_id }),
            Err(err) => err.into(),
        }
    }
    /// Unregister a room with the given ID.
    /// This will also unregister all sessions associated with this room.
    async fn unregister_room(&self, ctx: &Context<'_>, room_id: ID) -> UnregisterRoomResult {
        let relay_server = ctx.data_unchecked::<RelayServer>();
        match relay_server.unregister_room(ForeignRoomId::from(room_id.clone())) {
            Ok(_) => UnregisterRoomResult::Ok(Room { id: room_id }),
            Err(err) => err.into(),
        }
    }
    /// Register a Vulcast with the given session ID.
    /// This is intended to be done once, when the Vulcast is powered on.
    /// The session and corresponding token remains valid until unregistered.
    /// Vulcasts can present the returned token to connect to the Relay.
    async fn register_vulcast_session(
        &self,
        ctx: &Context<'_>,
        session_id: ID,
    ) -> RegisterSessionResult {
        let relay_server = ctx.data_unchecked::<RelayServer>();
        match relay_server.register_session(
            ForeignSessionId::from(session_id.clone()),
            SessionOptions::Vulcast,
        ) {
            Ok(session_token) => RegisterSessionResult::Ok(SessionWithToken {
                id: session_id,
                access_token: session_token.into(),
            }),
            Err(err) => err.into(),
        }
    }
    /// Register a web client session attached to a specific room, identifed by its room ID.
    /// The session and corresponding token remains valid until unregistered.
    /// Web clients can present the returned token to connect to the Relay,
    /// which will automatically place them in the correct room.
    async fn register_client_session(
        &self,
        ctx: &Context<'_>,
        room_id: ID,
        session_id: ID,
    ) -> RegisterSessionResult {
        let relay_server = ctx.data_unchecked::<RelayServer>();
        match relay_server.register_session(
            ForeignSessionId::from(session_id.clone()),
            SessionOptions::WebClient(ForeignRoomId::from(room_id)),
        ) {
            Ok(session_token) => RegisterSessionResult::Ok(SessionWithToken {
                id: session_id,
                access_token: session_token.into(),
            }),
            Err(err) => err.into(),
        }
    }
    /// Register a host session attached to a specific room, identifed by its room ID.
    /// The session and corresponding token remains valid until unregistered.
    /// Hosts can present the returned token to connect to the Relay,
    /// which will automatically place them in the correct room.
    async fn register_host_session(
        &self,
        ctx: &Context<'_>,
        room_id: ID,
        session_id: ID,
    ) -> RegisterSessionResult {
        let relay_server = ctx.data_unchecked::<RelayServer>();
        match relay_server.register_session(
            ForeignSessionId::from(session_id.clone()),
            SessionOptions::Host(ForeignRoomId::from(room_id)),
        ) {
            Ok(session_token) => RegisterSessionResult::Ok(SessionWithToken {
                id: session_id,
                access_token: session_token.into(),
            }),
            Err(err) => err.into(),
        }
    }
    /// Unregister a session by its session ID.
    /// This will also terminate all active connections made with this session.
    async fn unregister_session(
        &self,
        ctx: &Context<'_>,
        session_id: ID,
    ) -> UnregisterSessionResult {
        let relay_server = ctx.data_unchecked::<RelayServer>();
        match relay_server.unregister_session(ForeignSessionId::from(session_id.clone())) {
            Ok(_) => UnregisterSessionResult::Ok(Session { id: session_id }),
            Err(err) => err.into(),
        }
    }
}

#[derive(SimpleObject)]
struct Room {
    id: ID,
}

#[derive(SimpleObject)]
struct Session {
    id: ID,
}

#[derive(SimpleObject)]
struct SessionWithToken {
    id: ID,
    access_token: ID,
}

/// The Vulcast is already in another room.
#[derive(SimpleObject)]
struct VulcastInRoomError {
    vulcast: Session,
}
/// The specified room does not exist.
#[derive(SimpleObject)]
struct UnknownRoomError {
    room: Room,
}
/// The specified session does not exist.
#[derive(SimpleObject)]
struct UnknownSessionError {
    session: Session,
}
/// The specified ID is not unique.
#[derive(SimpleObject)]
struct NonUniqueIdError {
    id: ID,
}

#[derive(Union)]
enum RegisterRoomResult {
    Ok(Room),
    VulcastInRoom(VulcastInRoomError),
    UnknownSession(UnknownSessionError),
    NonUniqueId(NonUniqueIdError),
}
impl From<RegisterRoomError> for RegisterRoomResult {
    fn from(err: RegisterRoomError) -> Self {
        match err {
            RegisterRoomError::NonUniqueId(foreign_room_id) => {
                RegisterRoomResult::NonUniqueId(NonUniqueIdError {
                    id: foreign_room_id.into(),
                })
            }
            RegisterRoomError::UnknownSession(foreign_session_id) => {
                RegisterRoomResult::UnknownSession(UnknownSessionError {
                    session: Session {
                        id: foreign_session_id.into(),
                    },
                })
            }
            RegisterRoomError::VulcastInRoom(foreign_session_id) => {
                RegisterRoomResult::VulcastInRoom(VulcastInRoomError {
                    vulcast: Session {
                        id: foreign_session_id.into(),
                    },
                })
            }
        }
    }
}

#[derive(Union)]
enum UnregisterRoomResult {
    Ok(Room),
    UnknownRoom(UnknownRoomError),
}
impl From<UnregisterRoomError> for UnregisterRoomResult {
    fn from(err: UnregisterRoomError) -> Self {
        match err {
            UnregisterRoomError::UnknownRoom(foreign_room_id) => {
                UnregisterRoomResult::UnknownRoom(UnknownRoomError {
                    room: Room {
                        id: foreign_room_id.into(),
                    },
                })
            }
        }
    }
}

#[derive(Union)]
enum RegisterSessionResult {
    Ok(SessionWithToken),
    UnknownRoom(UnknownRoomError),
    NonUniqueId(NonUniqueIdError),
}
impl From<RegisterSessionError> for RegisterSessionResult {
    fn from(err: RegisterSessionError) -> Self {
        match err {
            RegisterSessionError::NonUniqueId(foreign_session_id) => {
                RegisterSessionResult::NonUniqueId(NonUniqueIdError {
                    id: foreign_session_id.into(),
                })
            }
            RegisterSessionError::UnknownRoom(foreign_room_id) => {
                RegisterSessionResult::UnknownRoom(UnknownRoomError {
                    room: Room {
                        id: foreign_room_id.into(),
                    },
                })
            }
        }
    }
}

#[derive(Union)]
enum UnregisterSessionResult {
    Ok(Session),
    UnknownSession(UnknownSessionError),
}
impl From<UnregisterSessionError> for UnregisterSessionResult {
    fn from(err: UnregisterSessionError) -> Self {
        match err {
            UnregisterSessionError::UnknownSession(foreign_session_id) => {
                UnregisterSessionResult::UnknownSession(UnknownSessionError {
                    session: Session {
                        id: foreign_session_id.into(),
                    },
                })
            }
        }
    }
}

impl From<ID> for ForeignSessionId {
    fn from(id: ID) -> Self {
        Self(String::from(id))
    }
}

impl From<ID> for ForeignRoomId {
    fn from(id: ID) -> Self {
        Self(String::from(id))
    }
}

pub type ControlSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub fn schema(relay_server: RelayServer) -> ControlSchema {
    ControlSchema::build(QueryRoot, MutationRoot, EmptySubscription)
        .data(relay_server)
        .finish()
}
