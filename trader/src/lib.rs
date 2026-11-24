//! Paper-first trader. Paper is the default and needs no credentials. The live
//! path is only reachable with an explicit `--live` flag plus credentials and a
//! user-supplied generic gateway URL; it refuses to start otherwise.
//!
//! No venue host, key or endpoint is embedded in this repository. The live
//! transport is deliberately tiny: it sends a plain JSON order to the URL the
//! operator supplies through `OCULUS_LIVE_URL`. Fills on that path are reported
//! by the venue, not inferred locally.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use oculus_backtester::{signals, Report};
use oculus_config_rs::Params;
use oculus_execution::client::{ExecutionClient, PaperClient};
use oculus_execution::models::{Bar, Fill, OrderType, Side};

#[derive(Debug, Clone, PartialEq)]
pub struct Credentials {
    pub api_key: String,
    pub api_secret: String,
    pub base_url: String,
}

/// Build live credentials from values supplied by the caller (normally env vars).
/// Returns the list of missing names instead of a generic error so the operator
/// knows exactly what to provide.
pub fn live_credentials(
    api_key: Option<&str>,
    api_secret: Option<&str>,
    base_url: Option<&str>,
) -> Result<Credentials, String> {
    let mut missing = Vec::new();
    if api_key.is_none_or(str::is_empty) {
        missing.push("OCULUS_API_KEY");
    }
    if api_secret.is_none_or(str::is_empty) {
        missing.push("OCULUS_API_SECRET");
    }
    if base_url.is_none_or(str::is_empty) {
        missing.push("OCULUS_LIVE_URL");
    }
    if !missing.is_empty() {
        return Err(format!("--live requires: {}", missing.join(", ")));
    }
    let base_url = base_url.unwrap_or_default().to_owned();
    if !base_url.starts_with("http://") {
        return Err("OCULUS_LIVE_URL must start with http:// (the lite edition has no TLS stack)".to_owned());
    }
    Ok(Credentials {
        api_key: api_key.unwrap_or_default().to_owned(),
        api_secret: api_secret.unwrap_or_default().to_owned(),
        base_url,
    })
}

/// Minimal submission interface shared by both trader paths.
pub trait Venue {
    fn name(&self) -> &'static str;
    fn submit(&mut self, side: Side, qty: f64, price: f64, ts: u64) -> Result<u64, String>;
}

/// Paper venue. Orders rest in the matching core and fill on the next bar.
#[derive(Debug, Default)]
pub struct PaperVenue {
    client: PaperClient,
}

impl PaperVenue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_bar(&mut self, bar: &Bar) -> Vec<Fill> {
        self.client.on_bar(bar)
    }

    pub fn fills(&self) -> &[Fill] {
        self.client.fills()
    }
}

impl Venue for PaperVenue {
    fn name(&self) -> &'static str {
        "paper"
    }

    fn submit(&mut self, side: Side, qty: f64, _price: f64, ts: u64) -> Result<u64, String> {
        self.client.submit_order("SYNTH", side, OrderType::Market, qty, None, None, ts)
    }
}

/// Live venue. Sends an order to an operator-supplied HTTP gateway.
#[derive(Debug)]
pub struct LiveVenue {
    credentials: Credentials,
    next_id: u64,
}

impl LiveVenue {
    pub fn new(credentials: Credentials) -> Self {
        Self {
            credentials,
            next_id: 1,
        }
    }
}

fn parse_url(url: &str) -> Result<(String, u16, String), String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| "live URL must start with http://".to_owned())?;
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, format!("/{path}")),
        None => (rest, "/".to_owned()),
    };
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (host.to_owned(), port.parse::<u16>().map_err(|e| e.to_string())?),
        None => (authority.to_owned(), 80u16),
    };
    if host.is_empty() {
        return Err("live URL has no host".to_owned());
    }
    Ok((host, port, path))
}

impl Venue for LiveVenue {
    fn name(&self) -> &'static str {
        "live"
    }

    fn submit(&mut self, side: Side, qty: f64, price: f64, ts: u64) -> Result<u64, String> {
        let (host, port, path) = parse_url(&self.credentials.base_url)?;
        let mut stream = TcpStream::connect((host.as_str(), port))
            .map_err(|e| format!("cannot reach live gateway: {e}"))?;
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
        stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

        let body = format!(
            "{{\"symbol\":\"SYNTH\",\"side\":\"{}\",\"qty\":{qty},\"limit_price\":{price},\"client_ts\":{ts}}}",
            side.as_str()
        );
        let request = format!(
            "POST {path} HTTP/1.1\r\nHost: {host}:{port}\r\nX-API-Key: {}\r\nX-API-Secret: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            self.credentials.api_key,
            self.credentials.api_secret,
            body.len()
        );
        stream
            .write_all(request.as_bytes())
            .map_err(|e| format!("live send failed: {e}"))?;
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .map_err(|e| format!("live read failed: {e}"))?;
        let status = response.lines().next().unwrap_or("");
        if !(status.contains(" 200 ") || status.contains(" 201 ") || status.contains(" 202 ")) {
            return Err(format!("live gateway rejected order ({})", status.trim()));
        }
        let id = self.next_id;
        self.next_id += 1;
        Ok(id)
    }
}

/// Paper run: identical engine to the backtester, no credentials, no network.
pub fn run_paper(params: &Params, bars: &[Bar]) -> Result<Report, String> {
    oculus_backtester::run(params, bars)
}

/// Live run: the same illustrative signal, but orders go through the live venue.
/// Fills are not simulated here; the venue is the source of truth.
pub fn run_live(params: &Params, credentials: Credentials, bars: &[Bar]) -> Result<Vec<u64>, String> {
    if params.risk.kill_switch {
        return Err("risk.kill_switch is true; live trading refused".to_owned());
    }
    let signal_series = signals(params, bars);
    let mut venue = LiveVenue::new(credentials);
    let mut accepted = Vec::new();
    let mut last_signal = 0i8;
    for (index, bar) in bars.iter().enumerate() {
        let signal = signal_series[index];
        if signal == 0 || signal == last_signal {
            continue;
        }
        last_signal = signal;
        let side = if signal > 0 { Side::Buy } else { Side::Sell };
        let qty = params.sizing.fixed_qty.min(params.risk.max_order_qty);
        let id = venue.submit(side, qty, bar.close, bar.ts)?;
        accepted.push(id);
    }
    Ok(accepted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(ts: u64) -> Bar {
        Bar {
            ts,
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.0,
            volume: 10.0,
        }
    }

    #[test]
    fn paper_venue_fills_on_next_bar() {
        let mut venue = PaperVenue::new();
        let id = venue.submit(Side::Buy, 1.0, 100.0, 1).unwrap();
        assert_eq!(venue.on_bar(&bar(2)).len(), 1);
        let fills = venue.fills();
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].order_id, id);
        assert_eq!(fills[0].price, 100.0);
    }

    #[test]
    fn live_gate_reports_every_missing_item() {
        let err = live_credentials(None, None, None).unwrap_err();
        assert!(err.contains("OCULUS_API_KEY"));
        assert!(err.contains("OCULUS_API_SECRET"));
        assert!(err.contains("OCULUS_LIVE_URL"));
        assert!(live_credentials(Some("k"), Some("s"), Some("http://127.0.0.1:9/orders")).is_ok());
        assert!(live_credentials(Some("k"), Some("s"), Some("https://example.invalid")).is_err());
    }
}
