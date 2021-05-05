use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql::{scalar, Context, Data, EmptyMutation, Object, Schema, Subscription};
use async_graphql_warp::{graphql_subscription_with_data, Response};
use futures::{stream, Stream};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use warp::{http::Response as HttpResponse, Filter};
mod messages;
mod relay_server;
mod signal_schema;
use crate::relay_server::{InvalidSessionError, RelayServer, SessionToken};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    env_logger::init();
    
    let relay_server = Arc::new(Mutex::new(RelayServer::new().await));
    let schema = signal_schema::schema(relay_server.clone());

    println!("Playground: http://localhost:8000");

    let with_relay = move |relay: Arc<Mutex<RelayServer>>| warp::any().map(move || relay.clone());

    let graphql_post = with_relay(relay_server.clone())
        .and(warp::header::optional::<String>("token"))
        .and(async_graphql_warp::graphql(schema.clone()))
        .and_then(
            |relay_server: Arc<Mutex<RelayServer>>, 
             token : Option<String>, 
             (schema, mut request): (
                signal_schema::SignalSchema,
                async_graphql::Request,
            )| async move {
                if let Some(token) = token {
                    if let Ok(token) = serde_json::from_str::<SessionToken>(token.as_str()) {
                        let mut server = relay_server.lock().await;
                        request = request.data(token);
                    }
                }
                let resp = schema.execute(request).await;
                Ok::<_, Infallible>(Response::from(resp))
            },
        );

    let graphql_playground = warp::path::end().and(warp::get()).map(|| {
        HttpResponse::builder()
            .header("content-type", "text/html")
            .body(playground_source(
                GraphQLPlaygroundConfig::new("/").subscription_endpoint("/"),
            ))
    });

    let routes = graphql_subscription_with_data(schema, |value| async move {
        let mut data = Data::default();
        if let Ok(token) = serde_json::from_value::<SessionToken>(value) {
            let mut server = relay_server.lock().await;
            let session = server.new_session(token).await?;
            data.insert(session.lock().await.room.clone());
            data.insert(session);
        }
        Ok(data)
    });
    // .or(graphql_playground)
    // .or(graphql_post);
    warp::serve(routes).run(([0, 0, 0, 0], 8080)).await;
}
