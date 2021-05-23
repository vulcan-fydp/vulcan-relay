use uuid::Uuid;

use vulcan_relay::relay_server::{
    ForeignRoomId, ForeignSessionId, RegisterRoomError, RegisterSessionError, SessionOptions,
    SessionToken, UnregisterRoomError, UnregisterSessionError,
};

pub mod fixture;

#[tokio::test]
async fn invalid_session_token_is_rejected() {
    let relay_server = fixture::relay_server().await;
    assert!(relay_server
        .session_from_token(SessionToken(Uuid::nil()))
        .is_none());
}

#[tokio::test]
async fn register_unknown_fails() {
    let relay_server = fixture::relay_server().await;

    // register client session to unknown room
    assert_eq!(
        relay_server.register_session(
            ForeignSessionId("client".into()),
            SessionOptions::WebClient(ForeignRoomId("unknownroom".into())),
        ),
        Err(RegisterSessionError::UnknownRoom(ForeignRoomId(
            "unknownroom".into()
        )))
    );

    // register room to unknown vulcast
    assert_eq!(
        relay_server.register_room(
            ForeignRoomId("room".into()),
            ForeignSessionId("unknownsession".into()),
        ),
        Err(RegisterRoomError::UnknownSession(ForeignSessionId(
            "unknownsession".into()
        )))
    );

    // unregister unknown room
    assert_eq!(
        relay_server.unregister_room(ForeignRoomId("unknownroom".into()),),
        Err(UnregisterRoomError::UnknownRoom(ForeignRoomId(
            "unknownroom".into()
        )))
    );

    // unregister unknown session
    assert_eq!(
        relay_server.unregister_session(ForeignSessionId("unknownsession".into()),),
        Err(UnregisterSessionError::UnknownSession(ForeignSessionId(
            "unknownsession".into()
        )))
    );
}

#[tokio::test]
async fn registration_must_be_unique() {
    let relay_server = fixture::relay_server().await;

    // register session
    assert!(matches!(
        relay_server.register_session(ForeignSessionId("vulcast".into()), SessionOptions::Vulcast,),
        Ok(SessionToken(_))
    ));
    // register existing session
    assert_eq!(
        relay_server.register_session(ForeignSessionId("vulcast".into()), SessionOptions::Vulcast,),
        Err(RegisterSessionError::NonUniqueId(ForeignSessionId(
            "vulcast".into()
        )))
    );
    // unregister session
    assert_eq!(
        relay_server.unregister_session(ForeignSessionId("vulcast".into())),
        Ok(())
    );

    // register session again
    assert!(matches!(
        relay_server.register_session(ForeignSessionId("vulcast".into()), SessionOptions::Vulcast,),
        Ok(SessionToken(_))
    ));
}

#[tokio::test]
async fn maximum_one_room_per_vulcast() {
    let relay_server = fixture::relay_server().await;

    // register session
    assert!(matches!(
        relay_server.register_session(ForeignSessionId("vulcast".into()), SessionOptions::Vulcast,),
        Ok(SessionToken(_))
    ));
    // register room
    assert_eq!(
        relay_server.register_room(
            ForeignRoomId("room".into()),
            ForeignSessionId("vulcast".into())
        ),
        Ok(())
    );
    // register room to vulcast already in another room
    assert_eq!(
        relay_server.register_room(
            ForeignRoomId("room2".into()),
            ForeignSessionId("vulcast".into())
        ),
        Err(RegisterRoomError::VulcastInRoom(ForeignSessionId(
            "vulcast".into()
        )))
    );
    // unregister room
    assert_eq!(
        relay_server.unregister_room(ForeignRoomId("room".into()),),
        Ok(())
    );
    // register room to vulcast again
    assert_eq!(
        relay_server.register_room(
            ForeignRoomId("room2".into()),
            ForeignSessionId("vulcast".into())
        ),
        Ok(())
    );
}
