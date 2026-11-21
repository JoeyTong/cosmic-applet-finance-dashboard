use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct Quote {
    pub price: f64,
    pub change: Option<f64>,
    pub percent: Option<f64>,
    pub decimals: usize,
    pub mode: String,
    /// Time of the last one-minute bar, NOT the time of the last trade.
    pub bar_time: Option<DateTime<Utc>>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct MarketSnapshot {
    pub quotes: BTreeMap<String, Quote>,
    pub unavailable: Vec<String>,
}

pub fn ticker(symbol: &str) -> String {
    match symbol {
        "BRN1" => "ICEEUR:BRN1!".into(),
        "GOLD" => "TVC:GOLD".into(),
        _ => format!("FX_IDC:{symbol}"),
    }
}

pub async fn fetch(client: &Client, pairs: &[String]) -> Result<MarketSnapshot> {
    let symbols: Vec<String> = pairs
        .iter()
        .cloned()
        .chain(["BRN1".into(), "GOLD".into()])
        .collect();
    // Public TradingView scanner snapshots; this is not a streaming/execution feed.
    let body = json!({
        "symbols": {"tickers": symbols.iter().map(|s| ticker(s)).collect::<Vec<_>>(), "query": {"types": []}},
        "columns": ["close", "change", "change_abs", "pricescale", "update_mode", "time|1"]
    });
    let response: Value = client
        .post("https://scanner.tradingview.com/global/scan")
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    parse_snapshot(&response, &symbols, Utc::now())
}

pub fn parse_snapshot(
    value: &Value,
    symbols: &[String],
    fetched_at: DateTime<Utc>,
) -> Result<MarketSnapshot> {
    let rows = value["data"]
        .as_array()
        .context("Market provider returned no data array")?;
    let mut snapshot = MarketSnapshot::default();
    for symbol in symbols {
        let source = ticker(symbol);
        let data = rows
            .iter()
            .find(|r| r["s"].as_str() == Some(&source))
            .and_then(|r| r["d"].as_array());
        if let Some(data) = data.filter(|d| d.len() >= 6)
            && let Some(price) = data[0].as_f64().filter(|p| p.is_finite() && *p > 0.0)
        {
            let scale = data[3]
                .as_f64()
                .filter(|v| v.is_finite() && *v >= 1.0)
                .unwrap_or(10000.0);
            snapshot.quotes.insert(
                symbol.clone(),
                Quote {
                    price,
                    percent: data[1].as_f64().filter(|v| v.is_finite()),
                    change: data[2].as_f64().filter(|v| v.is_finite()),
                    decimals: (scale.log10().round() as usize).min(8),
                    mode: data[4].as_str().unwrap_or("unknown").into(),
                    bar_time: data[5]
                        .as_i64()
                        .and_then(|t| DateTime::from_timestamp(t, 0)),
                    fetched_at,
                },
            );
            continue;
        }
        snapshot.unavailable.push(symbol.clone());
    }
    if snapshot.quotes.is_empty() && !symbols.is_empty() {
        bail!("No requested instruments are currently available from TradingView");
    }
    Ok(snapshot)
}

impl Quote {
    pub fn latency(&self) -> String {
        if let Some(seconds) = self
            .mode
            .strip_prefix("delayed_streaming_")
            .and_then(|s| s.parse::<u32>().ok())
        {
            format!("{} min delayed", seconds / 60)
        } else if self.mode == "streaming" {
            "Indicative snapshot".into()
        } else {
            "Delay unknown".into()
        }
    }
    pub fn stale(&self, now: DateTime<Utc>) -> bool {
        (now - self.fetched_at).num_seconds() > 1200
    }
    pub fn old_bar(&self, now: DateTime<Utc>) -> bool {
        self.bar_time
            .is_some_and(|time| (now - time).num_minutes() > 30)
    }
}

pub fn pair_description(symbol: &str) -> String {
    match symbol {
        "BRN1" => "Brent crude \u{b7} Front-month futures \u{b7} USD/barrel".to_owned(),
        "GOLD" => "Spot gold \u{b7} XAU/USD \u{b7} USD/troy ounce".to_owned(),
        _ if symbol.len() == 6 && symbol.is_ascii() => {
            let (base, _) = currency(&symbol[..3]);
            let (quote, _) = currency(&symbol[3..]);
            format!("{base} / {quote}")
        }
        _ => symbol.to_owned(),
    }
}

pub fn currency(code: &str) -> (&str, &str) {
    match code {
        "USD" => ("US Dollar", "US"),
        "CNH" => ("Offshore Chinese Yuan", "CN"),
        "CNY" => ("Chinese Yuan", "CN"),
        "JPY" => ("Japanese Yen", "JP"),
        "HKD" => ("Hong Kong Dollar", "HK"),
        "SGD" => ("Singapore Dollar", "SG"),
        "GBP" => ("British Pound", "GB"),
        "EUR" => ("Euro", "EU"),
        "AUD" => ("Australian Dollar", "AU"),
        "CAD" => ("Canadian Dollar", "CA"),
        "CHF" => ("Swiss Franc", "CH"),
        "NZD" => ("New Zealand Dollar", "NZ"),
        "INR" => ("Indian Rupee", "IN"),
        "KRW" => ("South Korean Won", "KR"),
        "TWD" => ("Taiwan Dollar", "TW"),
        "THB" => ("Thai Baht", "TH"),
        "MYR" => ("Malaysian Ringgit", "MY"),
        "IDR" => ("Indonesian Rupiah", "ID"),
        "AED" => ("UAE Dirham", "AE"),
        "SAR" => ("Saudi Riyal", "SA"),
        "ZAR" => ("South African Rand", "ZA"),
        "MXN" => ("Mexican Peso", "MX"),
        "BRL" => ("Brazilian Real", "BR"),
        "NOK" => ("Norwegian Krone", "NO"),
        "SEK" => ("Swedish Krona", "SE"),
        "DKK" => ("Danish Krone", "DK"),
        "PLN" => ("Polish Zloty", "PL"),
        "TRY" => ("Turkish Lira", "TR"),
        _ => (code, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_provider_daily_change_and_preserves_unavailable_pairs() {
        let data = json!({"data":[{"s":"FX_IDC:USDCNH","d":[6.67196,0.1289,0.00859,1000000,"streaming",1794002340]}, {"s":"ICEEUR:BRN1!","d":[98.32,0.03052,0.03,100,"delayed_streaming_600",null]}]});
        let parsed = parse_snapshot(
            &data,
            &["USDCNH".into(), "USDCNY".into(), "BRN1".into()],
            Utc::now(),
        )
        .unwrap();
        assert_eq!(parsed.quotes["USDCNH"].decimals, 6);
        assert_eq!(parsed.quotes["USDCNH"].change, Some(0.00859));
        assert_eq!(parsed.quotes["BRN1"].latency(), "10 min delayed");
        assert_eq!(parsed.unavailable, ["USDCNY"]);
        assert!(parsed.quotes["BRN1"].bar_time.is_none());
    }
    #[test]
    fn rejects_null_prices_instead_of_showing_zero() {
        assert!(
            parse_snapshot(
                &json!({"data":[{"s":"FX_IDC:EURUSD","d":[null,null,null,10000,null,null]}]}),
                &["EURUSD".into()],
                Utc::now()
            )
            .is_err()
        );
    }
}
