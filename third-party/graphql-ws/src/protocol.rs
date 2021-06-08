use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

use tokio_tungstenite::tungstenite::{protocol, Message};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientPayload {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "connection_init")]
    ConnectionInit {
        #[serde(skip_serializing_if = "Option::is_none")]
        payload: Option<serde_json::Value>,
    },

    #[serde(rename = "start")]
    Start { id: String, payload: ClientPayload },

    #[serde(rename = "stop")]
    Stop { id: String },

    #[serde(rename = "connection_terminate")]
    ConnectionTerminate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    #[serde(rename = "error")]
    ConnectionError { payload: serde_json::Value },

    #[serde(rename = "connection_ack")]
    ConnectionAck,

    #[serde(rename = "data")]
    Data {
        id: String,
        payload: serde_json::Value,
    },

    #[serde(rename = "error")]
    Error {
        id: String,
        payload: serde_json::Value,
    },

    #[serde(rename = "complete")]
    Complete { id: String },

    #[serde(rename = "ka")]
    ConnectionKeepAlive,
}

impl ServerMessage {
    pub fn id(&self) -> Option<&str> {
        match self {
            ServerMessage::Data { id, .. } => Some(&id),
            ServerMessage::Error { id, .. } => Some(&id),
            ServerMessage::Complete { id } => Some(&id),
            _ => None,
        }
    }
}

impl From<ClientMessage> for protocol::Message {
    fn from(message: ClientMessage) -> Self {
        Message::Text(serde_json::to_string(&message).unwrap())
    }
}

#[derive(Debug)]
pub enum MessageError {
    Decoding(serde_json::Error),
    InvalidMessage(protocol::Message),
    WebSocket(tokio_tungstenite::tungstenite::Error),
}

impl TryFrom<protocol::Message> for ServerMessage {
    type Error = MessageError;

    fn try_from(value: protocol::Message) -> Result<Self, MessageError> {
        match value {
            Message::Text(value) => {
                serde_json::from_str(&value).map_err(|e| MessageError::Decoding(e))
            }
            _ => Err(MessageError::InvalidMessage(value)),
        }
    }
}
