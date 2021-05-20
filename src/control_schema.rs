use async_graphql::{Context, EmptySubscription, Object, Result, Schema, SimpleObject, Union, ID};

#[derive(Default)]
pub struct QueryRoot;
#[Object]
impl QueryRoot {
    async fn dummy(&self, ctx: &Context<'_>) -> Result<bool> {
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
        ctx: &Context<'_>,
        room_id: ID,
        vulcast_session_id: ID,
    ) -> Result<RegisterRoomResult> {
        todo!();
    }
    /// Unregister a room with the given ID.
    /// This will also unregister all sessions associated with this room.
    async fn unregister_room(
        &self,
        ctx: &Context<'_>,
        room_id: ID,
    ) -> Result<UnregisterRoomResult> {
        todo!();
    }
    /// Register a Vulcast with the given session ID.
    /// This is intended to be done once, when the Vulcast is powered on.
    /// The session ID remains valid until unregistered.
    /// The Vulcast will be able to connect to this relay with the session ID.
    async fn register_vulcast_session(
        &self,
        ctx: &Context<'_>,
        session_id: ID,
    ) -> Result<RegisterSessionResult> {
        todo!();
    }
    /// Register a web client session attached to a specific room, identifed by its room ID.
    /// Web clients can present this session ID to connect to the Relay,
    /// which will automatically place them in the correct room.
    /// The session ID remains valid until unregistered.
    async fn register_client_session(
        &self,
        ctx: &Context<'_>,
        room_id: ID,
        session_id: ID,
    ) -> Result<RegisterSessionResult> {
        todo!();
    }
    /// Unregister a session by its session ID.
    /// This will also terminate all active connections made with this session ID.
    async fn unregister_session(
        &self,
        ctx: &Context<'_>,
        session_id: ID,
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
