//! Native play resolution — the walk half, over any provider.
//! Searches the provider across the canonical title and its fallbacks, picks a candidate,
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

use crate::commands::progress::ProgressLine;
use crate::error::AniError;
use crate::scraper::provider::{Provider, ProviderId, ShowKey, SubtitleTrack};

use super::play_native::pick_candidate_titled;
pub use super::play_native_episode::resolve_episode;
use super::play_native_episode::{classify_chain_failure, ChainOutcome};
use super::play_native_merge::former_tags_of;
use super::play_native_numbering::{extra_episode_tags, kitsu_episode_cap, numbering_offset};
use super::play_native_transient::{attempt_at, Transients, WalkStage};
use super::play_native_version_modes::resolve_in_any_version;

/// A fully resolved native play: what the orchestrator needs to open
/// a session, stamp caches, and write history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeResolved {
    /// The show key's string — the id history and caches key on:
    /// the provider's slug under its label.
    pub slug: String,
    /// The provider that answered.
    pub provider: ProviderId,
    /// The provider's display title for the show.
    pub title: String,
    /// The master-playlist URL the embed page carried.
    pub master_url: String,
    /// The referer the provider's CDN wants on every fetch of the
    /// master and what it lists — `None` when it wants none. Every
    /// consumer of the resolve sends it: the proxy session, the cache
    /// row, the download tool, the external player.
    pub referer: Option<String>,
    /// Sidecar subtitle tracks listed beside the stream — outside the
    /// playlist, so every consumer has to carry them itself.
    pub subtitles: Vec<SubtitleTrack>,
    /// Highest episode number the provider lists, for the
    /// availability cap stamp. Free — the picker already fetched the
    /// list.
    pub episode_cap: Option<u32>,
    /// The shift between the provider's episode numbers and the
    /// per-entry numbers the request used ([`numbering_offset`]).
    /// The offset store persists it for the read boundary, which
    /// translates history rows from before rows recorded the app's own
    /// number.
    pub numbering_offset: u32,
    /// The listing's fractional display tags
    /// ([`extra_episode_tags`]) — the availability stamp's
    /// `extra_episodes`, also already paid for.
    pub extra_tags: Vec<String>,
    /// The matched row's slot — its position in the provider's
    /// listing. Cached with the resolution; a history row records the
    /// app's own number read from the row's tag, or from the slot when
    /// it has none, under the offset
    /// ([`crate::commands::play_native_record::matched_episode`]).
    pub resolved_slot: u32,
    /// The matched row's display tag, when it carries one: with the
    /// slot, what the app's own number for the episode is read from
    /// ([`crate::commands::play_native_record::matched_episode`]).
    /// Display stamps are no longer written from it.
    pub resolved_tag: Option<String>,
    /// The pick's versions, in the provider's order — empty when it
    /// has one.
    pub versions: Vec<crate::commands::play_native_versions::VersionInfo>,
    /// The version that played, when the pick has several.
    pub version: Option<crate::commands::play_native_versions::VersionKind>,
    /// Rows of the picked listing an earlier version tagged otherwise:
    /// (slot, the tag it had then, the tag it has now) — a stitched
    /// recap before a cumulatively numbered part. Empty for any other
    /// listing.
    pub former_tags: Vec<(u32, String, String)>,
    /// The pick's other versions, each as its show key and its listing:
    /// a stamp a copy's history row reads through is put right from its
    /// own listing, whichever copy played. Empty for a pick with one
    /// version.
    pub other_copies: Vec<(String, Vec<crate::scraper::provider::EpisodeRef>)>,
    /// The pick's [`crate::commands::play_native::PickedShow::unread_past`]:
    /// Kitsu's count, when whether the listing's rows past it are
    /// appended extras could not be read.
    pub unread_past: Option<u32>,
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
    /// The instant the failure behind an AGGREGATED transient
    /// verdict was observed. The walk can fail one alias and finish
    /// a later one cleanly; the breaker record for the resulting
    /// verdict must carry the failing attempt's moment, or the
    /// gate's staleness filter cannot discard it after a recovery
    /// recorded in between. `None` when the error IS the chain's
    /// last attempt — the transport's own stamp is then correct —
    /// and for a deadline's elapse, which no attempt is stamped on;
    /// the breaker tells a stall the walk kept from the deadline's
    /// `Timeout` by this stamp.
    pub failed_at: Option<tokio::time::Instant>,
    /// True when the verdict is the answered dead end and every pool
    /// behind it was refused on how its hits are named
    /// ([`AniError::RefusedByName`]): the catalogue answered, and lists
    /// nothing named for the entry. Set by the availability walk
    /// ([`super::play_native_walk::pick_native_walk`]); false on every
    /// other verdict, a dead end on dead listings included.
    pub refused_by_name: bool,
}

