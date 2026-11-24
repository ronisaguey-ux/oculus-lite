//! Price/time priority matching over two `BTreeMap` books.
//!
//! Bids are iterated from the highest price down, asks from the lowest price up.
//! Each price level is a FIFO queue, so the oldest resting order at a level is
//! always the first to trade. Orders without an immediately usable price (market
//! and stop orders) live in a pending list and are evaluated on each market
//! snapshot. A stop-limit order that triggers is converted into a regular limit
//! order and joins the back of its new level, i.e. it loses queue position.

use std::collections::{BTreeMap, HashMap, VecDeque};

use crate::models::{Fill, Liquidity, Order, OrderStatus, OrderType, Side};

/// Prices are quantised to 1/10,000 so that `u64` keys can be used in the books.
/// Floating point equality is never used for ordering.
pub const PRICE_SCALE: f64 = 10_000.0;

pub fn to_ticks(price: f64) -> u64 {
    (price * PRICE_SCALE).round().max(0.0) as u64
}

pub fn from_ticks(ticks: u64) -> f64 {
    ticks as f64 / PRICE_SCALE
}

fn apply_fill(order: &mut Order, qty: f64, price: f64, ts: u64, liquidity: Liquidity) -> Fill {
    let id = order.id;
    let symbol = order.symbol.clone();
    let side = order.side;
    order.filled_qty += qty;
    order.status = if order.remaining() <= 1e-9 {
        OrderStatus::Filled
    } else {
        OrderStatus::PartiallyFilled
    };
    Fill {
        order_id: id,
        symbol,
        side,
        qty,
        price,
        liquidity,
        ts,
    }
}

#[derive(Debug, Default)]
pub struct MatchingCore {
    bids: BTreeMap<u64, VecDeque<Order>>,
    asks: BTreeMap<u64, VecDeque<Order>>,
    /// order id -> (side, price ticks); ticks == 0 means the order is pending.
    index: HashMap<u64, (Side, u64)>,
    pending: Vec<Order>,
}

