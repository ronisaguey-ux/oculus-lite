# Parameters

Exactly **50** parameters, in five categories of ten: signal, entry, exit, risk and sizing.
They are tuned **by hand** by editing `config/params.yaml`. There is no evolution,
genetic algorithm, optimiser or auto-tuning anywhere in this repository.

The defaults below are neutral illustrative values. They are not the values of
the private engine, and they are not tuned to any market or dataset. The Rust
struct that owns them is `Params` in `rust/oculus-config-rs/src/lib.rs`; the
parser validates every range and the cross-field constraints before any run.

To regenerate this table:

```bash
cargo run -p oculus-config-rs --example schema
```

| Parameter | Default | Range / options | Effect |
|---|---|---|---|
| `signal.lookback_fast` | `20` | 10 .. 60 | Fast moving-average window in bars |
| `signal.lookback_slow` | `50` | 20 .. 200 | Slow moving-average window in bars |
| `signal.rsi_period` | `14` | 2 .. 50 | Wilder RSI lookback in bars |
| `signal.rsi_oversold` | `30` | 5 .. 45 | RSI level below which long entries are allowed |
| `signal.rsi_overbought` | `70` | 55 .. 95 | RSI level above which short entries are allowed |
| `signal.atr_period` | `14` | 2 .. 50 | ATR lookback in bars |
| `signal.macd_fast` | `12` | 2 .. 40 | Fast MACD EMA period |
| `signal.macd_slow` | `26` | 5 .. 100 | Slow MACD EMA period |
| `signal.macd_signal` | `9` | 2 .. 30 | MACD signal EMA period |
| `signal.vwap_anchor` | `session` | session / day / none | VWAP reset anchor |
| `entry.entry_mode` | `crossover` | crossover / pullback / breakout | Signal rule used to open a position |
| `entry.entry_offset_ticks` | `1` | 0 .. 50 | Ticks added to a limit entry price |
| `entry.entry_slippage_bps` | `2` | 0 .. 100 | Simulated slippage in basis points |
| `entry.entry_min_atr_ratio` | `0.5` | 0 .. 5 | Minimum ATR/price ratio in percent required to trade |
| `entry.entry_max_spread_bps` | `5` | 0 .. 100 | Maximum simulated spread in basis points |
| `entry.entry_cooldown_bars` | `5` | 0 .. 100 | Bars to wait after an exit before re-entering |
| `entry.entry_order_type` | `market` | market / limit | Order type used for entries |
| `entry.entry_timeframe_minutes` | `5` | 1 .. 240 | Nominal bar timeframe recorded in the config |
| `entry.entry_confirm_bars` | `1` | 0 .. 10 | Bars a signal must persist before entry |
| `entry.entry_allow_flip` | `true` | true / false | Allow reversing directly from long to short |
| `exit.exit_mode` | `atr` | fixed / atr / trailing | Stop style applied to opened positions |
| `exit.stop_loss_atr` | `2` | 0.1 .. 10 | Initial stop distance in ATR multiples |
| `exit.take_profit_atr` | `4` | 0.1 .. 20 | Take-profit distance in ATR multiples |
| `exit.trail_activation_atr` | `1` | 0.1 .. 10 | Favourable move required to arm the trailing stop |
| `exit.trail_distance_atr` | `1.5` | 0.1 .. 10 | Trailing distance in ATR multiples |
| `exit.time_stop_bars` | `50` | 0 .. 500 | Maximum holding period in bars |
| `exit.breakeven_at_r` | `1` | 0 .. 5 | R multiple at which the stop moves to break even |
| `exit.exit_on_signal_flip` | `true` | true / false | Close a position when the signal reverses |
| `exit.exit_slippage_bps` | `2` | 0 .. 100 | Slippage applied to exit fills |
| `exit.partial_exit_fraction` | `0` | 0 .. 1 | Fraction closed at the take-profit level |
| `risk.max_position_qty` | `10` | 0.01 .. 1000000 | Absolute position size ceiling |
| `risk.max_order_qty` | `5` | 0.01 .. 1000000 | Maximum quantity per order |
| `risk.max_daily_loss_pct` | `3` | 0 .. 100 | Daily loss limit as a percent of equity |
| `risk.max_drawdown_pct` | `10` | 0 .. 100 | Peak-to-trough equity limit in percent |
| `risk.max_orders_per_bar` | `2` | 1 .. 100 | Order rate limit per bar |
| `risk.max_consecutive_losses` | `5` | 1 .. 50 | Loss streak that halts trading |
| `risk.max_open_positions` | `1` | 1 .. 20 | Maximum simultaneously open positions |
| `risk.risk_per_trade_pct` | `1` | 0 .. 10 | Equity fraction risked per trade |
| `risk.kill_switch` | `false` | true / false | Manual halt; no order may be submitted |
| `risk.cooldown_after_halt_bars` | `20` | 0 .. 500 | Bars to wait after a risk halt |
| `sizing.sizing_mode` | `fixed` | fixed / percent_equity / vol_target | Position sizing method |
| `sizing.fixed_qty` | `1` | 0.001 .. 1000000 | Quantity used by fixed sizing |
| `sizing.equity_fraction` | `0.05` | 0 .. 1 | Equity fraction used by percent sizing |
| `sizing.vol_target_annual` | `0.2` | 0 .. 2 | Annualised volatility target |
| `sizing.vol_lookback` | `20` | 2 .. 200 | Lookback for realised volatility |
| `sizing.min_qty` | `0.01` | 0.001 .. 1000000 | Minimum tradable quantity |
| `sizing.max_qty` | `10` | 0.001 .. 1000000 | Maximum tradable quantity |
| `sizing.qty_step` | `0.01` | 0.0001 .. 100 | Quantity rounding increment |
| `sizing.leverage_cap` | `1` | 1 .. 20 | Maximum notional-to-equity ratio |
| `sizing.round_qty` | `true` | true / false | Round quantities to qty_step |
