//! Reconciliation of local state against a venue snapshot. Produces a sorted
//! list of discrepancies rather than mutating either side, so that callers can
//! decide whether to cancel, requeue or halt.

use std::collections::BTreeMap;

use crate::models::Order;

#[derive(Debug, Clone, PartialEq)]
pub struct PositionSnapshot {
    pub symbol: String,
    pub qty: f64,
    pub avg_price: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Discrepancy {
    MissingAtVenue { order_id: u64 },
    MissingLocally { order_id: u64 },
    QuantityMismatch { order_id: u64, local: u64, venue: u64 },
    PositionMismatch { symbol: String, local: i64, venue: i64 },
}

fn rank(d: &Discrepancy) -> (u8, u64, i64) {
    match d {
        Discrepancy::MissingAtVenue { order_id } => (0, *order_id, 0),
        Discrepancy::MissingLocally { order_id } => (1, *order_id, 0),
        Discrepancy::QuantityMismatch { order_id, .. } => (2, *order_id, 0),
        Discrepancy::PositionMismatch { symbol, .. } => {
            (3, 0, symbol.bytes().take(8).fold(0i64, |acc, b| acc * 256 + b as i64))
        }
    }
}

/// Compare open orders and positions. Quantities are compared in integer ticks
/// (1e-4) to avoid spurious differences from floating point serialisation.
pub fn reconcile_orders(
    local: &[Order],
    venue: &[Order],
    local_positions: &[PositionSnapshot],
    venue_positions: &[PositionSnapshot],
) -> Vec<Discrepancy> {
    let ticks = |qty: f64| (qty * 10_000.0).round() as i64;
    let local_orders: BTreeMap<u64, &Order> = local.iter().map(|o| (o.id, o)).collect();
    let venue_orders: BTreeMap<u64, &Order> = venue.iter().map(|o| (o.id, o)).collect();

    let mut out = Vec::new();
    for id in local_orders.keys() {
        if !venue_orders.contains_key(id) {
            out.push(Discrepancy::MissingAtVenue { order_id: *id });
        }
    }
    for id in venue_orders.keys() {
        if !local_orders.contains_key(id) {
            out.push(Discrepancy::MissingLocally { order_id: *id });
        }
    }
    for (id, lo) in &local_orders {
        if let Some(vo) = venue_orders.get(id) {
            let l = ticks(lo.remaining());
            let v = ticks(vo.remaining());
            if l != v {
                out.push(Discrepancy::QuantityMismatch {
                    order_id: *id,
                    local: l as u64,
                    venue: v as u64,
                });
            }
        }
    }

    let local_pos: BTreeMap<&str, &PositionSnapshot> = local_positions
        .iter()
        .map(|p| (p.symbol.as_str(), p))
        .collect();
    let venue_pos: BTreeMap<&str, &PositionSnapshot> = venue_positions
        .iter()
        .map(|p| (p.symbol.as_str(), p))
        .collect();
    let mut symbols: Vec<&str> = local_pos.keys().copied().chain(venue_pos.keys().copied()).collect();
    symbols.sort_unstable();
    symbols.dedup();
    for symbol in symbols {
        let l = local_pos.get(symbol).map_or(0, |p| ticks(p.qty));
        let v = venue_pos.get(symbol).map_or(0, |p| ticks(p.qty));
        if l != v {
            out.push(Discrepancy::PositionMismatch {
                symbol: symbol.to_owned(),
                local: l,
                venue: v,
            });
        }
    }

    out.sort_by_key(rank);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{OrderType, Side};

    fn order(id: u64, remaining: f64) -> Order {
        let mut o = Order::new(id, "TEST", Side::Buy, OrderType::Limit, Some(100.0), None, 2.0, 0);
        o.filled_qty = 2.0 - remaining;
        o
    }

    fn pos(qty: f64) -> PositionSnapshot {
        PositionSnapshot {
            symbol: "TEST".to_owned(),
            qty,
            avg_price: 100.0,
        }
    }

    #[test]
    fn finds_missing_and_mismatched_state() {
        let local = vec![order(1, 1.0), order(2, 1.0)];
        let venue = vec![order(1, 0.5), order(3, 1.0)];
        let diffs = reconcile_orders(&local, &venue, &[pos(1.0)], &[pos(0.5)]);
        assert_eq!(
            diffs,
            vec![
                Discrepancy::MissingAtVenue { order_id: 2 },
                Discrepancy::MissingLocally { order_id: 3 },
                Discrepancy::QuantityMismatch {
                    order_id: 1,
                    local: 10_000,
                    venue: 5_000,
                },
                Discrepancy::PositionMismatch {
                    symbol: "TEST".to_owned(),
                    local: 10_000,
                    venue: 5_000,
                },
            ]
        );
    }

    #[test]
    fn clean_state_produces_no_discrepancies() {
        let orders = vec![order(1, 1.0)];
        assert!(reconcile_orders(&orders, &orders, &[pos(2.0)], &[pos(2.0)]).is_empty());
    }
}