impl MatchingCore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert an order. Duplicate ids and non-positive quantities are rejected.
    pub fn add_order(&mut self, order: Order) -> Result<(), String> {
        if order.qty <= 0.0 || !order.qty.is_finite() {
            return Err(format!("invalid quantity {}", order.qty));
        }
        if self.index.contains_key(&order.id) {
            return Err(format!("duplicate order id {}", order.id));
        }
        match order.order_type {
            OrderType::Limit => {
                let price = order
                    .limit_price
                    .ok_or_else(|| format!("limit order {} has no price", order.id))?;
                let tick = to_ticks(price);
                self.index.insert(order.id, (order.side, tick));
                self.book_mut(order.side).entry(tick).or_default().push_back(order);
            }
            OrderType::Market | OrderType::StopMarket | OrderType::StopLimit | OrderType::TrailingStop => {
                self.index.insert(order.id, (order.side, 0));
                self.pending.push(order);
            }
        }
        Ok(())
    }

    /// Remove an order from a book or from the pending list.
    pub fn cancel_order(&mut self, id: u64) -> Option<Order> {
        let (side, tick) = self.index.remove(&id)?;
        if tick == 0 {
            let pos = self.pending.iter().position(|o| o.id == id)?;
            return Some(self.pending.remove(pos));
        }
        let book = self.book_mut(side);
        let queue = book.get_mut(&tick)?;
        let pos = queue.iter().position(|o| o.id == id)?;
        let order = queue.remove(pos)?;
        if queue.is_empty() {
            book.remove(&tick);
        }
        Some(order)
    }

    pub fn order_exists(&self, id: u64) -> bool {
        self.index.contains_key(&id)
    }

    pub fn order_count(&self) -> usize {
        self.index.len()
    }

    pub fn best_bid(&self) -> Option<f64> {
        self.bids.keys().next_back().copied().map(from_ticks)
    }

    pub fn best_ask(&self) -> Option<f64> {
        self.asks.keys().next().copied().map(from_ticks)
    }

    /// Deterministic view of every resting order: (side, price, quantity, status).
    pub fn snapshot(&self) -> Vec<(Side, f64, f64, OrderStatus)> {
        let mut rows = Vec::new();
        for (book, side) in [(&self.bids, Side::Buy), (&self.asks, Side::Sell)] {
            for (tick, queue) in book {
                for order in queue {
                    rows.push((side, from_ticks(*tick), order.remaining(), order.status));
                }
            }
        }
        rows.sort_by(|a, b| {
            let side_rank = |s: Side| if s == Side::Buy { 0u8 } else { 1u8 };
            let price_rank = |s: Side, p: f64| if s == Side::Buy { -p } else { p };
            (side_rank(a.0), price_rank(a.0, a.1))
                .partial_cmp(&(side_rank(b.0), price_rank(b.0, b.1)))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        rows
    }

    /// Match everything that is executable at the supplied bid/ask snapshot.
    /// The returned fills are sorted by timestamp, order id and side.
    pub fn match_at(&mut self, bid: f64, ask: f64, ts: u64) -> Vec<Fill> {
        let mut fills = Vec::new();

        let pending = std::mem::take(&mut self.pending);
        for mut order in pending {
            let triggered = match order.order_type {
                OrderType::Market => true,
                _ => match order.side {
                    Side::Buy => order.stop_price.is_some_and(|p| ask >= p),
                    Side::Sell => order.stop_price.is_some_and(|p| bid <= p),
                },
            };
            if !triggered {
                self.pending.push(order);
                continue;
            }
            if order.order_type == OrderType::StopLimit {
                order.order_type = OrderType::Limit;
                let price = order.limit_price.unwrap_or(if order.is_buy() { ask } else { bid });
                let tick = to_ticks(price);
                self.index.insert(order.id, (order.side, tick));
                self.book_mut(order.side).entry(tick).or_default().push_back(order);
            } else {
                let price = if order.is_buy() { ask } else { bid };
                let qty = order.remaining();
                fills.push(apply_fill(&mut order, qty, price, ts, Liquidity::Taker));
                self.index.remove(&order.id);
            }
        }

        let mut rounds = 0usize;
        while rounds < 10_000 {
            rounds += 1;
            let (bid_tick, ask_tick) = match (self.bids.keys().next_back(), self.asks.keys().next()) {
                (Some(b), Some(a)) => (*b, *a),
                _ => break,
            };
            if from_ticks(bid_tick) < from_ticks(ask_tick) {
                break;
            }

            let mut bid_order = match self.pop_front(Side::Buy, bid_tick) {
                Some(order) => order,
                None => break,
            };
            let mut ask_order = match self.pop_front(Side::Sell, ask_tick) {
                Some(order) => order,
                None => {
                    self.push_front(Side::Buy, bid_tick, bid_order);
                    break;
                }
            };

            let qty = bid_order.remaining().min(ask_order.remaining());
            // The older order is the maker; its limit price is the execution price.
            let price = if bid_order.created_ts <= ask_order.created_ts {
                from_ticks(bid_tick)
            } else {
                from_ticks(ask_tick)
            };
            fills.push(apply_fill(&mut bid_order, qty, price, ts, Liquidity::Maker));
            fills.push(apply_fill(&mut ask_order, qty, price, ts, Liquidity::Maker));

            if bid_order.remaining() > 1e-9 {
                self.push_front(Side::Buy, bid_tick, bid_order);
            } else {
                self.index.remove(&bid_order.id);
            }
            if ask_order.remaining() > 1e-9 {
                self.push_front(Side::Sell, ask_tick, ask_order);
            } else {
                self.index.remove(&ask_order.id);
            }
        }

        fills.sort_by(|a, b| {
            let side_rank = |s: Side| if s == Side::Buy { 0u8 } else { 1u8 };
            (a.ts, a.order_id, side_rank(a.side)).cmp(&(b.ts, b.order_id, side_rank(b.side)))
        });
        fills
    }

    fn book_mut(&mut self, side: Side) -> &mut BTreeMap<u64, VecDeque<Order>> {
        match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        }
    }

    fn pop_front(&mut self, side: Side, tick: u64) -> Option<Order> {
        let book = self.book_mut(side);
        let order = match book.get_mut(&tick).and_then(VecDeque::pop_front) {
            Some(order) => order,
            None => return None,
        };
        if book.get(&tick).is_some_and(VecDeque::is_empty) {
            book.remove(&tick);
        }
        Some(order)
    }

    fn push_front(&mut self, side: Side, tick: u64, order: Order) {
        self.book_mut(side).entry(tick).or_default().push_front(order);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limit(id: u64, side: Side, price: f64, qty: f64, ts: u64) -> Order {
        Order::new(id, "TEST", side, OrderType::Limit, Some(price), None, qty, ts)
    }

    #[test]
    fn price_then_time_priority() {
        let mut core = MatchingCore::new();
        core.add_order(limit(1, Side::Buy, 100.0, 1.0, 1)).unwrap();
        core.add_order(limit(2, Side::Buy, 101.0, 1.0, 2)).unwrap();
        core.add_order(limit(3, Side::Buy, 101.0, 1.0, 3)).unwrap();
        core.add_order(limit(4, Side::Sell, 100.0, 3.0, 4)).unwrap();
        let fills = core.match_at(101.0, 100.0, 10);
        // Three aggressive fills against the resting ask, plus three maker fills.
        assert_eq!(fills.len(), 6);
        assert_eq!((fills[0].order_id, fills[0].price), (1, 100.0));
        assert_eq!((fills[1].order_id, fills[1].price), (2, 101.0));
        assert_eq!((fills[2].order_id, fills[2].price), (3, 101.0));
        assert_eq!((fills[3].order_id, fills[4].order_id, fills[5].order_id), (4, 4, 4));
        assert!(fills[3..].iter().all(|f| f.liquidity == Liquidity::Maker));
    }

    #[test]
    fn partial_fill_keeps_queue_position() {
        let mut core = MatchingCore::new();
        core.add_order(limit(1, Side::Buy, 100.0, 3.0, 1)).unwrap();
        core.add_order(limit(2, Side::Sell, 100.0, 1.0, 2)).unwrap();
        let first = core.match_at(100.0, 100.0, 3);
        assert_eq!(first.len(), 2);
        assert_eq!(core.best_bid(), Some(100.0));
        let snapshot = core.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert!((snapshot[0].2 - 2.0).abs() < 1e-9);
        assert_eq!(snapshot[0].3, OrderStatus::PartiallyFilled);
    }

    #[test]
    fn stop_and_market_orders() {
        let mut core = MatchingCore::new();
        let stop = Order::new(
            7,
            "TEST",
            Side::Buy,
            OrderType::StopMarket,
            None,
            Some(105.0),
            2.0,
            1,
        );
        core.add_order(stop).unwrap();
        assert!(core.match_at(100.0, 101.0, 2).is_empty());
        let fills = core.match_at(104.0, 106.0, 3);
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].price, 106.0);
        assert_eq!(fills[0].liquidity, Liquidity::Taker);
        assert!(!core.order_exists(7));
    }

    #[test]
    fn cancel_removes_from_book() {
        let mut core = MatchingCore::new();
        core.add_order(limit(1, Side::Buy, 90.0, 1.0, 1)).unwrap();
        let removed = core.cancel_order(1).unwrap();
        assert_eq!(removed.id, 1);
        assert_eq!(core.order_count(), 0);
        assert_eq!(core.best_bid(), None);
        assert!(core.match_at(100.0, 100.0, 2).is_empty());
    }
}
