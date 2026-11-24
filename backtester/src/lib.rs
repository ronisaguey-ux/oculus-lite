//! Deterministic, event-driven, bar-by-bar backtester.
//!
//! Pipeline: `Bar -> indicators -> signal -> risk -> execution client -> ledger`.
//! Orders are created after a bar closes and can only fill at the next bar's
//! open, so no calculation ever sees a future price. The engine uses integer
//! timestamps and fixed iteration order; two runs on the same inputs produce
//! byte-identical reports.

use std::fmt;
use std::fs;
use std::path::Path;

use oculus_config_rs::Params;
use oculus_execution::client::{ExecutionClient, PaperClient};
use oculus_execution::models::{Bar, Order, OrderType, Side};
use oculus_execution::protection::{AccountState, RiskGuard, RiskLimits};
use oculus_execution::trailing::TrailingStop;
use oculus_indicators::atr::Atr;
use oculus_indicators::rsi::Rsi;
use oculus_indicators::sma::Sma;
use oculus_indicators::{Indicator, Warmup};

pub const STARTING_EQUITY: f64 = 100_000.0;

/// Load `timestamp,symbol,open,high,low,close,volume` CSV. A directory argument
/// resolves to its `bars.csv` file.
pub fn load_bars(path: &str) -> Result<Vec<Bar>, String> {
    let file = if Path::new(path).is_dir() {
        Path::new(path).join("bars.csv")
    } else {
        Path::new(path).to_path_buf()
    };
    let text = fs::read_to_string(&file).map_err(|e| format!("cannot read {}: {e}", file.display()))?;
    let mut bars = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line_no = index + 1;
        let line = line.trim();
        if line.is_empty() || (index == 0 && line.starts_with("timestamp")) {
            continue;
        }
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() != 7 {
            return Err(format!("line {line_no}: expected 7 CSV fields, got {}", fields.len()));
        }
        let parse = |s: &str, name: &str| -> Result<f64, String> {
            s.trim()
                .parse::<f64>()
                .map_err(|e| format!("line {line_no}: bad {name} '{s}': {e}"))
        };
        let bar = Bar {
            ts: fields[0]
                .trim()
                .parse::<u64>()
                .map_err(|e| format!("line {line_no}: bad timestamp: {e}"))?,
            open: parse(fields[2], "open")?,
            high: parse(fields[3], "high")?,
            low: parse(fields[4], "low")?,
            close: parse(fields[5], "close")?,
            volume: parse(fields[6], "volume")?,
        };
        if !(bar.low <= bar.open && bar.open <= bar.high && bar.low <= bar.close && bar.close <= bar.high) {
            return Err(format!("line {line_no}: OHLC invariants violated"));
        }
        bars.push(bar);
    }
    if bars.is_empty() {
        return Err("no bars in data file".to_owned());
    }
    Ok(bars)
}

