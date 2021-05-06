use crate::relay_server::{RelayServer, Room, Session, SessionToken};
use async_graphql::guard::Guard;
use async_graphql::{
    scalar, Context, InputObject, Object, Result, Schema, SimpleObject, Subscription,
};
use futures::{stream, Stream, StreamExt};
use mediasoup::producer::ProducerOptions;
use mediasoup::transport::Transport;
use mediasoup::webrtc_transport::WebRtcTransportRemoteParameters;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task;
use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream};

struct SessionGuard;
#[async_trait::async_trait]
impl Guard for SessionGuard {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        match ctx.data_opt::<SessionToken>() {
            Some(_) => Ok(()),
            None => Err("Session token is required".into()),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(transparent)]
struct TransportId(mediasoup::transport::TransportId);
scalar!(TransportId);

#[derive(Deserialize, Serialize, Clone)]
#[serde(transparent)]
struct DtlsParameters(mediasoup::data_structures::DtlsParameters);
scalar!(DtlsParameters);

#[derive(Deserialize, Serialize, Clone)]
#[serde(transparent)]
struct IceCandidate(mediasoup::data_structures::IceCandidate);
scalar!(IceCandidate);

#[derive(Deserialize, Serialize, Clone)]
#[serde(transparent)]
struct IceParameters(mediasoup::data_structures::IceParameters);
scalar!(IceParameters);

#[derive(Deserialize, Serialize, Clone)]
#[serde(transparent)]
struct ConsumerId(mediasoup::consumer::ConsumerId);
scalar!(ConsumerId);

#[derive(Deserialize, Serialize, Clone)]
#[serde(transparent)]
struct ProducerId(mediasoup::producer::ProducerId);
scalar!(ProducerId);

#[derive(Deserialize, Serialize, Clone)]
#[serde(transparent)]
struct MediaKind(mediasoup::rtp_parameters::MediaKind);
scalar!(MediaKind);

#[derive(Deserialize, Serialize, Clone)]
#[serde(transparent)]
struct RtpParameters(mediasoup::rtp_parameters::RtpParameters);
scalar!(RtpParameters);

/// Initialization parameters for a transport
#[derive(SimpleObject)]
struct TransportOptions {
    id: TransportId,
    dtls_parameters: DtlsParameters,
    ice_candidates: Vec<IceCandidate>,
    ice_parameters: IceParameters,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
enum RtpCapabilities {
    Normal(mediasoup::rtp_parameters::RtpCapabilities),
    Finalized(mediasoup::rtp_parameters::RtpCapabilitiesFinalized),
}
scalar!(RtpCapabilities);

/// Initialization parameters for a client-side mediasoup device
#[derive(SimpleObject)]
struct ServerInitParameters {
    transport_options: TransportOptions,
    router_rtp_capabilities: RtpCapabilities,
}

/// Initialization parameters for a server-side mediasoup device
#[derive(InputObject)]
struct ClientInitParameters {
    rtp_capabilities: RtpCapabilities,
}

#[derive(SimpleObject, Clone)]
pub struct ConsumeParameters {
    id: ConsumerId,
    producer_id: ProducerId,
    kind: MediaKind,
    rtp_parameters: RtpParameters,
}

#[derive(InputObject)]
struct ConnectTransportParameters {
    dtls_parameters: DtlsParameters,
}

#[derive(InputObject)]
struct ProduceParameters {
    kind: MediaKind,
    rtp_parameters: RtpParameters,
}

#[derive(Serialize, Deserialize, Default)]
pub struct QueryRoot;
#[Object]
impl QueryRoot {
    /// Obtain initialization parameters for client-side mediasoup device
    #[graphql(guard(SessionGuard()))]
    async fn init(&self, ctx: &Context<'_>) -> ServerInitParameters {
        let session = ctx.data_unchecked::<Arc<Mutex<Session>>>().lock().await;
        let room = ctx.data_unchecked::<Arc<Mutex<Room>>>().lock().await;
        ServerInitParameters {
            transport_options: TransportOptions {
                id: TransportId(session.transport.id()),
                dtls_parameters: DtlsParameters(session.transport.dtls_parameters()),
                ice_candidates: session
                    .transport
                    .ice_candidates()
                    .into_iter()
                    .map(|x| IceCandidate(x.clone()))
                    .collect(),
                ice_parameters: IceParameters(session.transport.ice_parameters().clone()),
            },
            router_rtp_capabilities: RtpCapabilities::Finalized(
                room.router.rtp_capabilities().clone(),
            ),
        }
    }
}

#[derive(Default)]
pub struct MutationRoot;
#[Object]
impl MutationRoot {
    /// Provide initialization parameters for server-side mediasoup device
    #[graphql(guard(SessionGuard()))]
    async fn init(&self, ctx: &Context<'_>, init_params: ClientInitParameters) -> Result<bool> {
        let mut session = ctx.data_unchecked::<Arc<Mutex<Session>>>().lock().await;
        match init_params.rtp_capabilities {
            RtpCapabilities::Normal(rtp_caps) => {
                session.client_rtp_capabilities.replace(rtp_caps);
                Ok(true)
            }
            _ => Err("Cannot apply RtpCapabilitiesFinalized to server-side device".into()),
        }
    }

    /// Provide connection parameters for server-side transport
    #[graphql(guard(SessionGuard()))]
    async fn connect_transport(
        &self,
        ctx: &Context<'_>,
        connect_params: ConnectTransportParameters,
    ) -> Result<bool> {
        let session = ctx.data_unchecked::<Arc<Mutex<Session>>>().lock().await;
        match session
            .transport
            .connect(WebRtcTransportRemoteParameters {
                dtls_parameters: connect_params.dtls_parameters.0,
            })
            .await
        {
            Ok(_) => Ok(true),
            Err(err) => Err(err.into()),
        }
    }

    #[graphql(guard(SessionGuard()))]
    async fn produce(
        &self,
        ctx: &Context<'_>,
        produce_params: ProduceParameters,
    ) -> Result<bool> {
        let mut session = ctx.data_unchecked::<Arc<Mutex<Session>>>().lock().await;
        let transport = session.transport.clone();
        // fucking end me
        // match transport
        //     .produce(ProducerOptions::new(
        //         produce_params.kind.0,
        //         produce_params.rtp_parameters.0,
        //     ))
        //     .await
        // {
        //     Ok(producer) => {
        //         let id = producer.id();
        //         session.producers.push(producer);
        //         Ok(ProducerId(id))
        //     }
        //     Err(err) => Err(err.into()),
        // }
        Ok(true)
    }
}

#[derive(Default)]
pub struct SubscriptionRoot;
#[Subscription]
impl SubscriptionRoot {
    #[graphql(guard(SessionGuard()))]
    async fn consume(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<impl Stream<Item = Result<ConsumeParameters>>> {
        let room = ctx.data_unchecked::<Arc<Mutex<Room>>>().lock().await;
        Ok(
            BroadcastStream::new(room.consume_tx.subscribe()).map(|x| match x {
                Ok(x) => Ok(x),
                Err(_) => Err("Broadcast buffer overflow".into()),
            }),
        )
    }
}

pub type SignalSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

pub fn schema(server: Arc<Mutex<RelayServer>>) -> SignalSchema {
    SignalSchema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(server)
        .finish()
}
