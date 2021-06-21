use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use uuid::Uuid;

use anyhow::{anyhow, Result};
use derive_more::Display;
use mediasoup::{
    consumer::{Consumer, ConsumerId, ConsumerOptions},
    data_consumer::{DataConsumer, DataConsumerId, DataConsumerOptions},
    data_producer::{DataProducer, DataProducerId, DataProducerOptions},
    data_structures::{DtlsParameters, TransportListenIp},
    plain_transport::{PlainTransport, PlainTransportOptions},
    producer::{Producer, ProducerId, ProducerOptions},
    rtp_parameters::{MediaKind, RtpCapabilities, RtpParameters},
    sctp_parameters::SctpStreamParameters,
    transport::{Transport, TransportId},
    webrtc_transport::{
        TransportListenIps, WebRtcTransport, WebRtcTransportOptions,
        WebRtcTransportRemoteParameters,
    },
};

use crate::relay_server::SessionOptions;
use crate::room::Room;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Display, Hash, Default)]
pub struct SessionId(Uuid);
impl SessionId {
    pub fn new() -> Self {
        SessionId(Uuid::new_v4())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Session {
    shared: Arc<Shared>,
}

#[derive(Clone)]
pub struct WeakSession {
    shared: Weak<Shared>,
}

struct Shared {
    state: Mutex<State>,

    id: SessionId,
    room: Room,

    session_options: SessionOptions,
    transport_listen_ip: TransportListenIp,
}
impl PartialEq for Shared {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Shared {}

struct State {
    client_rtp_capabilities: Option<RtpCapabilities>,
    consumers: HashMap<ConsumerId, Consumer>,
    producers: HashMap<ProducerId, Producer>,
    data_consumers: HashMap<DataConsumerId, DataConsumer>,
    data_producers: HashMap<DataProducerId, DataProducer>,
    webrtc_transports: HashMap<TransportId, WebRtcTransport>,
    plain_transports: HashMap<TransportId, PlainTransport>,
}

impl Session {
    pub fn new(
        room: Room,
        session_options: SessionOptions,
        transport_listen_ip: TransportListenIp,
    ) -> Self {
        let id = SessionId::new();
        log::debug!("+session {}", id);
        let session = Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    client_rtp_capabilities: None,
                    consumers: HashMap::new(),
                    producers: HashMap::new(),
                    data_consumers: HashMap::new(),
                    data_producers: HashMap::new(),
                    webrtc_transports: HashMap::new(),
                    plain_transports: HashMap::new(),
                }),
                id,
                room: room.clone(),
                session_options,
                transport_listen_ip,
            }),
        };
        room.add_session(session.clone());
        session
    }

    /// Connect a local WebRTC transport with the remote transport.
    pub async fn connect_webrtc_transport(
        &self,
        id: TransportId,
        dtls_parameters: DtlsParameters,
    ) -> Result<TransportId> {
        let transport = self
            .get_webrtc_transport(id)
            .ok_or_else(|| anyhow!("transport does not exist"))?;

        transport
            .connect(WebRtcTransportRemoteParameters { dtls_parameters })
            .await?;
        log::debug!("<-> transport {} (session {})", transport.id(), self.id());
        Ok(transport.id())
    }

    /// Create a local consumer on the receive WebRTC transport.
    pub async fn consume(
        &self,
        local_pool: tokio_local::LocalPoolHandle,
        transport_id: TransportId,
        producer_id: ProducerId,
    ) -> Result<Consumer> {
        let transport = self
            .get_webrtc_transport(transport_id)
            .ok_or_else(|| anyhow!("transport does not exist"))?;
        // make sure client has provided rtp caps
        let rtp_capabilities = self
            .get_rtp_capabilities()
            .ok_or_else(|| anyhow!("missing rtp capabilities"))?;

        // initialize consumer as paused (recommended by mediasoup docs)
        let mut options = ConsumerOptions::new(producer_id, rtp_capabilities);
        options.paused = true;

        let consumer = local_pool
            .spawn_pinned(|| async move { transport.consume(options).await })
            .await
            .unwrap()?;

        log::debug!("+consumer {} (session {})", consumer.id(), self.id());
        self.add_consumer(consumer.clone());
        Ok(consumer)
    }

    /// Resume a local consumer.
    pub async fn consumer_resume(&self, consumer_id: ConsumerId) -> Result<()> {
        match self.get_consumer(consumer_id) {
            Some(consumer) => Ok(consumer.resume().await?),
            None => Err(anyhow!("consumer {} does not exist", consumer_id)),
        }
    }

    /// Create a local producer on the send WebRTC transport.
    pub async fn produce(
        &self,
        local_pool: tokio_local::LocalPoolHandle,
        transport_id: TransportId,
        kind: MediaKind,
        rtp_parameters: RtpParameters,
    ) -> Result<Producer> {
        let transport = self
            .get_webrtc_transport(transport_id)
            .ok_or_else(|| anyhow!("transport does not exist"))?;
        let producer = local_pool
            .spawn_pinned(move || async move {
                transport
                    .produce(ProducerOptions::new(kind, rtp_parameters))
                    .await
            })
            .await
            .unwrap()?;
        self.add_producer(producer.clone());

        log::debug!("+producer {} (session {})", producer.id(), self.id());

        Ok(producer)
    }

    pub async fn produce_plain(
        &self,
        local_pool: tokio_local::LocalPoolHandle,
        transport_id: TransportId,
        kind: MediaKind,
        rtp_parameters: RtpParameters,
    ) -> Result<Producer> {
        let transport = self
            .get_plain_transport(transport_id)
            .ok_or_else(|| anyhow!("plain transport does not exist"))?;

        let producer = local_pool
            .spawn_pinned(move || async move {
                transport
                    .produce(ProducerOptions::new(kind, rtp_parameters))
                    .await
            })
            .await
            .unwrap()?;
        self.add_producer(producer.clone());

        log::debug!(
            "+producer {} [plain] (session {})",
            producer.id(),
            self.id()
        );
        Ok(producer)
    }

    /// Create a local data consumer on the receive WebRTC transport.
    pub async fn consume_data(
        &self,
        local_pool: tokio_local::LocalPoolHandle,
        transport_id: TransportId,
        data_producer_id: DataProducerId,
    ) -> Result<DataConsumer> {
        let transport = self
            .get_webrtc_transport(transport_id)
            .ok_or_else(|| anyhow!("transport does not exist"))?;
        let options = DataConsumerOptions::new_sctp(data_producer_id);

        let data_consumer = local_pool
            .spawn_pinned(|| async move { transport.consume_data(options).await })
            .await
            .unwrap()?;

        log::debug!(
            "+data consumer {} (session {})",
            data_consumer.id(),
            self.id()
        );
        self.add_data_consumer(data_consumer.clone());
        Ok(data_consumer)
    }

    /// Create a local data producer on the send WebRTC transport.
    pub async fn produce_data(
        &self,
        local_pool: tokio_local::LocalPoolHandle,
        transport_id: TransportId,
        sctp_stream_parameters: SctpStreamParameters,
    ) -> Result<DataProducer> {
        let transport = self
            .get_webrtc_transport(transport_id)
            .ok_or_else(|| anyhow!("transport does not exist"))?;
        let data_producer = local_pool
            .spawn_pinned(move || async move {
                transport
                    .produce_data(DataProducerOptions::new_sctp(sctp_stream_parameters))
                    .await
            })
            .await
            .unwrap()?;

        self.add_data_producer(data_producer.clone());

        let room = self.get_room();
        room.announce_data_producer(data_producer.id());
        log::debug!(
            "+data producer {} (session {})",
            data_producer.id(),
            self.id()
        );

        Ok(data_producer)
    }

    pub fn id(&self) -> SessionId {
        self.shared.id
    }
    pub fn get_session_options(&self) -> SessionOptions {
        self.shared.session_options.clone()
    }
    pub fn get_room(&self) -> Room {
        self.shared.room.clone()
    }
    pub fn downgrade(&self) -> WeakSession {
        WeakSession {
            shared: Arc::downgrade(&self.shared),
        }
    }

    pub async fn create_webrtc_transport(&self) -> WebRtcTransport {
        let mut transport_options =
            WebRtcTransportOptions::new(TransportListenIps::new(self.shared.transport_listen_ip));
        transport_options.enable_sctp = true; // required for data channel
        let transport = self
            .shared
            .room
            .get_router()
            .await
            .create_webrtc_transport(transport_options)
            .await
            .unwrap();
        let mut state = self.shared.state.lock().unwrap();
        state
            .webrtc_transports
            .insert(transport.id(), transport.clone());
        log::debug!("+transport {} (session {})", transport.id(), self.id());
        transport
    }
    pub fn get_webrtc_transport(&self, id: TransportId) -> Option<WebRtcTransport> {
        let state = self.shared.state.lock().unwrap();
        state.webrtc_transports.get(&id).cloned()
    }
    pub async fn create_plain_transport(&self) -> PlainTransport {
        let mut plain_transport_options =
            PlainTransportOptions::new(self.shared.transport_listen_ip);
        plain_transport_options.comedia = true;
        let plain_transport = self
            .shared
            .room
            .get_router()
            .await
            .create_plain_transport(plain_transport_options)
            .await
            .unwrap();

        let mut state = self.shared.state.lock().unwrap();
        state
            .plain_transports
            .insert(plain_transport.id(), plain_transport.clone());
        log::debug!(
            "+transport {} [plain] (session {})",
            plain_transport.id(),
            self.id()
        );
        plain_transport
    }
    pub fn get_plain_transport(&self, id: TransportId) -> Option<PlainTransport> {
        let state = self.shared.state.lock().unwrap();
        state.plain_transports.get(&id).cloned()
    }

    pub fn set_rtp_capabilities(&self, rtp_capabilities: RtpCapabilities) {
        let mut state = self.shared.state.lock().unwrap();
        state.client_rtp_capabilities.replace(rtp_capabilities);
    }
    pub fn get_rtp_capabilities(&self) -> Option<RtpCapabilities> {
        let state = self.shared.state.lock().unwrap();
        state.client_rtp_capabilities.clone()
    }

    pub fn add_consumer(&self, consumer: Consumer) {
        let mut state = self.shared.state.lock().unwrap();
        state.consumers.insert(consumer.id(), consumer);
    }
    pub fn get_consumer(&self, id: ConsumerId) -> Option<Consumer> {
        let state = self.shared.state.lock().unwrap();
        state.consumers.get(&id).cloned()
    }

    pub fn add_producer(&self, producer: Producer) {
        let mut state = self.shared.state.lock().unwrap();
        self.get_room().announce_producer(producer.id());
        state.producers.insert(producer.id(), producer);
    }
    pub fn remove_producer(&self, producer: &Producer) {
        let mut state = self.shared.state.lock().unwrap();
        let _ = state.producers.remove(&producer.id()).unwrap();
    }
    pub fn get_producers(&self) -> Vec<Producer> {
        let state = self.shared.state.lock().unwrap();
        state.producers.values().cloned().collect::<Vec<Producer>>()
    }

    pub fn add_data_producer(&self, data_producer: DataProducer) {
        let mut state = self.shared.state.lock().unwrap();
        state
            .data_producers
            .insert(data_producer.id(), data_producer);
    }
    pub fn remove_data_producer(&self, data_producer: &DataProducer) {
        let mut state = self.shared.state.lock().unwrap();
        let _ = state.data_producers.remove(&data_producer.id()).unwrap();
    }
    pub fn get_data_producers(&self) -> Vec<DataProducer> {
        let state = self.shared.state.lock().unwrap();
        state
            .data_producers
            .values()
            .cloned()
            .collect::<Vec<DataProducer>>()
    }

    pub fn add_data_consumer(&self, data_consumer: DataConsumer) {
        let mut state = self.shared.state.lock().unwrap();
        state
            .data_consumers
            .insert(data_consumer.id(), data_consumer);
    }
}
impl WeakSession {
    pub fn upgrade(&self) -> Option<Session> {
        let shared = self.shared.upgrade()?;
        Some(Session { shared })
    }
}
impl Drop for Shared {
    fn drop(&mut self) {
        log::debug!("-session {}", self.id);
        self.room.remove_session(self.id);
    }
}
