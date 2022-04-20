tonic::include_proto!("orderbook");

use futures::{future, stream::BoxStream, StreamExt};
use orderbook_aggregator_server::{OrderbookAggregator, OrderbookAggregatorServer};

use crate::{
    aggregate, binance, bitstamp,
    exchange::{ExchangeId, MarketId},
    order_book::{OrderBook, PriceLevel, Sources},
};

pub struct Aggregator;

fn map_to_levels<'a, I>(levels: I) -> Vec<Level>
where
    I: Iterator<Item = (&'a PriceLevel, ExchangeId)>,
{
    levels
        .map(|(level, exchange_id)| Level {
            exchange: exchange_id.to_string(),
            price: level.price,
            amount: level.amount,
        })
        .collect()
}

fn map_to_summary((book, sources): (OrderBook<10>, Sources<10>)) -> Summary {
    let spread = book.ask[0].price - book.bid[0].price;

    Summary {
        spread,
        bids: map_to_levels(book.bid.iter().zip(sources.bid.into_iter())),
        asks: map_to_levels(book.ask.iter().zip(sources.ask.into_iter())),
    }
}

#[tonic::async_trait]
impl OrderbookAggregator for Aggregator {
    type BookSummaryStream = BoxStream<'static, Result<Summary, tonic::Status>>;

    async fn book_summary(
        &self,
        _request: tonic::Request<Empty>,
    ) -> Result<tonic::Response<Self::BookSummaryStream>, tonic::Status> {
        let market_id = MarketId("ethbtc".into());
        let binance = binance::connect(market_id.clone());
        let bitstamp = bitstamp::connect(market_id.clone());

        let (binance, bitstamp) = future::try_join(binance, bitstamp)
            .await
            .map_err(|e| tonic::Status::unavailable(e.to_string()))?;

        let agg = aggregate::build(vec![binance, bitstamp]);
        let stream = agg.stream.map(map_to_summary).map(Ok);

        Ok(tonic::Response::new(stream.boxed()))
    }
}

pub async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:50123".parse().unwrap();

    tonic::transport::Server::builder()
        .add_service(OrderbookAggregatorServer::new(Aggregator))
        .serve(addr)
        .await?;

    Ok(())
}
