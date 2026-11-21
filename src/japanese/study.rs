// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Per-library study order. The upcoming queue is the random order. The
//! viewed library changes once per local day. Thirty minutes of active
//! session time marks the replaced word learned; less than that puts it
//! back in the queue. Next marks the current word learned immediately.

use rand::Rng;
use rand::prelude::SliceRandom;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const RECENT_LIMIT: usize = 10;
pub const PREFETCH_COUNT: usize = 10;
pub const LEARN_LIMIT_MS: u64 = 30 * 60 * 1000;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cycle {
    pub deck: Deck,
    #[serde(default)]
    pub active_ms: u64,
    #[serde(default)]
    pub issued_day: Option<i32>,
}

/// Adds one second while the session is active and a word is showing.
pub fn accrue(cycle: &mut Cycle, session_active: bool) {
    if session_active && cycle.deck.current.is_some() {
        cycle.active_ms = cycle.active_ms.saturating_add(1_000);
    }
}

/// Stamps a newly viewed word, or takes one daily step when `today` is later.
/// Returns whether the cycle changed. The same `today` does not step again.
pub fn view_today(cycle: &mut Cycle, ids: &[String], today: i32, rng: &mut impl Rng) -> bool {
    if cycle.deck.current.is_none() && !is_complete(&cycle.deck, ids) {
        ensure(&mut cycle.deck, ids, rng);
    }
    if is_complete(&cycle.deck, ids) || cycle.deck.current.is_none() {
        return false;
    }
    match cycle.issued_day {
        Some(day) if day >= today => false,
        Some(_) => {
            let learned = cycle.active_ms >= LEARN_LIMIT_MS;
            advance(&mut cycle.deck, ids, learned, rng);
            cycle.active_ms = 0;
            cycle.issued_day = Some(today);
            true
        }
        None => {
            cycle.issued_day = Some(today);
            cycle.active_ms = 0;
            true
        }
    }
}

/// Marks the current word learned and shows the next one, starting its day now.
pub fn pass_today(cycle: &mut Cycle, ids: &[String], today: i32, rng: &mut impl Rng) {
    if is_complete(&cycle.deck, ids) || cycle.deck.current.is_none() {
        return;
    }
    advance(&mut cycle.deck, ids, true, rng);
    cycle.active_ms = 0;
    cycle.issued_day = Some(today);
}

