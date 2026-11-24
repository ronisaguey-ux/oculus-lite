//! Execution client abstraction and its deterministic paper implementation.
//! A live client would implement the same trait against a venue SDK; the lite
//! edition deliberately ships no venue SDK and no endpoint configuration.

use crate::matching_core::MatchingCore;
use crate::models::{Bar, Fill, Order, OrderStatus, OrderType, Side};
use crate::order_manager::OrderManager;

pub trait ExecutionClient {
    fn submit_order(
        &mut self,
        symbol: &str,
        side: Side,
        order_type: OrderType,
        qty: f64,
        limit_price: Option<f64>,
        stop_price: Option<f64>,
        ts: u64,
    ) -> Result<u64, String>;

    fn cancel_order(&mut self, id: u64) -> Result<(), String>;

    /// Deliver a new market snapshot; returns any fills produced by it.
    fn on_bar(&mut self, bar: &Bar) -> Vec<Fill>;

    fn fills(&self) -> &[Fill];
}

#[derive(Debug)]
pub struct PaperClient {
    core: MatchingCore,
    manager: OrderManager,
    fills: Vec<Fill>,
    slippage_bps: f64,
    next_id: u64,
}

impl Default for PaperClient {
    fn default() -> Self {
        Self::with_slippage_bps(0.0)
    }
}

impl PaperClient {
    pub fn with_slippage_bps(slippage_bps: f64) -> Self {
        Self {
            core: MatchingCore::new(),
            manager: OrderManager::new(),
            fills: Vec::new(),
            slippage_bps,
            next_id: 1,
        }
    }

    pub fn manager(&self) -> &OrderManager {
        &self.manager
    }

    pub fn core(&self) -> &MatchingCore {
        &self.core
    }
}

impl ExecutionClient for PaperClient {
    fn submit_order(
        &mut self,
        symbol: &str,
        side: Side,
        order_type: OrderType,
        qty: f64,
        limit_price: Option<f64>,
        stop_price: Option<f64>,
        ts: u64,
    ) -> Result<u64, String> {
        let id = self.next_id;
        let order = Order::new(id, symbol, side, order_type, limit_price, stop_price, qty, ts);
        self.core.add_order(order.clone())?;
        self.manager.add(order)?;
        self.next_id += 1;
        Ok(id)
    }

    fn cancel_order(&mut self, id: u64) -> Result<(), String> {
        if self.core.cancel_order(id).is_none() {
            return Err(format!("order {id} not found"));
        }
        self.manager.transition(id, OrderStatus::Canceled)
    }

    fn on_bar(&mut self, bar: &Bar) -> Vec<Fill> {
        let half_spread = bar.open * self.slippage_bps / 10_000.0 / 2.0;
        let bid = bar.open - half_spread;
        let ask = bar.open + half_spread;
        let fills = self.core.match_at(bid, ask, bar.ts);
        for fill in &fills {
            // A partial fill is valid; overfills cannot occur in the core.
            if let Err(reason) = self.manager.apply_fill(&fill) {
                self.manager.transition(fill.order_id, OrderStatus::Rejected).ok();
                debug_assert!(false, "{reason}");
            }
        }
        self.fills.extend(fills.iter().cloned());
        fills
    }

    fn fills(&self) -> &[Fill] {
        &self.fills
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(open: f64, ts: u64) -> Bar {
        Bar {
            ts,
            open,
            high: open + 1.0,
            low: open - 1.0,
            close: open,
            volume: 100.0,
        }
    }

    #[test]
    fn market_order_fills_at_next_bar_open() {
        let mut client = PaperClient::default();
        let id = client
            .submit_order("TEST", Side::Buy, OrderType::Market, 2.0, None, None, 1)
            .unwrap();
        assert!(!client.on_bar(&bar(100.0, 2)).is_empty());
        let fills = client.fills();
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].order_id, id);
        assert_eq!(fills[0].price, 100.0);
        assert_eq!(client.manager().status(id), Some(OrderStatus::Filled));
    }

    #[test]
    fn slippage_is_deterministic() {
        let mut client = PaperClient::with_slippage_bps(10.0);
        client
            .submit_order("TEST", Side::Sell, OrderType::Market, 1.0, None, None, 1)
            .unwrap();
        let fills = client.on_bar(&bar(100.0, 2));
        assert_eq!(fills[0].price, 99.95);
    }
}
