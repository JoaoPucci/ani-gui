//! Reading the tags an earlier version gave a stitched listing's rows
//! back off it. Split from [`super`] for the per-file complexity bar.

use crate::scraper::provider::EpisodeRef;

/// The rows of `episodes` an earlier version tagged otherwise: each
/// one's slot, the tag it had then and the tag it has now.
pub(crate) fn former_tags_of(episodes: &[EpisodeRef]) -> Vec<(u32, String, String)> {
    episodes
        .iter()
        .filter_map(|e| {
            let former = e.former_tag.clone()?;
            let now = e.number2.clone().unwrap_or_else(|| e.number.to_string());
            Some((e.number, former, now))
        })
        .collect()
}

/// The tag an earlier version gave the row in `slot`, from a resolve's
/// [`former_tags_of`].
pub(crate) fn former_tag_of(former_tags: &[(u32, String, String)], slot: u32) -> Option<&str> {
    former_tags
        .iter()
        .find(|(s, _, _)| *s == slot)
        .map(|(_, former, _)| former.as_str())
}

/// Whether `former`, a tag in the resolve's listing numbering, is one
/// a row of that listing carries now — among its fractional extras.
pub(crate) fn names_a_row_now(
    former: &str,
    native: &crate::commands::play_native_resolve::NativeResolved,
) -> bool {
    let own = crate::commands::numbering_offset::kitsu_ep_no(former, native.numbering_offset);
    carries_tag(&native.extra_tags, &own)
}

/// Whether `tags`, a listing's extra tags in the entry's numbering,
/// carry `number`: the same value where both read as numbers ("12.50"
/// is "12.5"), the same text otherwise.
pub(crate) fn carries_tag(tags: &[String], number: &str) -> bool {
    tags.iter()
        .any(|t| match (t.parse::<f64>(), number.parse::<f64>()) {
            (Ok(a), Ok(b)) => a == b,
            _ => t == number,
        })
}

/// The number, in the entry's numbering, an earlier version gave the
/// row `native` resolved to — none when it gave none, or when a row of
/// the listing carries that number now ([`names_a_row_now`]): what is
/// kept under it may be that row's.
pub(crate) fn former_number(
    native: &crate::commands::play_native_resolve::NativeResolved,
) -> Option<String> {
    former_tag_of(&native.former_tags, native.resolved_slot)
        .filter(|t| !names_a_row_now(t, native))
        .map(|t| crate::commands::numbering_offset::kitsu_ep_no(t, native.numbering_offset))
}

/// A single listing's rows, each carrying the tag an earlier version
/// resolved it by where that differs. Under a continuation offset, that
/// version passed a recap at the offset ("12.5" under 12) through as
/// the entry's own number, and asked the provider for it with the
/// offset added back ("24.5"); it is the entry's 0.5 now, and "24.5" is
/// the tag a request by the old number names.
pub(crate) fn mark_offset_recaps(mut episodes: Vec<EpisodeRef>) -> Vec<EpisodeRef> {
    use crate::commands::play_native_numbering::{numbering_offset, provider_fraction};
    let offset = numbering_offset(&episodes);
    if offset == 0 {
        return episodes;
    }
    for e in episodes.iter_mut().filter(|e| e.former_tag.is_none()) {
        let Some(tag) = e.number2.as_deref() else {
            continue;
        };
        let at_offset = tag
            .split_once('.')
            .and_then(|(int, _)| int.parse::<u32>().ok())
            .is_some_and(|n| n == offset);
        if at_offset {
            e.former_tag = Some(provider_fraction(tag, offset));
        }
    }
    episodes
}
