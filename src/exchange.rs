use futures::{stream::BoxStream, Future};
use std::fmt;
use std::pin::Pin;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ExchangeId {
    Binance,
    Bitstamp,

    Aggregate,
}

impl fmt::Display for ExchangeId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            ExchangeId::Binance => "binance",
            ExchangeId::Bitstamp => "bitstamp",
            _ => panic!("unexpected id"),
        })
    }
}

#[derive(Clone)]
pub struct MarketId(pub String);

impl std::borrow::Borrow<str> for MarketId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

pub struct Exchange<T> {
    pub id: ExchangeId,
    pub conn: Box<dyn Connection + Send>,
    pub stream: BoxStream<'static, T>,
}

pub trait Connection {
    fn close<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + 'a>>;
}
