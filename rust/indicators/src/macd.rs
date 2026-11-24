use crate::ema::Ema;
use crate::Indicator;

/// MACD line, signal line and histogram built from two EMAs and one signal EMA.
#[derive(Debug, Clone)]
pub struct Macd {
    fast: Ema,
    slow: Ema,
    signal: Ema,
    last: Option<(f64, f64, f64)>,
}

impl Macd {
    pub fn new(fast: usize, slow: usize, signal: usize) -> Self {
        assert!(fast < slow, "fast period must be shorter than slow period");
        Self {
            fast: Ema::new(fast),
            slow: Ema::new(slow),
            signal: Ema::new(signal),
            last: None,
        }
    }

    pub fn histogram(&self) -> Option<f64> {
        self.last.map(|(_, _, h)| h)
    }
}

impl Indicator for Macd {
    fn update(&mut self, value: f64) -> Option<f64> {
        self.fast.update(value);
        self.slow.update(value);
        match (self.fast.value(), self.slow.value()) {
            (Some(f), Some(s)) => {
                let macd = f - s;
                self.signal.update(macd);
                self.signal.value().map(|signal| {
                    let hist = macd - signal;
                    self.last = Some((macd, signal, hist));
                    macd
                })
            }
            _ => None,
        }
    }

    fn value(&self) -> Option<f64> {
        self.last.map(|(m, _, _)| m)
    }

    fn name(&self) -> &'static str {
        "macd"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_series_has_zero_macd() {
        let mut macd = Macd::new(3, 6, 2);
        for _ in 0..20 {
            macd.update(100.0);
        }
        let (line, signal, hist) = macd.last.unwrap();
        assert!(line.abs() < 1e-12);
        assert!(signal.abs() < 1e-12);
        assert!(hist.abs() < 1e-12);
    }
}
