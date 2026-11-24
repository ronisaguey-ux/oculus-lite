use std::collections::VecDeque;

use crate::{Indicator, Warmup};

#[derive(Debug, Clone)]
pub struct Sma {
    period: usize,
    window: VecDeque<f64>,
    sum: f64,
}

impl Sma {
    pub fn new(period: usize) -> Self {
        assert!(period > 0, "period must be positive");
        Self {
            period,
            window: VecDeque::with_capacity(period),
            sum: 0.0,
        }
    }
}

impl Indicator for Sma {
    fn update(&mut self, value: f64) -> Option<f64> {
        self.window.push_back(value);
        self.sum += value;
        if self.window.len() > self.period {
            if let Some(old) = self.window.pop_front() {
                self.sum -= old;
            }
        }
        (self.window.len() == self.period).then_some(self.sum / self.period as f64)
    }

    fn value(&self) -> Option<f64> {
        (self.window.len() == self.period).then_some(self.sum / self.period as f64)
    }

    fn name(&self) -> &'static str {
        "sma"
    }
}

impl Warmup for Sma {
    fn warmup(&self) -> usize {
        self.period - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolling_window() {
        let mut sma = Sma::new(3);
        assert_eq!(sma.update(1.0), None);
        assert_eq!(sma.update(2.0), None);
        assert_eq!(sma.update(3.0), Some(2.0));
        assert_eq!(sma.update(4.0), Some(3.0));
        assert_eq!(sma.value(), Some(3.0));
    }
}
