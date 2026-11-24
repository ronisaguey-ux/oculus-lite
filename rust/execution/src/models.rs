//! Value types shared by every execution component.

use std::fmt;

/// Side of an order or fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    pub fn opposite(self) -> Self {
        match self {
            Side::Buy => Side::Sell,
            Side::Sell => Side::Buy,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Supported order types. Stop and trailing orders are handled by the emulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Market,
    Limit,
    StopMarket,
    StopLimit,
    TrailingStop,
}

/// Order lifecycle. Terminal states cannot transition any further.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    Accepted,
    Working,
    PartiallyFilled,
    Filled,
    Canceled,
    Rejected,
    Expired,
}

impl OrderStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            OrderStatus::Filled | OrderStatus::Canceled | OrderStatus::Rejected | OrderStatus::Expired
        )
    }
}

impl fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            OrderStatus::Accepted => "ACCEPTED",
            OrderStatus::Working => "WORKING",
            OrderStatus::PartiallyFilled => "PARTIALLY_FILLED",
            OrderStatus::Filled => "FILLED",
            OrderStatus::Canceled => "CANCELED",
            OrderStatus::Rejected => "REJECTED",
            OrderStatus::Expired => "EXPIRED",
        })
    }
}

/// Whether a fill added or removed liquidity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Liquidity {
    Maker,
    Taker,
}

/// An order as seen by the local execution system.
#[derive(Debug, Clone, PartialEq)]
pub struct Order {
    pub id: u64,
    pub client_id: String,
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub limit_price: Option<f64>,
    pub stop_price: Option<f64>,
    pub qty: f64,
    pub filled_qty: f64,
    pub status: OrderStatus,
    pub created_ts: u64,
}

impl Order {
    pub fn new(
        id: u64,
        symbol: &str,
        side: Side,
        order_type: OrderType,
        limit_price: Option<f64>,
        stop_price: Option<f64>,
        qty: f64,
        created_ts: u64,
    ) -> Self {
        Self {
            id,
            client_id: format!("O-{id:06}"),
            symbol: symbol.to_owned(),
            side,
            order_type,
            limit_price,
            stop_price,
            qty,
            filled_qty: 0.0,
            status: OrderStatus::Accepted,
            created_ts,
        }
    }

    pub fn remaining(&self) -> f64 {
        (self.qty - self.filled_qty).max(0.0)
    }

    pub fn is_buy(&self) -> bool {
        self.side == Side::Buy
    }
}

/// An execution result. Fills are immutable once produced.
#[derive(Debug, Clone, PartialEq)]
pub struct Fill {
    pub order_id: u64,
    pub symbol: String,
    pub side: Side,
    pub qty: f64,
    pub price: f64,
    pub liquidity: Liquidity,
    pub ts: u64,
}

impl Fill {
    pub fn notional(&self) -> f64 {
        self.qty * self.price
    }
}

/// A single OHLCV bar. Timestamps are integer nanoseconds or any monotonic unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bar {
    pub ts: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// Events emitted by the execution layer, consumed by ledgers and dashboards.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Bar(Bar),
    Accepted(u64),
    Working(u64),
    Rejected { order_id: u64, reason: String },
    Fill(Fill),
    Canceled(u64),
    RiskHalt { reason: String },
}
