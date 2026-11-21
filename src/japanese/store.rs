// SPDX-License-Identifier: GPL-3.0-only

//! Finance Dashboard's own Japanese progress. This file is not read from the
//! Japanese Words applet, and that applet's progress is not imported.

use crate::config;
use crate::japanese::catalog::Library;
use crate::japanese::details::Detail;
use crate::japanese::study::Cycle;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

const VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StudyFile {
    pub version: u32,
    pub library: String,
    #[serde(default)]
    pub cycles: BTreeMap<String, Cycle>,
}

impl Default for StudyFile {
    fn default() -> Self {
        Self {
            version: VERSION,
            library: Library::Elementary.key().to_owned(),
            cycles: BTreeMap::new(),
        }
    }
}

impl StudyFile {
    pub fn from_cycles(library: Library, cycles: &BTreeMap<Library, Cycle>) -> Self {
        Self {
            version: VERSION,
            library: library.key().to_owned(),
            cycles: cycles
                .iter()
                .map(|(library, cycle)| (library.key().to_owned(), cycle.clone()))
                .collect(),
        }
    }

    pub fn library(&self) -> Library {
        Library::parse(&self.library).unwrap_or(Library::Elementary)
    }

    pub fn decoded_cycles(&self) -> BTreeMap<Library, Cycle> {
        self.cycles
            .iter()
            .filter_map(|(key, cycle)| Library::parse(key).map(|library| (library, cycle.clone())))
            .collect()
    }
}

pub fn study_path() -> PathBuf {
    data_dir().join("japanese-study.json")
}

pub fn cache_path() -> PathBuf {
    data_dir().join("japanese-details.json")
}

fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(config::CONFIG_DIRECTORY)
}

pub fn load_cache() -> BTreeMap<String, Detail> {
    let path = cache_path();
    let Ok(text) = fs::read_to_string(&path) else {
        return BTreeMap::new();
    };
    match serde_json::from_str::<CacheFile>(&text) {
        Ok(file) => file.entries,
        Err(_) => {
            quarantine(&path);
            BTreeMap::new()
        }
    }
}

pub fn save_cache(entries: &BTreeMap<String, Detail>) -> Result<(), String> {
    let file = CacheFile {
        version: VERSION,
        entries: entries.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&file).map_err(|error| error.to_string())?;
    config::private_write(&cache_path(), &bytes).map_err(|error| format!("{error:#}"))
}

#[derive(Serialize, Deserialize)]
struct CacheFile {
    version: u32,
    entries: BTreeMap<String, Detail>,
}

pub fn load() -> Result<StudyFile, String> {
    load_from(&study_path())
}

pub fn save(file: &StudyFile) -> Result<(), String> {
    save_to(&study_path(), file)
}

pub fn load_from(path: &Path) -> Result<StudyFile, String> {
    if !path.is_file() {
        return Ok(StudyFile::default());
    }
    let text = fs::read_to_string(path)
        .map_err(|error| format!("Cannot read {}: {error}", path.display()))?;
    match serde_json::from_str::<StudyFile>(&text) {
        Ok(file) => Ok(file),
        Err(error) => {
            quarantine(path);
            Err(format!(
                "Japanese study progress was unreadable and was set aside ({error})"
            ))
        }
    }
}

pub fn save_to(path: &Path, file: &StudyFile) -> Result<(), String> {
    let mut stored = file.clone();
    stored.version = VERSION;
    let bytes = serde_json::to_vec_pretty(&stored).map_err(|error| error.to_string())?;
    config::private_write(path, &bytes).map_err(|error| format!("{error:#}"))
}

fn quarantine(path: &Path) {
    if path.is_file() {
        let _ = fs::rename(path, path.with_extension("json.bad"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::japanese::study::Deck;

    #[test]
    fn progress_round_trips_without_reading_another_applet() {
        let root =
            std::env::temp_dir().join(format!("finance-japanese-store-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let path = root.join("japanese-study.json");
        let mut cycles = BTreeMap::new();
        cycles.insert(
            Library::JuniorHigh,
            Cycle {
                deck: Deck {
                    current: Some("42".into()),
                    ..Deck::default()
                },
                active_ms: 1_500,
                issued_day: Some(20),
            },
        );
        let file = StudyFile::from_cycles(Library::JuniorHigh, &cycles);
        save_to(&path, &file).unwrap();
        let loaded = load_from(&path).unwrap();
        assert_eq!(loaded.library(), Library::JuniorHigh);
        assert_eq!(
            loaded.decoded_cycles()[&Library::JuniorHigh]
                .deck
                .current
                .as_deref(),
            Some("42")
        );
        assert_eq!(
            loaded.decoded_cycles()[&Library::JuniorHigh].active_ms,
            1_500
        );
        fs::write(&path, b"{not json").unwrap();
        assert!(load_from(&path).is_err());
        assert!(path.with_extension("json.bad").is_file());
        assert!(!path.exists());
        let _ = fs::remove_dir_all(root);
    }
}
