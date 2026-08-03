//! Native play resolution — the walk half. Searches the provider
//! across the canonical title and its fallbacks, picks a candidate,
//! and resolves the requested episode down to a master-playlist URL,
//! emitting the same [`ProgressLine`] shapes the SSE overlay already
//! renders.
//!
//! Error policy mirrors what the subprocess path converged on: a
//! clean everywhere-searched-nothing-matched miss is the only verdict
//! the caller may persist as a negative availability write
//! ([`NativeError::clean_miss`]); every transport failure, upstream
//! refusal, or post-pick dead end is transient and must not hide a
//! real show behind the negative TTL. An upstream refusal
//! (cloudflare) stops the walk — a block on one query blocks them
//! all, and each further request deepens the hole.

use crate::anicli::parser::ProgressLine;
use crate::error::AniError;
use crate::scraper::anidb::{AnidbClient, AnidbFetch};
use crate::scraper::gate::{ScrapePriority, ScraperGate};

use super::play_native::{pick_candidate, PickedShow};

/// A fully resolved native play: what the orchestrator needs to open
/// a session, stamp caches, and write history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeResolved {
    /// The provider slug — the id history and caches key on.
    pub slug: String,
    /// The provider's display title for the show.
    pub title: String,
    /// The master-playlist URL the embed page carried.
    pub master_url: String,
    /// Highest episode number the provider lists, for the
    /// availability cap stamp. Free — the picker already fetched the
    /// list.
    pub episode_cap: Option<u32>,
}

/// A failed resolution, carrying the typed error plus whether the
/// verdict is a clean picker miss the caller may persist.
#[derive(Debug)]
pub struct NativeError {
    /// The error to surface.
    pub error: AniError,
    /// True only when every search completed cleanly and no candidate
    /// survived — the one shape that proves absence rather than
    /// weather.
    pub clean_miss: bool,
}

/// Search `title` then `alt_titles` in order, pick, and resolve
/// `episode` for `mode` to a master-playlist URL.
///
/// - Each provider request cluster is admitted through `gate` at
///   `priority` when a gate is given; a refused admit is transient.
/// - A pool whose pick is rejected keeps the walk going — the next
///   alias may carry the real show (the Stone Ocean recovery).
/// - [`AniError::Upstream`] from a search stops the walk.
/// - `episode` must name an episode the provider lists; anything else
///   is a transient dead end, not evidence of absence.
///
/// # Errors
/// [`NativeError`] with `clean_miss` set only for the
/// all-clean-no-match verdict.
pub async fn resolve_native<F, P>(
    client: &AnidbClient<F>,
    gate: Option<&ScraperGate>,
    priority: ScrapePriority,
    title: &str,
    alt_titles: &[String],
    episode: &str,
    mode: &str,
    expected_count: Option<u32>,
    on_progress: &mut P,
) -> std::result::Result<NativeResolved, NativeError>
where
    F: AnidbFetch,
    P: FnMut(ProgressLine) + Send,
{
    let _ = (
        client,
        gate,
        priority,
        title,
        alt_titles,
        episode,
        mode,
        expected_count,
        on_progress,
    );
    todo!()
}

/// Resolve the requested episode within a picked show down to the
/// master URL. Split from the walk for the per-file complexity bar
/// and because the orchestrator's cache-hit path may someday reuse
/// it.
///
/// # Errors
/// `NativeError` (never `clean_miss`): the show matched, so nothing
/// here is evidence of absence.
pub async fn resolve_episode<F: AnidbFetch>(
    client: &AnidbClient<F>,
    picked: &PickedShow,
    episode: &str,
    mode: &str,
) -> std::result::Result<String, NativeError> {
    let _ = (client, picked, episode, mode);
    todo!()
}

#[cfg(test)]
#[path = "play_native_resolve_test.rs"]
mod tests;
