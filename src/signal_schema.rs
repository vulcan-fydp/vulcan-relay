use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};

use anyhow::anyhow;
use async_graphql::{scalar, Context, Guard, Object, Result, Schema, Subscription};
use mediasoup::transport::Transport;

use crate::session::{Resource, ResourceType, Session, WeakSession};

fn session_from_ctx(ctx: &Context<'_>) -> Result<Session, anyhow::Error> {
    ctx.data_opt::<WeakSession>()
        .and_then(|weak_session| weak_session.upgrade())
        .ok_or_else(|| anyhow!("session is invalid or dropped"))
}

#[derive(Default)]
pub struct QueryRoot;
#[Object]
impl QueryRoot {
    /// Server-side WebRTC RTP capabilities for WebRTC negotiation.
    async fn server_rtp_capabilities(&self, ctx: &Context<'_>) -> Result<RtpCapabilitiesFinalized> {
        let session = session_from_ctx(ctx)?;
        let router = session.get_room().get_router().await;
        Ok(RtpCapabilitiesFinalized(router.rtp_capabilities().clone()))
    }
}

#[derive(Default)]
pub struct MutationRoot;
#[Object]
impl MutationRoot {
    /// Client-side RTP capabilities for WebRTC negotiation.
    async fn rtp_capabilities(
        &self,
        ctx: &Context<'_>,
        rtp_capabilities: RtpCapabilities,
    ) -> Result<bool> {
        let session = session_from_ctx(ctx)?;
        session.set_rtp_capabilities(rtp_capabilities.0);
        Ok(true)
    }

    /// WebRTC transport parameters.
    #[graphql(guard = "ResourceGuard::new(ResourceType::WebrtcTransport, 2, 1)")]
    async fn create_webrtc_transport(&self, ctx: &Context<'_>) -> Result<WebRtcTransportOptions> {
        let session = session_from_ctx(ctx)?;
        let transport = session.create_webrtc_transport().await;
        Ok(WebRtcTransportOptions {
            id: transport.id(),
            dtls_parameters: transport.dtls_parameters(),
            sctp_parameters: transport.sctp_parameters().unwrap(),
            ice_candidates: transport.ice_candidates().clone(),
            ice_parameters: transport.ice_parameters().clone(),
        })
    }
    /// Plain receive transport connection parameters.
    #[graphql(guard = "ResourceGuard::new(ResourceType::PlainTransport, 2, 1)")]
    async fn create_plain_transport(&self, ctx: &Context<'_>) -> Result<PlainTransportOptions> {
        let session = session_from_ctx(ctx)?;
        let plain_transport = session.create_plain_transport().await;
        Ok(PlainTransportOptions {
            id: plain_transport.id(),
            tuple: plain_transport.tuple(),
        })
    }

    /// Provide connection parameters for server-side WebRTC transport.
    async fn connect_webrtc_transport(
        &self,
        ctx: &Context<'_>,
        transport_id: TransportId,
        dtls_parameters: DtlsParameters,
    ) -> Result<TransportId> {
        let session = session_from_ctx(ctx)?;
        Ok(TransportId(
            session
                .connect_webrtc_transport(transport_id.0, dtls_parameters.0)
                .await?,
        ))
    }

