//! Which part of a show a provider title names — "2nd Stage",
//! "Part 3", "Season 2" after a shared stem. Split from
//! `play_native_split` for the per-file complexity bar.

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

#[cfg(test)]
#[path = "play_native_part_title_test.rs"]
mod tests;