/// What a native resolution is asked for — the play-relevant slice
/// of `PlayArgs`, borrowed.
#[derive(Debug, Clone, Copy)]
pub struct NativeResolveRequest<'a> {
    /// Canonical title, searched first.
    pub title: &'a str,
    /// Fallback titles, searched in order.
    pub alt_titles: &'a [String],
    /// Episode number as the frontend sent it.
    pub episode: &'a str,
    /// `"sub"` or `"dub"`.
    pub mode: &'a str,
    /// Quality setting (`best`, `worst`, or a height like `720`).
    /// `best` keeps the adaptive master; anything else selects a
    /// variant from it, falling back to the master on a miss.
    pub quality: &'a str,
    /// Kitsu's episode count, when known.
    pub expected_count: Option<u32>,
    /// The year the show premiered per Kitsu, when known. The
    /// picker's identity signal for cour and franchise siblings.
    pub year: Option<u32>,
    /// Kitsu's subtype (`TV`, `movie`, `special`, `OVA`, `ONA`),
    /// when the caller has it. Format disproof against the browse
    /// cards' badges.
    pub subtype: Option<&'a str>,
    /// The version to play when the pick has several; `None` plays
    /// the pick's default.
    pub version: Option<crate::commands::play_native_versions::VersionKind>,
    /// The history file whose numbering sidecar keeps each show's
    /// (slot, tag) display stamp — what a request from the user's
    /// history names a row by. `None` asks nothing of it.
    pub stamps_beside: Option<&'a std::path::Path>,
    /// Where the evidence about the entry's last episodes comes from,
    /// when the caller knows the entry
    /// ([`super::play_native_appended`]).
    pub tail: Option<&'a dyn super::play_native_appended::TailSource>,
}

/// Ceiling on one complete native resolution — every search, probe
/// and embed fetch together. Strictly below
/// [`crate::scraper::gate::HALF_OPEN_TRIAL_STALE`]: a resolve that
/// outlived the trial window would let a second half-open trial
/// start behind the still-running first, the overlap the sanction
/// chain forbids. 60s also matches the ceiling the subprocess path
/// enforced with its run timeout.
pub const RESOLVE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(60);

/// [`resolve_native`] under [`RESOLVE_DEADLINE`]. Elapsing is the
/// stall's own identity: [`AniError::Timeout`], never a clean miss —
/// a provider that accepts connections but answers nothing is
/// weather, and the breaker hears it as such.
///
/// # Errors
/// As [`resolve_native`], plus `Timeout` at the deadline.
pub async fn resolve_native_bounded<P, F>(
    client: &P,
    req: NativeResolveRequest<'_>,
    on_progress: &mut F,
) -> std::result::Result<NativeResolved, NativeError>
where
    P: Provider + ?Sized,
    F: FnMut(ProgressLine) + Send,
{
    match tokio::time::timeout(RESOLVE_DEADLINE, resolve_native(client, req, on_progress)).await {
        Ok(resolved) => resolved,
        Err(_elapsed) => Err(NativeError {
            error: AniError::Timeout,
            clean_miss: false,
            failed_at: None,
            refused_by_name: false,
        }),
    }
}

