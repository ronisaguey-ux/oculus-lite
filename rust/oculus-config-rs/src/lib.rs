//! The public, hand-tuned parameter schema for `oculus-lite`.
//!
//! Exactly 50 parameters are exposed, grouped into signal, entry, exit, risk and
//! sizing. Every parameter has one default, one documented range and one line of
//! explanation. There is deliberately no optimiser, genetic algorithm or any
//! other mechanism that changes these numbers automatically: tuning is a human
//! activity performed by editing `config/params.yaml`.
//!
//! The parser understands the small YAML subset used by that file: category
//! headers, `key: value` lines and trailing `#` comments. It validates types,
//! ranges, completeness and cross-field constraints, then returns a typed struct.

use std::collections::BTreeSet;
use std::fmt;
use std::fs;

/// Type and accepted range of one parameter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Int(i64, i64),
    Float(f64, f64),
    Bool,
    Choice(&'static [&'static str]),
}

impl Kind {
    pub fn describe(&self) -> String {
        match self {
            Kind::Int(lo, hi) => format!("{lo} .. {hi}"),
            Kind::Float(lo, hi) => format!("{lo} .. {hi}"),
            Kind::Bool => "true / false".to_owned(),
            Kind::Choice(options) => options.join(" / "),
        }
    }
}

/// Documentation and validation record for one parameter.
#[derive(Debug, Clone, Copy)]
pub struct Field {
    pub name: &'static str,
    pub category: &'static str,
    pub kind: Kind,
    pub default: &'static str,
    pub description: &'static str,
}

impl Field {
    pub fn full_name(&self) -> String {
        format!("{}.{}", self.category, self.name)
    }
}

