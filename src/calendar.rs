use crate::config::Settings;
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Utc};
use futures::{StreamExt, stream};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CalendarSource {
    pub id: String,
    pub name: String,
    pub color: String,
    pub holiday: bool,
}

#[derive(Debug, Clone)]
pub enum EventTime {
    AllDay {
        start: NaiveDate,
        end: NaiveDate,
    },
    Timed {
        start: DateTime<Local>,
        end: DateTime<Local>,
    },
}

#[derive(Debug, Clone)]
pub struct CalendarEvent {
    pub id: String,
    pub calendar_id: String,
    pub calendar_name: String,
    pub title: String,
    pub description: String,
    pub location: String,
    pub url: String,
    pub color: String,
    pub holiday: bool,
    pub time: EventTime,
}

impl CalendarEvent {
    pub fn on_day(&self, day: NaiveDate) -> bool {
        match self.time {
            EventTime::AllDay { start, end } => day >= start && day < end,
            EventTime::Timed { start, end } => {
                day >= start.date_naive() && day <= (end - Duration::nanoseconds(1)).date_naive()
            }
        }
    }
    pub fn start_day(&self) -> NaiveDate {
        match self.time {
            EventTime::AllDay { start, .. } => start,
            EventTime::Timed { start, .. } => start.date_naive(),
        }
    }
    pub fn short_time(&self) -> String {
        match self.time {
            EventTime::AllDay { .. } => "All day".into(),
            EventTime::Timed { start, .. } => start.format("%H:%M").to_string(),
        }
    }
    pub fn details(&self) -> String {
        let when = match self.time {
            EventTime::AllDay { start, end } if end == start + Duration::days(1) => {
                format!("{start} · All day")
            }
            EventTime::AllDay { start, end } => {
                format!("{start} – {} · All day", end - Duration::days(1))
            }
            EventTime::Timed { start, end } => format!(
                "{} – {} (local time)",
                start.format("%a %d %b %H:%M %Z"),
                end.format("%a %d %b %H:%M %Z")
            ),
        };
        let kind = if self.holiday { "Holiday" } else { "Event" };
        let mut result = format!("{}\n{when}\n{kind} · {}", self.title, self.calendar_name);
        if !self.location.is_empty() {
            result.push_str(&format!("\n{}", self.location));
        }
        if !self.description.is_empty() {
            result.push_str(&format!("\n{}", plain_text(&self.description)));
        }
        result
    }
}

#[derive(Debug, Clone, Default)]
pub struct CalendarSnapshot {
    pub calendars: Vec<CalendarSource>,
    pub events: Vec<CalendarEvent>,
    pub warnings: Vec<String>,
    pub failed_calendars: Vec<String>,
    pub fetched_at: Option<DateTime<Utc>>,
}

pub fn week_start(today: NaiveDate, first_day: u8) -> NaiveDate {
    let distance = (today.weekday().num_days_from_monday() + 7 - u32::from(first_day.min(6))) % 7;
    today - Duration::days(i64::from(distance))
}

/// Calculate each local midnight separately; DST weeks are not necessarily 336 hours.
pub fn window_bounds<T: TimeZone>(
    today: NaiveDate,
    first_day: u8,
    zone: T,
) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    let start = week_start(today, first_day);
    let midnight = |date: NaiveDate| -> Result<DateTime<Utc>> {
        // A handful of zones move their clocks at midnight; choose the first valid local time.
        for hour in 0..=3 {
            if let Some(value) = zone
                .from_local_datetime(&date.and_hms_opt(hour, 0, 0).unwrap())
                .earliest()
            {
                return Ok(value.with_timezone(&Utc));
            }
        }
        bail!("Cannot resolve local calendar boundary")
    };
    Ok((midnight(start)?, midnight(start + Duration::days(14))?))
}