pub fn reset_today(cycle: &mut Cycle, ids: &[String], today: i32, rng: &mut impl Rng) {
    reset(&mut cycle.deck, ids, rng);
    cycle.active_ms = 0;
    cycle.issued_day = Some(today);
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deck {
    pub current: Option<String>,
    #[serde(default)]
    pub recent: Vec<String>,
    #[serde(default)]
    pub upcoming: Vec<String>,
    #[serde(default)]
    pub learned: BTreeSet<String>,
}

pub fn ensure(deck: &mut Deck, ids: &[String], rng: &mut impl Rng) {
    sanitize(deck, ids);
    if deck.current.is_none() {
        if let Some(next) = pop_valid(deck, ids) {
            deck.current = Some(next);
        } else {
            rebuild_upcoming(deck, ids, rng);
            deck.current = pop_valid(deck, ids);
        }
    }
    finish(deck, ids, rng);
}

pub fn advance(deck: &mut Deck, ids: &[String], pass: bool, rng: &mut impl Rng) {
    sanitize(deck, ids);
    if let Some(current) = deck.current.take() {
        if pass {
            deck.learned.insert(current);
        } else if ids.iter().any(|id| id == &current) {
            deck.recent.push(current);
            trim_recent(deck);
        }
    }
    if let Some(next) = pop_valid(deck, ids) {
        deck.current = Some(next);
    } else {
        rebuild_upcoming(deck, ids, rng);
        deck.current = pop_valid(deck, ids);
    }
    finish(deck, ids, rng);
}

pub fn reset(deck: &mut Deck, ids: &[String], rng: &mut impl Rng) {
    *deck = Deck::default();
    ensure(deck, ids, rng);
}

pub fn prefetch_ids(deck: &Deck) -> Vec<&str> {
    let mut ids = Vec::new();
    if let Some(current) = deck.current.as_deref() {
        ids.push(current);
    }
    ids.extend(
        deck.upcoming
            .iter()
            .take(PREFETCH_COUNT)
            .map(String::as_str),
    );
    ids
}

pub fn is_complete(deck: &Deck, ids: &[String]) -> bool {
    !ids.is_empty() && ids.iter().all(|id| deck.learned.contains(id))
}

fn finish(deck: &mut Deck, ids: &[String], rng: &mut impl Rng) {
    if let Some(current) = deck.current.clone() {
        deck.recent.retain(|id| id != &current);
    }
    retain_upcoming(deck, ids);
    fill_missing(deck, ids, rng);
}

fn sanitize(deck: &mut Deck, ids: &[String]) {
    let known: BTreeSet<&str> = ids.iter().map(String::as_str).collect();
    deck.learned.retain(|id| known.contains(id.as_str()));
    deck.recent
        .retain(|id| known.contains(id.as_str()) && !deck.learned.contains(id));
    dedupe(&mut deck.recent);
    trim_recent(deck);
    if deck
        .current
        .as_ref()
        .is_some_and(|id| !known.contains(id.as_str()) || deck.learned.contains(id))
    {
        deck.current = None;
    }
    deck.upcoming
        .retain(|id| known.contains(id.as_str()) && !deck.learned.contains(id));
}

fn trim_recent(deck: &mut Deck) {
    if deck.recent.len() > RECENT_LIMIT {
        let overflow = deck.recent.len() - RECENT_LIMIT;
        deck.recent.drain(0..overflow);
    }
}

fn dedupe(ids: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    ids.retain(|id| seen.insert(id.clone()));
}

fn pop_valid(deck: &mut Deck, ids: &[String]) -> Option<String> {
    let allowed: BTreeSet<String> = candidate_pool(ids, &deck.learned, None, &deck.recent)
        .into_iter()
        .collect();
    while !deck.upcoming.is_empty() {
        let id = deck.upcoming.remove(0);
        if allowed.contains(&id) {
            return Some(id);
        }
    }
    None
}

fn rebuild_upcoming(deck: &mut Deck, ids: &[String], rng: &mut impl Rng) {
    let mut pool = candidate_pool(ids, &deck.learned, deck.current.as_deref(), &deck.recent);
    pool.shuffle(rng);
    deck.upcoming = pool;
}

fn retain_upcoming(deck: &mut Deck, ids: &[String]) {
    let allowed: BTreeSet<String> =
        candidate_pool(ids, &deck.learned, deck.current.as_deref(), &deck.recent)
            .into_iter()
            .collect();
    let mut seen = BTreeSet::new();
    deck.upcoming
        .retain(|id| allowed.contains(id) && seen.insert(id.clone()));
}

fn fill_missing(deck: &mut Deck, ids: &[String], rng: &mut impl Rng) {
    let present: BTreeSet<&str> = deck.upcoming.iter().map(String::as_str).collect();
    let mut missing: Vec<String> =
        candidate_pool(ids, &deck.learned, deck.current.as_deref(), &deck.recent)
            .into_iter()
            .filter(|id| !present.contains(id.as_str()))
            .collect();
    missing.shuffle(rng);
    let floor = deck.upcoming.len().min(PREFETCH_COUNT);
    for id in missing {
        let index = if deck.upcoming.len() <= floor {
            deck.upcoming.len()
        } else {
            rng.random_range(floor..=deck.upcoming.len())
        };
        deck.upcoming.insert(index, id);
    }
}

/// Unlearned words except the current one and as much of the recent window as
/// still leaves a candidate. With fewer than eleven words left, the window shrinks.
fn candidate_pool(
    ids: &[String],
    learned: &BTreeSet<String>,
    current: Option<&str>,
    recent: &[String],
) -> Vec<String> {
    let unlearned: Vec<String> = ids
        .iter()
        .filter(|id| !learned.contains(*id) && Some(id.as_str()) != current)
        .cloned()
        .collect();
    if unlearned.is_empty() {
        return Vec::new();
    }
    let mut ban = recent.len().min(RECENT_LIMIT);
    loop {
        let banned: BTreeSet<&str> = recent[recent.len() - ban..]
            .iter()
            .map(String::as_str)
            .collect();
        let pool: Vec<String> = unlearned
            .iter()
            .filter(|id| !banned.contains(id.as_str()))
            .cloned()
            .collect();
        if !pool.is_empty() || ban == 0 {
            return pool;
        }
        ban -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn ids(count: usize) -> Vec<String> {
        (1..=count).map(|number| number.to_string()).collect()
    }

    fn rng() -> StdRng {
        StdRng::seed_from_u64(7)
    }

    #[test]
    fn the_next_ten_shown_words_skip_the_word_just_left() {
        let ids = ids(30);
        let mut deck = Deck::default();
        let mut rng = rng();
        ensure(&mut deck, &ids, &mut rng);
        let first = deck.current.clone().unwrap();
        for _ in 0..10 {
            advance(&mut deck, &ids, false, &mut rng);
            assert_ne!(deck.current.as_deref(), Some(first.as_str()));
        }
        assert!(deck.recent.iter().any(|id| id == &first) || deck.current.as_ref() == Some(&first));
    }

    #[test]
    fn pass_hides_a_word_until_reset() {
        let ids = ids(12);
        let mut deck = Deck::default();
        let mut rng = rng();
        ensure(&mut deck, &ids, &mut rng);
        let passed = deck.current.clone().unwrap();
        advance(&mut deck, &ids, true, &mut rng);
        assert!(deck.learned.contains(&passed));
        assert_ne!(deck.current.as_deref(), Some(passed.as_str()));
        for _ in 0..20 {
            advance(&mut deck, &ids, false, &mut rng);
            assert_ne!(deck.current.as_deref(), Some(passed.as_str()));
        }
        reset(&mut deck, &ids, &mut rng);
        assert!(deck.learned.is_empty());
        assert!(deck.current.is_some());
    }

    #[test]
    fn a_small_pool_still_has_a_next_word() {
        let ids = ids(5);
        let mut deck = Deck::default();
        let mut rng = rng();
        ensure(&mut deck, &ids, &mut rng);
        for _ in 0..20 {
            advance(&mut deck, &ids, false, &mut rng);
            assert!(deck.current.is_some());
        }
    }

    #[test]
    fn the_only_remaining_word_can_repeat_and_passing_it_completes_the_library() {
        let ids = ids(1);
        let mut deck = Deck::default();
        let mut rng = rng();
        ensure(&mut deck, &ids, &mut rng);
        let only = deck.current.clone().unwrap();
        advance(&mut deck, &ids, false, &mut rng);
        assert_eq!(deck.current.as_deref(), Some(only.as_str()));
        advance(&mut deck, &ids, true, &mut rng);
        assert!(is_complete(&deck, &ids));
        assert!(deck.current.is_none());
    }

    #[test]
    fn a_second_ensure_keeps_the_current_word_and_the_queued_front() {
        let ids = ids(20);
        let mut deck = Deck::default();
        let mut rng = rng();
        ensure(&mut deck, &ids, &mut rng);
        let current = deck.current.clone();
        let front = deck.upcoming.first().cloned();
        ensure(&mut deck, &ids, &mut rng);
        assert_eq!(deck.current, current);
        assert_eq!(deck.upcoming.first().cloned(), front);
    }

    #[test]
    fn idle_time_does_not_count_and_the_same_day_keeps_the_word() {
        let ids = ids(12);
        let mut cycle = Cycle::default();
        let mut rng = rng();
        assert!(view_today(&mut cycle, &ids, 10, &mut rng));
        let word = cycle.deck.current.clone();
        accrue(&mut cycle, false);
        assert_eq!(cycle.active_ms, 0);
        accrue(&mut cycle, true);
        assert_eq!(cycle.active_ms, 1_000);
        let queued = cycle.deck.upcoming.clone();
        assert!(!view_today(&mut cycle, &ids, 10, &mut rng));
        assert_eq!(cycle.deck.current, word);
        assert_eq!(cycle.active_ms, 1_000);
        assert_eq!(cycle.deck.upcoming, queued);
    }

    #[test]
    fn a_short_day_replaces_the_word_once_and_leaves_it_unlearned() {
        let ids = ids(12);
        let mut cycle = Cycle::default();
        let mut rng = rng();
        view_today(&mut cycle, &ids, 3, &mut rng);
        let word = cycle.deck.current.clone().unwrap();
        cycle.active_ms = LEARN_LIMIT_MS - 1;
        assert!(view_today(&mut cycle, &ids, 9, &mut rng));
        assert_ne!(cycle.deck.current.as_deref(), Some(word.as_str()));
        assert!(!cycle.deck.learned.contains(&word));
        assert_eq!(cycle.active_ms, 0);
        assert_eq!(cycle.issued_day, Some(9));
        let shown = cycle.deck.current.clone();
        assert!(!view_today(&mut cycle, &ids, 9, &mut rng));
        assert_eq!(cycle.deck.current, shown);
    }

    #[test]
    fn thirty_minutes_marks_the_replaced_word_learned() {
        let ids = ids(12);
        let mut cycle = Cycle::default();
        let mut rng = rng();
        view_today(&mut cycle, &ids, 3, &mut rng);
        let word = cycle.deck.current.clone().unwrap();
        cycle.active_ms = LEARN_LIMIT_MS;
        assert!(view_today(&mut cycle, &ids, 4, &mut rng));
        assert!(cycle.deck.learned.contains(&word));
        assert_ne!(cycle.deck.current.as_deref(), Some(word.as_str()));
        assert_eq!(cycle.active_ms, 0);
    }

    #[test]
    fn next_marks_the_word_learned_before_thirty_minutes() {
        let ids = ids(8);
        let mut cycle = Cycle::default();
        let mut rng = rng();
        view_today(&mut cycle, &ids, 1, &mut rng);
        let word = cycle.deck.current.clone().unwrap();
        cycle.active_ms = 5_000;
        pass_today(&mut cycle, &ids, 1, &mut rng);
        assert!(cycle.deck.learned.contains(&word));
        assert_ne!(cycle.deck.current.as_deref(), Some(word.as_str()));
        assert_eq!(cycle.active_ms, 0);
        assert_eq!(cycle.issued_day, Some(1));
    }

    #[test]
    fn a_finished_library_does_not_rotate() {
        let ids = ids(1);
        let mut cycle = Cycle::default();
        let mut rng = rng();
        view_today(&mut cycle, &ids, 1, &mut rng);
        pass_today(&mut cycle, &ids, 1, &mut rng);
        assert!(is_complete(&cycle.deck, &ids));
        cycle.active_ms = LEARN_LIMIT_MS;
        assert!(!view_today(&mut cycle, &ids, 5, &mut rng));
        assert!(cycle.deck.current.is_none());
        pass_today(&mut cycle, &ids, 5, &mut rng);
        assert!(cycle.deck.current.is_none());
    }
}
