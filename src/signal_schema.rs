use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};

use anyhow::anyhow;
use async_graphql::{scalar, Context, Object, Result, Schema, SimpleObject, Subscription};
use mediasoup::transport::Transport;

use crate::session::{Session, WeakSession};

fn session_from_ctx(ctx: &Context<'_>) -> Result<Session, anyhow::Error> {
    ctx.data_opt::<WeakSession>()
        .and_then(|weak_session| weak_session.upgrade())
        .ok_or_else(|| anyhow!("session is invalid or dropped"))
}

#[derive(Default)]
pub struct QueryRoot;
#[Object]
impl QueryRoot {
    /// Obtain initialization parameters for client-side mediasoup device.
    async fn init(&self, ctx: &Context<'_>) -> Result<ServerInitParameters> {
        let session = session_from_ctx(ctx)?;
        let room = session.get_room();
        let send_transport = session.get_send_transport().await;
        let recv_transport = session.get_recv_transport().await;
        let router = room.get_router().await;
        Ok(ServerInitParameters {
            send_transport_options: TransportOptions {
                id: send_transport.id(),
                dtls_parameters: send_transport.dtls_parameters(),
                sctp_parameters: send_transport.sctp_parameters().unwrap(),
                ice_candidates: send_transport.ice_candidates().clone(),
                ice_parameters: send_transport.ice_parameters().clone(),
            },
            recv_transport_options: TransportOptions {
                id: recv_transport.id(),
                dtls_parameters: recv_transport.dtls_parameters(),
                sctp_parameters: send_transport.sctp_parameters().unwrap(),
                ice_candidates: recv_transport.ice_candidates().clone(),
                ice_parameters: recv_transport.ice_parameters().clone(),
            },
            router_rtp_capabilities: RtpCapabilitiesFinalized(router.rtp_capabilities().clone()),
        })
    }
}

#[derive(Default)]
pub struct MutationRoot;
#[Object]
impl MutationRoot {
    /// Provide initialization parameters for server-side mediasoup device.
    async fn init(&self, ctx: &Context<'_>, rtp_capabilities: RtpCapabilities) -> Result<bool> {
        let session = session_from_ctx(ctx)?;
        session.set_rtp_capabilities(rtp_capabilities.0);
        Ok(true)
    }

    /// Provide connection parameters for server-side transport.
    async fn connect_send_transport(
        &self,
        ctx: &Context<'_>,
        dtls_parameters: DtlsParameters,
    ) -> Result<TransportId> {
        let session = session_from_ctx(ctx)?;
        Ok(TransportId(
            session.connect_send_transport(dtls_parameters.0).await?,
        ))
    }

    /// Provide connection parameters for server-side transport.
    async fn connect_recv_transport(
        &self,
        ctx: &Context<'_>,
        dtls_parameters: DtlsParameters,
    ) -> Result<TransportId> {
        let session = session_from_ctx(ctx)?;
        Ok(TransportId(
            session.connect_recv_transport(dtls_parameters.0).await?,
        ))
    }

    /// Request consumption of media stream.
    async fn consume(&self, ctx: &Context<'_>, producer_id: ProducerId) -> Result<ConsumerOptions> {
        let local_pool = ctx.data_unchecked::<tokio_local::LocalPoolHandle>();
        let session = session_from_ctx(ctx)?;
        let consumer = session.consume(local_pool.clone(), producer_id.0).await?;
        Ok(ConsumerOptions {
            id: consumer.id(),
            kind: consumer.kind(),
            rtp_parameters: consumer.rtp_parameters().clone(),
            producer_id: producer_id.0,
        })
    }

    /// Resume existing consumer.
    async fn consumer_resume(&self, ctx: &Context<'_>, consumer_id: ConsumerId) -> Result<bool> {
        let session = session_from_ctx(ctx)?;
        session.consumer_resume(consumer_id.0).await?;
        Ok(true)
    }

    /// Request production of media stream.
    async fn produce(
        &self,
        ctx: &Context<'_>,
        kind: MediaKind,
        rtp_parameters: RtpParameters,
    ) -> Result<ProducerId> {
        let local_pool = ctx.data_unchecked::<tokio_local::LocalPoolHandle>();
        let session = session_from_ctx(ctx)?;
        Ok(ProducerId(
            session
                .produce(local_pool.clone(), kind.0, rtp_parameters.0)
                .await?
                .id(),
        ))
    }

