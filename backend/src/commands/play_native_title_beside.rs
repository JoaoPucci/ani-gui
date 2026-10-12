//! What a sibling names right after a broad listing's stem, beside the
//! season the broad listing carries — split from
//! `play_native_title_reading` for the per-file complexity bar.

use super::*;

/// The divisions `sibling` opens what follows `wide`'s stem with,
/// leaving out any `wide` itself ends on, and the season `wide`
/// carries — the highest season among the divisions it ends on, the
/// bare number its stem ends on included when `sibling` is read past
/// the stem without it ([`past_stem`]); the first when it carries
/// none. `None` when `sibling` does not start with the stem.
pub(crate) fn divisions_beside(sibling: &str, wide: &str) -> Option<(Vec<Marker>, u32)> {
    let (rest, wide_markers) = past_stem(sibling, wide)?;
    let season = wide_markers
        .iter()
        .filter(|m| m.kind == Kind::Season)
        .flat_map(|m| m.ordinals.iter().copied())
        .max()
        .unwrap_or(1);
    let named = scan(&words(&rest), true)
        .into_iter()
        .filter(|m| !wide_markers.contains(m))
        .collect();
    Some((named, season))
}