/// Illustrative SMA-crossover signal, filtered by RSI and ATR. This is demo
/// logic only: it is not the proprietary strategy and has no expected edge.
pub fn signals(params: &Params, bars: &[Bar]) -> Vec<i8> {
    let mut fast = Sma::new(params.signal.lookback_fast as usize);
    let mut slow = Sma::new(params.signal.lookback_slow as usize);
    let mut rsi = Rsi::new(params.signal.rsi_period as usize);
    let mut atr = Atr::new(params.signal.atr_period as usize);
    let warmup = fast.warmup().max(slow.warmup()).max(params.signal.rsi_period as usize);
    let confirm = params.entry.entry_confirm_bars.max(0) as usize;

    let mut out = Vec::with_capacity(bars.len());
    let mut previous_fast: Option<f64> = None;
    let mut previous_slow: Option<f64> = None;
    let mut last_raw = 0i8;
    let mut streak = 0usize;
    for (index, bar) in bars.iter().enumerate() {
        let prev_fast = previous_fast;
        let prev_slow = previous_slow;
        let fast_now = fast.update(bar.close);
        let slow_now = slow.update(bar.close);
        let rsi_now = rsi.update(bar.close);
        let atr_now = atr.update_bar(bar.high, bar.low, bar.close);
        previous_fast = fast_now;
        previous_slow = slow_now;

        let mut raw = 0i8;
        if index >= warmup {
            if let (Some(pf), Some(ps), Some(f), Some(s), Some(r), Some(a)) =
                (prev_fast, prev_slow, fast_now, slow_now, rsi_now, atr_now)
            {
                let volatile_enough = bar.close > 0.0
                    && a / bar.close * 100.0 >= params.entry.entry_min_atr_ratio;
                if volatile_enough && pf <= ps && f > s && r <= params.signal.rsi_overbought {
                    raw = 1;
                } else if volatile_enough && pf >= ps && f < s && r >= params.signal.rsi_oversold {
                    raw = -1;
                }
            }
        }
        streak = if raw == last_raw { streak + 1 } else { 1 };
        last_raw = raw;
        let confirmed = raw != 0 && streak >= confirm.max(1);
        out.push(if confirmed { raw } else { 0 });
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
pub struct Trade {
    pub entry_ts: u64,
    pub exit_ts: u64,
    pub side: Side,
    pub qty: f64,
    pub entry_price: f64,
    pub exit_price: f64,
    pub pnl: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub bars: usize,
    pub fills: usize,
    pub trades: usize,
    pub final_equity: f64,
    pub net_pnl: f64,
    pub max_drawdown_pct: f64,
    pub sharpe: f64,
    pub win_rate_pct: f64,
    pub expectancy: f64,
    pub turnover: f64,
    pub equity_first: f64,
    pub equity_last: f64,
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "oculus-lite deterministic backtest")?;
        writeln!(f, "bars={} fills={} trades={}", self.bars, self.fills, self.trades)?;
        writeln!(f, "equity_first={:.6} equity_last={:.6}", self.equity_first, self.equity_last)?;
        writeln!(f, "final_equity={:.6} net_pnl={:.6}", self.final_equity, self.net_pnl)?;
        writeln!(f, "sharpe={:.6} max_drawdown_pct={:.6}", self.sharpe, self.max_drawdown_pct)?;
        writeln!(f, "win_rate_pct={:.6} expectancy={:.6} turnover={:.6}", self.win_rate_pct, self.expectancy, self.turnover)
    }
}

