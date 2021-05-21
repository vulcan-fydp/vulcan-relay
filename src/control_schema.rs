use async_graphql::{Context, EmptySubscription, Object, Result, Schema, SimpleObject, Union, ID};

#[derive(Default)]
pub struct QueryRoot;
#[Object]
impl QueryRoot {
    async fn dummy(&self, _ctx: &Context<'_>) -> Result<bool> {
        Ok(true)
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
        _ctx: &Context<'_>,
        _room_id: ID,
        _vulcast_session_id: ID,
    ) -> Result<RegisterRoomResult> {
        todo!();
    }
    /// Unregister a room with the given ID.
    /// This will also unregister all sessions associated with this room.
    async fn unregister_room(
        &self,
        _ctx: &Context<'_>,
        _room_id: ID,
    ) -> Result<UnregisterRoomResult> {
        todo!();
    }
    /// Register a Vulcast with the given session ID.
    /// This is intended to be done once, when the Vulcast is powered on.
    /// The session and corresponding token remains valid until unregistered.
    /// Vulcasts can present the returned token to connect to the Relay.
    async fn register_vulcast_session(
        &self,
        _ctx: &Context<'_>,
        _session_id: ID,
    ) -> Result<RegisterSessionResult> {
        todo!();
    }
    /// Register a web client session attached to a specific room, identifed by its room ID.
    /// The session and corresponding token remains valid until unregistered.
    /// Web clients can present the returned token to connect to the Relay,
    /// which will automatically place them in the correct room.
    async fn register_client_session(
        &self,
        _ctx: &Context<'_>,
        _room_id: ID,
        _session_id: ID,
    ) -> Result<RegisterSessionResult> {
        todo!();
    }
    /// Unregister a session by its session ID.
    /// This will also terminate all active connections made with this session.
    async fn unregister_session(
        &self,
        _ctx: &Context<'_>,
        _session_id: ID,
    ) -> Result<UnregisterSessionResult> {
        todo!();
    }
}

#[derive(SimpleObject)]
struct Room {
    id: ID,
}

#[derive(SimpleObject)]
struct Session {
    id: ID,
    access_token: ID,
}

/// The Vulcast is already in another room.
#[derive(SimpleObject)]
struct VulcastInRoomError {
    vulcast: Session,
    room: Room,
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

#[derive(Union)]
enum UnregisterRoomResult {
    Ok(Room),
    UnknownRoom(UnknownRoomError),
}

#[derive(Union)]
enum RegisterSessionResult {
    Ok(Session),
    UnknownRoom(UnknownRoomError),
    NonUniqueId(NonUniqueIdError),
}

#[derive(Union)]
enum UnregisterSessionResult {
    Ok(Session),
    UnknownSession(UnknownSessionError),
}

pub type ControlSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub fn schema() -> ControlSchema {
    ControlSchema::build(QueryRoot, MutationRoot, EmptySubscription).finish()
}
