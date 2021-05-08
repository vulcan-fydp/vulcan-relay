use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use anyhow::{anyhow, Result};
use mediasoup::consumer::{Consumer, ConsumerId, ConsumerOptions};
use mediasoup::data_structures::{DtlsParameters, TransportListenIp};
use mediasoup::producer::{Producer, ProducerId, ProducerOptions};
use mediasoup::rtp_parameters::RtpCapabilities;
use mediasoup::rtp_parameters::{MediaKind, RtpParameters};
use mediasoup::transport::{Transport, TransportId};
use mediasoup::webrtc_transport::{
    TransportListenIps, WebRtcTransport, WebRtcTransportOptions, WebRtcTransportRemoteParameters,
};

use crate::room::{Room, RoomId, WeakRoom};

pub type SessionId = Uuid;

#[derive(Debug, Clone)]
pub struct Session {
    shared: Arc<Shared>,
}

#[derive(Debug)]
struct Shared {
    state: Mutex<State>,

    id: SessionId,
    room: WeakRoom,
    transport: WebRtcTransport,
}

#[derive(Debug)]
struct State {
    client_rtp_capabilities: Option<RtpCapabilities>,
    consumers: HashMap<ConsumerId, Consumer>,
    producers: Vec<Producer>,
}

impl Session {
    pub async fn new(room: Room) -> Self {
        // TODO parameterize this
        let transport_options =
            WebRtcTransportOptions::new(TransportListenIps::new(TransportListenIp {
                ip: "0.0.0.0".parse().unwrap(),
                announced_ip: "192.168.140.136".parse().ok(),
            }));

        let router = room.get_router();
        let transport = router
            .create_webrtc_transport(transport_options)
            .await
            .unwrap();

        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    client_rtp_capabilities: None,
                    consumers: HashMap::new(),
                    producers: vec![],
                }),
                id: Uuid::new_v4(),
                room: room.downgrade(),
                transport: transport,
            }),
        }
    }

    pub async fn connect_transport(&self, dtls_parameters: DtlsParameters) -> Result<TransportId> {
        let transport = self.get_transport();
        transport
            .connect(WebRtcTransportRemoteParameters {
                dtls_parameters: dtls_parameters,
            })
            .await?;
        log::info!(
            "connected transport {} from session {}",
            transport.id(),
            self.id()
        );
        Ok(transport.id())
    }

    pub async fn consume(
        &self,
        local_pool: tokio_local::LocalPoolHandle,
        producer_id: ProducerId,
    ) -> Result<Consumer> {
        let transport = self.get_transport();
        let rtp_capabilities = self
            .get_rtp_capabilities()
            .ok_or(anyhow!("missing rtp capabilities"))?;

        let mut options = ConsumerOptions::new(producer_id, rtp_capabilities);
        options.paused = true;

        let consumer = local_pool
            .spawn_pinned(|| async move { transport.consume(options).await })
            .await
            .unwrap()?;
        log::info!(
            "new consumer created {} for session {}",
            consumer.id(),
            self.id()
        );
        self.add_consumer(consumer.clone());
        Ok(consumer)
    }

    pub async fn consumer_resume(&self, consumer_id: ConsumerId) -> Result<()> {
        match self.get_consumer(consumer_id) {
            Some(consumer) => Ok(consumer.resume().await?),
            None => Err(anyhow!("consumer {} does not exist", consumer_id)),
        }
    }

    pub async fn produce(
        &self,
        local_pool: tokio_local::LocalPoolHandle,
        kind: MediaKind,
        rtp_parameters: RtpParameters,
    ) -> Result<Producer> {
        let room = self.get_room();

        // transport is async-trait with non-Send futures
        let transport = self.get_transport();
        let producer = local_pool
            .spawn_pinned(move || async move {
                transport
                    .produce(ProducerOptions::new(kind, rtp_parameters))
                    .await
            })
            .await
            .unwrap()?;

        let id = producer.id();
        self.add_producer(producer.clone());
        room.notify_new_producer(id);
        log::info!("new producer available {} from session {}", id, self.id());
        Ok(producer)
    }

    pub fn id(&self) -> SessionId {
        self.shared.id
    }
    pub fn get_room(&self) -> Room {
        self.shared.room.upgrade().unwrap()
    }
    pub fn get_transport(&self) -> WebRtcTransport {
        self.shared.transport.clone()
    }
    pub fn set_rtp_capabilities(&self, rtp_capabilities: RtpCapabilities) {
        let mut state = self.shared.state.lock().unwrap();
        state.client_rtp_capabilities.replace(rtp_capabilities);
    }
    pub fn get_rtp_capabilities(&self) -> Option<RtpCapabilities> {
        let state = self.shared.state.lock().unwrap();
        state.client_rtp_capabilities.clone()
    }
    pub fn add_producer(&self, producer: Producer) {
        let mut state = self.shared.state.lock().unwrap();
        state.producers.push(producer);
    }
    pub fn add_consumer(&self, consumer: Consumer) {
        let mut state = self.shared.state.lock().unwrap();
        state.consumers.insert(consumer.id(), consumer);
    }
    pub fn get_producers(&self) -> Vec<Producer> {
        let state = self.shared.state.lock().unwrap();
        state.producers.clone()
    }
    pub fn get_consumer(&self, id: ConsumerId) -> Option<Consumer> {
        let state = self.shared.state.lock().unwrap();
        state.consumers.get(&id).cloned()
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub enum Role {
    Vulcast,
    WebClient,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionToken {
    pub room_id: RoomId,
    pub role: Role,
}
