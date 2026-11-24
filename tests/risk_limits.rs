use oculus_config_rs::{parse, Params};
use oculus_execution::models::{Order, OrderType, Side};
use oculus_execution::protection::{AccountState, RiskError, RiskGuard};
use oculus_trader::live_credentials;

const SAMPLE: &str = include_str!("../config/params.yaml");

fn order(side: Side, qty: f64) -> Order {
    Order::new(1, "TEST", side, OrderType::Market, None, None, qty, 0)
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
fn size_exposure_and_rate_limits_hold() {
    let guard = RiskGuard::default();
    assert_eq!(guard.check(&order(Side::Buy, 5.1), &account()), Err(RiskError::MaxOrderQty));
    let mut acct = account();
    acct.position_qty = 9.0;
    assert_eq!(guard.check(&order(Side::Buy, 2.0), &acct), Err(RiskError::MaxPosition));
    let mut acct = account();
    acct.orders_this_bar = 2;
    assert_eq!(guard.check(&order(Side::Buy, 1.0), &acct), Err(RiskError::OrderRate));
}

#[test]
fn loss_drawdown_and_kill_switch_halt_trading() {
    let mut guard = RiskGuard::default();
    let mut acct = account();
    acct.daily_pnl = -4_000.0;
    assert_eq!(guard.check(&order(Side::Buy, 1.0), &acct), Err(RiskError::DailyLoss));
    let mut acct = account();
    acct.equity = 89_000.0;
    assert_eq!(guard.check(&order(Side::Buy, 1.0), &acct), Err(RiskError::Drawdown));
    guard.kill_switch = true;
    assert_eq!(guard.check(&order(Side::Buy, 1.0), &account()), Err(RiskError::KillSwitch));
}

#[test]
fn config_validation_rejects_bad_values_and_inconsistent_pairs() {
    let bad_range = SAMPLE.replace("lookback_fast: 20", "lookback_fast: 999");
    assert!(parse(&bad_range).is_err());
    let bad_pair = SAMPLE.replace("lookback_slow: 50", "lookback_slow: 15");
    assert!(parse(&bad_pair).is_err());
    let bad_bool = SAMPLE.replace("kill_switch: false", "kill_switch: maybe");
    assert!(parse(&bad_bool).is_err());
    assert_eq!(parse(SAMPLE).unwrap(), Params::defaults());
}

#[test]
fn live_path_refuses_without_all_credentials() {
    let err = live_credentials(None, None, None).unwrap_err();
    assert!(err.contains("OCULUS_API_KEY"));
    assert!(err.contains("OCULUS_API_SECRET"));
    assert!(err.contains("OCULUS_LIVE_URL"));
}