pub const FIELDS: [Field; 50] = [
    Field { name: "lookback_fast", category: "signal", kind: Kind::Int(10, 60), default: "20", description: "Fast moving-average window in bars" },
    Field { name: "lookback_slow", category: "signal", kind: Kind::Int(20, 200), default: "50", description: "Slow moving-average window in bars" },
    Field { name: "rsi_period", category: "signal", kind: Kind::Int(2, 50), default: "14", description: "Wilder RSI lookback in bars" },
    Field { name: "rsi_oversold", category: "signal", kind: Kind::Float(5.0, 45.0), default: "30", description: "RSI level below which long entries are allowed" },
    Field { name: "rsi_overbought", category: "signal", kind: Kind::Float(55.0, 95.0), default: "70", description: "RSI level above which short entries are allowed" },
    Field { name: "atr_period", category: "signal", kind: Kind::Int(2, 50), default: "14", description: "ATR lookback in bars" },
    Field { name: "macd_fast", category: "signal", kind: Kind::Int(2, 40), default: "12", description: "Fast MACD EMA period" },
    Field { name: "macd_slow", category: "signal", kind: Kind::Int(5, 100), default: "26", description: "Slow MACD EMA period" },
    Field { name: "macd_signal", category: "signal", kind: Kind::Int(2, 30), default: "9", description: "MACD signal EMA period" },
    Field { name: "vwap_anchor", category: "signal", kind: Kind::Choice(&["session", "day", "none"]), default: "session", description: "VWAP reset anchor" },
    Field { name: "entry_mode", category: "entry", kind: Kind::Choice(&["crossover", "pullback", "breakout"]), default: "crossover", description: "Signal rule used to open a position" },
    Field { name: "entry_offset_ticks", category: "entry", kind: Kind::Int(0, 50), default: "1", description: "Ticks added to a limit entry price" },
    Field { name: "entry_slippage_bps", category: "entry", kind: Kind::Float(0.0, 100.0), default: "2", description: "Simulated slippage in basis points" },
    Field { name: "entry_min_atr_ratio", category: "entry", kind: Kind::Float(0.0, 5.0), default: "0.5", description: "Minimum ATR/price ratio in percent required to trade" },
    Field { name: "entry_max_spread_bps", category: "entry", kind: Kind::Float(0.0, 100.0), default: "5", description: "Maximum simulated spread in basis points" },
    Field { name: "entry_cooldown_bars", category: "entry", kind: Kind::Int(0, 100), default: "5", description: "Bars to wait after an exit before re-entering" },
    Field { name: "entry_order_type", category: "entry", kind: Kind::Choice(&["market", "limit"]), default: "market", description: "Order type used for entries" },
    Field { name: "entry_timeframe_minutes", category: "entry", kind: Kind::Int(1, 240), default: "5", description: "Nominal bar timeframe recorded in the config" },
    Field { name: "entry_confirm_bars", category: "entry", kind: Kind::Int(0, 10), default: "1", description: "Bars a signal must persist before entry" },
    Field { name: "entry_allow_flip", category: "entry", kind: Kind::Bool, default: "true", description: "Allow reversing directly from long to short" },
    Field { name: "exit_mode", category: "exit", kind: Kind::Choice(&["fixed", "atr", "trailing"]), default: "atr", description: "Stop style applied to opened positions" },
    Field { name: "stop_loss_atr", category: "exit", kind: Kind::Float(0.1, 10.0), default: "2", description: "Initial stop distance in ATR multiples" },
    Field { name: "take_profit_atr", category: "exit", kind: Kind::Float(0.1, 20.0), default: "4", description: "Take-profit distance in ATR multiples" },
    Field { name: "trail_activation_atr", category: "exit", kind: Kind::Float(0.1, 10.0), default: "1", description: "Favourable move required to arm the trailing stop" },
    Field { name: "trail_distance_atr", category: "exit", kind: Kind::Float(0.1, 10.0), default: "1.5", description: "Trailing distance in ATR multiples" },
    Field { name: "time_stop_bars", category: "exit", kind: Kind::Int(0, 500), default: "50", description: "Maximum holding period in bars" },
    Field { name: "breakeven_at_r", category: "exit", kind: Kind::Float(0.0, 5.0), default: "1", description: "R multiple at which the stop moves to break even" },
    Field { name: "exit_on_signal_flip", category: "exit", kind: Kind::Bool, default: "true", description: "Close a position when the signal reverses" },
    Field { name: "exit_slippage_bps", category: "exit", kind: Kind::Float(0.0, 100.0), default: "2", description: "Slippage applied to exit fills" },
    Field { name: "partial_exit_fraction", category: "exit", kind: Kind::Float(0.0, 1.0), default: "0", description: "Fraction closed at the take-profit level" },
    Field { name: "max_position_qty", category: "risk", kind: Kind::Float(0.01, 1_000_000.0), default: "10", description: "Absolute position size ceiling" },
    Field { name: "max_order_qty", category: "risk", kind: Kind::Float(0.01, 1_000_000.0), default: "5", description: "Maximum quantity per order" },
    Field { name: "max_daily_loss_pct", category: "risk", kind: Kind::Float(0.0, 100.0), default: "3", description: "Daily loss limit as a percent of equity" },
    Field { name: "max_drawdown_pct", category: "risk", kind: Kind::Float(0.0, 100.0), default: "10", description: "Peak-to-trough equity limit in percent" },
    Field { name: "max_orders_per_bar", category: "risk", kind: Kind::Int(1, 100), default: "2", description: "Order rate limit per bar" },
    Field { name: "max_consecutive_losses", category: "risk", kind: Kind::Int(1, 50), default: "5", description: "Loss streak that halts trading" },
    Field { name: "max_open_positions", category: "risk", kind: Kind::Int(1, 20), default: "1", description: "Maximum simultaneously open positions" },
    Field { name: "risk_per_trade_pct", category: "risk", kind: Kind::Float(0.0, 10.0), default: "1", description: "Equity fraction risked per trade" },
    Field { name: "kill_switch", category: "risk", kind: Kind::Bool, default: "false", description: "Manual halt; no order may be submitted" },
    Field { name: "cooldown_after_halt_bars", category: "risk", kind: Kind::Int(0, 500), default: "20", description: "Bars to wait after a risk halt" },
    Field { name: "sizing_mode", category: "sizing", kind: Kind::Choice(&["fixed", "percent_equity", "vol_target"]), default: "fixed", description: "Position sizing method" },
    Field { name: "fixed_qty", category: "sizing", kind: Kind::Float(0.001, 1_000_000.0), default: "1", description: "Quantity used by fixed sizing" },
    Field { name: "equity_fraction", category: "sizing", kind: Kind::Float(0.0, 1.0), default: "0.05", description: "Equity fraction used by percent sizing" },
    Field { name: "vol_target_annual", category: "sizing", kind: Kind::Float(0.0, 2.0), default: "0.2", description: "Annualised volatility target" },
    Field { name: "vol_lookback", category: "sizing", kind: Kind::Int(2, 200), default: "20", description: "Lookback for realised volatility" },
    Field { name: "min_qty", category: "sizing", kind: Kind::Float(0.001, 1_000_000.0), default: "0.01", description: "Minimum tradable quantity" },
    Field { name: "max_qty", category: "sizing", kind: Kind::Float(0.001, 1_000_000.0), default: "10", description: "Maximum tradable quantity" },
    Field { name: "qty_step", category: "sizing", kind: Kind::Float(0.0001, 100.0), default: "0.01", description: "Quantity rounding increment" },
    Field { name: "leverage_cap", category: "sizing", kind: Kind::Float(1.0, 20.0), default: "1", description: "Maximum notional-to-equity ratio" },
    Field { name: "round_qty", category: "sizing", kind: Kind::Bool, default: "true", description: "Round quantities to qty_step" },
];

