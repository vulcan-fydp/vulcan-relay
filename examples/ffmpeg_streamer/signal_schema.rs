use serde::{Deserialize, Serialize};

use graphql_client::GraphQLQuery;
use mediasoup::{
    data_structures::TransportTuple, producer::ProducerId, rtp_parameters::MediaKind,
    rtp_parameters::RtpParameters, transport::TransportId,
};

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "examples/ffmpeg_streamer/signal_schema.gql",
    query_path = "examples/ffmpeg_streamer/signal_query.gql"
)]
pub struct CreateRecvPlainTransport;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "examples/ffmpeg_streamer/signal_schema.gql",
    query_path = "examples/ffmpeg_streamer/signal_query.gql"
)]
pub struct ProducePlain;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlainTransportOptions {
    pub id: TransportId,
    pub tuple: TransportTuple,
}
