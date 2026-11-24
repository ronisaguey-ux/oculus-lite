use crate::Indicator;

/// Wilder's RSI. Returns `None` until `period + 1` observations are available.
#[derive(Debug, Clone)]
pub struct Rsi {
    period: usize,
    prev: Option<f64>,
    avg_gain: f64,
    avg_loss: f64,
    seen: usize,
    last: Option<f64>,
}

impl Rsi {
    pub fn new(period: usize) -> Self {
        assert!(period > 0, "period must be positive");
        Self {
            period,
            prev: None,
            avg_gain: 0.0,
            avg_loss: 0.0,
            seen: 0,
            last: None,
        }
    }
}

impl Indicator for Rsi {
    fn update(&mut self, value: f64) -> Option<f64> {
        if let Some(prev) = self.prev {
            let change = value - prev;
            let gain = change.max(0.0);
            let loss = (-change).max(0.0);
            if self.seen <= self.period {
                self.avg_gain += gain / self.period as f64;
                self.avg_loss += loss / self.period as f64;
            } else {
                let n = self.period as f64;
                self.avg_gain = (self.avg_gain * (n - 1.0) + gain) / n;
                self.avg_loss = (self.avg_loss * (n - 1.0) + loss) / n;
            }
            self.seen += 1;
            if self.seen > self.period {
                let rsi = if self.avg_loss <= 1e-12 {
                    100.0
                } else {
                    let rs = self.avg_gain / self.avg_loss;
                    100.0 - 100.0 / (1.0 + rs)
                };
                self.last = Some(rsi);
            }
        } else {
            self.prev = Some(value);
            self.seen = 1;
        }
        self.prev = Some(value);
        self.last
    }

    fn value(&self) -> Option<f64> {
        self.last
    }

    fn name(&self) -> &'static str {
        "rsi"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotone_up_is_100() {
        let mut rsi = Rsi::new(3);
        for i in 0..10 {
            rsi.update(i as f64);
        }
        assert_eq!(rsi.value(), Some(100.0));
    }

    #[test]
    fn monotone_down_is_zero() {
        let mut rsi = Rsi::new(3);
        for i in 0..10 {
            rsi.update(-(i as f64));
        }
        assert_eq!(rsi.value(), Some(0.0));
    }
}