#[derive(Debug, Clone, PartialEq)]
pub struct SignalParams {
    pub lookback_fast: i64,
    pub lookback_slow: i64,
    pub rsi_period: i64,
    pub rsi_oversold: f64,
    pub rsi_overbought: f64,
    pub atr_period: i64,
    pub macd_fast: i64,
    pub macd_slow: i64,
    pub macd_signal: i64,
    pub vwap_anchor: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntryParams {
    pub entry_mode: String,
    pub entry_offset_ticks: i64,
    pub entry_slippage_bps: f64,
    pub entry_min_atr_ratio: f64,
    pub entry_max_spread_bps: f64,
    pub entry_cooldown_bars: i64,
    pub entry_order_type: String,
    pub entry_timeframe_minutes: i64,
    pub entry_confirm_bars: i64,
    pub entry_allow_flip: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExitParams {
    pub exit_mode: String,
    pub stop_loss_atr: f64,
    pub take_profit_atr: f64,
    pub trail_activation_atr: f64,
    pub trail_distance_atr: f64,
    pub time_stop_bars: i64,
    pub breakeven_at_r: f64,
    pub exit_on_signal_flip: bool,
    pub exit_slippage_bps: f64,
    pub partial_exit_fraction: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RiskParams {
    pub max_position_qty: f64,
    pub max_order_qty: f64,
    pub max_daily_loss_pct: f64,
    pub max_drawdown_pct: f64,
    pub max_orders_per_bar: i64,
    pub max_consecutive_losses: i64,
    pub max_open_positions: i64,
    pub risk_per_trade_pct: f64,
    pub kill_switch: bool,
    pub cooldown_after_halt_bars: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SizingParams {
    pub sizing_mode: String,
    pub fixed_qty: f64,
    pub equity_fraction: f64,
    pub vol_target_annual: f64,
    pub vol_lookback: i64,
    pub min_qty: f64,
    pub max_qty: f64,
    pub qty_step: f64,
    pub leverage_cap: f64,
    pub round_qty: bool,
}

/// The complete typed configuration: exactly fifty hand-tuned values.
#[derive(Debug, Clone, PartialEq)]
pub struct Params {
    pub signal: SignalParams,
    pub entry: EntryParams,
    pub exit: ExitParams,
    pub risk: RiskParams,
    pub sizing: SizingParams,
}

impl Params {
    /// Neutral, illustrative defaults. These are not tuned to any market.
    pub fn defaults() -> Self {
        Self {
            signal: SignalParams {
                lookback_fast: 20,
                lookback_slow: 50,
                rsi_period: 14,
                rsi_oversold: 30.0,
                rsi_overbought: 70.0,
                atr_period: 14,
                macd_fast: 12,
                macd_slow: 26,
                macd_signal: 9,
                vwap_anchor: "session".to_owned(),
            },
            entry: EntryParams {
                entry_mode: "crossover".to_owned(),
                entry_offset_ticks: 1,
                entry_slippage_bps: 2.0,
                entry_min_atr_ratio: 0.5,
                entry_max_spread_bps: 5.0,
                entry_cooldown_bars: 5,
                entry_order_type: "market".to_owned(),
                entry_timeframe_minutes: 5,
                entry_confirm_bars: 1,
                entry_allow_flip: true,
            },
            exit: ExitParams {
                exit_mode: "atr".to_owned(),
                stop_loss_atr: 2.0,
                take_profit_atr: 4.0,
                trail_activation_atr: 1.0,
                trail_distance_atr: 1.5,
                time_stop_bars: 50,
                breakeven_at_r: 1.0,
                exit_on_signal_flip: true,
                exit_slippage_bps: 2.0,
                partial_exit_fraction: 0.0,
            },
            risk: RiskParams {
                max_position_qty: 10.0,
                max_order_qty: 5.0,
                max_daily_loss_pct: 3.0,
                max_drawdown_pct: 10.0,
                max_orders_per_bar: 2,
                max_consecutive_losses: 5,
                max_open_positions: 1,
                risk_per_trade_pct: 1.0,
                kill_switch: false,
                cooldown_after_halt_bars: 20,
            },
            sizing: SizingParams {
                sizing_mode: "fixed".to_owned(),
                fixed_qty: 1.0,
                equity_fraction: 0.05,
                vol_target_annual: 0.2,
                vol_lookback: 20,
                min_qty: 0.01,
                max_qty: 10.0,
                qty_step: 0.01,
                leverage_cap: 1.0,
                round_qty: true,
            },
        }
    }

    /// Cross-field sanity checks that cannot be expressed as a single range.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let invalid = |msg: String| ConfigError::Validation(msg);
        if self.signal.lookback_fast >= self.signal.lookback_slow {
            return Err(invalid("signal.lookback_fast must be < signal.lookback_slow".to_owned()));
        }
        if self.signal.rsi_oversold >= self.signal.rsi_overbought {
            return Err(invalid("signal.rsi_oversold must be < signal.rsi_overbought".to_owned()));
        }
        if self.signal.macd_fast >= self.signal.macd_slow {
            return Err(invalid("signal.macd_fast must be < signal.macd_slow".to_owned()));
        }
        if !(self.sizing.min_qty <= self.sizing.fixed_qty
            && self.sizing.fixed_qty <= self.sizing.max_qty)
        {
            return Err(invalid("sizing.fixed_qty must lie between min_qty and max_qty".to_owned()));
        }
        if self.exit.stop_loss_atr >= self.exit.take_profit_atr {
            return Err(invalid("exit.take_profit_atr must exceed exit.stop_loss_atr".to_owned()));
        }
        if self.entry.entry_slippage_bps + self.exit.exit_slippage_bps > 100.0 {
            return Err(invalid("combined slippage must not exceed 100 bps".to_owned()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigError {
    Malformed { line: usize, text: String },
    UnknownKey(String),
    MissingSection,
    DuplicateKey(String),
    InvalidValue { name: String, raw: String },
    OutOfRange { name: String, raw: String, range: String },
    MissingKeys(Vec<String>),
    Validation(String),
    Io(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Malformed { line, text } => write!(f, "line {line}: malformed entry '{text}'"),
            ConfigError::UnknownKey(name) => write!(f, "unknown parameter '{name}'"),
            ConfigError::MissingSection => write!(f, "parameter appears before a category header"),
            ConfigError::DuplicateKey(name) => write!(f, "duplicate parameter '{name}'"),
            ConfigError::InvalidValue { name, raw } => write!(f, "invalid value '{raw}' for '{name}'"),
            ConfigError::OutOfRange { name, raw, range } => {
                write!(f, "'{raw}' for '{name}' is outside {range}")
            }
            ConfigError::MissingKeys(names) => write!(f, "missing {} parameter(s): {}", names.len(), names.join(", ")),
            ConfigError::Validation(msg) => write!(f, "validation failed: {msg}"),
            ConfigError::Io(msg) => write!(f, "I/O error: {msg}"),
        }
    }
}

impl std::error::Error for ConfigError {}

fn field_for(name: &str) -> Option<&'static Field> {
    FIELDS.iter().find(|f| f.full_name() == name)
}

fn parse_typed(kind: Kind, name: &str, raw: &str) -> Result<(), ConfigError> {
    let invalid = || ConfigError::InvalidValue {
        name: name.to_owned(),
        raw: raw.to_owned(),
