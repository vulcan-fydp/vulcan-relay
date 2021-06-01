use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use uuid::Uuid;

use anyhow::{anyhow, Result};
use derive_more::Display;
use mediasoup::consumer::{Consumer, ConsumerId, ConsumerOptions};
use mediasoup::data_consumer::{DataConsumer, DataConsumerId, DataConsumerOptions};
use mediasoup::data_producer::{DataProducer, DataProducerId, DataProducerOptions};
use mediasoup::data_structures::DtlsParameters;
use mediasoup::producer::{Producer, ProducerId, ProducerOptions};
use mediasoup::rtp_parameters::RtpCapabilities;
use mediasoup::rtp_parameters::{MediaKind, RtpParameters};
use mediasoup::sctp_parameters::SctpStreamParameters;
use mediasoup::transport::{Transport, TransportId};
use mediasoup::webrtc_transport::{
    WebRtcTransport, WebRtcTransportOptions, WebRtcTransportRemoteParameters,
};
use tokio::sync::OnceCell;

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
    transport_options: WebRtcTransportOptions,
    room: Room,
    send_transport: OnceCell<WebRtcTransport>, // client -> server
    recv_transport: OnceCell<WebRtcTransport>, // server -> client
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
}

impl Session {
    pub fn new(room: Room, transport_options: WebRtcTransportOptions) -> Self {
        let id = SessionId::new();
        log::debug!("created new session {}", id);
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    client_rtp_capabilities: None,
                    consumers: HashMap::new(),
                    producers: HashMap::new(),
                    data_consumers: HashMap::new(),
                    data_producers: HashMap::new(),
                }),
                id,
                transport_options,
                room,
                send_transport: OnceCell::new(),
                recv_transport: OnceCell::new(),
            }),
        }
    }

    /// Connect the local send transport with remote.
    pub async fn connect_send_transport(
        &self,
        dtls_parameters: DtlsParameters,
    ) -> Result<TransportId> {
        self.connect_transport(self.get_send_transport().await, dtls_parameters)
            .await
    }
    /// Connect the local receive transport with remote.
    pub async fn connect_recv_transport(
        &self,
        dtls_parameters: DtlsParameters,
    ) -> Result<TransportId> {
        self.connect_transport(self.get_recv_transport().await, dtls_parameters)
            .await
    }
    /// Connect a local transport with remote.
    async fn connect_transport(
        &self,
        transport: WebRtcTransport,
        dtls_parameters: DtlsParameters,
    ) -> Result<TransportId> {
        transport
            .connect(WebRtcTransportRemoteParameters { dtls_parameters })
            .await?;
        log::debug!(
            "connected transport {} from session {}",
            transport.id(),
            self.id()
        );
        Ok(transport.id())
    }

    /// Create a local consumer.
    pub async fn consume(
        &self,
        local_pool: tokio_local::LocalPoolHandle,
        producer_id: ProducerId,
    ) -> Result<Consumer> {
        let transport = self.get_recv_transport().await;
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

        log::debug!(
            "new consumer created {} for session {}",
            consumer.id(),
            self.id()
        );
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

    /// Create a local producer.
    pub async fn produce(
        &self,
        local_pool: tokio_local::LocalPoolHandle,
        kind: MediaKind,
        rtp_parameters: RtpParameters,
    ) -> Result<Producer> {
        let transport = self.get_send_transport().await;
        let producer = local_pool
            .spawn_pinned(move || async move {
                transport
                    .produce(ProducerOptions::new(kind, rtp_parameters))
                    .await
            })
            .await
            .unwrap()?;

        self.add_producer(producer.clone());

        let room = self.get_room();
        room.announce_producer(producer.id());
        log::debug!(
            "new producer available {} for session {}",
            producer.id(),
            self.id()
        );

        Ok(producer)
    }

    /// Create a local data consumer.
    pub async fn consume_data(
        &self,
        local_pool: tokio_local::LocalPoolHandle,
        data_producer_id: DataProducerId,
    ) -> Result<DataConsumer> {
        let transport = self.get_recv_transport().await;
        let options = DataConsumerOptions::new_sctp(data_producer_id);

        let data_consumer = local_pool
            .spawn_pinned(|| async move { transport.consume_data(options).await })
            .await
            .unwrap()?;

        log::debug!(
            "new data consumer created {} for session {}",
            data_consumer.id(),
            self.id()
        );
        self.add_data_consumer(data_consumer.clone());
        Ok(data_consumer)
    }

    /// Create a local data producer.
    pub async fn produce_data(
        &self,
        local_pool: tokio_local::LocalPoolHandle,
        sctp_stream_parameters: SctpStreamParameters,
    ) -> Result<DataProducer> {
        let transport = self.get_send_transport().await;
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
            "new data producer available {} for session {}",
            data_producer.id(),
            self.id()
        );

        Ok(data_producer)
    }

    pub fn id(&self) -> SessionId {
        self.shared.id
    }
    pub fn get_room(&self) -> Room {
        self.shared.room.clone()
    }
    pub fn downgrade(&self) -> WeakSession {
        WeakSession {
            shared: Arc::downgrade(&self.shared),
        }
    }

    pub async fn get_send_transport(&self) -> WebRtcTransport {
        self.shared
            .send_transport
            .get_or_init(|| async {
                self.shared
                    .room
                    .get_router()
                    .await
                    .create_webrtc_transport(self.shared.transport_options.clone())
                    .await
                    .unwrap()
            })
            .await
            .clone()
    }
    pub async fn get_recv_transport(&self) -> WebRtcTransport {
        self.shared
            .recv_transport
            .get_or_init(|| async {
                self.shared
                    .room
                    .get_router()
                    .await
                    .create_webrtc_transport(self.shared.transport_options.clone())
                    .await
                    .unwrap()
            })
            .await
            .clone()
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
        log::debug!("dropped session {}", self.id);
        self.room.remove_session(self.id);
    }
}
