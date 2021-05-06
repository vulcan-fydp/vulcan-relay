use crate::relay_server::{InvalidSessionError, RelayServer, SessionToken};
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql::{scalar, Context, Data, EmptyMutation, Object, Schema, Subscription};
use async_graphql_warp::{graphql_subscription_with_data, Response};
use futures::{stream, Stream};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::{http::Response as HttpResponse, Filter};

mod relay_server;
mod signal_schema;

#[tokio::main]
async fn main() {
    env_logger::init();

    let relay_server = Arc::new(Mutex::new(RelayServer::new().await));
    let schema = signal_schema::schema(relay_server.clone());
    println!("{}", &schema.sdl());

    let routes = graphql_subscription_with_data(schema, |value| async move {
        let mut data = Data::default();
        if let Ok(token) = serde_json::from_value::<SessionToken>(value) {
            let mut server = relay_server.lock().await;
            let session = server.new_session(token.clone()).await?;
            data.insert(token.clone()); // TODO verify token...
            data.insert(session.lock().await.room.upgrade().unwrap());
            data.insert(session);
        }
        Ok(data)
    });
    warp::serve(routes).run(([0, 0, 0, 0], 8080)).await;
}
