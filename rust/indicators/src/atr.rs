/// Wilder's Average True Range over bars (high, low, previous close).
#[derive(Debug, Clone)]
pub struct Atr {
    period: usize,
    prev_close: Option<f64>,
    avg: f64,
    seen: usize,
}

impl Atr {
    pub fn new(period: usize) -> Self {
        assert!(period > 0, "period must be positive");
        Self {
            period,
            prev_close: None,
            avg: 0.0,
            seen: 0,
        }
    }

    pub fn update_bar(&mut self, high: f64, low: f64, close: f64) -> Option<f64> {
        let true_range = match self.prev_close {
            Some(prev) => (high - low)
                .max((high - prev).abs())
                .max((low - prev).abs()),
            None => high - low,
        };
        self.seen += 1;
        if self.seen <= self.period {
            self.avg += true_range / self.period as f64;
        } else {
            let n = self.period as f64;
            self.avg = (self.avg * (n - 1.0) + true_range) / n;
        }
        self.prev_close = Some(close);
        (self.seen >= self.period).then_some(self.avg)
    }

    pub fn value(&self) -> Option<f64> {
        (self.seen >= self.period).then_some(self.avg)
    }

    pub fn name(&self) -> &'static str {
        "atr"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_range() {
        let mut atr = Atr::new(2);
        assert_eq!(atr.update_bar(11.0, 9.0, 10.0), None);
        assert_eq!(atr.update_bar(12.0, 10.0, 11.0), Some(2.0));
        assert_eq!(atr.update_bar(12.0, 10.0, 11.0), Some(2.0));
    }
}