/// Run a complete simulation. Deterministic and side-effect free.
pub fn run(params: &Params, bars: &[Bar]) -> Result<Report, String> {
    if bars.is_empty() {
        return Err("no bars supplied".to_owned());
    }
    let signals = signals(params, bars);
    let limits = RiskLimits {
        max_position_qty: params.risk.max_position_qty,
        max_order_qty: params.risk.max_order_qty,
        max_daily_loss_pct: params.risk.max_daily_loss_pct,
        max_drawdown_pct: params.risk.max_drawdown_pct,
        max_orders_per_bar: params.risk.max_orders_per_bar as u32,
        max_consecutive_losses: params.risk.max_consecutive_losses as u32,
    };
    let mut guard = RiskGuard::new(limits);
    guard.kill_switch = params.risk.kill_switch;

    let mut client = PaperClient::with_slippage_bps(params.entry.entry_slippage_bps);
    let mut atr = Atr::new(params.signal.atr_period as usize);
    let mut cash = STARTING_EQUITY;
    let mut position = 0.0f64;
    let mut avg_entry = 0.0f64;
    let mut entry_ts = 0u64;
    let mut entry_index = 0usize;
    let mut peak_equity = STARTING_EQUITY;
    let mut daily_pnl = 0.0f64;
    let mut consecutive_losses = 0u32;
    let mut orders_this_bar = 0u32;
    let mut cooldown = 0i64;
    let mut turnover_notional = 0.0f64;
    let mut equity_curve = Vec::with_capacity(bars.len());
    let mut trades: Vec<Trade> = Vec::new();
    let mut daily_anchor = STARTING_EQUITY;
    let mut trail: Option<TrailingStop> = None;

    for (index, bar) in bars.iter().enumerate() {
        orders_this_bar = 0;

        // 1. Consume fills produced by the previous bar's orders at this bar's open.
        for fill in client.on_bar(bar) {
            let signed_qty = if fill.side == Side::Buy { fill.qty } else { -fill.qty };
            cash -= fill.price * signed_qty;
            turnover_notional += fill.notional();

            let old_position = position;
            let new_position = old_position + signed_qty;
            let reducing = old_position.abs() > 1e-9
                && (new_position.abs() < old_position.abs() - 1e-9
                    || new_position.signum() != old_position.signum());
            if reducing {
                let closed_qty = signed_qty.abs().min(old_position.abs());
                let pnl = (fill.price - avg_entry) * closed_qty * old_position.signum();
                trades.push(Trade {
                    entry_ts,
                    exit_ts: fill.ts,
                    side: if old_position > 0.0 { Side::Buy } else { Side::Sell },
                    qty: closed_qty,
                    entry_price: avg_entry,
                    exit_price: fill.price,
                    pnl,
                });
                daily_pnl += pnl;
                consecutive_losses = if pnl < 0.0 { consecutive_losses + 1 } else { 0 };
            }

            if new_position.abs() <= 1e-9 {
                position = 0.0;
                avg_entry = 0.0;
                trail = None;
            } else if old_position.abs() <= 1e-9 || old_position.signum() != new_position.signum() {
                position = new_position;
                avg_entry = fill.price;
                entry_ts = fill.ts;
                entry_index = index;
                if params.exit.exit_mode == "trailing" {
                    let direction = if new_position > 0.0 { Side::Buy } else { Side::Sell };
                    let activation = avg_entry + direction_long(direction) * params.exit.trail_activation_atr * (atr.value().unwrap_or(bar.close * 0.01));
                    trail = Some(TrailingStop::new(direction, activation, params.exit.trail_distance_atr * atr.value().unwrap_or(bar.close * 0.01)));
                }
            } else {
                let total = old_position.abs() + signed_qty.abs();
                avg_entry = (avg_entry * old_position.abs() + fill.price * signed_qty.abs()) / total;
                position = new_position;
            }
        }

        let atr_value = atr.update_bar(bar.high, bar.low, bar.close).unwrap_or(0.0);
        let equity = cash + position * bar.close;
        peak_equity = peak_equity.max(equity);
        equity_curve.push(equity);

        let account = AccountState {
            equity,
            peak_equity,
            position_qty: position,
            daily_pnl,
            starting_equity: daily_anchor,
            orders_this_bar,
            consecutive_losses,
        };
        let halted = guard.check_post_trade(&account).is_err();
        if halted {
            cooldown = params.risk.cooldown_after_halt_bars;
            daily_pnl = 0.0;
            daily_anchor = equity;
        }

        let has_open_order = !client.manager().open_orders().is_empty();
        if !has_open_order && !halted && cooldown <= 0 && position.abs() > 1e-9 {
            let exit_price_hint = avg_entry;
            let stop = params.exit.stop_loss_atr * atr_value;
            let take = params.exit.take_profit_atr * atr_value;
            let long = position > 0.0;
            let hit_stop = if long { bar.low <= exit_price_hint - stop } else { bar.high >= exit_price_hint + stop };
            let hit_take = if long { bar.high >= exit_price_hint + take } else { bar.low <= exit_price_hint - take };
            let hit_trail = trail.as_mut().is_some_and(|t| {
                t.update(if long { bar.high } else { bar.low });
                t.is_hit(bar.close)
            });
            let timed_out = params.exit.time_stop_bars > 0 && (index - entry_index) as i64 >= params.exit.time_stop_bars;
            let flipped = params.exit.exit_on_signal_flip && signals[index] != 0 && signals[index] != position.signum() as i8;
            if hit_stop || hit_take || hit_trail || timed_out || flipped {
                let close_side = if long { Side::Sell } else { Side::Buy };
                let account = AccountState { orders_this_bar, ..account };
                if guard.check(&probe_order(close_side, position.abs()), &account).is_ok() {
                    if client.submit_order("SYNTH", close_side, OrderType::Market, position.abs(), None, None, bar.ts).is_ok() {
                        orders_this_bar += 1;
                    }
                }
            }
        }

        if !has_open_order && !halted && cooldown <= 0 && position.abs() <= 1e-9 {
            let signal = signals[index];
            if signal != 0 {
                let mut qty = match params.sizing.sizing_mode.as_str() {
                    "percent_equity" => (equity * params.sizing.equity_fraction / bar.close).max(params.sizing.min_qty),
                    _ => params.sizing.fixed_qty,
                };
                if params.sizing.round_qty && params.sizing.qty_step > 0.0 {
                    qty = (qty / params.sizing.qty_step).round() * params.sizing.qty_step;
                }
                qty = qty.clamp(params.sizing.min_qty, params.sizing.max_qty);
                let side = if signal > 0 { Side::Buy } else { Side::Sell };
                let account = AccountState {
                    equity,
                    peak_equity,
                    position_qty: position,
                    daily_pnl,
                    starting_equity: daily_anchor,
                    orders_this_bar,
                    consecutive_losses,
                };
                if guard.check(&probe_order(side, qty), &account).is_ok() {
                    if client.submit_order("SYNTH", side, OrderType::Market, qty, None, None, bar.ts).is_ok() {
                        orders_this_bar += 1;
                    }
                }
            }
        }

        if cooldown > 0 {
            cooldown -= 1;
        }
    }

    let _orders_in_final_bar = orders_this_bar;
    let equity_first = equity_curve.first().copied().unwrap_or(STARTING_EQUITY);
    let equity_last = equity_curve.last().copied().unwrap_or(STARTING_EQUITY);
    let returns: Vec<f64> = equity_curve
        .windows(2)
        .map(|w| if w[0].abs() > 1e-12 { w[1] / w[0] - 1.0 } else { 0.0 })
        .collect();
    let mean = returns.iter().sum::<f64>() / returns.len().max(1) as f64;
    let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len().max(1) as f64;
    let std = variance.sqrt();
    let bars_per_year: f64 = 252.0 * (1440.0 / params.entry.entry_timeframe_minutes.max(1) as f64);
    let sharpe = if std > 1e-12 {
        mean / std * bars_per_year.sqrt()
    } else {
        0.0
    };
    let mut max_drawdown_pct = 0.0f64;
    let mut running_peak = equity_curve.first().copied().unwrap_or(STARTING_EQUITY);
    for equity in &equity_curve {
        running_peak = running_peak.max(*equity);
        if running_peak > 0.0 {
            max_drawdown_pct = max_drawdown_pct.max((running_peak - equity) / running_peak * 100.0);
        }
    }
    let winners = trades.iter().filter(|t| t.pnl > 0.0).count();
    let total_pnl: f64 = trades.iter().map(|t| t.pnl).sum();
    let avg_equity = equity_curve.iter().sum::<f64>() / equity_curve.len() as f64;
    Ok(Report {
        bars: bars.len(),
        fills: client.fills().len(),
        trades: trades.len(),
        final_equity: equity_last,
        net_pnl: total_pnl,
        max_drawdown_pct,
        sharpe,
        win_rate_pct: if trades.is_empty() { 0.0 } else { winners as f64 / trades.len() as f64 * 100.0 },
        expectancy: if trades.is_empty() { 0.0 } else { total_pnl / trades.len() as f64 },
        turnover: if avg_equity > 0.0 { turnover_notional / avg_equity } else { 0.0 },
        equity_first,
        equity_last,
    })
}

fn probe_order(side: Side, qty: f64) -> Order {
    Order::new(0, "SYNTH", side, OrderType::Market, None, None, qty, 0)
}

fn direction_long(side: Side) -> f64 {
    match side {
        Side::Buy => 1.0,
        Side::Sell => -1.0,
    }
}