/// Search the request's title then its fallbacks in order, pick, and
/// resolve the episode for the mode to a master-playlist URL.
///
/// - Gate admission is the transport's job: the production client
///   wraps its fetch in [`crate::scraper::gated::GatedFetch`], which
///   admits every provider request and carries the half-open trial
///   sanction across the chain. A second admission here would consume
///   that trial before the fetch's own admit runs — the fetch would
///   then be refused by its own chain's sanction, and the breaker
///   would reopen on its own refusal.
/// - A pool whose pick is rejected keeps the walk going — the next
///   alias may carry the real show (the Stone Ocean recovery).
/// - [`AniError::Upstream`] from a search stops the walk.
/// - `episode` must name an episode the provider lists; anything else
///   is a transient dead end, not evidence of absence.
///
/// # Errors
/// [`NativeError`] with `clean_miss` set only for the
/// all-clean-no-match verdict.
pub async fn resolve_native<P, F>(
    client: &P,
    req: NativeResolveRequest<'_>,
    on_progress: &mut F,
) -> std::result::Result<NativeResolved, NativeError>
where
    P: Provider + ?Sized,
    F: FnMut(ProgressLine) + Send,
{
    on_progress(ProgressLine::Searching {
        provider: client.label().into(),
    });
    let entry_titles: Vec<&str> = std::iter::once(req.title)
        .chain(req.alt_titles.iter().map(String::as_str))
        .collect();
    // The transient failures met along the way — none ends the walk,
    // and the exhausted walk surfaces one of them.
    let mut transients = Transients::default();
    let mut any_answered_dead_end = false;
    // A show was picked and its episode chain answered a dead end —
    // the episode's verdict, distinct from a pool that matched nothing.
    let mut any_episode_dead_end = false;
    for t in std::iter::once(req.title).chain(req.alt_titles.iter().map(String::as_str)) {
        match client.search(t).await {
            Ok(hits) => {
                if hits.is_empty() {
                    continue;
                }
                match pick_candidate_titled(
                    client,
                    &hits,
                    req.expected_count,
                    t,
                    &entry_titles,
                    req.year,
                    req.subtype,
                )
                .await
                {
                    Ok(picked) => {
                        // The version asked for first, so the listing
                        // the appended-extras rule reads is the one that
                        // plays.
                        let picked = super::play_native_appended::fit_appended(
                            picked.choose(req.version),
                            req.expected_count,
                            req.tail,
                        )
                        .await;
                        on_progress(ProgressLine::Matched {
                            title: picked.hit.title.clone(),
                        });
                        // The version asked for, or the next that
                        // carries the episode in the mode, each read
                        // through its own display stamp.
                        let (picked, resolved) = resolve_in_any_version(
                            client,
                            picked,
                            req.episode,
                            req.stamps_beside,
                            req.mode,
                            req.quality,
                        )
                        .await;
                        let (versions, version) =
                            crate::commands::play_native_versions::named_versions(
                                client.id(),
                                &picked,
                            );
                        let resolved = match resolved {
                            Ok(resolved) => resolved,
                            Err(ne) => match classify_chain_failure(ne) {
                                ChainOutcome::Stop(ne) => return Err(ne),
                                ChainOutcome::DeadEnd => {
                                    any_answered_dead_end = true;
                                    any_episode_dead_end = true;
                                    continue;
                                }
                                ChainOutcome::Transient(ne) => {
                                    // Stamped as the provider stamps
                                    // it: with the attempt that
                                    // produced it, or with none when
                                    // no attempt did — its walk cut a
                                    // server off while the gate still
                                    // held the request — which the
                                    // breaker reads as a deadline's
                                    // elapse.
                                    transients.note_at(
                                        WalkStage::Chain,
                                        ne.error,
                                        client.last_attempt_at(),
                                    );
                                    continue;
                                }
                            },
                        };
                        on_progress(ProgressLine::LinksFetched {
                            provider: client.label().into(),
                        });
                        let other_copies = picked
                            .copies()
                            .into_iter()
                            .filter(|(hit, _)| hit.slug != picked.hit.slug)
                            .map(|(hit, episodes)| {
                                (
                                    ShowKey::new(client.id(), hit.slug.clone()).to_string(),
                                    episodes.to_vec(),
                                )
                            })
                            .collect();
                        let episode_cap = kitsu_episode_cap(&picked.episodes);
                        let offset = numbering_offset(&picked.episodes);
                        let extra_tags = extra_episode_tags(&picked.episodes);
                        return Ok(NativeResolved {
                            slug: ShowKey::new(client.id(), picked.hit.slug).to_string(),
                            provider: client.id(),
                            title: picked.hit.title,
                            master_url: resolved.master_url,
                            referer: resolved.referer,
                            // Already bounded by the episode step,
                            // which the range download shares.
                            subtitles: resolved.subtitles,
                            episode_cap,
                            numbering_offset: offset,
                            extra_tags,
                            resolved_slot: resolved.slot,
                            resolved_tag: resolved.tag,
                            versions,
                            version,
                            former_tags: former_tags_of(&picked.episodes),
                            other_copies,
                            unread_past: None,
                        });
                    }
                    // A rejected pool is a clean verdict about THIS
                    // pool; the next alias may carry the real show.
                    Err(AniError::NoResults) => {}
                    // A refusal or rate limit from a probe is the
                    // provider blocking this client — and a gate
                    // refusal is the gate's own stop — either way
                    // the walk ends, identity intact.
                    Err(e) if e.is_provider_block() || matches!(e, AniError::GateRefused) => {
                        return Err(NativeError {
                            error: e,
                            clean_miss: false,
                            failed_at: None,
                            refused_by_name: false,
                        });
                    }
                    // An all-answered-not-found pool — dead candidates
                    // on a healthy provider — or one refused on how
                    // its hits are named. The walk moves on, and the
                    // verdict stays answered — the breaker must not
                    // open on a provider that answered.
                    Err(AniError::Upstream { .. } | AniError::RefusedByName) => {
                        any_answered_dead_end = true;
                    }
                    Err(e) => {
                        tracing::debug!(error = ?e, "walk: pick failed transiently");
                        transients.note(WalkStage::Pick, e, attempt_at(client));
                    }
                }
            }
            Err(AniError::Upstream { status }) => {
                // Blocked: every further request deepens the hole.
                return Err(NativeError {
                    error: AniError::Upstream { status },
                    clean_miss: false,
                    failed_at: None,
                    refused_by_name: false,
                });
            }
            // The gate refused this chain: every subsequent fetch
            // would be refused the same way. The identity survives
            // to the caller, which records no breaker outcome for it.
            Err(AniError::GateRefused) => {
                return Err(NativeError {
                    error: AniError::GateRefused,
                    clean_miss: false,
                    failed_at: None,
                    refused_by_name: false,
                });
            }
            Err(e) => {
                tracing::debug!(error = ?e, "walk: search failed transiently");
                transients.note(WalkStage::Search, e, attempt_at(client));
            }
        }
    }
    // The transient verdict, when a failure the walk did not stop on
    // was met — a search that never answered among them: no title
    // means a search that neither answered nor failed.
    if let Some(ne) = transients.verdict() {
        return Err(ne);
    }
    if any_answered_dead_end {
        // Answered dead ends prove nothing about the show — never
        // the persistable clean miss — but the provider answering is
        // health to the breaker (both verdicts record success). When
        // a picked show's episode chain was the dead end, the verdict
        // is the episode's: the show is there, this episode is not.
        return Err(NativeError {
            error: if any_episode_dead_end {
                AniError::EpisodeUnavailable
            } else {
                AniError::NoResults
            },
            clean_miss: false,
            failed_at: None,
            refused_by_name: false,
        });
    }
    Err(NativeError {
        error: AniError::NoResults,
        clean_miss: true,
        failed_at: None,
        refused_by_name: false,
    })
}

