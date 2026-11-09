use anyhow::{Context, Result, bail};
use chrono::{DateTime, NaiveDateTime, Utc};
use reqwest::Client;
use serde::Deserialize;
use std::time::{Duration, Instant};

pub fn client() -> Client {
    Client::builder()
        .user_agent("cosmic-applet-finance-dashboard/0.1 (local desktop dashboard)")
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(20))
        .https_only(true)
        .build()
        .expect("TLS client initialization")
}

#[derive(Debug, Clone)]
pub struct TimeSync {
    pub offset_ms: i64,
    pub source: String,
    pub fetched_at: DateTime<Utc>,
}

pub async fn sync_time(client: &Client) -> Result<TimeSync> {
    let providers = [
        "https://timeapi.io/api/time/current/zone?timeZone=UTC",
        "https://worldtimeapi.org/api/timezone/Etc/UTC",
    ];
    let mut errors = Vec::new();
    for endpoint in providers {
        let start = Utc::now();
        let elapsed = Instant::now();
        let result: Result<DateTime<Utc>> = async {
            let value: serde_json::Value = client
                .get(endpoint)
                .header("Cache-Control", "no-cache")
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if let Some(datetime) = value["dateTime"].as_str() {
                Ok(NaiveDateTime::parse_from_str(datetime, "%Y-%m-%dT%H:%M:%S%.f")?.and_utc())
            } else {
                Ok(DateTime::parse_from_rfc3339(
                    value["utc_datetime"]
                        .as_str()
                        .context("Time server omitted UTC time")?,
                )?
                .with_timezone(&Utc))
            }
        }
        .await;
        match result {
            Ok(remote) => {
                let midpoint = start
                    + chrono::Duration::milliseconds((elapsed.elapsed().as_millis() / 2) as i64);
                return Ok(TimeSync {
                    offset_ms: (remote - midpoint).num_milliseconds(),
                    source: if endpoint.contains("timeapi.io") {
                        "TimeAPI.io"
                    } else {
                        "WorldTimeAPI"
                    }
                    .into(),
                    fetched_at: Utc::now(),
                });
            }
            Err(e) => errors.push(e.to_string()),
        }
    }
    bail!(
        "Public time sync failed; using OS time. {}",
        errors.join("; ")
    )
}

#[derive(Debug, Clone)]
pub struct PublicIp {
    pub ip: String,
    pub country: String,
    pub country_code: String,
    pub city: String,
    pub fetched_at: DateTime<Utc>,
    pub source: String,
}

pub async fn public_ip(client: &Client) -> Result<PublicIp> {
    #[derive(Deserialize)]
    struct Response {
        ip: Option<String>,
        success: Option<bool>,
        country: Option<String>,
        country_code: Option<String>,
        city: Option<String>,
    }
    for (url, source) in [
        ("https://ipwho.is/", "ipwho.is"),
        ("https://ipapi.co/json/", "ipapi.co"),
    ] {
        let result: Result<PublicIp> = async {
            let value: serde_json::Value = client
                .get(url)
                .header("Cache-Control", "no-cache")
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let data: Response = serde_json::from_value(value.clone())?;
            if data.success == Some(false) || value["error"].as_bool() == Some(true) {
                bail!("IP lookup rejected");
            }
            let ip = data.ip.context("IP service omitted address")?;
            ip.parse::<std::net::IpAddr>()
                .context("IP service returned an invalid address")?;
            Ok(PublicIp {
                ip,
                country: value["country_name"]
                    .as_str()
                    .map(str::to_owned)
                    .or(data.country)
                    .unwrap_or_else(|| "Unknown country".into()),
                country_code: data.country_code.unwrap_or_default(),
                city: data
                    .city
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "Unknown city".into()),
                fetched_at: Utc::now(),
                source: source.into(),
            })
        }
        .await;
        if let Ok(ip) = result {
            return Ok(ip);
        }
    }
    bail!("Public IP lookup unavailable (ipwho.is and ipapi.co)")
}
