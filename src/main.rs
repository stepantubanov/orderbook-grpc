use std::time::Duration;

use exchange::MarketId;
use futures::{future, StreamExt};

mod aggregate;
mod binance;
mod bitstamp;
mod deserialize_helper;
mod exchange;
mod grpc;
mod order_book;

// Example usage
#[allow(dead_code)]
async fn print_aggregated() {
    let binance = binance::connect(MarketId("ethbtc".into()))
        .await
        .expect("binance connection failed");

    let bitstamp = bitstamp::connect(MarketId("ethbtc".into()))
        .await
        .expect("bitstamp connection failed");

    let mut agg = aggregate::build(vec![binance, bitstamp]);

    let messages = agg.stream.for_each(|(book, sources)| {
        println!("{:?}, {:?}", book.bid, sources.bid);
        future::ready(())
    });

    tokio::select! {
        _ = messages => {},
        _ = tokio::time::sleep(Duration::from_secs(5)) => {
            agg.conn.close().await;
            println!("closed");
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    grpc::serve().await.unwrap();
}
