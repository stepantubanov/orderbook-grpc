use crate::{
    exchange::*,
    order_book::{Aggregator, OrderBook, Sources},
};
use futures::{Future, StreamExt};
use std::pin::Pin;

pub fn build(exchanges: Vec<Exchange<OrderBook<10>>>) -> Exchange<(OrderBook<10>, Sources<10>)> {
    let mut aggregator = Aggregator::new(exchanges.iter().map(|e| e.id));

    let (conns, streams): (Vec<_>, Vec<_>) = exchanges
        .into_iter()
        .enumerate()
        .map(|(i, e)| (e.conn, e.stream.map(move |book| (i, book))))
        .unzip();

    let stream = futures::stream::select_all(streams)
        .map(move |(i, book)| {
            aggregator.update(i, book);
            aggregator.aggregate()
        })
        .boxed();

    Exchange {
        id: ExchangeId::Aggregate,
        conn: Box::new(AggregateConn { conns }),
        stream,
    }
}

struct AggregateConn {
    conns: Vec<Box<dyn Connection + Send>>,
}

impl Connection for AggregateConn {
    fn close<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        Box::pin(async {
            for conn in self.conns.iter_mut() {
                conn.close().await;
            }
        })
    }
}