    /// Request consumption of data stream.
    async fn consume_data(
        &self,
        ctx: &Context<'_>,
        data_producer_id: DataProducerId,
    ) -> Result<DataConsumerOptions> {
        let local_pool = ctx.data_unchecked::<tokio_local::LocalPoolHandle>();
        let session = ctx.data_unchecked::<WeakSession>().upgrade().unwrap();
        let data_consumer = session
            .consume_data(local_pool.clone(), data_producer_id.0)
            .await?;
        Ok(DataConsumerOptions {
            id: data_consumer.id(),
            data_producer_id: data_producer_id.0,
            sctp_stream_parameters: data_consumer.sctp_stream_parameters().unwrap(),
        })
    }

    /// Request production of data stream.
    async fn produce_data(
        &self,
        ctx: &Context<'_>,
        sctp_stream_parameters: SctpStreamParameters,
    ) -> Result<DataProducerId> {
        let local_pool = ctx.data_unchecked::<tokio_local::LocalPoolHandle>();
        let session = session_from_ctx(ctx)?;
        Ok(DataProducerId(
            session
                .produce_data(local_pool.clone(), sctp_stream_parameters.0)
                .await?
                .id(),
        ))
    }
}

#[derive(Default)]
pub struct SubscriptionRoot;
#[Subscription]
impl SubscriptionRoot {
    /// Notify when new producers are available.
    async fn producer_available(
        &self,
        ctx: &Context<'_>,
    ) -> Result<impl Stream<Item = ProducerId>> {
        let session = session_from_ctx(ctx)?;
        let room = session.get_room();
        Ok(room.available_producers().map(ProducerId))
    }
    /// Notify when new data producers are available.
    async fn data_producer_available(
        &self,
        ctx: &Context<'_>,
    ) -> Result<impl Stream<Item = DataProducerId>> {
        let session = session_from_ctx(ctx)?;
        let room = session.get_room();
        Ok(room.available_data_producers().map(DataProducerId))
    }
}

pub type SignalSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

pub fn schema() -> SignalSchema {
    SignalSchema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(tokio_local::new_local_pool(2))
        .finish()
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

#[derive(Deserialize, Serialize, Clone, Copy)]
#[serde(transparent)]
struct DataProducerId(mediasoup::data_producer::DataProducerId);
scalar!(DataProducerId);

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

#[derive(Serialize, Deserialize, Clone)]
struct RtpCapabilities(mediasoup::rtp_parameters::RtpCapabilities);
scalar!(RtpCapabilities);

#[derive(Serialize, Deserialize, Clone)]
struct RtpCapabilitiesFinalized(mediasoup::rtp_parameters::RtpCapabilitiesFinalized);
scalar!(RtpCapabilitiesFinalized);

#[derive(Serialize, Deserialize, Clone)]
struct SctpStreamParameters(mediasoup::sctp_parameters::SctpStreamParameters);
scalar!(SctpStreamParameters);

/// Initialization parameters for a client-side mediasoup device
#[derive(SimpleObject)]
struct ServerInitParameters {
    send_transport_options: TransportOptions,
    recv_transport_options: TransportOptions,
    router_rtp_capabilities: RtpCapabilitiesFinalized,
}

/// Initialization parameters for a transport
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TransportOptions {
    id: mediasoup::transport::TransportId,
    dtls_parameters: mediasoup::data_structures::DtlsParameters,
    sctp_parameters: mediasoup::sctp_parameters::SctpParameters,
    ice_candidates: Vec<mediasoup::data_structures::IceCandidate>,
    ice_parameters: mediasoup::data_structures::IceParameters,
}
scalar!(TransportOptions);

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ConsumerOptions {
    id: mediasoup::consumer::ConsumerId,
    producer_id: mediasoup::producer::ProducerId,
    kind: mediasoup::rtp_parameters::MediaKind,
    rtp_parameters: mediasoup::rtp_parameters::RtpParameters,
}
scalar!(ConsumerOptions);

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DataConsumerOptions {
    id: mediasoup::data_consumer::DataConsumerId,
    data_producer_id: mediasoup::data_producer::DataProducerId,
    sctp_stream_parameters: mediasoup::sctp_parameters::SctpStreamParameters,
}
scalar!(DataConsumerOptions);
