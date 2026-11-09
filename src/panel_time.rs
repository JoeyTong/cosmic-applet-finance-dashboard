// Formatting follows cosmic-applet-time 1.11, Copyright 2023 System76.
// SPDX-License-Identifier: GPL-3.0-only
use cosmic_applet_finance_dashboard::config::PanelClock;
use icu::{
    datetime::{
        DateTimeFormatter, DateTimeFormatterPreferences, fieldsets,
        input::{Date, DateTime, Time},
        options::TimePrecision,
    },
    locale::{Locale, preferences::extensions::unicode::keywords::HourCycle},
};

pub fn locale() -> Locale {
    for variable in ["LC_TIME", "LC_ALL", "LANG"] {
        if let Ok(value) = std::env::var(variable) {
            let cleaned = value.split('.').next().unwrap_or(&value).replace('_', "-");
            if let Ok(locale) = Locale::try_from_str(&cleaned) {
                return locale;
            }
            if let Some(language) = cleaned.split('-').next()
                && let Ok(locale) = Locale::try_from_str(language)
            {
                return locale;
            }
        }
    }
    Locale::try_from_str("en-US").unwrap()
}

fn input(now: &jiff::Zoned) -> DateTime<icu::calendar::Gregorian> {
    DateTime {
        date: Date::try_new_gregorian(i32::from(now.year()), now.month() as u8, now.day() as u8)
            .unwrap(),
        time: Time::try_new(now.hour() as u8, now.minute() as u8, now.second() as u8, 0).unwrap(),
    }
}

pub fn horizontal(now: &jiff::Zoned, config: &PanelClock, locale: &Locale) -> String {
    if !config.format_strftime.is_empty()
        && let Ok(text) = jiff::fmt::strtime::format(&config.format_strftime, now)
    {
        return text;
    }
    let datetime = input(now);
    let mut prefs = DateTimeFormatterPreferences::from(locale.clone());
    prefs.hour_cycle = Some(if config.military_time {
        HourCycle::H23
    } else {
        HourCycle::H12
    });
    if config.show_date_in_top_panel {
        if config.show_weekday {
            let mut fields = fieldsets::MDET::medium();
            if !config.show_seconds {
                fields = fields.with_time_precision(TimePrecision::Minute);
            }
            DateTimeFormatter::try_new(prefs, fields)
                .unwrap()
                .format(&datetime)
                .to_string()
        } else {
            let mut fields = fieldsets::MDT::medium();
            if !config.show_seconds {
                fields = fields.with_time_precision(TimePrecision::Minute);
            }
            DateTimeFormatter::try_new(prefs, fields)
                .unwrap()
                .format(&datetime)
                .to_string()
        }
    } else {
        let mut fields = fieldsets::T::medium();
        if !config.show_seconds {
            fields = fields.with_time_precision(TimePrecision::Minute);
        }
        DateTimeFormatter::try_new(prefs, fields)
            .unwrap()
            .format(&datetime)
            .to_string()
    }
}

pub fn vertical(now: &jiff::Zoned, config: &PanelClock, locale: &Locale) -> Vec<String> {
    if !config.format_strftime.is_empty()
        && let Ok(text) = jiff::fmt::strtime::format(&config.format_strftime, now)
    {
        return text.split_whitespace().map(str::to_owned).collect();
    }
    let datetime = input(now);
    let mut prefs = DateTimeFormatterPreferences::from(locale.clone());
    prefs.hour_cycle = Some(if config.military_time {
        HourCycle::H23
    } else {
        HourCycle::H12
    });
    let mut parts = Vec::new();
    if config.show_date_in_top_panel {
        parts.extend(
            DateTimeFormatter::try_new(prefs, fieldsets::MD::medium())
                .unwrap()
                .format(&datetime)
                .to_string()
                .split_whitespace()
                .map(str::to_owned),
        );
        parts.push(String::new()); // Native horizontal divider between date and time.
    }
    let mut fields = fieldsets::T::medium();
    if !config.show_seconds {
        fields = fields.with_time_precision(TimePrecision::Minute);
    }
    parts.extend(
        DateTimeFormatter::try_new(prefs, fields)
            .unwrap()
            .format(&datetime)
            .to_string()
            .split_whitespace()
            .flat_map(|part| part.split(':'))
            .map(str::to_owned),
    );
    parts
}
