//! Symmetric trailing stop. The stop only becomes active once the market has
//! moved `activation` in the favourable direction, after which it follows the
//! most extreme price by `distance`. The update function is pure with respect
//! to its inputs: no randomness, no clock, no hidden state beyond the extremes.

use crate::models::Side;

#[derive(Debug, Clone, PartialEq)]
pub struct TrailingStop {
    pub side: Side,
    pub activation: f64,
    pub distance: f64,
    pub extreme: f64,
    pub armed: bool,
    pub stop_price: Option<f64>,
}

impl TrailingStop {
    pub fn new(side: Side, activation: f64, distance: f64) -> Self {
        Self {
            side,
            activation,
            distance,
            extreme: match side {
                Side::Buy => f64::NEG_INFINITY,
                Side::Sell => f64::INFINITY,
            },
            armed: false,
            stop_price: None,
        }
    }

    /// Feed the latest price (or the bar extreme) and return the current stop.
    pub fn update(&mut self, price: f64) -> Option<f64> {
        match self.side {
            Side::Buy => {
                if price >= self.activation {
                    self.armed = true;
                }
                if price > self.extreme {
                    self.extreme = price;
                }
                if self.armed {
                    self.stop_price = Some(self.extreme - self.distance);
                }
            }
            Side::Sell => {
                if price <= self.activation {
                    self.armed = true;
                }
                if price < self.extreme {
                    self.extreme = price;
                }
                if self.armed {
                    self.stop_price = Some(self.extreme + self.distance);
                }
            }
        }
        self.stop_price
    }

    /// True when the market has traded through the stop.
    pub fn is_hit(&self, price: f64) -> bool {
        match (self.stop_price, self.side) {
            (Some(stop), Side::Buy) => price <= stop,
            (Some(stop), Side::Sell) => price >= stop,
            (None, _) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_stop_follows_the_high() {
        let mut trail = TrailingStop::new(Side::Buy, 105.0, 2.0);
        assert_eq!(trail.update(100.0), None);
        assert_eq!(trail.update(106.0), Some(104.0));
        assert_eq!(trail.update(110.0), Some(108.0));
        assert_eq!(trail.update(108.5), Some(108.0));
        assert!(trail.is_hit(107.9));
        assert!(!trail.is_hit(108.1));
    }

    #[test]
    fn short_stop_follows_the_low() {
        let mut trail = TrailingStop::new(Side::Sell, 95.0, 2.0);
        assert_eq!(trail.update(100.0), None);
        assert_eq!(trail.update(94.0), Some(96.0));
        assert_eq!(trail.update(90.0), Some(92.0));
        assert_eq!(trail.update(91.0), Some(92.0));
        assert!(trail.is_hit(92.5));
    }
}
