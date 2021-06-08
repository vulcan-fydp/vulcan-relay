use futures::{SinkExt, Stream, StreamExt};
use std::convert::TryInto;
use std::marker::PhantomData;

use graphql_client::{GraphQLQuery, QueryBody, Response};
use tokio::{
    net::TcpStream,
    sync::{broadcast, mpsc},
};
use tokio_stream::wrappers::ReceiverStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::protocol::{ClientMessage, ClientPayload, ServerMessage};

mod protocol;

pub struct GraphQLWebSocket {
    id: u32,
    client_tx: broadcast::Sender<ClientMessage>,
    server_tx: broadcast::Sender<ServerMessage>,
}

impl GraphQLWebSocket {
    pub fn new() -> Self {
        Self {
            id: 0,
            client_tx: broadcast::channel(16).0,
            server_tx: broadcast::channel(16).0,
        }
    }

    pub fn connect(
        &mut self,
        socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
        payload: Option<serde_json::Value>,
    ) {
        let (mut write, read) = socket.split();
        let server_tx = self.server_tx.clone();
        tokio::spawn(async move {
            read.for_each(|message| async {
                let _ = server_tx.send(message.unwrap().try_into().unwrap()); // TODO error handling
            })
            .await;
        });
        let mut client_rx = self.client_tx.subscribe();
        tokio::spawn(async move {
            write
                .send(ClientMessage::ConnectionInit { payload }.into())
                .await?;
            while let Ok(message) = client_rx.recv().await {
                write.send(message.into()).await?;
            }
            // TODO do something with this error
            Ok::<(), tokio_tungstenite::tungstenite::Error>(())
        });
    }

    pub async fn query<Query: GraphQLQuery>(
        &mut self,
        variables: Query::Variables,
    ) -> Result<Response<Query::ResponseData>, serde_json::Value> {
        let op = self.subscribe::<Query>(variables);
        let mut stream = op.execute();
        stream.next().await.unwrap()
    }

    pub async fn query_unchecked<Query: GraphQLQuery>(
        &mut self,
        variables: Query::Variables,
    ) -> Query::ResponseData {
        let result = self.query::<Query>(variables).await;
        let response = result.unwrap();
        assert_eq!(response.errors, None);
        response.data.unwrap()
    }

    pub fn subscribe<Query: GraphQLQuery>(
        &mut self,
        variables: Query::Variables,
    ) -> GraphQLOperation<Query> {
        let op = GraphQLOperation::<Query>::new(
            self.id.to_string(),
            Query::build_query(variables),
            self.server_tx.clone(),
            self.client_tx.clone(),
        );
        self.id += 1;
        op
    }
}

pub struct GraphQLOperation<Query: GraphQLQuery> {
    id: String,
    payload: ClientPayload,
    server_tx: broadcast::Sender<ServerMessage>,
    client_tx: broadcast::Sender<ClientMessage>,
    _query: PhantomData<Query>,
}
impl<Query: GraphQLQuery> GraphQLOperation<Query> {
    pub fn new(
        id: String,
        query_body: QueryBody<Query::Variables>,
        server_tx: broadcast::Sender<ServerMessage>,
        client_tx: broadcast::Sender<ClientMessage>,
    ) -> Self {
        Self {
            id,
            payload: ClientPayload {
                query: query_body.query.to_owned(),
                operation_name: Some(query_body.operation_name.to_owned()),
                variables: Some(serde_json::to_value(query_body.variables).unwrap()),
            },
            server_tx,
            client_tx,
            _query: PhantomData,
        }
    }

    pub fn execute(
        self,
    ) -> impl Stream<Item = Result<Response<Query::ResponseData>, serde_json::Value>> {
        let (tx, rx) = mpsc::channel(16);

        let client_tx = self.client_tx.clone();
        let mut server_rx = self.server_tx.subscribe();
        let op_id = self.id.clone();
        let query_msg = ClientMessage::Start {
            id: op_id.to_string(),
            payload: self.payload.clone(),
        };
        tokio::spawn(async move {
            let _ = client_tx.send(query_msg).unwrap(); // TODO error handling
            while let Ok(msg) = server_rx.recv().await {
                match msg {
                    ServerMessage::Data { id, payload } if id == op_id => {
                        let _ = tx.send(Ok(payload)).await.unwrap();
                    }
                    ServerMessage::Complete { id } if id == op_id => {
                        return;
                    }
                    ServerMessage::ConnectionError { payload } => {
                        let _ = tx.send(Err(payload)).await.unwrap();
                        return;
                    }
                    ServerMessage::Error { id, payload } if id == op_id => {
                        let _ = tx.send(Err(payload)).await.unwrap();
                    }
                    ServerMessage::ConnectionAck => {}
                    ServerMessage::ConnectionKeepAlive => {}
                    _ => {}
                }
            }
        });
        ReceiverStream::new(rx).map(|result| {
            result.map(|payload| {
                serde_json::from_value::<Response<Query::ResponseData>>(payload).unwrap()
            })
        })
    }
}

impl<Query: GraphQLQuery> Drop for GraphQLOperation<Query> {
    fn drop(&mut self) {
        self.client_tx
            .send(ClientMessage::Stop {
                id: self.id.clone(),
            })
            .unwrap();
    }
}
