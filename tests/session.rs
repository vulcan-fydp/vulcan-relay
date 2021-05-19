use futures::stream::StreamExt;

use mediasoup::rtp_parameters::MediaKind;

use vulcan_relay::relay_server::RelayServer;
use vulcan_relay::session::{Role, SessionToken};

mod fixture;

// TODO malformed data tests

#[tokio::test]
async fn producer_consumer_connected_after_signalling() {
    let relay_server =
        RelayServer::new(fixture::local_transport_options(), fixture::media_codecs()).await;
    let local_pool = tokio_local::new_local_pool(2);

    let vulcast = relay_server
        .session_from_token(SessionToken {
            room_id: "ayush".into(),
            role: Role::Vulcast,
        })
        .await
        .unwrap();
    let webclient = relay_server
        .session_from_token(SessionToken {
            room_id: "ayush".into(),
            role: Role::WebClient,
        })
        .await
        .unwrap();

    vulcast.set_rtp_capabilities(fixture::consumer_device_capabilities());
    webclient.set_rtp_capabilities(fixture::consumer_device_capabilities());

    vulcast
        .connect_transport(vulcast.get_send_transport(), fixture::dtls_parameters())
        .await
        .unwrap();
    vulcast
        .connect_transport(vulcast.get_recv_transport(), fixture::dtls_parameters())
        .await
        .unwrap();

    webclient
        .connect_transport(webclient.get_send_transport(), fixture::dtls_parameters())
        .await
        .unwrap();
    webclient
        .connect_transport(webclient.get_recv_transport(), fixture::dtls_parameters())
        .await
        .unwrap();

    let room = vulcast.get_room();

    let mut producer_stream = room.available_producers();
    let mut data_producer_stream = room.available_data_producers();

    let _audio_producer = vulcast
        .produce(
            local_pool.clone(),
            MediaKind::Audio,
            fixture::audio_producer_device_parameters(),
        )
        .await
        .unwrap();
    let _video_producer = vulcast
        .produce(
            local_pool.clone(),
            MediaKind::Video,
            fixture::video_producer_device_parameters(),
        )
        .await
        .unwrap();

    let _data_producer = webclient
        .produce_data(local_pool.clone(), fixture::sctp_stream_parameters())
        .await
        .unwrap();

    let producer_id1 = producer_stream.next().await.unwrap();
    let producer_id2 = producer_stream.next().await.unwrap();

    let _consumer1 = webclient
        .consume(local_pool.clone(), producer_id1)
        .await
        .unwrap();

    let _consumer2 = webclient
        .consume(local_pool.clone(), producer_id2)
        .await
        .unwrap();

    let data_producer_id1 = data_producer_stream.next().await.unwrap();

    let _data_consumer1 = vulcast
        .consume_data(local_pool.clone(), data_producer_id1)
        .await
        .unwrap();
}

#[tokio::test]
async fn empty_room_is_dropped() {
    let relay_server =
        RelayServer::new(fixture::local_transport_options(), fixture::media_codecs()).await;
    let session1 = relay_server
        .session_from_token(SessionToken {
            room_id: "ayush".into(),
            role: Role::WebClient,
        })
        .await
        .unwrap();
    let session2 = relay_server
        .session_from_token(SessionToken {
            room_id: "ayush".into(),
            role: Role::Vulcast,
        })
        .await
        .unwrap();
    let weak_room = session1.get_room().downgrade();
    drop(session1);
    assert!(weak_room.upgrade().is_some());
    drop(session2);
    assert!(weak_room.upgrade().is_none());
}

#[tokio::test]
async fn multiple_vulcasts_same_room_rejected() {
    let relay_server =
        RelayServer::new(fixture::local_transport_options(), fixture::media_codecs()).await;
    let _session1 = relay_server
        .session_from_token(SessionToken {
            room_id: "ayush".into(),
            role: Role::Vulcast,
        })
        .await
        .unwrap();
    let session2 = relay_server
        .session_from_token(SessionToken {
            room_id: "ayush".into(),
            role: Role::Vulcast,
        })
        .await;
    assert!(session2.is_err());
}
