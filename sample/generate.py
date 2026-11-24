#!/usr/bin/env python3
"""Generate the small synthetic OHLCV sample used by the backtest and paper trader.

No real market data is used. The generator is a fixed linear congruential PRNG,
so the committed CSV can be reproduced byte-for-byte on any machine:

    python3 sample/generate.py
"""

from pathlib import Path

STATE = 0x1234_5678
MODULUS = 0x8000_0000


def rand() -> float:
    global STATE
    STATE = (STATE * 1_103_515_245 + 12_345) & 0x7FFF_FFFF
    return STATE / MODULUS


def main() -> None:
    STATE = 0x1234_5678
    bars = []
    price = 100.0
    start_ts = 1_767_225_600  # 2026-01-01T00:00:00Z
    for i in range(2000):
        open_price = price
        close_price = max(1.0, open_price + (rand() - 0.5) * 0.8)
        high = max(open_price, close_price) + rand() * 0.35
        low = min(open_price, close_price) - rand() * 0.35
        volume = 1000 + int(rand() * 4000)
        bars.append(
            f"{start_ts + i * 300},SYNTH,{open_price:.4f},{high:.4f},"
            f"{low:.4f},{close_price:.4f},{volume}"
        )
        price = close_price

    out = Path(__file__).with_name("bars.csv")
    out.write_text("timestamp,symbol,open,high,low,close,volume\n" + "\n".join(bars) + "\n")
    print(f"wrote {out} ({len(bars)} bars)")


if __name__ == "__main__":
    main()
