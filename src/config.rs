use anyhow::{Context, Result, bail};
use cosmic::cosmic_config::ConfigGet;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

pub const DEFAULT_BANNER_MESSAGE: &str = "Thrived Today, Leading Tomorrow";
pub const MAX_BANNER_CHARACTERS: usize = 160;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clock {
    pub label: String,
    pub timezone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub banner_message: String,
    pub clocks: Vec<Clock>,
    pub forex: Vec<String>,
    pub google_client_path: String,
    pub google_email: String,
    pub disabled_calendars: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            banner_message: DEFAULT_BANNER_MESSAGE.into(),
            clocks: [
                ("Singapore", "Asia/Singapore"),
                ("GMT", "Etc/UTC"),
                ("New York", "America/New_York"),
                ("Tokyo", "Asia/Tokyo"),
            ]
            .into_iter()
            .map(|(label, timezone)| Clock {
                label: label.into(),
                timezone: timezone.into(),
            })
            .collect(),
            forex: [
                "USDCNH", "USDJPY", "USDHKD", "JPYCNY", "SGDCNY", "GBPCNH", "EURCNH", "GBPUSD",
                "GBPJPY", "GBPSGD", "GBPEUR",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            google_client_path: String::new(),
            google_email: String::new(),
            disabled_calendars: Vec::new(),
        }
    }
}

pub const CONFIG_DIRECTORY: &str = "cosmic-applet-finance-dashboard";
const PREVIOUS_CONFIG_DIRECTORY: &str = "daily-work-cosmic-applet";

pub fn config_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    let current = base.join(CONFIG_DIRECTORY);
    adopt_previous_config(&base.join(PREVIOUS_CONFIG_DIRECTORY), &current);
    current
}

fn adopt_previous_config(previous: &Path, current: &Path) {
    let previous_settings = previous.join("settings.json");
    let current_settings = current.join("settings.json");
    if current_settings.exists() || !previous_settings.is_file() {
        return;
    }
    if !current.exists() && fs::rename(previous, current).is_ok() {
        retarget_client_path(&current.join("settings.json"), previous, current);
        return;
    }
    if fs::create_dir_all(current).is_err() {
        return;
    }
    for name in ["settings.json", "google-client.json"] {
        let from = previous.join(name);
        let to = current.join(name);
        if from.is_file() && !to.exists() {
            let _ = fs::rename(&from, &to);
        }
    }
    retarget_client_path(&current.join("settings.json"), previous, current);
}

fn retarget_client_path(settings: &Path, previous: &Path, current: &Path) {
    let Ok(bytes) = fs::read(settings) else {
        return;
    };
    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return;
    };
    let Some(path) = value
        .get("google_client_path")
        .and_then(|item| item.as_str())
    else {
        return;
    };
    if Path::new(path) != previous.join("google-client.json") {
        return;
    }
    value["google_client_path"] =
        serde_json::Value::String(current.join("google-client.json").display().to_string());
    let Ok(encoded) = serde_json::to_vec_pretty(&value) else {
        return;
    };
    let _ = private_write(settings, &encoded);
}

pub fn private_write(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path.parent().context("Configuration path has no parent")?;
    fs::create_dir_all(parent)?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    let temporary = path.with_extension("tmp");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temporary)?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    file.write_all(contents)?;
    file.sync_all()?;
    fs::rename(temporary, path)?;
    Ok(())
}

