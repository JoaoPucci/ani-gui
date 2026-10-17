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