/// The tracks of a listing the resolve carries: the first
/// [`SUBTITLE_TRACK_CAP`](crate::proxy::upstream::SUBTITLE_TRACK_CAP)
/// in listing order, with one exception — a default track the listing
/// put past the cap takes the last kept slot, so the provider's own
/// choice is never dropped and no more than one track moves. A
/// listing that fits is kept as it is. Applied once, in the episode
/// step every resolve goes through, so every projection of the
/// resolve — and the range download, which resolves its episodes
/// through the same step — shares the bound instead of each
/// guarding its own.
#[must_use]
pub(crate) fn tracks_within_cap(mut tracks: Vec<SubtitleTrack>) -> Vec<SubtitleTrack> {
    use crate::proxy::upstream::SUBTITLE_TRACK_CAP;
    if tracks.len() <= SUBTITLE_TRACK_CAP {
        return tracks;
    }
    let dropped = tracks.len() - SUBTITLE_TRACK_CAP;
    tracing::warn!(
        dropped,
        "resolve: subtitle listing longer than the track cap, the rest dropped"
    );
    if let Some(default_at) = tracks.iter().position(|t| t.default) {
        if default_at >= SUBTITLE_TRACK_CAP {
            let default = tracks.swap_remove(default_at);
            tracks.truncate(SUBTITLE_TRACK_CAP - 1);
            tracks.push(default);
            return tracks;
        }
    }
    tracks.truncate(SUBTITLE_TRACK_CAP);
    tracks
}

#[cfg(test)]
#[path = "play_native_resolve_test.rs"]
mod tests;

#[cfg(test)]
#[path = "play_native_resolve_prop_test.rs"]
mod prop_tests;
