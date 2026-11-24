//! Emulation of venue order types that are not natively supported: stop and
//! stop-limit orders are held locally until their trigger is crossed, then
//! converted into an ordinary market or limit order.

use std::collections::BTreeMap;

use crate::models::{Order, OrderStatus, OrderType, Side};

#[derive(Debug, Clone, PartialEq)]
pub enum EmulatorAction {
    Submit(Order),
    Cancel(u64),
}

#[derive(Debug, Default)]
pub struct OrderEmulator {
    stops: BTreeMap<u64, Order>,
    next_id: u64,
}

impl OrderEmulator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, mut order: Order) -> Result<u64, String> {
        if !matches!(
            order.order_type,
            OrderType::StopMarket | OrderType::StopLimit | OrderType::TrailingStop
        ) {
            return Err("emulator only accepts stop orders".to_owned());
        }
        if order.stop_price.is_none() {
            return Err("stop order has no trigger price".to_owned());
        }
        let id = order.id;
        order.status = OrderStatus::Working;
        self.stops.insert(id, order);
        self.next_id = self.next_id.max(id + 1);
        Ok(id)
    }

    pub fn cancel(&mut self, id: u64) -> Option<Order> {
        self.stops.remove(&id)
    }

    pub fn pending(&self) -> Vec<&Order> {
        self.stops.values().collect()
    }

    /// Evaluate all stop orders against a bid/ask snapshot. Triggered stops are
    /// removed locally and returned as ordinary orders for the caller to submit.
    pub fn on_market(&mut self, bid: f64, ask: f64, ts: u64) -> Vec<EmulatorAction> {
        let triggered: Vec<u64> = self
            .stops
            .values()
            .filter_map(|order| {
                let trigger = order.stop_price?;
                let hit = match order.side {
                    Side::Buy => ask >= trigger,
                    Side::Sell => bid <= trigger,
                };
                hit.then_some(order.id)
            })
            .collect();

        let mut actions = Vec::new();
        for id in triggered {
            let mut order = match self.stops.remove(&id) {
                Some(order) => order,
                None => continue,
            };
            let converted_type = if order.order_type == OrderType::StopLimit {
                OrderType::Limit
            } else {
                OrderType::Market
            };
            order.id = self.next_id;
            order.client_id = format!("E-{:06}", self.next_id);
            order.order_type = converted_type;
            order.status = OrderStatus::Accepted;
            order.created_ts = ts;
            self.next_id += 1;
            actions.push(EmulatorAction::Submit(order));
            actions.push(EmulatorAction::Cancel(id));
        }
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stop(id: u64, side: Side, trigger: f64) -> Order {
        Order::new(
            id,
            "TEST",
            side,
            OrderType::StopMarket,
            None,
            Some(trigger),
            1.0,
            0,
        )
    }

    #[test]
    fn triggers_only_when_price_crosses() {
        let mut emu = OrderEmulator::new();
        emu.add(stop(1, Side::Buy, 105.0)).unwrap();
        emu.add(stop(2, Side::Sell, 95.0)).unwrap();
        assert!(emu.on_market(100.0, 101.0, 1).is_empty());
        let actions = emu.on_market(100.0, 106.0, 2);
        assert_eq!(actions.len(), 2);
        match &actions[0] {
            EmulatorAction::Submit(order) => {
                assert_eq!(order.order_type, OrderType::Market);
                assert_eq!(order.side, Side::Buy);
            }
            other => panic!("unexpected action {other:?}"),
        }
        assert_eq!(actions[1], EmulatorAction::Cancel(1));
        assert_eq!(emu.pending().len(), 1);
        assert!(!emu.on_market(94.0, 96.0, 3).is_empty());
        assert!(emu.pending().is_empty());
    }

    #[test]
    fn stop_limit_becomes_limit_order() {
        let mut emu = OrderEmulator::new();
        let mut order = stop(9, Side::Sell, 90.0);
        order.order_type = OrderType::StopLimit;
        order.limit_price = Some(89.5);
        emu.add(order).unwrap();
        let actions = emu.on_market(89.0, 90.0, 5);
        match &actions[0] {
            EmulatorAction::Submit(order) => {
                assert_eq!(order.order_type, OrderType::Limit);
                assert_eq!(order.limit_price, Some(89.5));
            }
            other => panic!("unexpected action {other:?}"),
        }
    }
}
