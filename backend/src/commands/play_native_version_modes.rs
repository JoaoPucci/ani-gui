//! A mode one version carries and another does not — split from
//! `play_native_versions` for the per-file complexity bar. The copies
//! of a season are the same episodes, but the provider need not carry
//! the same audio for each: a play falls through to the next version
//! that carries the episode in its mode, and the availability probe
//! asks every version whether it carries the mode.

use crate::error::Result;
use crate::scraper::provider::Provider;

use super::play_native::PickedShow;
use super::play_native_episode::{resolve_episode, ResolvedEpisode};
use super::play_native_resolve::NativeError;
use super::play_native_versions::VersionKind;

/// Whether a failed episode chain is an answered dead end — the
/// episode, its language or its stream missing — which another version
/// may not share; blocks, gate refusals and weather are not.
fn dead_end(ne: &NativeError) -> bool {
    !ne.error.is_provider_block()
        && (matches!(
            ne.error,
            crate::error::AniError::EpisodeUnavailable | crate::error::AniError::NoResults
        ) || ne.error.is_not_found_shaped())
}

/// The pick's other versions, in the provider's order, after the one it
/// plays.
fn others(picked: &PickedShow) -> Vec<PickedShow> {
    picked
        .versions
        .iter()
        .filter(|v| v.hit.slug != picked.hit.slug)
        .map(|v| picked.clone().choose(Some(v.kind)))
        .collect()
}

/// `episode` in `mode` from the version `picked` plays — and, when its
/// chain answers a dead end and the pick has other versions, from the
/// next of them in the provider's order that carries it, the version
/// that answered returned beside its outcome. Each further version
/// costs its own episode chain, asked only after the one before it
/// dead-ended; a block, a gate refusal or weather ends the search with
/// that version's error.
pub(crate) async fn resolve_in_any_version<P: Provider + ?Sized>(
    client: &P,
    picked: PickedShow,
    episode: &str,
    mode: &str,
    quality: &str,
) -> (
    PickedShow,
    std::result::Result<ResolvedEpisode, NativeError>,
) {
    let mut tried = resolve_episode(client, &picked, episode, mode, quality).await;
    let mut playing = picked;
    for next in others(&playing) {
        match &tried {
            Err(ne) if dead_end(ne) => {}
            _ => break,
        }
        tried = resolve_episode(client, &next, episode, mode, quality).await;
        playing = next;
    }
    (playing, tried)
}

/// Whether each of the pick's versions carries `mode`, in the
/// provider's order: `default` for the version it plays, already asked,
/// and one mode probe for each other version — the same bounded scan
/// of its rows ([`super::availability_mode::mode_present`]). Another
/// version's probe failing leaves its mode unknown; the playing copy
/// has answered, and the answer stands on it.
///
/// # Errors
/// A block or the gate's refusal on another version's probe: every
/// further request repeats it.
pub(crate) async fn modes_of_versions<P: Provider + ?Sized>(
    client: &P,
    picked: &PickedShow,
    default: Option<bool>,
    mode: &str,
) -> Result<Vec<(VersionKind, Option<bool>)>> {
    let mut out = Vec::new();
    for v in &picked.versions {
        let present = if v.hit.slug == picked.hit.slug {
            default
        } else {
            match super::availability_mode::mode_present(client, &v.episodes, mode).await {
                Ok(present) => present,
                Err(e)
                    if e.is_provider_block()
                        || matches!(e, crate::error::AniError::GateRefused) =>
                {
                    return Err(e)
                }
                Err(_) => None,
            }
        };
        out.push((v.kind, present));
    }
    Ok(out)
}

/// The version to answer with for a mode, and whether the show carries
/// it: the one `picked` plays when it carries the mode, else the first
/// in the provider's order that does — the show carrying it; absent
/// only when every version answered absence; unknown otherwise. A pick
/// with one version answers as its own probe did.
pub(crate) fn version_for_mode(
    picked: PickedShow,
    modes: &[(VersionKind, Option<bool>)],
    default: Option<bool>,
) -> (PickedShow, Option<bool>) {
    if default == Some(true) || modes.is_empty() {
        return (picked, default);
    }
    if let Some((kind, _)) = modes.iter().find(|(_, m)| *m == Some(true)) {
        return (picked.choose(Some(*kind)), Some(true));
    }
    let absent = modes.iter().all(|(_, m)| *m == Some(false));
    (picked, absent.then_some(false))
}
