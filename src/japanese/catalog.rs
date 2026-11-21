// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Library {
    Elementary,
    JuniorHigh,
    SeniorHigh,
    University,
}

impl Library {
    pub const ALL: [Self; 4] = [
        Self::Elementary,
        Self::JuniorHigh,
        Self::SeniorHigh,
        Self::University,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::Elementary => "elementary",
            Self::JuniorHigh => "junior-high",
            Self::SeniorHigh => "senior-high",
            Self::University => "university",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Elementary => "Elementary",
            Self::JuniorHigh => "Junior high",
            Self::SeniorHigh => "Senior high",
            Self::University => "University",
        }
    }

    pub fn jlpt(self) -> &'static str {
        match self {
            Self::Elementary => "N5",
            Self::JuniorHigh => "N4",
            Self::SeniorHigh => "N3",
            Self::University => "N2 and N1",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|library| library.key() == text)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Entry {
    pub id: String,
    pub word: String,
    pub reading: String,
    pub gloss: String,
    pub jlpt: String,
}

pub struct Catalog {
    entries: BTreeMap<Library, Vec<Entry>>,
    ids: BTreeMap<Library, Vec<String>>,
    by_id: BTreeMap<String, Entry>,
}

impl Catalog {
    pub fn library(&self, library: Library) -> &[Entry] {
        self.entries.get(&library).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn ids(&self, library: Library) -> &[String] {
        self.ids.get(&library).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn entry(&self, id: &str) -> Option<&Entry> {
        self.by_id.get(id)
    }
}

pub fn catalog() -> &'static Catalog {
    static CATALOG: LazyLock<Catalog> = LazyLock::new(load);
    &CATALOG
}

fn load() -> Catalog {
    let file: File = serde_json::from_str(include_str!("../../data/japanese/words.json"))
        .expect("bundled Japanese word library");
    let mut entries = BTreeMap::new();
    let mut ids = BTreeMap::new();
    let mut by_id = BTreeMap::new();
    for library in Library::ALL {
        let words = file
            .libraries
            .get(library.key())
            .cloned()
            .unwrap_or_default();
        let library_ids = words.iter().map(|entry| entry.id.clone()).collect();
        for entry in &words {
            by_id.insert(entry.id.clone(), entry.clone());
        }
        entries.insert(library, words);
        ids.insert(library, library_ids);
    }
    Catalog {
        entries,
        ids,
        by_id,
    }
}

#[derive(Deserialize)]
struct File {
    libraries: BTreeMap<String, Vec<Entry>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn libraries_are_nonempty_and_do_not_share_words() {
        let catalog = catalog();
        let mut ids = BTreeSet::new();
        let mut pairs = BTreeSet::new();
        for library in Library::ALL {
            let words = catalog.library(library);
            assert!(words.len() > 100, "{}", library.label());
            assert_eq!(words.len(), catalog.ids(library).len());
            for entry in words {
                assert!(ids.insert(entry.id.clone()), "duplicate id {}", entry.id);
                assert!(
                    pairs.insert((entry.word.clone(), entry.reading.clone())),
                    "duplicate pair {} {}",
                    entry.word,
                    entry.reading
                );
                assert!(catalog.entry(&entry.id).is_some());
            }
        }
    }

    #[test]
    fn elementary_includes_neko() {
        assert!(
            catalog()
                .library(Library::Elementary)
                .iter()
                .any(|entry| entry.word == "猫" && entry.reading == "ねこ")
        );
    }
}
