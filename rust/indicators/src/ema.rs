use crate::{Indicator, Warmup};

#[derive(Debug, Clone)]
pub struct Ema {
    period: usize,
    alpha: f64,
    last: Option<f64>,
    seen: usize,
}

impl Ema {
    pub fn new(period: usize) -> Self {
        assert!(period > 0, "period must be positive");
        Self {
            period,
            alpha: 2.0 / (period as f64 + 1.0),
            last: None,
            seen: 0,
        }
    }
}

impl Indicator for Ema {
    fn update(&mut self, value: f64) -> Option<f64> {
        self.seen += 1;
        let next = match self.last {
            None => value,
            Some(prev) => prev + self.alpha * (value - prev),
        };
        self.last = Some(next);
        // Seeding with the first observation is the conventional EMA warmup.
        (self.seen >= self.period).then_some(next)
    }

    fn value(&self) -> Option<f64> {
        self.last.filter(|_| self.seen >= self.period)
    }

    fn name(&self) -> &'static str {
        "ema"
    }
}

impl Warmup for Ema {
    fn warmup(&self) -> usize {
        self.period - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_with_first_value() {
        let mut ema = Ema::new(3);
        assert_eq!(ema.update(10.0), None);
        assert_eq!(ema.update(10.0), None);
        let v = ema.update(10.0).unwrap();
        assert!((v - 10.0).abs() < 1e-12);
        let v = ema.update(20.0).unwrap();
        assert!((v - 15.0).abs() < 1e-12);
    }
}
