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
/// after one of `wide_markers` divides that division of `wide` and is
/// read as a part: "Season 3 Second Half" beside "Season 3" names part
/// 2, not a season 2.
fn own_divisions(words: &[String], wide_markers: &[Marker]) -> Vec<Marker> {
    let mut named = Vec::new();
    let mut after_wide = false;
    let mut i = 0;
    while i < words.len() {
        let (found, taken) = match glued_divisions(&words[i]) {
            Some((glued, _)) => (glued, 0),
            None => match division_at(words, i) {
                Some((m, taken)) => (vec![m], taken),
                None => break,
            },
        };
        i += taken.max(1);
        for m in found {
            if after_wide && taken == 1 && m.kind == Kind::Season {
                named.push(Marker {
                    kind: Kind::Part,
                    ordinals: m.ordinals,
                });
            } else if wide_markers.contains(&m) {
                after_wide = true;
            } else {
                named.push(m);
            }
        }
    }
    named
}
