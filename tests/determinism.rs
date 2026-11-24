use oculus_backtester::{load_bars, run};
use oculus_config_rs::load as load_params;
use oculus_execution::matching_core::MatchingCore;
use oculus_execution::models::{Order, OrderType, Side};

fn config_path() -> String {
    format!("{}/../config/params.yaml", env!("CARGO_MANIFEST_DIR"))
}

fn data_path() -> String {
    format!("{}/../sample/", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn backtest_is_bit_for_bit_repeatable() {
    let params = load_params(&config_path()).unwrap();
    let bars = load_bars(&data_path()).unwrap();
    assert_eq!(bars.len(), 2000);

    let first = run(&params, &bars).unwrap();
    let second = run(&params, &bars).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.to_string(), second.to_string());
    assert_eq!(first.bars, 2000);
    assert!(first.fills > 0, "sample data should produce fills");
    assert!(first.final_equity.is_finite());
    assert!(first.sharpe.is_finite());
    assert!((0.0..=100.0).contains(&first.max_drawdown_pct));
}

#[test]
fn matching_core_is_order_independent_of_hash_seed() {
    let build = || {
        let mut core = MatchingCore::new();
        for id in 1..=40u64 {
            let side = if id % 2 == 0 { Side::Buy } else { Side::Sell };
            let price = if side == Side::Buy { 99.0 } else { 101.0 };
            core.add_order(Order::new(
                id,
                "TEST",
                side,
                OrderType::Limit,
                Some(price),
                None,
                1.0,
                id,
            ))
            .unwrap();
        }
        core
    };
    let mut a = build();
    let mut b = build();
    let fa = a.match_at(100.0, 100.0, 50);
    let fb = b.match_at(100.0, 100.0, 50);
    assert_eq!(fa, fb);
    assert_eq!(a.snapshot(), b.snapshot());
}