impl Settings {
    pub fn load() -> Result<Self> {
        let path = config_dir().join("settings.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let settings: Self = serde_json::from_slice(&fs::read(path)?)
            .context("Invalid settings.json; the original file has been preserved")?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn save(&self) -> Result<()> {
        self.validate()?;
        private_write(
            &config_dir().join("settings.json"),
            &serde_json::to_vec_pretty(self)?,
        )
    }

    pub fn validate(&self) -> Result<()> {
        validate_banner(&self.banner_message)?;
        if self.clocks.len() > 32 || self.forex.len() > 100 {
            bail!("At most 32 clocks and 100 forex pairs are supported");
        }
        for clock in &self.clocks {
            validate_clock(&clock.label, &clock.timezone)?;
        }
        for pair in &self.forex {
            normalize_pair(pair)?;
        }
        Ok(())
    }

    pub fn update_banner(&mut self, message: &str) -> Result<()> {
        let message = message.trim();
        validate_banner(message)?;
        self.banner_message = message.into();
        Ok(())
    }

    pub fn update_clock(&mut self, index: usize, label: &str, timezone: &str) -> Result<()> {
        let label = label.trim();
        let timezone = timezone.trim();
        validate_clock(label, timezone)?;
        let clock = self
            .clocks
            .get_mut(index)
            .context("This clock no longer exists")?;
        *clock = Clock {
            label: label.into(),
            timezone: timezone.into(),
        };
        Ok(())
    }
}

fn validate_banner(message: &str) -> Result<()> {
    if message.trim().is_empty() || message.chars().count() > MAX_BANNER_CHARACTERS {
        bail!("Use a banner message of 1–{MAX_BANNER_CHARACTERS} characters");
    }
    if message
        .chars()
        .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
    {
        bail!("Keep the banner on a single line without tabs or control characters");
    }
    Ok(())
}

pub fn validate_clock(label: &str, timezone: &str) -> Result<()> {
    if label.trim().is_empty() || label.chars().count() > 30 {
        bail!("Use a clock label of 1–30 characters");
    }
    timezone
        .parse::<chrono_tz::Tz>()
        .context("Use an IANA time zone, for example Europe/London or America/New_York")?;
    Ok(())
}

pub fn normalize_pair(input: &str) -> Result<String> {
    let pair = input.trim().replace(['/', ' '], "").to_ascii_uppercase();
    if pair.len() != 6 || !pair.bytes().all(|c| c.is_ascii_uppercase()) {
        bail!("Enter two three-letter currencies, for example EUR/USD or USDCNH");
    }
    if pair[..3] == pair[3..] {
        bail!("Base and quote currencies must differ");
    }
    Ok(pair)
}

// Defaults and field names match cosmic-applets-config 1.11 (System76, GPL-3.0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelClock {
    pub military_time: bool,
    pub show_seconds: bool,
    pub first_day_of_week: u8,
    pub show_date_in_top_panel: bool,
    pub show_weekday: bool,
    pub format_strftime: String,
}

impl PanelClock {
    pub fn load() -> Self {
        let config = cosmic::cosmic_config::Config::new("com.system76.CosmicAppletTime", 1).ok();
        Self {
            military_time: config
                .as_ref()
                .and_then(|c| c.get("military_time").ok())
                .unwrap_or(false),
            show_seconds: config
                .as_ref()
                .and_then(|c| c.get("show_seconds").ok())
                .unwrap_or(false),
            first_day_of_week: config
                .as_ref()
                .and_then(|c| c.get::<u8>("first_day_of_week").ok())
                .unwrap_or(6)
                .min(6),
            show_date_in_top_panel: config
                .as_ref()
                .and_then(|c| c.get("show_date_in_top_panel").ok())
                .unwrap_or(true),
            show_weekday: config
                .as_ref()
                .and_then(|c| c.get("show_weekday").ok())
                .unwrap_or(false),
            format_strftime: config
                .as_ref()
                .and_then(|c| c.get("format_strftime").ok())
                .unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_settings_gain_banner_without_replacing_saved_preferences() {
        let mut legacy = serde_json::to_value(Settings::default()).unwrap();
        legacy.as_object_mut().unwrap().remove("banner_message");
        legacy["clocks"][0] = serde_json::json!({"label": "Beijing", "timezone": "Asia/Shanghai"});
        legacy["clocks"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"label": "Custom", "timezone": "Pacific/Auckland"}));
        legacy["forex"] = serde_json::json!(["EURUSD", "USDCNH"]);
        legacy["google_client_path"] = serde_json::json!("/example/google-client.json");
        legacy["google_email"] = serde_json::json!("test@example.invalid");
        legacy["disabled_calendars"] = serde_json::json!(["example-calendar"]);
        let settings: Settings = serde_json::from_value(legacy.clone()).unwrap();
        settings.validate().unwrap();
        legacy["banner_message"] = serde_json::json!(DEFAULT_BANNER_MESSAGE);
        assert_eq!(serde_json::to_value(settings).unwrap(), legacy);
    }

    #[test]
    fn banner_edits_round_trip_without_changing_other_preferences() {
        let mut settings = Settings::default();
        let mut expected = serde_json::to_value(&settings).unwrap();
        let message = "新的一天 · Keep growing 🌱";
        settings.update_banner(&format!("  {message}  ")).unwrap();
        expected["banner_message"] = serde_json::json!(message);
        let restored: Settings =
            serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
        restored.validate().unwrap();
        assert_eq!(serde_json::to_value(restored).unwrap(), expected);

        settings
            .update_banner(&"新".repeat(MAX_BANNER_CHARACTERS))
            .unwrap();
        let original = serde_json::to_value(&settings).unwrap();
        for invalid in [
            String::from("   "),
            String::from("First\nSecond"),
            String::from("First\tSecond"),
            String::from("First\u{2028}Second"),
            "新".repeat(MAX_BANNER_CHARACTERS + 1),
        ] {
            assert!(settings.update_banner(&invalid).is_err());
            assert_eq!(serde_json::to_value(&settings).unwrap(), original);
        }
    }

