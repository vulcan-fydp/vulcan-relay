use futures::stream::StreamExt;

use mediasoup::{rtp_parameters::MediaKind, transport::Transport};

use vulcan_relay::relay_server::{ForeignRoomId, ForeignSessionId, SessionOptions};

pub mod fixture;

// TODO malformed data tests

#[tokio::test]
async fn producer_consumer_connected_after_signalling() {
    let relay_server = fixture::relay_server().await;

    let foreign_room_id = ForeignRoomId("ayush".into());
    let vulcast_session_id = ForeignSessionId("vulcast".into());

    let vulcast = relay_server
        .session_from_token(
            relay_server
                .register_session(vulcast_session_id.clone(), SessionOptions::Vulcast)
                .unwrap(),
        )
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
        .unwrap();

    let vulcast_send_transport = vulcast.create_webrtc_transport().await;
    let vulcast_recv_transport = vulcast.create_webrtc_transport().await;

    let webclient_send_transport = webclient.create_webrtc_transport().await;
    let webclient_recv_transport = webclient.create_webrtc_transport().await;

    vulcast.set_rtp_capabilities(fixture::consumer_device_capabilities());
    webclient.set_rtp_capabilities(fixture::consumer_device_capabilities());

    vulcast
        .connect_webrtc_transport(vulcast_send_transport.id(), fixture::dtls_parameters())
        .await
        .unwrap();
    vulcast
        .connect_webrtc_transport(vulcast_recv_transport.id(), fixture::dtls_parameters())
        .await
        .unwrap();

    webclient
        .connect_webrtc_transport(webclient_send_transport.id(), fixture::dtls_parameters())
        .await
        .unwrap();
    webclient
        .connect_webrtc_transport(webclient_recv_transport.id(), fixture::dtls_parameters())
        .await
        .unwrap();

    let room = vulcast.get_room();

    let producer_stream = room.available_producers();
    let data_producer_stream = room.available_data_producers();
    tokio::pin!(producer_stream);
    tokio::pin!(data_producer_stream);

    let _audio_producer = vulcast
        .produce(
            vulcast_send_transport.id(),
            MediaKind::Audio,
            fixture::audio_producer_device_parameters(),
        )
        .await
        .unwrap();
    let _video_producer = vulcast
        .produce(
            vulcast_send_transport.id(),
            MediaKind::Video,
            fixture::video_producer_device_parameters(),
        )
        .await
        .unwrap();

    let _data_producer = webclient
        .produce_data(
            webclient_send_transport.id(),
            fixture::sctp_stream_parameters(),
        )
        .await
        .unwrap();

    let producer_id1 = producer_stream.next().await.unwrap();
    let producer_id2 = producer_stream.next().await.unwrap();

    let _consumer1 = webclient
        .consume(webclient_recv_transport.id(), producer_id1)
        .await
        .unwrap();

    let _consumer2 = webclient
        .consume(webclient_recv_transport.id(), producer_id2)
        .await
        .unwrap();

    let data_producer_id1 = data_producer_stream.next().await.unwrap();

    let _data_consumer1 = vulcast
        .consume_data(vulcast_recv_transport.id(), data_producer_id1)
        .await
        .unwrap();
}
