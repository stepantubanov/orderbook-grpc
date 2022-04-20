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
    let (stream, _) = connect_async("wss://ws.bitstamp.net").await?;

    let subscribe = format!(
        r#"
        {{
            "event": "bts:subscribe",
            "data": {{
                "channel": "order_book_{}"
            }}
        }}"#,
        market_id.0
    );

    let (mut sink, stream) = stream.split();
    sink.send(Message::Text(subscribe)).await?;

    let stream = stream
        .filter_map(|message| match message.unwrap() {
            Message::Text(text) => {
                let order_book = parse::parse_order_book(&text);
                future::ready(order_book)
            }
            _ => future::ready(None),
        })
        .boxed();

    Ok(Exchange {
        id: ExchangeId::Bitstamp,
        conn: Box::new(BitstampConn { sink }),
        stream,
    })
}

struct BitstampConn<S: Sink<Message> + Unpin> {
    sink: S,
}

impl<S: Sink<Message> + Unpin + 'static> Connection for BitstampConn<S> {
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

    #[derive(Deserialize)]
    struct Message<'a> {
        #[serde(borrow)]
        event: &'a str,

        #[serde(borrow)]
        data: &'a serde_json::value::RawValue,
    }

    pub fn parse_order_book(raw: &str) -> Option<OrderBook<10>> {
        let message: Message = serde_json::from_str(raw).unwrap();
        match message.event {
            "data" => {
                let book: Book = serde_json::from_str(message.data.get()).unwrap();
                Some(OrderBook::parse(&book.bids, &book.asks))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_book_message() {
        let message = include_str!("../test/bitstamp_book.json");
        let book = parse::parse_order_book(message).unwrap();

        assert_eq!(book.bid[0], (0.07469407, 0.05000000).into());
        assert_eq!(book.bid[2], (0.07468831, 0.46500000).into());

        assert_eq!(book.ask[0], (0.07473221, 1.61774202).into());
        assert_eq!(book.ask[2], (0.07473638, 16.18133971).into());
    }
}
