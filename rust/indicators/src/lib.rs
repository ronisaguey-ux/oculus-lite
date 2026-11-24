//! Small, bounded-memory, streaming indicators.
//!
//! Each indicator accepts one observation at a time and never retains more than
//! a fixed window (or a fixed number of scalar accumulators). No indicator
//! looks ahead, so the same code works in a backtest and on a live bar stream.

pub mod atr;
pub mod ema;
pub mod macd;
pub mod rsi;
pub mod sma;
pub mod vwap;

/// Common interface for single-input indicators.
pub trait Indicator {
    /// Consume one value. Returns `Some` once enough observations exist.
    fn update(&mut self, value: f64) -> Option<f64>;
    /// Current value without consuming a new observation.
    fn value(&self) -> Option<f64>;
    /// Stable name, used by the dashboard parameter panel.
    fn name(&self) -> &'static str;
}

/// Number of observations required before an indicator produces a value.
pub trait Warmup {
    fn warmup(&self) -> usize;
}
