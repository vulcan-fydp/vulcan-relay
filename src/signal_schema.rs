use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};

use async_graphql::guard::Guard;
use async_graphql::{scalar, Context, Object, Result, Schema, SimpleObject, Subscription};
use mediasoup::transport::Transport;

use crate::relay_server::RelayServer;
use crate::session::Session;

#[derive(Serialize, Deserialize, Default)]
pub struct QueryRoot;
#[Object]
impl QueryRoot {
    /// Obtain initialization parameters for client-side mediasoup device
    #[graphql(guard(SessionGuard()))]
    async fn init(&self, ctx: &Context<'_>) -> ServerInitParameters {
        let session = ctx.data_unchecked::<Session>();
        let room = session.get_room();
        let transport = session.get_transport();
        ServerInitParameters {
            transport_options: TransportOptions {
                id: transport.id(),
                dtls_parameters: transport.dtls_parameters(),
                ice_candidates: transport.ice_candidates().clone(),
                ice_parameters: transport.ice_parameters().clone(),
            },
            router_rtp_capabilities: RtpCapabilitiesFinalized(
                room.get_router().rtp_capabilities().clone(),
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
    async fn init(&self, ctx: &Context<'_>, rtp_capabilities: RtpCapabilities) -> Result<bool> {
        let session = ctx.data_unchecked::<Session>();
        session.set_rtp_capabilities(rtp_capabilities.0);
        Ok(true)
    }

    /// Provide connection parameters for server-side transport
    #[graphql(guard(SessionGuard()))]
    async fn connect_transport(
        &self,
        ctx: &Context<'_>,
        dtls_parameters: DtlsParameters,
    ) -> Result<TransportId> {
        let session = ctx.data_unchecked::<Session>();
        Ok(session
            .connect_transport(dtls_parameters.0)
            .await
            .map(|transport_id| TransportId(transport_id))?)
    }

    /// Request consumption of media stream
    #[graphql(guard(SessionGuard()))]
    async fn consume(&self, ctx: &Context<'_>, producer_id: ProducerId) -> Result<ConsumerOptions> {
        let local_pool = ctx.data_unchecked::<tokio_local::LocalPoolHandle>();
        let session = ctx.data_unchecked::<Session>();
        let consumer = session.consume(local_pool.clone(), producer_id.0).await?;
        Ok(ConsumerOptions {
            id: consumer.id(),
            kind: consumer.kind(),
            rtp_parameters: consumer.rtp_parameters().clone(),
            producer_id: producer_id.0,
        })
    }

    /// Resume existing consumer
    #[graphql(guard(SessionGuard()))]
    async fn consumer_resume(&self, ctx: &Context<'_>, consumer_id: ConsumerId) -> Result<bool> {
        let session = ctx.data_unchecked::<Session>();
        session.consumer_resume(consumer_id.0).await?;
        Ok(true)
    }

    /// Request production of media stream
    #[graphql(guard(SessionGuard()))]
    async fn produce(
        &self,
        ctx: &Context<'_>,
        kind: MediaKind,
        rtp_parameters: RtpParameters,
    ) -> Result<ProducerId> {
        let local_pool = ctx.data_unchecked::<tokio_local::LocalPoolHandle>();
        let session = ctx.data_unchecked::<Session>();
        Ok(ProducerId(
            session
                .produce(local_pool.clone(), kind.0, rtp_parameters.0)
                .await?
                .id(),
        ))
    }
}

#[derive(Default)]
pub struct SubscriptionRoot;
#[Subscription]
impl SubscriptionRoot {
    /// Notify when new producers are available
    #[graphql(guard(SessionGuard()))]
    async fn producer_available(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<impl Stream<Item = ProducerId>> {
        let session = ctx.data_unchecked::<Session>();
        let room = session.get_room();
        Ok(room.available_producers().map(|x| ProducerId(x)))
    }
}

pub type SignalSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

pub fn schema(server: &RelayServer) -> SignalSchema {
    SignalSchema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(server.clone())
        .data(tokio_local::new_local_pool(2))
        .finish()
}

struct SessionGuard;
#[async_trait::async_trait]
impl Guard for SessionGuard {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        match ctx.data_opt::<Session>() {
            Some(_) => Ok(()),
            None => Err("session is required".into()),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Copy)]
#[serde(transparent)]
struct TransportId(mediasoup::transport::TransportId);
scalar!(TransportId);

#[derive(Deserialize, Serialize, Clone, Copy)]
#[serde(transparent)]
struct ConsumerId(mediasoup::consumer::ConsumerId);
scalar!(ConsumerId);

#[derive(Deserialize, Serialize, Clone, Copy)]
#[serde(transparent)]
struct ProducerId(mediasoup::producer::ProducerId);
scalar!(ProducerId);

#[derive(Deserialize, Serialize, Clone)]
#[serde(transparent)]
struct DtlsParameters(mediasoup::data_structures::DtlsParameters);
scalar!(DtlsParameters);

#[derive(Deserialize, Serialize, Clone)]
#[serde(transparent)]
struct MediaKind(mediasoup::rtp_parameters::MediaKind);
scalar!(MediaKind);

#[derive(Deserialize, Serialize, Clone)]
#[serde(transparent)]
struct RtpParameters(mediasoup::rtp_parameters::RtpParameters);
scalar!(RtpParameters);

/// Initialization parameters for a transport
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TransportOptions {
    id: mediasoup::transport::TransportId,
    dtls_parameters: mediasoup::data_structures::DtlsParameters,
    ice_candidates: Vec<mediasoup::data_structures::IceCandidate>,
    ice_parameters: mediasoup::data_structures::IceParameters,
}
scalar!(TransportOptions);

#[derive(Serialize, Deserialize, Clone)]
struct RtpCapabilities(mediasoup::rtp_parameters::RtpCapabilities);
scalar!(RtpCapabilities);

#[derive(Serialize, Deserialize, Clone)]
struct RtpCapabilitiesFinalized(mediasoup::rtp_parameters::RtpCapabilitiesFinalized);
scalar!(RtpCapabilitiesFinalized);

/// Initialization parameters for a client-side mediasoup device
#[derive(SimpleObject)]
struct ServerInitParameters {
    transport_options: TransportOptions,
    router_rtp_capabilities: RtpCapabilitiesFinalized,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConsumerOptions {
    pub id: mediasoup::consumer::ConsumerId,
    pub producer_id: mediasoup::producer::ProducerId,
    pub kind: mediasoup::rtp_parameters::MediaKind,
    pub rtp_parameters: mediasoup::rtp_parameters::RtpParameters,
}
scalar!(ConsumerOptions);
