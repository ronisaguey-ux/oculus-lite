/// Session volume-weighted average price with an optional rolling window.
#[derive(Debug, Clone)]
pub struct Vwap {
    period: Option<usize>,
    prices: std::collections::VecDeque<(f64, f64)>,
    pv: f64,
    volume: f64,
}

impl Vwap {
    pub fn new(period: Option<usize>) -> Self {
        Self {
            period,
            prices: std::collections::VecDeque::new(),
            pv: 0.0,
            volume: 0.0,
        }
    }

    pub fn update(&mut self, price: f64, volume: f64) -> Option<f64> {
        self.prices.push_back((price, volume));
        self.pv += price * volume;
        self.volume += volume;
        if let Some(period) = self.period {
            while self.prices.len() > period {
                if let Some((p, v)) = self.prices.pop_front() {
                    self.pv -= p * v;
                    self.volume -= v;
                }
            }
        }
        (self.volume > 0.0).then_some(self.pv / self.volume)
    }

    pub fn value(&self) -> Option<f64> {
        (self.volume > 0.0).then_some(self.pv / self.volume)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_average() {
        let mut vwap = Vwap::new(None);
        assert_eq!(vwap.update(10.0, 1.0), Some(10.0));
        let v = vwap.update(20.0, 3.0).unwrap();
        assert!((v - 17.5).abs() < 1e-12);
    }

    #[test]
    fn rolling_window_expires_old_volume() {
        let mut vwap = Vwap::new(Some(2));
        vwap.update(10.0, 1.0);
        vwap.update(20.0, 1.0);
        assert_eq!(vwap.update(30.0, 1.0), Some(25.0));
    }
}
