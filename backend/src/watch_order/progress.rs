//! Progress per entry, from history rows: an entry is watched once a row
//! reaches its last known episode, part-way otherwise. Each entry is
//! handed the row it resumes from, under its id; a row with no Kitsu id
//! cannot be placed and is left out, and several rows for one entry
//! count the furthest.

use std::collections::BTreeMap;

use super::{EntryProgress, HistoryRow, Progress, WatchEntry};

/// The episode a history row names, read by the episode it starts with:
/// a recap's "7.5" is 7.5, and a range-shaped row ("1-12") is its first
/// episode, as the history resolver reads it. None when the row names no
/// positive episode.
#[must_use]
pub fn history_episode(raw: &str) -> Option<f64> {
    let rest = raw.trim_start();
    let whole = rest.bytes().take_while(u8::is_ascii_digit).count();
    if whole == 0 {
        return None;
    }
    let mut end = whole;
    let tail = &rest.as_bytes()[whole..];
    if tail.first() == Some(&b'.') {
        let frac = tail[1..].iter().take_while(|b| b.is_ascii_digit()).count();
        if frac > 0 {
            end += 1 + frac;
        }
    }
    let n: f64 = rest[..end].parse().ok()?;
    (n > 0.0).then_some(n)
}

/// Progress for each of `entries` that `rows` name.
#[must_use]
pub fn progress_from_history(rows: &[HistoryRow], entries: &[WatchEntry]) -> Progress {
    let mut furthest: BTreeMap<&str, f64> = BTreeMap::new();
    for row in rows {
        let (Some(id), Some(ep)) = (&row.kitsu_id, history_episode(&row.ep_no)) else {
            continue;
        };
        let best = furthest.entry(id).or_insert(0.0);
        *best = best.max(ep);
    }
    let mut out = Progress::new();
    for e in entries {
        let Some(&ep) = furthest.get(e.id.as_str()) else {
            continue;
        };
        let done = e.episodes.is_some_and(|n| ep >= f64::from(n));
        out.insert(
            e.id.clone(),
            if done {
                EntryProgress::Watched
            } else {
                EntryProgress::Episode(ep)
            },
        );
    }
    out
}
