//! What a sibling names right after a broad listing's stem, beside the
//! season the broad listing carries — split from
//! `play_native_title_reading` for the per-file complexity bar.

use super::*;

/// The divisions `sibling` opens what follows `wide`'s stem with,
/// leaving out any `wide` itself ends on, and the season `wide`
/// carries — the highest season among the divisions it ends on, the
/// bare number its stem ends on included when `sibling` is read past
/// the stem without it ([`past_stem`]); the first when it carries
/// none. An ordinal standing alone after `wide`'s own division divides
/// it ([`own_divisions`]). `None` when `sibling` does not start with the
/// stem.
pub(crate) fn divisions_beside(sibling: &str, wide: &str) -> Option<(Vec<Marker>, u32)> {
    let (rest, wide_markers) = past_stem(sibling, wide)?;
    let season = wide_markers
        .iter()
        .filter(|m| m.kind == Kind::Season)
        .flat_map(|m| m.ordinals.iter().copied())
        .max()
        .unwrap_or(1);
    Some((own_divisions(&words(&rest), &wide_markers), season))
}

/// The divisions `words` open with, up to the first word that names
/// none, leaving out any in `wide_markers`. An ordinal standing alone
/// right after one of `wide`'s own seasons — apart, or glued to it in
/// Japanese — divides that season and is read as a part: "Season 3
/// Second Half" beside "Season 3" names part 2, not a season 2. After
/// a part of `wide`'s it stays as it is read.
fn own_divisions(words: &[String], wide_markers: &[Marker]) -> Vec<Marker> {
    let wide_season = |m: &Marker| m.kind == Kind::Season && wide_markers.contains(m);
    let mut named = Vec::new();
    let mut after_season = false;
    let mut i = 0;
    while i < words.len() {
        if let Some(n) = number_after_wide_season(&words[i], &wide_season) {
            named.push(Marker {
                kind: Kind::Part,
                ordinals: vec![n],
            });
            after_season = false;
            i += 1;
            continue;
        }
        let (found, taken) = match glued_divisions(&words[i]) {
            Some((glued, _)) => (glued, 0),
            None => match division_at(words, i) {
                Some((m, taken)) => (vec![m], taken),
                None => break,
            },
        };
        i += taken.max(1);
        for m in found {
            let divides = after_season && taken == 1 && m.kind == Kind::Season;
            // Only the division right before an ordinal decides
            // whether it divides W's season.
            after_season = !divides && wide_season(&m);
            if divides {
                named.push(Marker {
                    kind: Kind::Part,
                    ordinals: m.ordinals,
                });
            } else if !wide_markers.contains(&m) {
                named.push(m);
            }
        }
    }
    named
}

/// The number glued right after one of `wide`'s own seasons written
/// in Japanese, in one word, with no other division between: the ２ of "第3期２" beside "ショー第3期".
fn number_after_wide_season(word: &str, wide_season: &impl Fn(&Marker) -> bool) -> Option<u32> {
    let head = word.trim_end_matches(|c: char| c.is_ascii_digit());
    let n = small_number(&word[head.len()..])?;
    let (glued, _) = glued_divisions(head)?;
    glued.last().is_some_and(wide_season).then_some(n)
}
