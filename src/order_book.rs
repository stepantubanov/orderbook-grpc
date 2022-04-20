use std::{cell::RefCell, cmp::Ordering, mem::MaybeUninit};

use crate::exchange::ExchangeId;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PriceLevel {
    pub price: f64,
    pub amount: f64,
}

impl From<(f64, f64)> for PriceLevel {
    fn from(t: (f64, f64)) -> Self {
        Self {
            price: t.0,
            amount: t.1,
        }
    }
}

impl From<[&str; 2]> for PriceLevel {
    fn from(src: [&str; 2]) -> Self {
        let price = src[0].parse::<f64>().expect("invalid price");
        let amount = src[1].parse::<f64>().expect("invalid amount");

        Self { price, amount }
    }
}

#[derive(Clone)]
pub struct OrderBook<const N: usize> {
    pub bid: [PriceLevel; N],
    pub ask: [PriceLevel; N],
}

pub struct Sources<const N: usize> {
    pub bid: [ExchangeId; N],
    pub ask: [ExchangeId; N],
}

impl<const N: usize> Default for OrderBook<N> {
    fn default() -> Self {
        Self {
            bid: [PriceLevel {
                price: 0.0,
                amount: 0.0,
            }; N],
            ask: [PriceLevel {
                price: f64::INFINITY,
                amount: 0.0,
            }; N],
        }
    }
}

impl<const N: usize> OrderBook<N> {
    pub fn parse(bid_str: &[[&str; 2]; N], ask_str: &[[&str; 2]; N]) -> Self {
        Self {
            bid: bid_str.map(|s| PriceLevel::from(s)),
            ask: ask_str.map(|s| PriceLevel::from(s)),
        }
    }

    pub fn from(bid: &[(f64, f64); N], ask: &[(f64, f64); N]) -> Self {
        Self {
            bid: bid.map(|s| PriceLevel::from(s)),
            ask: ask.map(|s| PriceLevel::from(s)),
        }
    }
}

pub struct Aggregator<const N: usize> {
    indexes: RefCell<Vec<usize>>,
    exchange_ids: Vec<ExchangeId>,
    order_books: Vec<OrderBook<N>>,
}

impl<const N: usize> Aggregator<N> {
    pub fn new<I: ExactSizeIterator<Item = ExchangeId>>(exchange_ids: I) -> Self {
        let indexes = RefCell::new(vec![0; exchange_ids.len()]);
        let order_books = vec![OrderBook::default(); exchange_ids.len()];

        Self {
            indexes,
            exchange_ids: exchange_ids.collect(),
            order_books,
        }
    }

    pub fn update(&mut self, idx: usize, order_book: OrderBook<N>) {
        self.order_books[idx] = order_book;
    }

    pub fn aggregate(&self) -> (OrderBook<N>, Sources<N>) {
        let (bid, bid_sources) = merge(
            |i, j| (self.exchange_ids[i], &self.order_books[i].bid[j]),
            &self.indexes,
            |a, b| b.partial_cmp(&a).unwrap(),
        );
        let (ask, ask_sources) = merge(
            |i, j| (self.exchange_ids[i], &self.order_books[i].ask[j]),
            &self.indexes,
            |a, b| a.partial_cmp(&b).unwrap(),
        );

        #[cfg(debug_assertions)]
        {
            for i in 1..N {
                assert!(bid[i - 1].price >= bid[i].price);
                assert!(ask[i - 1].price <= ask[i].price);
            }
        }

        (
            OrderBook { bid, ask },
            Sources {
                bid: bid_sources,
                ask: ask_sources,
            },
        )
    }
}

fn merge<'a, B, F, const N: usize>(
    price_levels: B,
    indexes: &RefCell<Vec<usize>>,
    cmp: F,
) -> ([PriceLevel; N], [ExchangeId; N])
where
    B: Fn(usize, usize) -> (ExchangeId, &'a PriceLevel),
    F: Fn(f64, f64) -> Ordering,
{
    let mut indexes = indexes.borrow_mut();
    indexes.fill(0);

    // SAFETY: `assume_init` is safe because each element is MaybeUninit
    // which is not requiring initialization.
    let mut levels: [MaybeUninit<PriceLevel>; N] = unsafe { MaybeUninit::uninit().assume_init() };
    // SAFETY: `assume_init` is safe because each element is MaybeUninit
    // which is not requiring initialization.
    let mut sources: [MaybeUninit<ExchangeId>; N] = unsafe { MaybeUninit::uninit().assume_init() };

    for i in 0..N {
        let (j, exchange_id, level) = (0..indexes.len())
            .map(|j| {
                let (exchange_id, level) = price_levels(j, indexes[j]);
                (j, exchange_id, level)
            })
            .min_by(|(_, _, a), (_, _, b)| cmp(a.price, b.price))
            .unwrap();

        levels[i].write(*level);
        sources[i].write(exchange_id);
        indexes[j] += 1;
    }

    // SAFETY: Both arrays have been fully initialized in the previous loop (0..N).
    unsafe { (assume_init_arr(&levels), assume_init_arr(&sources)) }
}

unsafe fn assume_init_arr<T, const N: usize>(arr: &[MaybeUninit<T>; N]) -> [T; N] {
    arr.as_ptr().cast::<[T; N]>().read()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_order_book() {
        let book = OrderBook::<2>::parse(
            &[["0.07500500", "0.09700000"], ["0.07500300", "10.85240000"]],
            &[["0.07501000", "0.16140000"], ["0.07501300", "0.10000000"]],
        );

        assert_eq!(book.bid[0].price, 0.07500500);
        assert_eq!(book.bid[0].amount, 0.09700000);
        assert_eq!(book.ask[0].price, 0.07501000);
        assert_eq!(book.ask[0].amount, 0.16140000);
    }

    #[test]
    fn aggregates_order_books() {
        let b0 = OrderBook::from(&[(100.0, 1.0), (95.0, 0.5)], &[(110.0, 0.25), (115.0, 0.5)]);
        let b1 = OrderBook::from(&[(101.0, 1.5), (94.0, 0.4)], &[(112.0, 0.25), (118.0, 0.5)]);

        let mut aggregator =
            Aggregator::new([ExchangeId::Binance, ExchangeId::Bitstamp].into_iter());
        aggregator.update(0, b0);
        aggregator.update(1, b1);

        let (book, sources) = aggregator.aggregate();

        assert_eq!(book.bid[0], (101.0, 1.5).into());
        assert_eq!(sources.bid[0], ExchangeId::Bitstamp);
        assert_eq!(book.bid[1], (100.0, 1.0).into());
        assert_eq!(sources.bid[1], ExchangeId::Binance);

        assert_eq!(book.ask[0], (110.0, 0.25).into());
        assert_eq!(sources.ask[0], ExchangeId::Binance);
        assert_eq!(book.ask[1], (112.0, 0.25).into());
        assert_eq!(sources.ask[1], ExchangeId::Bitstamp);
    }
}
