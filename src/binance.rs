use crate::exchange::*;
use crate::order_book::OrderBook;
use futures::{future, Future, Sink, SinkExt, StreamExt};
use std::error;
use std::pin::Pin;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

pub async fn connect(
    market_id: MarketId,
) -> Result<Exchange<OrderBook<10>>, Box<dyn error::Error>> {
    let (stream, _) = connect_async(format!(
        "wss://stream.binance.com:9443/ws/{}@depth20@100ms",
        market_id.0
    ))
    .await?;

    let (sink, stream) = stream.split();

    let stream = stream
        .filter_map(|message| match message.unwrap() {
            Message::Text(text) => {
                let order_book = parse::parse_order_book(&text);
                future::ready(Some(order_book))
            }
            _ => future::ready(None),
        })
        .boxed();

    Ok(Exchange {
        id: ExchangeId::Binance,
        conn: Box::new(BinanceConn { sink }),
        stream,
    })
}

struct BinanceConn<S: Sink<Message> + Unpin> {
    sink: S,
}

impl<S: Sink<Message> + Unpin + 'static> Connection for BinanceConn<S> {
    fn close<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        Box::pin(async {
            self.sink.close().await.unwrap_or(());
        })
    }
}

mod parse {
    use crate::deserialize_helper::*;
    use crate::order_book::OrderBook;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Book<'a> {
        #[serde(borrow)]
        #[serde(deserialize_with = "deserialize_first_10")]
        bids: [[&'a str; 2]; 10],

        #[serde(borrow)]
        #[serde(deserialize_with = "deserialize_first_10")]
        asks: [[&'a str; 2]; 10],
    }

    pub fn parse_order_book(raw: &str) -> OrderBook<10> {
        let book: Book = serde_json::from_str(raw).unwrap();
        OrderBook::parse(&book.bids, &book.asks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_depth20_message() {
        let message = include_str!("../test/binance_book.json");
        let book = parse::parse_order_book(message);

        assert_eq!(book.bid[0], (0.07500200, 17.24140000).into());
        assert_eq!(book.bid[5], (0.07498800, 6.35150000).into());

        assert_eq!(book.ask[0], (0.07500300, 10.85240000).into());
        assert_eq!(book.ask[5], (0.07502100, 7.16630000).into());
    }
}