    /// Request consumption of media stream.
    #[graphql(guard = "ResourceGuard::new(ResourceType::Consumer, 2, 1)")]
    async fn consume(
        &self,
        ctx: &Context<'_>,
        transport_id: TransportId,
        producer_id: ProducerId,
    ) -> Result<ConsumerOptions> {
        let session = session_from_ctx(ctx)?;
        let consumer = session.consume(transport_id.0, producer_id.0).await?;
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
    #[graphql(guard = "ResourceGuard::new(ResourceType::Producer, 2, 1)")]
    async fn produce(
        &self,
        ctx: &Context<'_>,
        transport_id: TransportId,
        kind: MediaKind,
        rtp_parameters: RtpParameters,
    ) -> Result<ProducerId> {
        let session = session_from_ctx(ctx)?;
        Ok(ProducerId(
            session
                .produce(transport_id.0, kind.0, rtp_parameters.0)
                .await?
                .id(),
        ))
    }

    /// Request production of a media stream on plain transport.
    #[graphql(guard = "ResourceGuard::new(ResourceType::Producer, 2, 1)")]
    async fn produce_plain(
        &self,
        ctx: &Context<'_>,
        transport_id: TransportId,
        kind: MediaKind,
        rtp_parameters: RtpParameters,
    ) -> Result<ProducerId> {
        let session = session_from_ctx(ctx)?;
        Ok(ProducerId(
            session
                .produce_plain(transport_id.0, kind.0, rtp_parameters.0)
                .await?
                .id(),
        ))
    }

    /// Request consumption of data stream.
    #[graphql(guard = "ResourceGuard::new(ResourceType::DataConsumer, 128, 1)")]
    async fn consume_data(
        &self,
        ctx: &Context<'_>,
        transport_id: TransportId,
        data_producer_id: DataProducerId,
    ) -> Result<DataConsumerOptions> {
        let session = ctx.data_unchecked::<WeakSession>().upgrade().unwrap();
        let data_consumer = session
            .consume_data(transport_id.0, data_producer_id.0)
            .await?;
        Ok(DataConsumerOptions {
            id: data_consumer.id(),
            data_producer_id: data_producer_id.0,
            sctp_stream_parameters: data_consumer.sctp_stream_parameters().unwrap(),
        })
    }

    /// Request production of data stream.
    #[graphql(guard = "ResourceGuard::new(ResourceType::DataProducer, 2, 1)")]
    async fn produce_data(
        &self,
        ctx: &Context<'_>,
        transport_id: TransportId,
        sctp_stream_parameters: SctpStreamParameters,
    ) -> Result<DataProducerId> {
        let session = session_from_ctx(ctx)?;
        Ok(DataProducerId(
            session
                .produce_data(transport_id.0, sctp_stream_parameters.0)
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
    /// Notify when client-side transport should close.
    async fn transport_closed(&self, ctx: &Context<'_>) -> Result<impl Stream<Item = TransportId>> {
        let session = session_from_ctx(ctx)?;
        Ok(session
            .closed_resources()
            .filter_map(|x| async move {
                match x {
                    Resource::WebrtcTransport(id) => Some(id),
                    Resource::PlainTransport(id) => Some(id),
                    _ => None,
                }
            })
            .map(TransportId))
    }
    /// Notify when client-side producer should close.
    async fn producer_closed(&self, ctx: &Context<'_>) -> Result<impl Stream<Item = ProducerId>> {
        let session = session_from_ctx(ctx)?;
        Ok(session
            .closed_resources()
            .filter_map(|x| async move {
                match x {
                    Resource::Producer(id) => Some(id),
                    _ => None,
                }
            })
            .map(ProducerId))
    }
    /// Notify when client-side consumer should close.
    async fn consumer_closed(&self, ctx: &Context<'_>) -> Result<impl Stream<Item = ConsumerId>> {
        let session = session_from_ctx(ctx)?;
        Ok(session
            .closed_resources()
            .filter_map(|x| async move {
                match x {
                    Resource::Consumer(id) => Some(id),
                    _ => None,
                }
            })
            .map(ConsumerId))
    }
    /// Notify when client-side data producer should close.
    async fn data_producer_closed(
        &self,
        ctx: &Context<'_>,
    ) -> Result<impl Stream<Item = DataProducerId>> {
        let session = session_from_ctx(ctx)?;
        Ok(session
            .closed_resources()
            .filter_map(|x| async move {
                match x {
                    Resource::DataProducer(id) => Some(id),
                    _ => None,
                }
            })
            .map(DataProducerId))
    }
    /// Notify when client-side data consumer should close.
    async fn data_consumer_closed(
        &self,
        ctx: &Context<'_>,
    ) -> Result<impl Stream<Item = DataConsumerId>> {
        let session = session_from_ctx(ctx)?;
        Ok(session
            .closed_resources()
            .filter_map(|x| async move {
                match x {
                    Resource::DataConsumer(id) => Some(id),
                    _ => None,
                }
            })
            .map(DataConsumerId))
    }
}

struct ResourceGuard {
    /// Name of resource to enforce limits for.
    resource: ResourceType,
    /// Expected count of this resource allocated as a result of this operation.
    expected: usize,
    /// Maximum allowable count of this resource.
    limit: usize,
}
impl ResourceGuard {
    fn new(resource: ResourceType, limit: usize, expected: usize) -> Self {
        ResourceGuard {
            resource,
            limit,
            expected,
        }
    }
}
#[async_trait::async_trait]
impl Guard for ResourceGuard {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        let session = session_from_ctx(ctx)?;
        if session.get_resource_count(&self.resource) + self.expected <= self.limit {
            Ok(())
        } else {
            Err(format!(
                "resource limit of {} exceeded (max {})",
                self.resource, self.limit
            )
            .into())
        }
    }
}

pub type SignalSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

pub fn schema() -> SignalSchema {
    SignalSchema::build(QueryRoot, MutationRoot, SubscriptionRoot).finish()
}

// TODO all UUID based types need to be migrated to either:
// - accept ID instead of scalar type (lose type safety)
// - manually serialize as String rather than UUID
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

#[derive(Deserialize, Serialize, Clone, Copy)]
#[serde(transparent)]
struct DataConsumerId(mediasoup::data_consumer::DataConsumerId);
scalar!(DataConsumerId);

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
#[serde(transparent)]
struct RtpCapabilities(mediasoup::rtp_parameters::RtpCapabilities);
scalar!(RtpCapabilities);

#[derive(Serialize, Deserialize, Clone)]
#[serde(transparent)]
struct RtpCapabilitiesFinalized(mediasoup::rtp_parameters::RtpCapabilitiesFinalized);
scalar!(RtpCapabilitiesFinalized);

#[derive(Serialize, Deserialize, Clone)]
#[serde(transparent)]
struct SctpStreamParameters(mediasoup::sctp_parameters::SctpStreamParameters);
scalar!(SctpStreamParameters);

#[derive(Serialize, Deserialize, Clone)]
#[serde(transparent)]
struct TransportTuple(mediasoup::data_structures::TransportTuple);
scalar!(TransportTuple);

/// Initialization parameters for a transport
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct WebRtcTransportOptions {
    id: mediasoup::transport::TransportId,
    dtls_parameters: mediasoup::data_structures::DtlsParameters,
    sctp_parameters: mediasoup::sctp_parameters::SctpParameters,
    ice_candidates: Vec<mediasoup::data_structures::IceCandidate>,
    ice_parameters: mediasoup::data_structures::IceParameters,
}
scalar!(WebRtcTransportOptions);

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PlainTransportOptions {
    id: mediasoup::transport::TransportId,
    tuple: mediasoup::data_structures::TransportTuple,
}
scalar!(PlainTransportOptions);

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
