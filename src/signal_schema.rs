use crate::relay_server::{RelayServer, SessionToken};
use async_graphql::guard::Guard;
use async_graphql::{scalar, Context, Data, EmptyMutation, Object, Schema, Subscription};
use futures::{stream, Stream};
use std::sync::Arc;
use tokio::sync::Mutex;

struct SessionGuard;
#[async_trait::async_trait]
impl Guard for SessionGuard {
    async fn check(&self, ctx: &Context<'_>) -> async_graphql::Result<()> {
        match ctx.data_opt::<SessionToken>() {
            Some(_) => Ok(()),
            None => Err("Session token is required".into()),
        }
    }
}

#[derive(Default)]
pub struct QueryRoot;
#[Object]
impl QueryRoot {
    // async fn current_token<'a>(&self, ctx: &'a Context<'_>) -> Option<&'a str> {
    //     ctx.data_opt::<SessionToken>().map(|token| token.0.as_str())
    // }
    async fn test(&self) -> bool {
        false
    }

    // async fn init(&self, ctx: &Context<'_>) -> ServerInitMessage {}
}

#[derive(Default)]
pub struct MutationRoot;
#[Object]
impl MutationRoot {
    #[graphql(guard(SessionGuard()))]
    async fn init(&self) -> bool {
        false
    }

    async fn connect_transport(&self) -> bool {
        false
    }

    async fn produce(&self) -> bool {
        false
    }

    async fn produce_data(&self) -> bool {
        false
    }

    async fn consumer_resume(&self) -> bool {
        false
    }
}

#[derive(Default)]
pub struct SubscriptionRoot;
#[Subscription]
impl SubscriptionRoot {
    async fn init(&self, ctx: &Context<'_>) -> async_graphql::Result<impl Stream<Item = i32>> {
        // if ctx.data::<SessionToken>()?.0 != "123456" {
        //     return Err("Forbidden".into());
        // }
        // ctx.data()
        Ok(stream::once(async move { 10 }))
    }
}

pub type SignalSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

pub fn schema(server: Arc<Mutex<RelayServer>>) -> SignalSchema {
    SignalSchema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(server)
        .finish()
}