async fn pages(
    client: &Client,
    token: &str,
    url: &str,
    query: &[(&str, String)],
) -> Result<Vec<Value>> {
    let mut all = Vec::new();
    let mut next = String::new();
    let mut seen = HashSet::new();
    loop {
        let mut request = client.get(url).bearer_auth(token).query(query);
        if !next.is_empty() {
            request = request.query(&[("pageToken", &next)]);
        }
        let response: Value = request.send().await?.error_for_status()?.json().await?;
        if let Some(items) = response["items"].as_array() {
            all.extend(items.iter().cloned());
        }
        next = response["nextPageToken"]
            .as_str()
            .unwrap_or_default()
            .into();
        if next.is_empty() {
            break;
        }
        if !seen.insert(next.clone()) {
            bail!("Google returned a repeated pagination token");
        }
    }
    Ok(all)
}

pub async fn fetch(
    client: &Client,
    settings: &Settings,
    today: NaiveDate,
    first_day: u8,
) -> Result<CalendarSnapshot> {
    let token = crate::oauth::access_token(client, &settings.google_client_path).await?;
    let items = pages(
        client,
        &token,
        "https://www.googleapis.com/calendar/v3/users/me/calendarList",
        &[("maxResults", "250".into())],
    )
    .await?;
    let mut calendars: Vec<_> = items
        .iter()
        .filter(|v| v["hidden"].as_bool() != Some(true))
        .filter_map(|v| {
            let id = v["id"].as_str()?;
            Some(CalendarSource {
                id: id.into(),
                name: v["summaryOverride"]
                    .as_str()
                    .or(v["summary"].as_str())
                    .unwrap_or(id)
                    .into(),
                color: v["backgroundColor"].as_str().unwrap_or("#7aa2f7").into(),
                holiday: id.contains("#holiday@") || id.contains(".holiday@"),
            })
        })
        .collect();
    // Google often gives every holiday calendar the same green. Give countries separate
    // colors (and a diamond marker) while preserving Google's colors for ordinary events.
    let holiday_colors = [
        "#7aa2f7", "#e0af68", "#bb9af7", "#73daca", "#f7768e", "#9ece6a", "#ff9e64", "#7dcfff",
        "#dca5f5", "#c0caf5", "#c8a77a", "#a1d1b1",
    ];
    let mut holiday_ids: Vec<_> = calendars
        .iter()
        .filter(|c| c.holiday)
        .map(|c| c.id.clone())
        .collect();
    holiday_ids.sort();
    for source in &mut calendars {
        if let Some(index) = holiday_ids.iter().position(|id| id == &source.id) {
            source.color = holiday_colors[index % holiday_colors.len()].into();
        }
    }
    let (start, end) = window_bounds(today, first_day, Local)?;
    let query = vec![
        ("timeMin", start.to_rfc3339()),
        ("timeMax", end.to_rfc3339()),
        ("singleEvents", "true".into()),
        ("orderBy", "startTime".into()),
        ("showDeleted", "false".into()),
        ("maxResults", "2500".into()),
    ];
    let selected: Vec<CalendarSource> = calendars
        .iter()
        .filter(|c| !settings.disabled_calendars.contains(&c.id))
        .cloned()
        .collect();
    let results = stream::iter(selected.into_iter().map(|source| {
        let token = token.clone();
        let query = query.clone();
        let client = client.clone();
        async move {
            let mut url =
                url::Url::parse("https://www.googleapis.com/calendar/v3/calendars/").unwrap();
            url.path_segments_mut()
                .unwrap()
                .pop_if_empty()
                .push(&source.id)
                .push("events");
            (source, pages(&client, &token, url.as_str(), &query).await)
        }
    }))
    .buffer_unordered(4)
    .collect::<Vec<_>>()
    .await;
    let mut snapshot = CalendarSnapshot::default();
    for (source, result) in results {
        match result {
            Ok(items) => {
                for value in items {
                    if value["status"].as_str() == Some("cancelled") {
                        continue;
                    }
                    match parse_event(&value, &source) {
                        Ok(event) => snapshot.events.push(event),
                        Err(_) => snapshot
                            .warnings
                            .push(format!("An event in {} could not be read", source.name)),
                    }
                }
            }
            Err(e) => {
                snapshot.failed_calendars.push(source.id.clone());
                snapshot.warnings.push(format!("{}: {e}", source.name));
            }
        }
    }
    snapshot
        .events
        .sort_by_key(|e| (e.start_day(), e.short_time(), e.title.clone()));
    snapshot.calendars = calendars;
    snapshot.fetched_at = Some(Utc::now());
    Ok(snapshot)
}

