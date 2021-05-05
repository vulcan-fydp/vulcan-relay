use crate::relay_server::{RelayServer, Room, Session, SessionToken};
use async_graphql::guard::Guard;
use async_graphql::{
    scalar, Context, InputObject, Object, Result, Schema, SimpleObject, Subscription,
};
use futures::{stream, Stream};
use mediasoup::transport::Transport;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

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
#[serde(rename_all = "camelCase")]
struct TransportOptions {
    id: mediasoup::transport::TransportId,
    dtls_parameters: mediasoup::data_structures::DtlsParameters,
    ice_candidates: Vec<mediasoup::data_structures::IceCandidate>,
    ice_parameters: mediasoup::data_structures::IceParameters,
}
scalar!(TransportOptions);

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
struct CilentInitParameters {
    rtp_capabilities: RtpCapabilities,
}

#[derive(Serialize, Deserialize, Clone)]
struct ConsumerOptions {
    id: mediasoup::consumer::ConsumerId,
    producer_id: mediasoup::producer::ProducerId,
    kind: mediasoup::rtp_parameters::MediaKind,
    rtp_parameters: mediasoup::rtp_parameters::RtpParameters,
}
scalar!(ConsumerOptions);

#[derive(Default)]
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
                id: session.transport.id(),
                dtls_parameters: session.transport.dtls_parameters(),
                ice_candidates: session.transport.ice_candidates().clone(),
                ice_parameters: session.transport.ice_parameters().clone(),
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
    /// Provide initizliation parameters for server-side mediasoup device
    #[graphql(guard(SessionGuard()))]
    async fn init(&self, ctx: &Context<'_>, init_params: CilentInitParameters) -> Result<bool> {
        let mut session = ctx.data_unchecked::<Arc<Mutex<Session>>>().lock().await;
        match init_params.rtp_capabilities {
            RtpCapabilities::Normal(rtp_caps) => {
                session.client_rtp_capabilities.replace(rtp_caps);
                Ok(true)
            }
            _ => Err("Cannot apply RtpCapabilitiesFinalized".into()),
        }
    }
}

#[derive(Default)]
pub struct SubscriptionRoot;
#[Subscription]
impl SubscriptionRoot {
    async fn init(&self, ctx: &Context<'_>) -> async_graphql::Result<impl Stream<Item = i32>> {
        Ok(stream::once(async move { 10 }))
    }
}

pub type SignalSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

pub fn schema(server: Arc<Mutex<RelayServer>>) -> SignalSchema {
    SignalSchema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(server)
        .finish()
}
