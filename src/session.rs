use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use mediasoup::consumer::{Consumer, ConsumerId};
use mediasoup::data_structures::TransportListenIp;
use mediasoup::producer::Producer;
use mediasoup::rtp_parameters::RtpCapabilities;
use mediasoup::webrtc_transport::{TransportListenIps, WebRtcTransport, WebRtcTransportOptions};

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
                room: WeakRoom::from(room),
                transport: transport,
            }),
        }
    }

    pub fn id(&self) -> SessionId {
        self.shared.id
    }
    pub fn get_room(&self) -> Room {
        Room::from(self.shared.room.clone())
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
