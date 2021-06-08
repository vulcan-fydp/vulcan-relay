use std::io::{Read, Write};

use graphql_client::{GraphQLQuery, Response};
use serde::{Deserialize, Serialize};
use tungstenite::{Message, WebSocket};

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

/// My shitty synchronous graphql client over WS that panics on errors
pub struct GraphQLClient<Stream> {
    socket: WebSocket<Stream>,
    id: u32,
}

impl<Stream: Read + Write> GraphQLClient<Stream> {
    pub fn new(socket: WebSocket<Stream>, payload: Option<serde_json::Value>) -> Self {
        let mut client = Self { socket, id: 0 };
        client.send(ClientMessage::ConnectionInit { payload });
        assert!(matches!(client.recv(), ServerMessage::ConnectionAck));
        client
    }

    pub fn query<Query: GraphQLQuery>(
        &mut self,
        variables: Query::Variables,
    ) -> Query::ResponseData {
        let query = Query::build_query(variables);
        self.send(ClientMessage::Start {
            id: self.id.to_string(),
            payload: ClientPayload {
                query: query.query.to_owned(),
                operation_name: Some(query.operation_name.to_owned()),
                variables: Some(serde_json::to_value(query.variables).unwrap()),
            },
        });
        let data = {
            match self.recv() {
                ServerMessage::Data { id, payload } => {
                    assert_eq!(id, self.id.to_string());
                    let response: Response<Query::ResponseData> =
                        serde_json::from_value(payload).unwrap();
                    match self.recv() {
                        ServerMessage::Complete { id } => {
                            assert_eq!(id, self.id.to_string());
                            assert_eq!(response.errors, None);
                            response.data.unwrap()
                        }
                        _ => panic!(),
                    }
                }
                _ => panic!(),
            }
        };
        self.id += 1;
        data
    }

    fn send(&mut self, message: ClientMessage) {
        self.socket
            .write_message(Message::Text(serde_json::to_string(&message).unwrap()))
            .unwrap();
    }

    fn recv(&mut self) -> ServerMessage {
        serde_json::from_str::<ServerMessage>(
            self.socket.read_message().unwrap().to_text().unwrap(),
        )
        .unwrap()
    }
}
