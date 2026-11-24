//! Stable-Rust microbenchmark (no nightly, no criterion) for the matching core.
//! Run with `cargo bench -p oculus-execution --bench matching`.

use std::hint::black_box;
use std::time::Instant;

use oculus_execution::matching_core::MatchingCore;
use oculus_execution::models::{Order, OrderType, Side};

fn main() {
    let orders = 20_000usize;
    let snapshots = 200usize;

    let mut core = MatchingCore::new();
    let start = Instant::now();
    for i in 0..orders {
        let id = i as u64 + 1;
        let side = if i % 2 == 0 { Side::Buy } else { Side::Sell };
        let price = if side == Side::Buy { 101.0 } else { 99.0 };
        let order = Order::new(id, "BENCH", side, OrderType::Limit, Some(price), None, 1.0, id);
        core.add_order(order).unwrap();
    }
    let insert = start.elapsed();

    let start = Instant::now();
    let mut fills = 0usize;
    for tick in 0..snapshots {
        let bid = 100.0 + (tick % 3) as f64 * 0.5;
        let ask = 100.5 + (tick % 3) as f64 * 0.5;
        fills += core.match_at(bid, ask, tick as u64).len();
    }
    let matching = start.elapsed();

    println!("oculus-lite matching benchmark");
    println!("orders_inserted={orders} snapshots={snapshots} fills={fills}");
    println!(
        "insert_total={:.3}ms insert_per_order={:.1}ns",
        insert.as_secs_f64() * 1e3,
        insert.as_secs_f64() * 1e9 / orders as f64
    );
    println!(
        "match_total={:.3}ms match_per_snapshot={:.1}us",
        matching.as_secs_f64() * 1e3,
        matching.as_secs_f64() * 1e6 / snapshots as f64
    );
    black_box(core.order_count());
}
