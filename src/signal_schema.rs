use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};

use async_graphql::guard::Guard;
use async_graphql::{scalar, Context, Object, Result, Schema, SimpleObject, Subscription};
use mediasoup::consumer::ConsumerOptions;
use mediasoup::producer::ProducerOptions;
use mediasoup::transport::Transport;
use mediasoup::webrtc_transport::WebRtcTransportRemoteParameters;

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
                id: TransportId(transport.id()),
                dtls_parameters: DtlsParameters(transport.dtls_parameters()),
                ice_candidates: transport
                    .ice_candidates()
                    .into_iter()
                    .map(|x| IceCandidate(x.clone()))
                    .collect(),
                ice_parameters: IceParameters(transport.ice_parameters().clone()),
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
        let transport = session.get_transport();
        match transport
            .connect(WebRtcTransportRemoteParameters {
                dtls_parameters: dtls_parameters.0,
            })
            .await
        {
            Ok(_) => {
                log::info!(
                    "connected transport {} from session {}",
                    transport.id(),
                    session.id()
                );
                Ok(TransportId(transport.id()))
            }
            Err(err) => Err(err.into()),
        }
    }

    /// Request consumption of media stream
    #[graphql(guard(SessionGuard()))]
    async fn consume(
        &self,
        ctx: &Context<'_>,
        producer_id: ProducerId,
    ) -> Result<ConsumeParameters> {
        let local_pool = ctx.data_unchecked::<tokio_local::LocalPoolHandle>();
        let session = ctx.data_unchecked::<Session>();
        let transport = session.get_transport();
        let rtp_capabilities = match session.get_rtp_capabilities() {
            Some(rtp_capabilities) => rtp_capabilities,
            None => return Err("client must send RTP capabilities before consuming".into()),
        };

        let mut options = ConsumerOptions::new(producer_id.0, rtp_capabilities);
        options.paused = true;

        match local_pool
            .spawn_pinned(|| async move { transport.consume(options).await })
            .await
            .unwrap()
        {
            Ok(consumer) => {
                log::info!(
                    "new consumer created {} for session {}",
                    consumer.id(),
                    session.id()
                );
                let consume_parameters = ConsumeParameters {
                    id: ConsumerId(consumer.id()),
                    kind: MediaKind(consumer.kind()),
                    rtp_parameters: RtpParameters(consumer.rtp_parameters().clone()),
                    producer_id: producer_id,
                };
                session.add_consumer(consumer);
                Ok(consume_parameters)
            }
            Err(err) => Err(err.into()),
        }
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
        let room = session.get_room();

        // transport is async-trait with non-Send futures
        let transport = session.get_transport();
        match local_pool
            .spawn_pinned(move || async move {
                transport
                    .produce(ProducerOptions::new(kind.0, rtp_parameters.0))
                    .await
            })
            .await
            .unwrap()
        {
            Ok(producer) => {
                let id = producer.id();
                session.add_producer(producer);
                room.notify_new_producer(id);
                log::info!(
                    "new producer available {} from session {}",
                    id,
                    session.id()
                );
                Ok(ProducerId(id))
            }
            Err(err) => Err(err.into()),
        }
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

#[derive(Deserialize, Serialize, Clone, Copy)]
#[serde(transparent)]
struct ConsumerId(mediasoup::consumer::ConsumerId);
scalar!(ConsumerId);

#[derive(Deserialize, Serialize, Clone, Copy)]
#[serde(transparent)]
struct ProducerId(mediasoup::producer::ProducerId);
scalar!(ProducerId);

#[derive(Deserialize, Serialize, Clone, Copy)]
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

#[derive(SimpleObject, Clone)]
pub struct ConsumeParameters {
    id: ConsumerId,
    producer_id: ProducerId,
    kind: MediaKind,
    rtp_parameters: RtpParameters,
}
