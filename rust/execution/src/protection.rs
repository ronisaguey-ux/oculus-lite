//! Pre-trade and post-trade risk checks. This module contains no alpha: it is a
//! set of mechanical limits on size, exposure, loss and order rate.

use std::fmt;

use crate::models::Order;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiskLimits {
    pub max_position_qty: f64,
    pub max_order_qty: f64,
    pub max_daily_loss_pct: f64,
    pub max_drawdown_pct: f64,
    pub max_orders_per_bar: u32,
    pub max_consecutive_losses: u32,
}

impl Default for RiskLimits {
    fn default() -> Self {
        Self {
            max_position_qty: 10.0,
            max_order_qty: 5.0,
            max_daily_loss_pct: 3.0,
            max_drawdown_pct: 10.0,
            max_orders_per_bar: 2,
            max_consecutive_losses: 5,
        }
    }
}

/// Mutable account state supplied by the ledger on every check.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AccountState {
    pub equity: f64,
    pub peak_equity: f64,
    pub position_qty: f64,
    pub daily_pnl: f64,
    pub starting_equity: f64,
    pub orders_this_bar: u32,
    pub consecutive_losses: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskError {
    MaxPosition,
    MaxOrderQty,
    DailyLoss,
    Drawdown,
    OrderRate,
    ConsecutiveLosses,
    KillSwitch,
}

impl fmt::Display for RiskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            RiskError::MaxPosition => "max position exceeded",
            RiskError::MaxOrderQty => "max order quantity exceeded",
            RiskError::DailyLoss => "daily loss limit breached",
            RiskError::Drawdown => "max drawdown breached",
            RiskError::OrderRate => "order rate limit exceeded",
            RiskError::ConsecutiveLosses => "consecutive loss limit breached",
            RiskError::KillSwitch => "kill switch engaged",
        })
    }
}

impl std::error::Error for RiskError {}

#[derive(Debug, Clone, Copy)]
pub struct RiskGuard {
    pub limits: RiskLimits,
    pub kill_switch: bool,
}

impl Default for RiskGuard {
    fn default() -> Self {
        Self {
            limits: RiskLimits::default(),
            kill_switch: false,
        }
    }
}

impl RiskGuard {
    pub fn new(limits: RiskLimits) -> Self {
        Self {
            limits,
            kill_switch: false,
        }
    }

    /// Pre-trade check: called before an order is sent to the execution layer.
    pub fn check(&self, order: &Order, account: &AccountState) -> Result<(), RiskError> {
        if self.kill_switch {
            return Err(RiskError::KillSwitch);
        }
        if order.qty <= 0.0 || order.qty > self.limits.max_order_qty {
            return Err(RiskError::MaxOrderQty);
        }
        let signed = account.position_qty + if order.is_buy() { order.qty } else { -order.qty };
        if signed.abs() > self.limits.max_position_qty + 1e-9 {
            return Err(RiskError::MaxPosition);
        }
        if account.orders_this_bar >= self.limits.max_orders_per_bar {
            return Err(RiskError::OrderRate);
        }
        self.check_post_trade(account)
    }

    /// Post-trade check: called after every fill and at each bar close.
    pub fn check_post_trade(&self, account: &AccountState) -> Result<(), RiskError> {
        let base = if account.starting_equity > 0.0 {
            account.starting_equity
        } else {
            account.equity.max(1.0)
        };
        if account.daily_pnl <= -base * self.limits.max_daily_loss_pct / 100.0 {
            return Err(RiskError::DailyLoss);
        }
        if account.peak_equity > 0.0 {
            let dd = (account.peak_equity - account.equity) / account.peak_equity * 100.0;
            if dd >= self.limits.max_drawdown_pct {
                return Err(RiskError::Drawdown);
            }
        }
        if account.consecutive_losses >= self.limits.max_consecutive_losses {
            return Err(RiskError::ConsecutiveLosses);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{OrderType, Side};

    fn order(qty: f64) -> Order {
        Order::new(1, "TEST", Side::Buy, OrderType::Market, None, None, qty, 0)
    }

    fn account() -> AccountState {
        AccountState {
            equity: 100_000.0,
            peak_equity: 100_000.0,
            position_qty: 0.0,
            daily_pnl: 0.0,
            starting_equity: 100_000.0,
            orders_this_bar: 0,
            consecutive_losses: 0,
        }
    }

    #[test]
    fn rejects_excess_size_and_exposure() {
        let guard = RiskGuard::default();
        assert_eq!(guard.check(&order(6.0), &account()), Err(RiskError::MaxOrderQty));
        let mut acct = account();
        acct.position_qty = 9.0;
        assert_eq!(guard.check(&order(2.0), &acct), Err(RiskError::MaxPosition));
        assert!(guard.check(&order(1.0), &acct).is_ok());
    }

    #[test]
    fn rejects_after_losses_and_halt() {
        let guard = RiskGuard::default();
        let mut acct = account();
        acct.daily_pnl = -4_000.0;
        assert_eq!(guard.check(&order(1.0), &acct), Err(RiskError::DailyLoss));
        let mut acct = account();
        acct.equity = 89_000.0;
        assert_eq!(guard.check(&order(1.0), &acct), Err(RiskError::Drawdown));
        let mut acct = account();
        acct.consecutive_losses = 5;
        assert_eq!(guard.check(&order(1.0), &acct), Err(RiskError::ConsecutiveLosses));
        let mut halted = guard;
        halted.kill_switch = true;
        assert_eq!(halted.check(&order(1.0), &account()), Err(RiskError::KillSwitch));
    }
}
