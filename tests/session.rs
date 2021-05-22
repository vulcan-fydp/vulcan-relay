use futures::stream::StreamExt;

use mediasoup::rtp_parameters::MediaKind;

use vulcan_relay::relay_server::{ForeignRoomId, ForeignSessionId, SessionOptions};

pub mod fixture;

// TODO malformed data tests

#[tokio::test]
async fn producer_consumer_connected_after_signalling() {
    let relay_server = fixture::relay_server().await;
    let local_pool = tokio_local::new_local_pool(2);

    let foreign_room_id = ForeignRoomId("ayush".into());
    let vulcast_session_id = ForeignSessionId("vulcast".into());

    let vulcast = relay_server
        .session_from_token(
            relay_server
                .register_session(vulcast_session_id.clone(), SessionOptions::Vulcast)
                .unwrap(),
        )
        .unwrap()
        .upgrade()
        .unwrap();
    relay_server
        .register_room(foreign_room_id, vulcast_session_id)
        .unwrap();
    let webclient = relay_server
        .session_from_token(
            relay_server
                .register_session(
                    ForeignSessionId("webclient".into()),
                    SessionOptions::WebClient(ForeignRoomId("ayush".into())),
                )
                .unwrap(),
        )
        .unwrap()
        .upgrade()
        .unwrap();

    vulcast.set_rtp_capabilities(fixture::consumer_device_capabilities());
    webclient.set_rtp_capabilities(fixture::consumer_device_capabilities());

    vulcast
        .connect_send_transport(fixture::dtls_parameters())
        .await
        .unwrap();
    vulcast
        .connect_recv_transport(fixture::dtls_parameters())
        .await
        .unwrap();

    webclient
        .connect_send_transport(fixture::dtls_parameters())
        .await
        .unwrap();
    webclient
        .connect_recv_transport(fixture::dtls_parameters())
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
