//! Local order lifecycle management. Every accepted order and every state
//! change is recorded in an append-only event log.

use std::collections::BTreeMap;

use crate::models::{Event, Fill, Order, OrderStatus};

#[derive(Debug, Default)]
pub struct OrderManager {
    orders: BTreeMap<u64, Order>,
    events: Vec<Event>,
}

impl OrderManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, order: Order) -> Result<(), String> {
        if order.qty <= 0.0 {
            return Err("order quantity must be positive".to_owned());
        }
        if self.orders.contains_key(&order.id) {
            return Err(format!("order {} already tracked", order.id));
        }
        self.events.push(Event::Accepted(order.id));
        self.orders.insert(order.id, order);
        Ok(())
    }

    pub fn mark_working(&mut self, id: u64) -> Result<(), String> {
        let order = self.orders.get_mut(&id).ok_or_else(|| format!("unknown order {id}"))?;
        if order.status != OrderStatus::Accepted {
            return Err(format!("order {id} is {}", order.status));
        }
        order.status = OrderStatus::Working;
        self.events.push(Event::Working(id));
        Ok(())
    }

    pub fn transition(&mut self, id: u64, status: OrderStatus) -> Result<(), String> {
        let order = self.orders.get_mut(&id).ok_or_else(|| format!("unknown order {id}"))?;
        if order.status.is_terminal() {
            return Err(format!("order {id} is already terminal"));
        }
        order.status = status;
        match status {
            OrderStatus::Canceled => self.events.push(Event::Canceled(id)),
            OrderStatus::Rejected => self.events.push(Event::Rejected {
                order_id: id,
                reason: "rejected by execution layer".to_owned(),
            }),
            _ => {}
        }
        Ok(())
    }

    /// Apply a fill, rejecting overfills and fills of terminal orders.
    pub fn apply_fill(&mut self, fill: &Fill) -> Result<(), String> {
        if fill.qty <= 0.0 || !fill.qty.is_finite() {
            return Err("fill quantity must be positive and finite".to_owned());
        }
        let order = self
            .orders
            .get_mut(&fill.order_id)
            .ok_or_else(|| format!("unknown order {}", fill.order_id))?;
        if order.status.is_terminal() {
            return Err(format!("order {} is already terminal", fill.order_id));
        }
        if fill.qty > order.remaining() + 1e-9 {
            return Err(format!("fill {} exceeds remaining {}", fill.qty, order.remaining()));
        }
        order.filled_qty += fill.qty;
        order.status = if order.remaining() <= 1e-9 {
            OrderStatus::Filled
        } else {
            OrderStatus::PartiallyFilled
        };
        self.events.push(Event::Fill(fill.clone()));
        Ok(())
    }

    pub fn get(&self, id: u64) -> Option<&Order> {
        self.orders.get(&id)
    }

    pub fn status(&self, id: u64) -> Option<OrderStatus> {
        self.orders.get(&id).map(|o| o.status)
    }

    /// Open orders in ascending id order, so the result is deterministic.
    pub fn open_orders(&self) -> Vec<&Order> {
        self.orders
            .values()
            .filter(|o| !o.status.is_terminal())
            .collect()
    }

    pub fn all_orders(&self) -> Vec<&Order> {
        self.orders.values().collect()
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Liquidity, OrderType, Side};

    fn order(id: u64, qty: f64) -> Order {
        Order::new(id, "TEST", Side::Buy, OrderType::Market, None, None, qty, 1)
    }

    fn fill(id: u64, qty: f64) -> Fill {
        Fill {
            order_id: id,
            symbol: "TEST".to_owned(),
            side: Side::Buy,
            qty,
            price: 100.0,
            liquidity: Liquidity::Taker,
            ts: 2,
        }
    }

    #[test]
    fn tracks_partial_then_full_fill() {
        let mut mgr = OrderManager::new();
        mgr.add(order(1, 3.0)).unwrap();
        mgr.apply_fill(&fill(1, 1.0)).unwrap();
        assert_eq!(mgr.status(1), Some(OrderStatus::PartiallyFilled));
        mgr.apply_fill(&fill(1, 2.0)).unwrap();
        assert_eq!(mgr.status(1), Some(OrderStatus::Filled));
        assert!(mgr.open_orders().is_empty());
    }

    #[test]
    fn rejects_overfill_and_terminal_transition() {
        let mut mgr = OrderManager::new();
        mgr.add(order(1, 1.0)).unwrap();
        mgr.apply_fill(&fill(1, 1.0)).unwrap();
        assert!(mgr.apply_fill(&fill(1, 0.5)).is_err());
        assert!(mgr.transition(1, OrderStatus::Canceled).is_err());
    }

    #[test]
    fn event_log_is_append_only() {
        let mut mgr = OrderManager::new();
        mgr.add(order(1, 1.0)).unwrap();
        mgr.mark_working(1).unwrap();
        mgr.apply_fill(&fill(1, 1.0)).unwrap();
        assert_eq!(
            mgr.events(),
            &[
                Event::Accepted(1),
                Event::Working(1),
                Event::Fill(fill(1, 1.0)),
            ]
        );
    }
}