pub fn parse_event(value: &Value, source: &CalendarSource) -> Result<CalendarEvent> {
    let time = if let Some(date) = value["start"]["date"].as_str() {
        let start = date.parse::<NaiveDate>()?;
        let end = value["end"]["date"]
            .as_str()
            .context("Missing all-day end")?
            .parse::<NaiveDate>()?;
        anyhow::ensure!(end > start, "Invalid all-day range");
        EventTime::AllDay { start, end }
    } else {
        let start = DateTime::parse_from_rfc3339(
            value["start"]["dateTime"]
                .as_str()
                .context("Missing event start")?,
        )?
        .with_timezone(&Local);
        let end = DateTime::parse_from_rfc3339(
            value["end"]["dateTime"]
                .as_str()
                .context("Missing event end")?,
        )?
        .with_timezone(&Local);
        // Google permits zero-duration events. Give them an instant on their start day.
        anyhow::ensure!(end >= start, "Invalid timed event range");
        EventTime::Timed {
            start,
            end: if end == start {
                end + Duration::nanoseconds(1)
            } else {
                end
            },
        }
    };
    let event_colors = [
        "#7986cb", "#33b679", "#8e24aa", "#e67c73", "#f6c026", "#f5511d", "#039be5", "#616161",
        "#3f51b5", "#0b8043", "#d50000",
    ];
    let color = if source.holiday {
        source.color.clone()
    } else {
        value["colorId"]
            .as_str()
            .and_then(|s| s.parse::<usize>().ok())
            .and_then(|i| i.checked_sub(1))
            .and_then(|i| event_colors.get(i))
            .copied()
            .unwrap_or(&source.color)
            .to_owned()
    };
    Ok(CalendarEvent {
        id: value["id"].as_str().context("Missing event ID")?.into(),
        calendar_id: source.id.clone(),
        calendar_name: source.name.clone(),
        title: value["summary"]
            .as_str()
            .unwrap_or("(Untitled event)")
            .into(),
        description: value["description"].as_str().unwrap_or_default().into(),
        location: value["location"].as_str().unwrap_or_default().into(),
        url: value["htmlLink"]
            .as_str()
            .unwrap_or("https://calendar.google.com/calendar/u/0/r")
            .into(),
        color,
        holiday: source.holiday,
        time,
    })
}

fn plain_text(html: &str) -> String {
    let mut inside = false;
    let text: String = html
        .chars()
        .filter(|&c| match c {
            '<' => {
                inside = true;
                false
            }
            '>' => {
                inside = false;
                false
            }
            _ => !inside,
        })
        .collect();
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn source() -> CalendarSource {
        CalendarSource {
            id: "test".into(),
            name: "Test".into(),
            color: "#123456".into(),
            holiday: false,
        }
    }
    #[test]
    fn this_week_and_next_cross_year_and_sunday_boundary() {
        let date = NaiveDate::from_ymd_opt(2027, 1, 3).unwrap();
        assert_eq!(week_start(date, 0).to_string(), "2026-12-28");
        assert_eq!(week_start(date, 6), date);
    }
    #[test]
    fn dst_window_uses_local_midnights_not_fixed_hours() {
        let (start, end) = window_bounds(
            NaiveDate::from_ymd_opt(2026, 3, 4).unwrap(),
            0,
            chrono_tz::America::New_York,
        )
        .unwrap();
        assert_eq!((end - start).num_hours(), 335);
    }
    #[test]
    fn all_day_end_is_exclusive_and_zero_duration_timed_events_are_visible() {
        let event = parse_event(
            &json!({"id":"a", "start":{"date":"2026-11-08"}, "end":{"date":"2026-11-10"}}),
            &source(),
        )
        .unwrap();
        assert!(event.on_day("2026-11-09".parse().unwrap()));
        assert!(!event.on_day("2026-11-10".parse().unwrap()));
        let event = parse_event(&json!({"id":"b", "start":{"dateTime":"2026-11-08T12:00:00Z"}, "end":{"dateTime":"2026-11-08T12:00:00Z"}}), &source()).unwrap();
        assert!(event.on_day(event.start_day()));
    }
}
