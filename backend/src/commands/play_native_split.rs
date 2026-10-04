//! One anime-database entry the provider lists as several shows.
//!
//! Kitsu can keep a show as one entry where the provider splits it:
//! Steel Ball Run is twelve episodes on Kitsu, while hianime lists a
//! one-episode "Steel Ball Run: JoJo no Kimyou na Bouken" (the March
//! premiere) and a "... 2nd Stage" whose episode 1 is Kitsu's episode
//! 2. Picking either alone resolves every episode against the wrong
//! numbering. The parts are recognised from what the pick already
//! fetched — titles, years and listings — and stitched into one
//! listing in the entry's own numbering, so nothing downstream of the
//! pick has to know there were several.

use crate::scraper::provider::EpisodeRef;

/// What the chain detection reads of one probed candidate.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PartCandidate<'a> {
    /// The provider's display title.
    pub title: &'a str,
    /// Regular (integer) episodes listed.
    pub count: u32,
    /// The candidate's own premiere year positively matched Kitsu's.
    pub confirmed: bool,
}

/// Words that name a part without numbering it.
const MARKERS: &[&str] = &["part", "stage", "season", "cour"];

fn normalized(title: &str) -> String {
    title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The ordinal one word spells, if it spells one.
fn word_ordinal(word: &str) -> Option<u32> {
    const SPELLED: &[(&str, u32)] = &[
        ("first", 1),
        ("second", 2),
        ("third", 3),
        ("fourth", 4),
        ("fifth", 5),
        ("ii", 2),
        ("iii", 3),
        ("iv", 4),
        ("v", 5),
    ];
    if let Some((_, n)) = SPELLED.iter().find(|(w, _)| *w == word) {
        return Some(*n);
    }
    let digits = ["st", "nd", "rd", "th"]
        .iter()
        .find_map(|suffix| word.strip_suffix(suffix))
        .unwrap_or(word);
    digits.parse().ok()
}

/// The ordinal a short part marker names ("2nd stage", "part 3"):
/// every word a marker or an ordinal, exactly one ordinal, and a bare
/// number only beside a marker.
fn marker_ordinal(rest: &str) -> Option<u32> {
    let words: Vec<&str> = rest.split_whitespace().collect();
    if words.is_empty() || words.len() > 3 {
        return None;
    }
    let has_marker = words.iter().any(|w| MARKERS.contains(w));
    let mut ordinal = None;
    for w in &words {
        if MARKERS.contains(w) {
            continue;
        }
        let bare = w.chars().all(|c| c.is_ascii_digit());
        let n = word_ordinal(w).filter(|_| has_marker || !bare)?;
        if ordinal.replace(n).is_some() {
            return None;
        }
    }
    ordinal
}

/// Which part of `stem` a title names: 1 for the stem itself, `k` for
/// the stem followed by a short `k`-th part marker ("2nd Stage",
/// "Part 3", "Season 2"), `None` for anything else.
#[must_use]
pub(crate) fn part_ordinal(stem: &str, title: &str) -> Option<u32> {
    let (stem, title) = (normalized(stem), normalized(title));
    if title == stem {
        return Some(1);
    }
    let rest = title.strip_prefix(&stem)?;
    // "The Showdown" is not a part of "The Show".
    if rest.chars().next().is_some_and(char::is_alphanumeric) {
        return None;
    }
    let rest = rest.trim_start_matches(|c: char| !c.is_alphanumeric());
    marker_ordinal(rest).filter(|k| *k >= 2)
}

/// The parts that follow candidate `lead`, in order, while each next
/// ordinal is present: `[lead, part 2, part 3, ...]`.
fn chain_from(cands: &[PartCandidate<'_>], lead: usize) -> Vec<usize> {
    let usable = |c: &PartCandidate<'_>| c.confirmed && c.count > 0;
    let mut chain = vec![lead];
    for k in 2.. {
        let next = cands.iter().enumerate().position(|(j, c)| {
            j != lead && usable(c) && part_ordinal(cands[lead].title, c.title) == Some(k)
        });
        match next {
            Some(j) => chain.push(j),
            None => break,
        }
    }
    chain
}

/// The candidates, in part order, that together make up the expected
/// entry — or `None` when no such chain explains the expected count
/// better than the best single candidate (`best_single` is its
/// distance).
///
/// Every part's own premiere year has to match Kitsu's: the parts of
/// one entry air close together, and that is what separates them from
/// a sequel season the provider names the same way. The chain is
/// accepted where it fits the expected count within the picker's
/// tolerance, or falls short of it — an airing entry, which has not
/// aired everything Kitsu counts.
#[must_use]
pub(crate) fn split_chain(
    cands: &[PartCandidate<'_>],
    expected: u32,
    best_single: u32,
) -> Option<Vec<usize>> {
    let tolerance = super::play_native::ep_count_threshold(expected);
    let mut best: Option<(u32, Vec<usize>)> = None;
    for lead in (0..cands.len()).filter(|&i| cands[i].confirmed && cands[i].count > 0) {
        let chain = chain_from(cands, lead);
        for len in 2..=chain.len() {
            let sum: u32 = chain[..len].iter().map(|&i| cands[i].count).sum();
            let dist = sum.abs_diff(expected);
            let fits = dist <= tolerance || sum < expected;
            if fits && dist < best_single && best.as_ref().is_none_or(|(d, _)| dist < *d) {
                best = Some((dist, chain[..len].to_vec()));
            }
        }
    }
    best.map(|(_, chain)| chain)
}

/// A fractional tag moved `shift` episodes later: "1.5" by 2 is "3.5".
fn shifted_fraction(tag: &str, shift: u32) -> String {
    match tag.split_once('.') {
        Some((int, frac)) => match int.parse::<u32>() {
            Ok(n) => format!("{}.{frac}", n.saturating_add(shift)),
            Err(_) => tag.to_string(),
        },
        None => tag.to_string(),
    }
}

/// The parts' listings as one, in the entry's own numbering: each
/// part normalised to per-entry numbers, then shifted past every part
/// before it. Episode ids are kept, so a stitched row still streams
/// from the part that lists it. An integer display tag becomes the
/// row's number; a fractional one (a recap) moves with its part.
#[must_use]
pub(crate) fn merge_parts(parts: &[&[EpisodeRef]]) -> Vec<EpisodeRef> {
    use super::play_native_numbering::{kitsu_episode_cap, numbering_offset, per_entry_fraction};
    let mut out = Vec::new();
    let mut shift = 0u32;
    for part in parts {
        let offset = numbering_offset(part);
        for e in *part {
            let per_entry = |n: u32| n.saturating_sub(offset).saturating_add(shift);
            let tag_value = e.number2.as_deref().map(|t| (t, t.parse::<f64>().ok()));
            let (number, number2) = match tag_value {
                Some((_, Some(v))) if v.fract() == 0.0 && v >= 0.0 => (per_entry(v as u32), None),
                Some((t, _)) => (
                    e.number.saturating_add(shift),
                    Some(shifted_fraction(&per_entry_fraction(t, offset), shift)),
                ),
                None => (per_entry(e.number), None),
            };
            out.push(EpisodeRef {
                id: e.id,
                number,
                number2,
            });
        }
        shift = shift.saturating_add(kitsu_episode_cap(part).unwrap_or(0));
    }
    out
}

/// The pick a split entry makes: the first part's hit, with every
/// part's listing stitched under it — or `None` when the probed
/// candidates are not a split entry.
pub(crate) fn stitched(
    probed: &[(
        &crate::scraper::provider::BrowseHit,
        Vec<EpisodeRef>,
        u32,
        bool,
    )],
    expected: u32,
    best_single: u32,
) -> Option<super::play_native::PickedShow> {
    let cands: Vec<PartCandidate<'_>> = probed
        .iter()
        .map(|(h, eps, _, confirmed)| PartCandidate {
            title: &h.title,
            count: super::play_native_numbering::regular_episode_count(eps),
            confirmed: *confirmed,
        })
        .collect();
    let chain = split_chain(&cands, expected, best_single)?;
    let listings: Vec<&[EpisodeRef]> = chain.iter().map(|&i| probed[i].1.as_slice()).collect();
    Some(super::play_native::PickedShow {
        hit: probed[chain[0]].0.clone(),
        episodes: merge_parts(&listings),
    })
}

#[cfg(test)]
#[path = "play_native_split_test.rs"]
mod tests;