    #[test]
    fn editing_clock_replaces_only_that_clock_in_its_saved_position() {
        let mut settings = Settings::default();
        settings.clocks.push(Clock {
            label: "Custom".into(),
            timezone: "Pacific/Auckland".into(),
        });
        let mut expected = serde_json::to_value(&settings).unwrap();
        settings
            .update_clock(1, " London ", " Europe/London ")
            .unwrap();
        expected["clocks"][1] = serde_json::json!({"label": "London", "timezone": "Europe/London"});
        assert_eq!(serde_json::to_value(&settings).unwrap(), expected);
    }

    #[test]
    fn invalid_clock_edits_leave_all_settings_intact() {
        let mut settings = Settings::default();
        let original = serde_json::to_value(&settings).unwrap();
        for (index, label, timezone) in [
            (1, " ", "Etc/UTC"),
            (1, "A clock label exceeding thirty characters", "Etc/UTC"),
            (1, "London", "Not/A_Timezone"),
            (usize::MAX, "London", "Europe/London"),
        ] {
            assert!(settings.update_clock(index, label, timezone).is_err());
            assert_eq!(serde_json::to_value(&settings).unwrap(), original);
        }
    }

    #[test]
    fn currency_normalization_keeps_onshore_and_offshore_distinct() {
        assert_eq!(normalize_pair(" usd/cnh ").unwrap(), "USDCNH");
        assert_ne!(
            normalize_pair("USDCNY").unwrap(),
            normalize_pair("USDCNH").unwrap()
        );
        for invalid in ["USDUSD", "EUR", "éééJPY", "USD<script>"] {
            assert!(normalize_pair(invalid).is_err());
        }
    }

    #[test]
    fn previous_config_directory_moves_with_its_google_client_path() {
        let root = std::env::temp_dir().join(format!(
            "finance-dashboard-config-{}",
            rand::random::<u64>()
        ));
        let previous = root.join(PREVIOUS_CONFIG_DIRECTORY);
        let current = root.join(CONFIG_DIRECTORY);
        fs::create_dir_all(&previous).unwrap();
        let client = previous.join("google-client.json");
        fs::write(&client, b"{}").unwrap();
        fs::write(
            previous.join("settings.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "google_client_path": client.display().to_string()
            }))
            .unwrap(),
        )
        .unwrap();
        adopt_previous_config(&previous, &current);
        assert!(!previous.exists());
        assert!(current.join("google-client.json").is_file());
        let saved: serde_json::Value =
            serde_json::from_slice(&fs::read(current.join("settings.json")).unwrap()).unwrap();
        assert_eq!(
            saved["google_client_path"],
            current.join("google-client.json").display().to_string()
        );

        fs::create_dir_all(&previous).unwrap();
        fs::write(
            previous.join("settings.json"),
            b"{\"banner_message\":\"later\"}",
        )
        .unwrap();
        adopt_previous_config(&previous, &current);
        assert!(previous.join("settings.json").is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn populated_config_directory_adopts_previous_files_without_replacing_backups() {
        let root = std::env::temp_dir().join(format!(
            "finance-dashboard-config-{}",
            rand::random::<u64>()
        ));
        let previous = root.join(PREVIOUS_CONFIG_DIRECTORY);
        let current = root.join(CONFIG_DIRECTORY);
        fs::create_dir_all(&previous).unwrap();
        fs::create_dir_all(current.join("panel-backups")).unwrap();
        fs::write(current.join("panel-backups").join("keep.ron"), b"backup").unwrap();
        let client = previous.join("google-client.json");
        fs::write(&client, b"{}").unwrap();
        fs::write(
            previous.join("settings.json"),
            serde_json::to_vec(&serde_json::json!({
                "google_client_path": "/keep/original-client.json"
            }))
            .unwrap(),
        )
        .unwrap();
        adopt_previous_config(&previous, &current);
        assert!(current.join("panel-backups").join("keep.ron").is_file());
        assert!(current.join("settings.json").is_file());
        assert!(current.join("google-client.json").is_file());
        assert!(!previous.join("settings.json").exists());
        let saved: serde_json::Value =
            serde_json::from_slice(&fs::read(current.join("settings.json")).unwrap()).unwrap();
        assert_eq!(saved["google_client_path"], "/keep/original-client.json");
        fs::remove_dir_all(root).unwrap();
    }
}
