# oculus-lite

A small, deterministic, event-driven trading system in Rust: matching core, order lifecycle, risk gates, a backtester, a paper trader, a live-execution path, and a Flutter dashboard.

> **The full engine is private and proprietary.** This lite edition is a deliberately reduced subset of it. It keeps the engineering — the matching core, the order manager, the risk layer, the ledger, the execution plumbing — and ships **none** of the proprietary signals, tuned parameters or strategy logic. Nothing here gives away the trading edge.

> **This lite version has not been tested as a trading system.** It has not been run against real markets, real venues or real money. It is **not investment advice**, and there is no warranty of any kind. It exists as a technically impressive portfolio / résumé piece, and as a reference implementation of the plumbing an execution stack needs.

## What is here / what is not

| Kept (the engineering) | Stripped (the edge) |
|---|---|
| Rust matching core, order manager, order emulator, reconciliation, protection and trailing logic | The proprietary signal model and its tuned parameters |
| Event-driven bar-by-bar backtester with metrics (Sharpe, drawdown, win rate, expectancy, turnover) | Genetic algorithm / evolution / auto-optimisation of any kind |
| Paper trader (default) and a live path that refuses to start without explicit flags and credentials | The 350+ parameters beyond the 50 documented here |
| Risk layer: position size, exposure caps, daily loss limit, order-rate limit, kill switch | FTROM weighting, regime scoring, courtroom/consensus layers, champion registries |
| 50 strategy parameters in one documented YAML file, tuned **by hand** | Historical data, private endpoints, exchange keys, internal hosts |
| Flutter client: equity curve, positions, recent fills, parameter panel | Proprietary research pipelines and reports |

**There is no optimiser in this repository.** No genetic algorithm, no hyper-parameter search, no walk-forward automation. You set the fifty parameters in `config/params.yaml` yourself and you read the result. That is the whole interface.

## Layout

```
rust/execution          matching core, order manager/emulator, protection, trailing, reconciliation, client
rust/indicators         the handful of indicators the example strategy reads
rust/oculus-config-rs   typed config loader for config/params.yaml
backtester/             library + CLI: event-driven historical run, metrics, ledger
trader/                 library + CLI: paper (default) and gated live execution
flutter_app/            Flutter dashboard (ships as source; not built here)
config/params.yaml      the 50 parameters, each with a one-line comment
sample/                 synthetic OHLCV bars — generated, not market data
tests/                  integration tests: determinism, fills, risk limits
docs/ARCHITECTURE.md    pipeline diagram and module responsibilities
docs/PARAMETERS.md      the 50 parameters, their ranges and what they move
```

## Build

```console
$ cargo build --release
$ cargo test
```

## Run a backtest

```console
$ cargo run --release -p oculus-backtester -- backtest --config config/params.yaml --data sample/
```

## Run the paper trader

```console
$ cargo run --release -p oculus-trader -- paper --config config/params.yaml --data sample/
```

The live path is gated on purpose: it needs `--live`, a venue client and credentials, and it aborts before connecting if any of them is missing. Paper is the default everywhere.

## Flutter dashboard

```console
$ cd flutter_app && flutter pub get && flutter run
```

It ships as source and has **not** been built in this environment. It is a read-only consumer of the ledger: equity curve, open positions, recent fills, and the parameter panel.

## Notes on the parameters

Fifty slots across five groups — `signal`, `entry`, `exit`, `risk`, `sizing`. Every value in `config/params.yaml` is a neutral, illustrative default, documented with the range it accepts. They are **not** tuned to any market, and changing them is how you explore the system: edit, re-run, compare. Nothing adapts itself.

## License

Apache License 2.0 — see [LICENSE](LICENSE).

## Contact

**business@rs-systems-web.com · Telegram [@RS_SYSTEMS](https://t.me/RS_SYSTEMS) · Discord @happypower1k**

Inquiries about the full private engine, or about this lite edition, go to those — not to GitHub issues.
