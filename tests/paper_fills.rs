use oculus_execution::client::{ExecutionClient, PaperClient};
use oculus_execution::models::{Bar, Event, Liquidity, OrderStatus, OrderType, Side};
use oculus_trader::{run_paper, PaperVenue, Venue};
use oculus_config_rs::Params;

fn bar(open: f64, ts: u64) -> Bar {
    Bar {
        ts,
        open,
        high: open + 0.5,
        low: open - 0.5,
        close: open,
        volume: 100.0,
    }
}

#[test]
fn paper_client_fills_market_order_at_next_open() {
    let mut client = PaperClient::default();
    let id = client
        .submit_order("TEST", Side::Buy, OrderType::Market, 2.0, None, None, 1)
        .unwrap();
    let fills = client.on_bar(&bar(100.0, 2));
    assert_eq!(fills.len(), 1);
    assert_eq!(fills[0].order_id, id);
    assert_eq!(fills[0].qty, 2.0);
    assert_eq!(fills[0].price, 100.0);
    assert_eq!(fills[0].liquidity, Liquidity::Taker);
    assert_eq!(client.manager().status(id), Some(OrderStatus::Filled));
    assert_eq!(client.fills().len(), 1);
    assert!(client.manager().events().contains(&Event::Fill(fills[0].clone())));
}

#[test]
fn paper_venue_reports_fills_without_credentials() {
    let mut venue = PaperVenue::new();
    assert_eq!(venue.name(), "paper");
    venue.submit(Side::Sell, 1.0, 100.0, 1).unwrap();
    let fills = venue.on_bar(&bar(100.0, 2));
    assert_eq!(fills.len(), 1);
    assert_eq!(fills[0].side, Side::Sell);
}

#[test]
fn full_paper_run_produces_fills_and_finite_equity() {
    let params = Params::defaults();
    let bars = oculus_backtester::load_bars(&format!(
        "{}/../sample/bars.csv",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let report = run_paper(&params, &bars).unwrap();
    assert!(report.fills > 0);
    assert!(report.final_equity.is_finite());
}
