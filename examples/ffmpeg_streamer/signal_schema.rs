use graphql_client::GraphQLQuery;
use mediasoup::{
    data_structures::TransportTuple, producer::ProducerId, rtp_parameters::MediaKind,
    rtp_parameters::RtpParameters,
};

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "examples/ffmpeg_streamer/signal_schema.gql",
    query_path = "examples/ffmpeg_streamer/signal_query.gql"
)]
pub struct RecvPlainTransportOptions;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "examples/ffmpeg_streamer/signal_schema.gql",
    query_path = "examples/ffmpeg_streamer/signal_query.gql"
)]
pub struct ProducePlain;