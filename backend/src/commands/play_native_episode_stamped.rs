//! A request from the user's history, resolved against the stamp the
//! numbering sidecar keeps for the show. Split from
//! [`super`] for the per-file complexity bar.

use super::{
    numbering_offset, provider_fraction, resolve_episode, tag_matches, NativeError, PickedShow,
    Provider, ResolvedEpisode,
};

/// [`resolve_episode`] for a request from the user's history: `stamp`
/// is the (slot, tag) display stamp the numbering sidecar keeps for
/// the show. While it names its row by a tag that row had before — a
/// stitched recap stamped "24.5" before it was "12.5" — a request for
/// that tag is the watch the stamp recorded, and goes to the row by
/// its tag now, whichever row the listing tags so today.
///
/// # Errors
/// As [`resolve_episode`].
pub async fn resolve_episode_stamped<P: Provider + ?Sized>(
    client: &P,
    picked: &PickedShow,
    episode: &str,
    stamp: Option<&(u32, String)>,
    mode: &str,
    quality: &str,
) -> std::result::Result<ResolvedEpisode, NativeError> {
    let request = stamp
        .and_then(|stamp| stamped_request(picked, episode, stamp))
        .unwrap_or_else(|| episode.to_string());
    resolve_episode(client, picked, &request, mode, quality).await
}

/// The request for the stamp's row by its tag now, in the entry's
/// numbering, when `episode` names it by the former tag the stamp
/// still carries.
fn stamped_request(picked: &PickedShow, episode: &str, stamp: &(u32, String)) -> Option<String> {
    let (slot, tag) = stamp;
    let offset = numbering_offset(&picked.episodes);
    let target = provider_fraction(episode.trim(), offset);
    if !tag_matches(tag, &target) {
        return None;
    }
    let row = picked.episodes.iter().find(|e| e.number == *slot)?;
    row.former_tag.as_deref().filter(|t| tag_matches(t, tag))?;
    let now = row
        .number2
        .clone()
        .unwrap_or_else(|| row.number.to_string());
    Some(crate::commands::numbering_offset::kitsu_ep_no(&now, offset))
}
