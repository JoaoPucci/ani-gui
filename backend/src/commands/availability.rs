//! "Is this title playable?" probe — runs the same provider search
//! the play path does, returns true iff any candidate exists for the
//! canonical title or any alt-title.
//!
//! The detail page hits this on mount so it can gate the Play +
//! Download CTAs ahead of a click — better than letting the user
//! discover the provider does not carry the show by clicking and
//! getting an error overlay (the prior failure mode for shows like Kitsu's
//! Western-animation entries: "Arcane Season 2", etc.).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::cache::{meta_cache_get, meta_cache_put};
use crate::commands::availability_refresh::hold_if_still_ours;
use crate::commands::play_native_walk::pick_native_walk;
use crate::error::Result;

/// Cache TTL for positive results on FINISHED shows — 30 days.
/// Catalog is stable; episode_count won't move.
const AVAILABILITY_TTL_FINISHED_SECS: u64 = 30 * 24 * 60 * 60;
/// Cache TTL for positive results on ONGOING shows (status =
/// "current" / "upcoming" / "tba") — 24 hours. Most shows release
/// once a week; a 1-day window means new episodes surface within
/// a day of the next probe rather than waiting up to a month for
/// the cap to refresh.
const AVAILABILITY_TTL_ONGOING_SECS: u64 = 24 * 60 * 60;
/// Cache TTL for negative results — 7 days. Catalog adds are rarer
/// than removals but still happen (late-season uploads, region
/// availability shifts), so refresh negatives more often.
const AVAILABILITY_TTL_NEGATIVE_SECS: u64 = 7 * 24 * 60 * 60;

/// Inputs for `availability_check` — a Kitsu title (plus alts) and
/// the audio mode under which to ask "does the provider carry this?".
/// Carries
/// optional Kitsu metadata (`episode_count`, `status`, `kitsu_id`)
/// that lets the resolver disambiguate name collisions and pick the
/// right cache TTL bucket.
#[derive(Debug, Clone, Deserialize)]
pub struct AvailabilityArgs {
    /// Canonical Kitsu title — first search target.
    pub title: String,
    /// `"sub"` or `"dub"` — gates the kind of result the provider returns.
    pub mode: String,
    /// Fallback titles to try when canonical returns no hits (e.g.
    /// romanized + native name pulled from Kitsu's `titles.*`).
    #[serde(default)]
    pub alt_titles: Vec<String>,
    /// Kitsu's authoritative episode count — feeds the same picker
    /// disambiguation the play path uses (Stone Ocean Part 6 etc.).
    /// Without it, the pick has only the entry's year and its titles
    /// to go on: a hit is taken when the title searched, its own year
    /// or the entry's name vouches for it, and a pool with none is a
    /// miss rather than a "yes" for a show with a colliding name.
    #[serde(default)]
    pub episode_count: Option<u32>,
    /// Year the show first aired, parsed from Kitsu's `start_date`.
    /// Plumbed through the picker as the primary tie-break against
    /// the provider's own aired-start year. See [`PlayArgs::year`].
    #[serde(default)]
    pub year: Option<u32>,
    /// Kitsu's subtype (`TV`, `movie`, `special`, `OVA`, `ONA`),
    /// when the caller has it — the same format disproof the play
    /// and download pickers apply, so availability cannot cache a
    /// different candidate than they would select.
    #[serde(default)]
    pub subtype: Option<String>,
    /// Kitsu id — cache key. When omitted (legacy callers), or when
    /// the value is not an id ([`crate::kitsu_id`]) and so reads as
    /// omitted, the check still runs but its result isn't persisted.
    #[serde(default, deserialize_with = "crate::kitsu_id::deserialize_optional")]
    pub kitsu_id: Option<String>,
    /// Kitsu's airing status (`"current"`, `"finished"`,
    /// `"upcoming"`, `"tba"`, `"unreleased"`). When known, it
    /// branches the positive cache TTL: ongoing shows refresh in
    /// 24h so weekly drops surface within a day; finished shows
    /// hold for 30d. When omitted, defaults to the ongoing TTL —
    /// safer to refresh too often than to serve a stale cap.
    #[serde(default)]
    pub status: Option<String>,
    /// True for opportunistic probes nobody is waiting on (rail
    /// warms, the home loader's cache fills). Background probes pass
    /// through the scraper gate: paced, and skipped entirely while
    /// the breaker is open. Interactive checks (detail-page CTA)
    /// leave this false and always go through. Defaults false so
    /// legacy callers keep interactive semantics.
    #[serde(default)]
    pub background: bool,
    /// Skip the cached row and ask the provider directly.
    ///
    /// The stored count is a snapshot — 24h for an ongoing show — and
    /// the provider catalogues episodes inside that window. A user
    /// clicking a tile the stored count says is unavailable is asking
    /// a question the cache cannot answer, so replaying it back is
    /// worse than not answering: it confirms the tile's own claim
    /// without anyone having checked.
    ///
    /// Only the READ is skipped. The result still goes through
    /// `write_cache_full` below, so a fresher answer replaces the
    /// stale row for every other reader rather than being discarded.
    ///
    /// Defaults false: every existing caller keeps the cheap path.
    #[serde(default)]
    pub bypass_cache: bool,
}

/// Result of an availability probe — does the provider carry the show in
/// the requested mode, and (when it does) what's the truthful episode
/// cap and recap-tag list?
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailabilityResponse {
    /// True when the provider has at least one candidate matching any
    /// of the queried titles. False = the show is not in its
    /// catalog (e.g. Western animation Kitsu happens to index).
    pub available: bool,
    /// Highest INTEGER episode number streamable in the requested
    /// mode (One Piece sub: 1160). Authoritative cap for resume
    /// CTA, Download All / range max, episode-strip pagination.
    /// Excludes half-episode tags like `"1061.5"` — those live in
    /// `extra_episodes`. None when available=false or legacy cache.
    #[serde(default)]
    pub episode_count: Option<u32>,
    /// Non-integer episode tags the provider has streamable (recap /
    /// special episodes — e.g. `["1061.5"]` for One Piece).
    /// Frontend splices these into the episode strip at their
    /// numeric position. Empty when there are no extras.
    #[serde(default)]
    pub extra_episodes: Vec<String>,
    /// True when `episode_count` came from the search hit rather than
    /// the per-show detail fetch — the gate-refused fallback. That
    /// number counts half episodes as whole ones, so it runs one high
    /// for shows carrying them, and advancing to it forwards an
    /// episode that does not exist. Rows carrying this are treated as
    /// cache misses so they re-probe; see [`cache_hit_is_usable`].
    /// Absent from rows written before the flag existed, which
    /// `default` reads as exact.
    #[serde(default)]
    pub episode_count_approximate: bool,
    /// True when the pacer refused the detail fetch outright, as
    /// opposed to the fetch being attempted and failing. Both leave
    /// the count approximate; only this one means nobody asked.
    ///
    /// Describes THIS response, not the show — so it is never part of
    /// what gets cached. The cached row is built separately and
    /// leaves this false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub gate_refused: bool,
    /// The provider that answered: on a positive row the one whose
    /// catalogue carries the show, on a negative row the one whose
    /// miss it is — served only while that provider is answering and
    /// every provider ahead of it is refusing (breaker open, or a
    /// rate-limit pause running). None on rows written before the
    /// field existed.
    #[serde(default)]
    pub provider: Option<crate::scraper::provider::ProviderId>,
    /// Present when the probe reached the provider and came back
    /// without a verdict, naming why ([`UnsettledReason`]). `available`
    /// is false and means nothing; nothing is persisted, and the next
    /// look asks again. Readers treat every reason but a refusal by
    /// name as unknown — the card stays, the page keeps its lazy play —
    /// where an error status sent them to the same place by way of the
    /// console; a refusal by name says the catalogue lists nothing
    /// named for the entry, and they show the show as not in it. A
    /// reader before reasons reads any of them as the `true` it was.
    /// Absent from every settled answer and from every cached row,
    /// which hold verdicts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsettled: Option<UnsettledReason>,
    /// The show's versions, in the provider's order, when its pick has
    /// several — copies of one season under a version tag ("Versions of
    /// one pick" in `docs/title-resolution.md`); absent otherwise. The
    /// uncensored version is the one a play naming none plays.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub versions: Vec<crate::commands::play_native_versions::VersionInfo>,
}

/// Why an availability probe reached no verdict, on the wire as a
/// snake-case string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnsettledReason {
    /// Every pool the walk met was refused on how its hits are named
    /// (`NativeError::refused_by_name`): the catalogue answered, and
    /// lists nothing named for the entry. An inference, so it is not
    /// the clean miss and is not persisted — and the one reason that
    /// says the show cannot be played.
    Refused,
    /// The probe found something and nothing it found answered: dead
    /// listings on a healthy provider, or a mode nobody answered for.
    Unanswered,
    /// An absence answered past a provider a positive row remembers,
    /// which has not denied the show.
    PastAffinity,
    /// A negative that yielded to a positive row written while the
    /// probe was out.
    Superseded,
}

impl AvailabilityResponse {
    /// The answer of a probe that reached no verdict for `reason`
    /// ([`Self::unsettled`]): not available, no count, no extras, no
    /// provider, and nothing to persist.
    #[must_use]
    pub fn unsettled(reason: UnsettledReason) -> Self {
        Self {
            available: false,
            episode_count: None,
            extra_episodes: Vec::new(),
            episode_count_approximate: false,
            gate_refused: false,
            provider: None,
            unsettled: Some(reason),
            versions: Vec::new(),
        }
    }
}

/// Whether a cached response may be served as-is, or has to re-probe.
///
/// Two kinds of row self-heal rather than being served. A legacy
/// available row with no count predates count caching and re-probes
/// to populate it. An approximate row carries the gate-refused
/// fallback's count, which the detail fetch would have corrected —
/// serving it would make every later read, including a click trying
/// to confirm the cap, replay the same wrong number.
///
/// A negative row is usable while the provider whose verdict it is
/// can stand behind it — see [`negative_row_is_backed`]; its
/// `episode_count` is meaningless when there is no candidate.
///
/// Either way a row is served only on the word of a provider the
/// state walks ([`row_is_walked`]).
fn cache_hit_is_usable(state: &AppState, parsed: &AvailabilityResponse) -> bool {
    if !row_is_walked(state, parsed) {
        return false;
    }
    if !parsed.available {
        return negative_row_is_backed(state, parsed);
    }
    parsed.episode_count.is_some() && !parsed.episode_count_approximate
}

/// Whether a negative row's provider can stand behind it now. A miss
/// is one provider's verdict — a miss does not fail over — so the
/// row is served only while that provider has been seen answering
/// and every provider ahead of it in the order is refusing: the
/// primary's negative through the primary's own outage would hide a
/// show the fallback carries, and the fallback's negative, written
/// during that outage, proves nothing about the primary once it is
/// back. The two halves ask the gate two different questions. The
/// row's own provider is asked whether it has recovered — seen
/// answering since it last failed: its breaker closed by a success,
/// or never opened, no failure since, and no pause running —
/// because past the cooldown the breaker refuses nobody while
/// nothing has yet answered, and a negative served on that alone
/// would short-circuit the very probe whose trial could fail over;
/// and because the rows a recovered gate serves make no request, so
/// a failure run short of the threshold could never grow past it
/// while they were served. The providers ahead are asked whether they refuse — an open
/// breaker or a running rate-limit pause, either of which sends a
/// walk asked now to the next provider — so a half-open primary lets
/// the fallback's row yield to the trial. Otherwise the row is not
/// served and the probe runs again. A negative that names no
/// provider, or one the state no longer walks, has nobody to stand
/// behind it ([`row_is_walked`]).
fn negative_row_is_backed(state: &AppState, parsed: &AvailabilityResponse) -> bool {
    let order = state.providers.order();
    let Some(provider) = parsed.provider else {
        return false;
    };
    let Some(position) = order.iter().position(|p| *p == provider) else {
        return false;
    };
    let gate = |p: &crate::scraper::provider::ProviderId| state.providers.gate(*p);
    gate(&provider).is_recovered() && order[..position].iter().all(|p| gate(p).is_refusing())
}

/// Whether a row is the verdict of a provider the state walks. A row
/// is one provider's word on its own catalogue — its cap counted in
/// its numbering, its miss a search of its listing — and a provider
/// the app no longer walks stands behind none of it: its negative
/// would hide a show the walked providers carry, its positive would
/// gate the strip with another catalogue's cap. A row that names no
/// provider predates attribution and is nobody's verdict. Neither is
/// served; the probe runs again.
fn row_is_walked(state: &AppState, parsed: &AvailabilityResponse) -> bool {
    parsed
        .provider
        .is_some_and(|p| state.providers.order().contains(&p))
}

/// What a native resolve learned about a show, for its availability
/// row: a stream served, or a clean miss, and whose verdict it is.
#[derive(Debug, Clone, Copy)]
pub struct ResolveVerdict<'a> {
    /// Whether the resolve served a stream.
    pub available: bool,
    /// The provider that served it, or whose clean miss it is.
    pub provider: Option<crate::scraper::provider::ProviderId>,
    /// The cap the resolve's own listing paid for, when it served.
    pub episode_cap: Option<u32>,
    /// The listing's fractional extras, when it served.
    pub extra_tags: &'a [String],
    /// The pick's versions, when it served one with several.
    pub versions: &'a [crate::commands::play_native_versions::VersionInfo],
    /// The version that served, when the pick has several: it carries
    /// the mode, whatever an earlier probe sampled.
    pub played: Option<crate::commands::play_native_versions::VersionKind>,
    /// Whether the listing ran past Kitsu's count with no reading of
    /// the rows past it ([`crate::commands::play_native::PickedShow::unread_past`]):
    /// its cap and extras are no answer about the count.
    pub count_unread: bool,
}

impl<'a> ResolveVerdict<'a> {
    /// A stream served by `provider`, from a listing with this cap
    /// and these extras.
    #[must_use]
    pub fn served(
        provider: crate::scraper::provider::ProviderId,
        episode_cap: Option<u32>,
        extra_tags: &'a [String],
    ) -> Self {
        Self {
            available: true,
            provider: Some(provider),
            episode_cap,
            extra_tags,
            versions: &[],
            played: None,
            count_unread: false,
        }
    }

    /// The verdict naming the served pick's `versions`.
    #[must_use]
    pub fn with_versions(
        mut self,
        versions: &'a [crate::commands::play_native_versions::VersionInfo],
    ) -> Self {
        self.versions = versions;
        self
    }

    /// The verdict whose listing's rows past Kitsu's count were not
    /// read ([`Self::count_unread`]).
    #[must_use]
    pub fn with_count_unread(mut self, count_unread: bool) -> Self {
        self.count_unread = count_unread;
        self
    }

    /// The verdict naming the version that served.
    #[must_use]
    pub fn with_played(
        mut self,
        played: Option<crate::commands::play_native_versions::VersionKind>,
    ) -> Self {
        self.played = played;
        self
    }

    /// A clean miss — every search completed and nothing matched —
    /// from the provider that answered it.
    #[must_use]
    pub fn missed(provider: Option<crate::scraper::provider::ProviderId>) -> Self {
        Self {
            available: false,
            provider,
            episode_cap: None,
            extra_tags: &[],
            versions: &[],
            played: None,
            count_unread: false,
        }
    }
}

/// What a writer of an availability row reads before its network
/// work, in one snapshot under the row's lock: the row's refresh
/// generation, which says whether the writer still owns the row at
/// all ([`crate::commands::availability_refresh`]); how many
/// positive rows had been written for it, and how many replays
/// served from its standing row, which together say whether the
/// positive row standing at the write is one the walk set out from;
/// and the provider the row remembers, for the walk to start from.
/// A positive stamp is two steps — the row into the cache, then the
/// count — and every writer holds the row across both, so a reader
/// that takes the row sees the finished stamp or none of it, never
/// the new row with the count from before it; read apart, a walk
/// could set out from a row whose count it had not seen and have its
/// own full-walk miss refused as though the row had appeared behind
/// its back.
///
/// A negative is measured against the row as it stands when it is
/// written, under the lock, not the row the walk set out from. The
/// walk saw the row that stood when it began — the provider to
/// start from was read from it, and the affinity rules decided what
/// that provider's denial or silence made of the verdict. A positive
/// row written while the walk was out it never saw: a resolve, a
/// probe or a replay stamped it, none of which is a cache-bypassing
/// refresh, so the generation guard lets the write through. That
/// includes a row written again exactly as it stood — the same
/// provider proving the show again, with the same cap — which is
/// proof newer than the miss that the row's bytes cannot show and
/// the count of positive writes can. Such a row proved the show
/// playable on evidence the walk did not weigh, so it stands, and
/// the negative is the verdict the caller sees and nothing more.
/// Written, it would hide the stream for its whole lifetime and take
/// the affinity with it — every walk after it starting from the
/// primary and ending on the same miss.
///
/// A success is measured the same way, against the rows written:
/// two resolves out at once from the same snapshot can both
/// succeed, and when one has failed over and written the fallback's
/// row with the cap its listing paid for, the other's later success
/// through the primary is older evidence than that row — written
/// over it, it would gate the episodes just proven playable and
/// send the next operation back to the slower provider. A replay
/// served meanwhile is proof a stream played, which holds a miss
/// back, but not a row written, so it leaves a later success free
/// to write its own.
#[derive(Debug, Clone, Copy)]
pub struct RowAtStart {
    generation: u64,
    positives: u64,
    replays: u64,
    remembered: Option<crate::scraper::provider::ProviderId>,
}

impl RowAtStart {
    /// The row as it stands, read before any network work, under the
    /// row's lock. With no Kitsu id there is no row: both counts
    /// zero, nothing remembered.
    pub async fn read(state: &AppState, kitsu_id: Option<&str>, mode: &str) -> Self {
        let Some(id) = kitsu_id.filter(|s| !s.is_empty()) else {
            return Self {
                generation: 0,
                positives: 0,
                replays: 0,
                remembered: None,
            };
        };
        let key = cache_key(id, mode);
        let _held = state
            .availability_refreshes
            .for_row(&key)
            .lock_owned()
            .await;
        Self {
            generation: state.availability_refreshes.generation(&key),
            positives: state.availability_refreshes.positives(&key),
            replays: state.availability_refreshes.replays(&key),
            remembered: cached_provider(state, id, mode),
        }
    }

    /// The provider a positive row remembered as the walk set out —
    /// the one it starts from — read in the same snapshot as the
    /// counts.
    #[must_use]
    pub fn remembered(&self) -> Option<crate::scraper::provider::ProviderId> {
        self.remembered
    }

    /// Whether a positive row was written since the start, and
    /// stands — one the walk never saw, even if it reads as the row
    /// it set out from — so that a later success is not written over
    /// it. Asked under the row lock.
    fn positive_row_written_since(&self, state: &AppState, row: &str) -> bool {
        positive_row_stands(state, row)
            && state.availability_refreshes.positives(row) != self.positives
    }

    /// Whether a positive stands now that the walk never weighed: a
    /// row written since the start, or a replay served from the
    /// standing row since — so that a negative is not written over
    /// it. Asked under the row lock.
    fn positive_row_appeared_since(&self, state: &AppState, row: &str) -> bool {
        self.positive_row_written_since(state, row)
            || (positive_row_stands(state, row)
                && state.availability_refreshes.replays(row) != self.replays)
    }
}

/// Stamp the availability row with a native resolve's verdict,
/// guarded against a refresh that answered while the resolve was in
/// flight: `at_start` was read before the resolve ([`RowAtStart`]),
/// and a write over a newer answer would disable (or falsely enable)
/// a show the user was just told about. Every path that resolves
/// natively — the embedded player, a download, a range, a handoff —
/// records through here, so a positive row names the provider that
/// served the show and the next operation for it starts there.
///
/// A served resolve with a cap writes the full row for sub: the cap
/// is exact for sub, and dropping it would evict an exact row into a
/// count-less one for the whole TTL. A dub resolve only proves the
/// requested episode has an English embed, so the dub row stays
/// boolean and self-heals via the next mode-aware probe. The row
/// takes the ongoing TTL either way, status being unknown here. A
/// resolve whose listing's rows past Kitsu's count no reading reached
/// (`count_unread`) writes no count, and leaves a row with an exact
/// count from the provider that served as it stands, lifetime and all
/// ([`super::availability_unread`]).
///
/// A clean miss is measured against the row that stands when it is
/// written, as the probe's is: two resolves for the same show can be
/// out at once with the same generation, and when one has failed
/// over and stamped the fallback's success while the other was
/// still waiting on the primary, the late miss is not written over
/// it — nor is the other's later success, older evidence than the
/// row that stands ([`RowAtStart`]).
pub async fn stamp_after_native(
    state: &AppState,
    kitsu_id: Option<&str>,
    mode: &str,
    at_start: &RowAtStart,
    verdict: ResolveVerdict<'_>,
) {
    let Some(id) = kitsu_id.filter(|s| !s.is_empty()) else {
        return;
    };
    let row = cache_key(id, mode);
    crate::commands::availability_refresh::with_row_if_ours(
        &state.availability_refreshes,
        &row,
        at_start.generation,
        false,
        || {
            if verdict.available {
                if at_start.positive_row_written_since(state, &row) {
                    return;
                }
            } else if at_start.positive_row_appeared_since(state, &row) {
                return;
            }
            if !verdict.available {
                write_cache(state, id, mode, false, verdict.provider);
                return;
            }
            // A count no reading reached leaves a counted row as it
            // stands, lifetime and all.
            let standing = verdict.count_unread
                && super::availability_unread::standing_counted_row(state, &row, verdict.provider)
                    .is_some();
            match super::availability_unread::served_stamp(
                verdict.episode_cap,
                mode == "dub",
                verdict.count_unread,
                standing,
            ) {
                super::availability_unread::ServedStamp::Count(cap) => {
                    write_cache_full(
                        state,
                        id,
                        mode,
                        None,
                        &AvailabilityResponse {
                            available: true,
                            episode_count: Some(cap),
                            // Derived from the listing the resolve paid
                            // for — the same tags a fractional play
                            // matches against number2, so they outrank
                            // whatever an older probe stored.
                            extra_episodes: verdict.extra_tags.to_vec(),
                            episode_count_approximate: false,
                            gate_refused: false,
                            unsettled: None,
                            provider: verdict.provider,
                            versions: with_modes_kept(
                                state,
                                &row,
                                verdict.versions,
                                verdict.played,
                            ),
                        },
                    );
                }
                // A positive whose cap is no evidence of the count (a
                // dub, none listed, or rows past Kitsu's count no
                // reading reached) writes the boolean row, the versions
                // kept.
                super::availability_unread::ServedStamp::NoCount => write_boolean_row(
                    state,
                    id,
                    mode,
                    true,
                    verdict.provider,
                    with_modes_kept(state, &row, verdict.versions, verdict.played),
                ),
                super::availability_unread::ServedStamp::Standing => {}
            }
        },
    )
    .await;
}

/// `versions` as a resolve names them, each given what the row standing
/// at `row` says of its mode where the resolve says nothing: a play
/// knows which version it played, not which carry the mode, and a stamp
/// must not take back what the probe learned of each.
fn with_modes_kept(
    state: &AppState,
    row: &str,
    versions: &[crate::commands::play_native_versions::VersionInfo],
    played: Option<crate::commands::play_native_versions::VersionKind>,
) -> Vec<crate::commands::play_native_versions::VersionInfo> {
    let standing: Vec<crate::commands::play_native_versions::VersionInfo> =
        meta_cache_get(&state.cache_pool, row)
            .ok()
            .flatten()
            .and_then(|body| serde_json::from_str::<AvailabilityResponse>(&body).ok())
            .map(|r| r.versions)
            .unwrap_or_default();
    versions
        .iter()
        .map(|v| crate::commands::play_native_versions::VersionInfo {
            // The version that served carries the mode, whatever a
            // probe's sample said of it.
            has_mode: (played == Some(v.kind))
                .then_some(true)
                .or(v.has_mode)
                .or_else(|| {
                    standing
                        .iter()
                        .find(|s| s.show_key == v.show_key)
                        .and_then(|s| s.has_mode)
                }),
            ..v.clone()
        })
        .collect()
}

/// Stamp a served replay's positive, where no positive row stands.
/// The resolution cache outlives the availability row — a cached
/// stream replays for days after an airing show's positive row has
/// expired — so without this the next probe would start from the
/// primary, whose clean miss could hide a stream the cache just
/// served. A replay learns nothing new about the listing beyond the
/// mode of the version it served, and nothing about the provider it
/// came through — the CDN answered for a URL resolved when that
/// provider last served — so a standing positive row is left as it
/// is but for that one mode, whichever provider proved it, lifetime
/// included: written back as a fresh row it would take a fresh
/// lifetime, and an ongoing show's exact
/// cap, replayed daily, would never expire into the reprobe that
/// learns its new episodes. Only where no positive row stands does
/// the replay write the boolean positive row a served resolve
/// without a cap writes, naming `provider` — count-less, so the next
/// look at the show reprobes it for its cap, starting from that
/// provider. Where a positive row stands, the replay is counted as
/// served from it: a stream just played, which a miss out at the
/// time never weighed, so a resolve that set out before the replay
/// does not write its miss over the row — while a later success,
/// which weighs against rows written, is still free to write its
/// own. The row's lifetime, and all of it but the served version's
/// mode, are untouched. Under the same refresh guard as a resolve's
/// stamp.
///
/// `played` is the version the cached resolution served: the replay
/// has just proved a stream of it in this mode, so where the replay
/// writes the row it says that version carries the mode, as a fresh
/// play's stamp does. Where a positive row stands, that one fact is
/// merged into it ([`merge_served_mode`]) — the row's window and every
/// other field kept, a mode it already knows never taken back.
///
/// [`merge_served_mode`]: crate::commands::availability_served_mode::merge_served_mode
///
/// The row is read inside the lock, not on the way to it. A native
/// resolve stamping the same row is not a cache-bypassing refresh
/// and leaves the generation where it was, so a replay behind it
/// still passes the guard; read under the lock, the row it decides
/// on is the one that stands when it writes — the resolve's, exact
/// cap and failed-over provider included, not the one it saw on its
/// way in.
pub async fn stamp_after_cache_hit(
    state: &AppState,
    kitsu_id: Option<&str>,
    mode: &str,
    at_start: &RowAtStart,
    provider: crate::scraper::provider::ProviderId,
    versions: &[crate::commands::play_native_versions::VersionInfo],
    played: Option<crate::commands::play_native_versions::VersionKind>,
) {
    let Some(id) = kitsu_id.filter(|s| !s.is_empty()) else {
        return;
    };
    let row = cache_key(id, mode);
    crate::commands::availability_refresh::with_row_if_ours(
        &state.availability_refreshes,
        &row,
        at_start.generation,
        false,
        || {
            if positive_row_stands(state, &row) {
                state.availability_refreshes.note_replay(&row);
                crate::commands::availability_served_mode::merge_served_mode(
                    state, &row, versions, played,
                );
            } else {
                write_boolean_row(
                    state,
                    id,
                    mode,
                    true,
                    Some(provider),
                    with_modes_kept(state, &row, versions, played),
                );
            }
        },
    )
    .await;
}

/// Whether a positive row stands for the key — one within its
/// lifetime that says the show is carried, on the word of a walked
/// provider ([`row_is_walked`]). A row nobody walked is never served,
/// so nothing is weighed against it.
fn positive_row_stands(state: &AppState, row: &str) -> bool {
    meta_cache_get(&state.cache_pool, row)
        .ok()
        .flatten()
        .is_some_and(|body| {
            serde_json::from_str::<AvailabilityResponse>(&body)
                .is_ok_and(|parsed| parsed.available && row_is_walked(state, &parsed))
        })
}

/// Inputs for the batch `availability_cached` lookup — a list of
/// Kitsu ids and the mode to read cached results for. Skips the
/// network entirely; only returns entries that already have a value
/// in `meta_cache`.
#[derive(Debug, Clone, Deserialize)]
pub struct AvailabilityBatchArgs {
    /// Kitsu ids to look up, each trimmed; a value that is not an id
    /// ([`crate::kitsu_id`]) is dropped and so reads no row. Mode
    /// disambiguates sub/dub since a title may exist in one and not
    /// the other.
    #[serde(deserialize_with = "crate::kitsu_id::deserialize_list")]
    pub kitsu_ids: Vec<String>,
    /// `"sub"` or `"dub"` — selects which cached probe to read.
    pub mode: String,
}

/// Cached-only batch response — `kitsu_id → available` for the rows
/// the cache had a value for. Missing ids are absent from the map and
/// the caller should render them optimistically while the lazy probe
/// fills in.
#[derive(Debug, Clone, Serialize)]
pub struct AvailabilityBatchResponse {
    /// Map of kitsu_id → available. Only contains entries that have
    /// a cached value; missing ids should be treated as "unknown,
    /// render normally" by the caller.
    pub cached: HashMap<String, bool>,
    /// Map of kitsu_id → the provider's playable episode count, for cached
    /// rows where the probe found the show. Lets the home Continue
    /// Watching card cap pickNextEpisode against the provider's true count
    /// instead of Kitsu's (sometimes stale) announced total — the
    /// detail page reads the same value via an inline probe, so home
    /// and detail end up computing the same defaultEpisode without an
    /// extra round trip from the home strip.
    ///
    /// Absent for rows cached as unavailable, and for legacy rows
    /// from before episode_count was written into the cache body
    /// (those parse with episode_count=None).
    #[serde(default)]
    pub playable_episode_counts: HashMap<String, u32>,
}

pub(crate) fn cache_key(kitsu_id: &str, mode: &str) -> String {
    // v24: a listing whose rows past Kitsu's count no reading reached
    //      gives no count, and does not write over one that stands. A
    //      v23 row may hold such a listing's whole count as its cap,
    //      for a play or probe that could not read whether the rows
    //      past the count are extras; a row from a reading that refused
    //      them holds the same shape, so re-keying re-probes.
    // v23: the appended-extras rule checks the rows past the count
    //      against every aired TV entry Kitsu relates the show to,
    //      whatever the role, and refuses an entry of any role airing
    //      now; a stitched listing is never read for them. A v22 row's
    //      cap and extras may come from a reading the narrower rule
    //      proved and this one refuses, and nothing in a row says which
    //      rule stamped it, so re-keying re-probes.
    // v22: a season the provider lists twice — an "(Uncensored)" copy
    //      and a plain one — is one pick with two versions, and the
    //      answer names them and says of each whether it carries the
    //      mode. A row from before cannot say whether its show has
    //      versions, and served for its lifetime it would hide the
    //      choice; re-keying re-probes. Keyed past the v21 and v20
    //      below, whose rows name no versions either.
    // v21: a recap tagged at a continuation's offset ("12.5" under
    //      offset 12) advertises as the entry's 0.5, and a stitched
    //      split's such recap as the tag between its parts ("12.5",
    //      not "24.5"), a second recap between the same episodes with
    //      a tag of its own ("12.75"); rows from before carry the old
    //      extras, and the probe that replaces them puts a history
    //      row's stamp from then right.
    // v20: the extras a provider appends after a show's last episode
    //      are read as extras beside it when Kitsu and AniList end the
    //      show at its count. A v19 row for such a show holds the
    //      listing's whole count as its cap (Noragami's fourteen, its
    //      two OADs as episodes 13 and 14); nothing in a row says
    //      which reading stamped it, so re-keying re-probes.
    // v19: a dated entry with a count takes only a hit a name vouches
    //      for, and a dated entry's hit its own page files under
    //      Hentai is never a pick. A v18 row for such an entry can be
    //      a positive for a show the provider does not list, capped at
    //      a same-year show's count (HenSemi (TV) holding Medaka Box
    //      Abnormal's 12); nothing in a row says which hit it was
    //      stamped from, so re-keying re-probes.
    // v18: an entry with a count and no year takes only a hit the
    //      title searched or one of its own names vouches for. A v17
    //      row for such an entry can be a positive for a show the
    //      provider does not list, capped at another show's count
    //      whose count merely fit (Mo Du Zhe holding Girlfriend,
    //      Girlfriend's 12); nothing in a row says whether its entry
    //      was dated, so re-keying re-probes.
    // v17: a countless entry with a year takes only a hit its year or
    //      its name vouches for, and the probe head's evidence reaches
    //      as far as its five. A v16 row can be a positive for a
    //      subtitled sequel the provider does not list, capped at its
    //      series' count — taken without a count on a year the filter
    //      only tolerated — or a verdict picked while a broad listing
    //      went uncut, its completing sibling past the evidence
    //      window; re-keying re-probes.
    // v16: names vouch for a countless or single-video pick. An
    //      entry with no count or year took the provider's first hit,
    //      and a single video was picked on its count of one: a v15
    //      row can be a positive for a show the provider does not
    //      list, capped at another show's count (Dragon Ball Super:
    //      Ginga Patrol holding Dragon Ball Super's 131); re-keying
    //      re-probes.
    // v15: the entry's titles rule the pick. A v14 row can be a
    //      clean miss persisted for a show the picker now finds
    //      (Stone Ocean, TV on Kitsu and ONA on hianime), or a
    //      positive capped at a sibling's count (Attack on Titan
    //      Season 3 capped at Part 2's 10); re-keying re-probes.
    // v14: an entry the provider splits into several shows is probed
    //      against all of them, its cap counted across the parts in
    //      Kitsu's numbering. A v13 row for such a show carries one
    //      part's count (Steel Ball Run: the 2nd Stage's 2 where the
    //      entry has 3) and holds it for the row's lifetime; re-keying
    //      re-probes on the next page open.
    // v13: a negative row names the provider whose clean miss it is
    //      and is served only while that provider is answering — a
    //      miss does not fail over, so the row is one provider's
    //      verdict, and served through that provider's outage it hid
    //      a show the fallback carries for its whole lifetime. A v12
    //      negative names nobody and would keep doing exactly that;
    //      re-keying re-probes.
    // v12: the picker gained the subtype disproof — Kitsu's subtype
    //      rejects format-mismatched candidates. A v11 verdict reached
    //      without it can be available:true on a pool whose only
    //      matches the new picker refuses (movie badges against a
    //      non-movie subtype), and the cached row short-circuits the
    //      probe for the whole positive TTL; re-keying re-probes.
    // v11: the cap moved to per-entry (Kitsu) numbering. A v10 row
    //      probed against a continuation cour carries the provider's
    //      cumulative number as its count (TYBW part four: 42 for a
    //      2-episode entry) and would keep unlocking phantom strip
    //      tiles for the whole positive TTL; re-keying re-probes.
    // v10: the provider changed (allanime → anidb.app). A verdict
    //      answers "is this on the provider", so every row probed
    //      against the old catalogue is about a question nobody asks
    //      anymore: it can say yes for shows anidb.app doesn't carry
    //      (the click then errors) and no for shows it does (the
    //      title stays hidden), for up to the 30-day positive TTL.
    //      Re-keying orphans them all; the stale rows age out of
    //      SQLite by TTL as usual.
    // v9: gate-refused probes now mark their count approximate so it
    //     re-probes instead of being served as exact. v8 rows written
    //     by that path carry the degraded count with no flag, and
    //     `serde(default)` reads them as exact — they would keep
    //     resolving a phantom final episode for up to 30 days.
    //     Bumping forces them to re-probe.
    // v8: negative TTLs became premiere-aware (negative_ttl_for +
    //     the airing-cache seed), but check_availability
    //     short-circuits on cached false rows before either runs. A
    //     v7 negative written for an unreleased show carries the old
    //     flat 7-day window and would keep the title blocked well
    //     past its premiere; bumping re-probes those rows so the new
    //     TTL applies to existing installs too.
    // v7: resolver now also queries Kitsu's abbreviatedTitles as
    //     fallback titles. Shows whose canonical/localized titles only
    //     matched a wrong allmanga sibling (Yu-Gi-Oh! 5D's: every
    //     official title resolves a 1-ep OVA; only an abbreviated alias
    //     finds the 154-ep series) were cached available:false under v6.
    //     check_availability short-circuits on cached false rows before
    //     the picker runs, so those rows would keep pruning the show for
    //     the full negative TTL; bumping forces a re-probe with the new
    //     resolver input so list/detail filters self-heal.
    // v6: picker gained a year-closeness tie-break for same-episode-count
    //     sibling cours (Stone Ocean Part 1 & Part 2, both 12 eps). A v5
    //     row probed under the old order-dependent pick could carry the
    //     wrong cour's episode_count / extra_episodes; bumping evicts
    //     them so list/detail availability re-derives from the new pick.
    // v5: picker now reads allmanga's `type`/`status`/`episodeCount`
    //     (b629127) and hard-rejects same-year
    //     OVA/Movie/Special and planned-count divergence. v4 rows
    //     could be true-positive "available" for a 1-ep OVA that the
    //     new picker correctly rejects — bumping forces a re-probe
    //     through the disambiguator so list filters self-heal.
    // v4: picker uses Kitsu start_date year as the primary tie-break
    //     and rejects pools outside max(3, expected*10%) ep-count
    //     distance (see pick_by_ep_count_v2). v3 rows were decided by
    //     the old ep-count-only picker and could be true-positive
    //     "available" for the wrong allmanga show — bumping evicts
    //     them so list filters re-derive from the new picker.
    // v3: SHOW_GQL fix — availableEpisodesDetail is a scalar, not
    //     an object; the v2-rollout query subselected `{ sub dub }`
    //     and got empty lists back, so episode_count fell through
    //     to the buggy COUNT path. Bumping again so v2 rows
    //     (already-poisoned with the count) get superseded.
    // v2: episode_count switched from "len of availableEpisodes list"
    //     to "max integer episode" via fetch_show.
    let m = if mode == "dub" { "dub" } else { "sub" };
    format!("availability:v24:{kitsu_id}:{m}")
}

/// Reuses the play path's `pick_title_and_index` so the cache
/// reflects the SAME disambiguation play does. Returns `available:
/// true` only when the picker lands on a specific candidate (without
/// a Kitsu episode_count to disambiguate, only one the title
/// searched, its own year or the entry's name vouches for — the rule
/// play applies, so the signal matches what play would do at the
/// same time).
///
/// # Errors
/// - Network errors from the underlying provider search propagate;
///   the frontend can fall back to the lazy click path.
pub async fn check_availability(
    state: &AppState,
    args: &AvailabilityArgs,
) -> Result<AvailabilityResponse> {
    let mode = if args.mode == "dub" { "dub" } else { "sub" };
    // Read before any network work, compared again before the write:
    // a refresh answering in between means this lookup's row is the
    // stale one (see `AvailabilityRefreshes`), and a positive row
    // appearing in between is one a negative is not written over.
    let at_start = RowAtStart::read(state, args.kitsu_id.as_deref(), mode).await;

    // Cache short-circuit. Skipped when no kitsu_id is supplied.
    // Which rows may be served is [`cache_hit_is_usable`]'s call —
    // legacy count-less rows and gate-refused approximate counts both
    // re-probe so they self-heal.
    if let Some(id) = args
        .kitsu_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .filter(|_| !args.bypass_cache)
    {
        let key = cache_key(id, mode);
        if let Ok(Some(body)) = meta_cache_get(&state.cache_pool, &key) {
            if let Ok(parsed) = serde_json::from_str::<AvailabilityResponse>(&body) {
                if cache_hit_is_usable(state, &parsed) {
                    return Ok(parsed);
                }
            }
        }
    }

    // A positive row the probe does not serve — a resolve's count-less
    // stamp, an approximate count — still names the provider that
    // proved the show, and the reprobe starts from it: from the
    // primary, its clean miss would end the walk and overwrite that
    // proof with a negative.
    let remembered = at_start.remembered();

    // Funnel through the native walk so availability honours the
    // same alias recovery and disambiguation play uses. The pick's
    // episodes probe already pays for the full list, so the cap comes
    // back exact and free — the second detail fetch, its gate-refused
    // fallback and the approximate-count provenance all retire with
    // the provider.
    let prio = if args.background {
        crate::scraper::gate::ScrapePriority::Background
    } else {
        crate::scraper::gate::ScrapePriority::Interactive
    };
    // One request, one outcome: the pick and the mode check are two
    // stages of a single probe, so each provider's breaker hears
    // their combined verdict once — recording the pick's success
    // first would reset the failure run every time a show's servers
    // endpoint is down, and the breaker would never open. The runner
    // records it under the play path's mapping: answered verdicts
    // (the clean miss included) are health, weather is distress, a
    // gate refusal records nothing.
    let tail = super::kitsu_tail::KitsuTail::for_entry(state, args.kitsu_id.as_deref());
    let mut attempt = ProbeAttempt {
        args,
        mode,
        tail: tail
            .as_ref()
            .map(|t| t as &dyn super::play_native_appended::TailSource),
        answered_by: None,
    };
    let probed = crate::commands::providers::run_from(state, remembered, prio, &mut attempt).await;
    let (available, episode_count, extra_episodes, provider, versions, count_unread) = match probed
    {
        Ok(attempted) => {
            let (p, present, modes) = attempted.value;
            let Some(present) = present else {
                // Every row the mode search touched was missing — or,
                // for a show with versions, no version carries the mode
                // and one never answered. The walk moves on from that
                // answer and surfaces the unknown verdict itself when
                // nobody answers; kept
                // for the day an inconclusive answer reaches here
                // regardless — nothing about this mode was said,
                // which is not absence, so persist nothing.
                return Ok(AvailabilityResponse::unsettled(UnsettledReason::Unanswered));
            };
            // The listing is at hand, whatever the mode answered: a
            // history row's stamp an earlier version wrote with a
            // stitched row's former tag reads right before its Continue
            // card is clicked, and before the verdict is cached — the
            // next probe that takes it fetches no listing. Every version
            // of the pick keys stamps of its own, each put right from its
            // own listing.
            for (hit, episodes) in p.copies() {
                crate::commands::numbering_offset::retag_from_listing(
                    state,
                    &crate::scraper::provider::ShowKey::new(attempted.provider, hit.slug.clone())
                        .to_string(),
                    episodes,
                    p.unread_past,
                );
            }
            if present {
                let (cap, extras) = super::availability_unread::probe_counts(&p);
                (
                    true,
                    cap,
                    extras,
                    Some(attempted.provider),
                    crate::commands::play_native_versions::named_versions(attempted.provider, &p)
                        .0
                        .into_iter()
                        .map(|v| crate::commands::play_native_versions::VersionInfo {
                            has_mode: modes
                                .iter()
                                .find(|(k, _)| *k == v.kind)
                                .and_then(|(_, m)| *m),
                            ..v
                        })
                        .collect(),
                    p.unread_past.is_some(),
                )
            } else if attempted.past_undenied_affinity {
                // The provider answered absence, but past the
                // provider a positive row remembers, which has not
                // denied the show — unreachable, or heard from with
                // an episode dead end: like the clean miss reached
                // the same way, it is the verdict the caller sees and
                // proves nothing about the audio the row's provider
                // listed.
                // Persisted, it would be backed by a healthy primary
                // and outlive the row's provider's recovery. Surface
                // it as the unknown it is, persist nothing; the row
                // stands.
                return Ok(AvailabilityResponse::unsettled(
                    UnsettledReason::PastAffinity,
                ));
            } else {
                // The provider ANSWERED absence for this mode.
                // Cacheable, like the clean search miss, and named as
                // that provider's: the read side serves the row only
                // while that provider is answering and every provider
                // ahead of it is refusing — so a fallback's absence stands
                // through the primary's outage and not a moment past
                // it, since it proves nothing about the primary.
                (
                    false,
                    None,
                    Vec::new(),
                    Some(attempted.provider),
                    Vec::new(),
                    false,
                )
            }
        }
        // Clean miss: the only verdict that proves absence — flows
        // into the cache write below, naming the provider whose miss
        // it is, under the same read rule.
        Err(ne) if ne.clean_miss => (
            false,
            None,
            Vec::new(),
            attempt.answered_by,
            Vec::new(),
            false,
        ),
        // An answered dead end — a pool refused on how its hits are
        // named, dead listings on a healthy provider, a mode nobody
        // answered for — proves nothing about the show's absence and is
        // no failure of the probe: unsettled, persisting nothing, and
        // saying which. A refusal by name says the catalogue lists
        // nothing named for the entry; the rest say nothing.
        Err(ne) if matches!(ne.error, crate::error::AniError::NoResults) => {
            return Ok(AvailabilityResponse::unsettled(dead_end_reason(
                state,
                args,
                mode,
                &at_start,
                ne.refused_by_name,
            )));
        }
        // Weather (transport failures, upstream refusals, a refused
        // background admit): surface typed, persist nothing.
        Err(ne) => return Err(ne.error),
    };
    let episode_count_approximate = false;
    let gate_refused = false;

    if let Some(id) = args.kitsu_id.as_deref().filter(|s| !s.is_empty()) {
        // A refresh that answered while this was out has already put a
        // cache-skipping reading in the row. Writing over it would
        // reinstate the count this lookup read THROUGH the cache to
        // get, and hold it for the row's whole TTL.
        //
        // Held from before that reading until after the write, so it
        // still describes the row when the write lands. Reading it and
        // acting on it are otherwise separated by the airing fetch
        // below, and a refresh can begin and finish inside that gap.
        let row = cache_key(id, mode);
        // A count no reading reached, beside a counted row that stands,
        // writes nothing: answered before the row is held, so a
        // cache-skipping refresh that writes nothing takes the row from
        // none of the writers that set out before it.
        if let Some(stood) = count_unread
            .then(|| super::availability_unread::standing_counted_row(state, &row, provider))
            .flatten()
        {
            return Ok(stood);
        }
        let Some(_writing) = hold_if_still_ours(
            &state.availability_refreshes,
            &row,
            at_start.generation,
            args.bypass_cache,
        )
        .await
        else {
            return Ok(AvailabilityResponse {
                available,
                episode_count,
                extra_episodes,
                episode_count_approximate,
                gate_refused,
                unsettled: None,
                provider,
                versions: versions.clone(),
            });
        };
        // A verdict is measured against the row as it stands now,
        // under the lock, not the row the probe set out from: a
        // positive row written while the walk was out stands, and
        // the verdict is the caller's and nothing more — a negative
        // yields to a replay served meanwhile as well (see
        // `RowAtStart`).
        if !available && at_start.positive_row_appeared_since(state, &row) {
            return Ok(AvailabilityResponse::unsettled(UnsettledReason::Superseded));
        }
        if available && at_start.positive_row_written_since(state, &row) {
            return Ok(AvailabilityResponse {
                available,
                episode_count,
                extra_episodes,
                episode_count_approximate,
                gate_refused,
                unsettled: None,
                provider,
                versions: versions.clone(),
            });
        }
        // A count no reading reached does not write over one that
        // stands: the row, and its count, are the answer. Asked again
        // under the lock for a row written since the check above.
        if let Some(stood) = count_unread
            .then(|| super::availability_unread::standing_counted_row(state, &row, provider))
            .flatten()
        {
            return Ok(stood);
        }
        seed_airing_for_negative(state, id, available, args.status.as_deref()).await;
        write_cache_full(
            state,
            id,
            mode,
            args.status.as_deref(),
            &AvailabilityResponse {
                available,
                episode_count,
                extra_episodes: extra_episodes.clone(),
                episode_count_approximate,
                // Not cached: it describes this request, not the show.
                gate_refused: false,
                unsettled: None,
                // Cached: a cache hit must still say whose catalogue
                // carries the show.
                provider,
                versions: versions.clone(),
            },
        );
    }

    Ok(AvailabilityResponse {
        available,
        episode_count,
        extra_episodes,
        episode_count_approximate,
        gate_refused,
        unsettled: None,
        provider,
        versions: versions.clone(),
    })
}

/// Why an answered dead end settled nothing. A refusal by name says the
/// catalogue lists nothing named for the show — unless a positive row
/// appeared while the probe was out, a play having served the show
/// meanwhile, which a refusal no more outranks than a clean miss does:
/// the answer is then the superseded unknown. Read without the row's
/// lock, since nothing is written.
fn dead_end_reason(
    state: &AppState,
    args: &AvailabilityArgs,
    mode: &str,
    at_start: &RowAtStart,
    refused_by_name: bool,
) -> UnsettledReason {
    let overtaken = args
        .kitsu_id
        .as_deref()
        .filter(|id| !id.is_empty())
        .is_some_and(|id| at_start.positive_row_appeared_since(state, &cache_key(id, mode)));
    match (refused_by_name, overtaken) {
        (true, false) => UnsettledReason::Refused,
        (true, true) => UnsettledReason::Superseded,
        (false, _) => UnsettledReason::Unanswered,
    }
}

/// The probe's provider half: pick the show, then measure how far
/// the requested mode reaches.
///
/// Both stages are the same request as far as the caller and the
/// breaker are concerned — the runner bounds them together under
/// [`RESOLVE_DEADLINE`], since the walk alone can spend tens of
/// seconds per alias against a slow provider, and a probe that
/// outlives the gate's half-open trial window is the overlap the
/// sanction chain forbids — and they produce a single outcome.
///
/// # Errors
/// The walk's own verdicts and the mode probe's transport failures.
/// `clean_miss` survives from the walk, so a persistable absence is
/// still persistable.
///
/// [`RESOLVE_DEADLINE`]: crate::commands::play_native_resolve::RESOLVE_DEADLINE
async fn probe_show<P: crate::scraper::provider::Provider + ?Sized>(
    client: &P,
    args: &AvailabilityArgs,
    mode: &str,
    tail: Option<&dyn super::play_native_appended::TailSource>,
) -> std::result::Result<ShowModes, crate::commands::play_native_resolve::NativeError> {
    let picked = pick_native_walk(
        client,
        &args.title,
        &args.alt_titles,
        args.episode_count,
        args.year,
        args.subtype.as_deref(),
        tail,
    )
    .await?;
    let fail = |error| crate::commands::play_native_resolve::NativeError {
        error,
        clean_miss: false,
        failed_at: None,
        refused_by_name: false,
    };
    let present = crate::commands::availability_mode::mode_present(client, &picked.episodes, mode)
        .await
        .map_err(fail)?;
    // Each version is asked too: the provider need not carry one mode
    // for every copy of a season.
    let modes = crate::commands::play_native_version_modes::modes_of_versions(
        client, &picked, present, mode,
    )
    .await
    .map_err(fail)?;
    let (picked, present) =
        crate::commands::play_native_version_modes::version_for_mode(picked, &modes, present);
    Ok((picked, present, modes))
}

/// What a probe found: the pick, playing a version that carries the
/// mode when one does; whether the show carries the mode; and whether
/// each of its versions does.
type ShowModes = (
    crate::commands::play_native::PickedShow,
    Option<bool>,
    Vec<(
        crate::commands::play_native_versions::VersionKind,
        Option<bool>,
    )>,
);

/// [`probe_show`] as an attempt the runner can move between
/// providers.
struct ProbeAttempt<'r> {
    args: &'r AvailabilityArgs,
    mode: &'r str,
    /// The evidence about the entry's last episodes, when the probe
    /// knows the entry.
    tail: Option<&'r dyn super::play_native_appended::TailSource>,
    /// The provider whose miss the walk returned — set by the walk,
    /// which knows whose verdict survived.
    answered_by: Option<crate::scraper::provider::ProviderId>,
}

#[async_trait::async_trait]
impl crate::commands::providers::Attempt for ProbeAttempt<'_> {
    type Output = ShowModes;

    async fn run(
        &mut self,
        provider: &dyn crate::scraper::provider::Provider,
    ) -> std::result::Result<Self::Output, crate::commands::play_native_resolve::NativeError> {
        probe_show(provider, self.args, self.mode, self.tail).await
    }

    fn missed_by(&mut self, provider: crate::scraper::provider::ProviderId) {
        self.answered_by = Some(provider);
    }

    /// A show found without the requested mode is a negative verdict:
    /// the walk owes the skipped providers their trial before it
    /// surfaces, as it does for a miss.
    fn is_negative(output: &Self::Output) -> bool {
        matches!(output.1, Some(false))
    }

    /// A mode nobody answered for — every sampled row a status that
    /// says nothing, stale ids the provider no longer serves — is
    /// inconclusive: it says nothing either way, and the walk moves
    /// on to the next provider.
    fn is_inconclusive(output: &Self::Output) -> bool {
        output.1.is_none()
    }
}

/// The provider a positive availability row remembers for
/// `(kitsu_id, mode)` — the one that proved the show playable — or
/// nothing: negative rows, rows from before the field, and rows the
/// cache does not hold remember no provider. A play starts from it.
#[must_use]
pub(crate) fn cached_provider(
    state: &AppState,
    kitsu_id: &str,
    mode: &str,
) -> Option<crate::scraper::provider::ProviderId> {
    let body = meta_cache_get(&state.cache_pool, &cache_key(kitsu_id, mode)).ok()??;
    let row: AvailabilityResponse = serde_json::from_str(&body).ok()?;
    row.available.then_some(row.provider).flatten()
}

/// Persist a boolean availability row with no count and no versions:
/// what a resolve's clean miss stamps ([`stamp_after_native`]). A
/// positive a resolve or a replay stamps without a count goes through
/// [`write_boolean_row`], which keeps the show's versions. Public for
/// the tests that seed rows; the next detail-page probe fills in a
/// count-less row.
pub fn write_cache(
    state: &AppState,
    kitsu_id: &str,
    mode: &str,
    available: bool,
    provider: Option<crate::scraper::provider::ProviderId>,
) {
    write_boolean_row(state, kitsu_id, mode, available, provider, Vec::new());
}

/// [`write_cache`] naming the show's `versions`: a positive stamp keeps
/// them, so the row still says the show has versions — and what was
/// learned of each one's mode — where its count is not known.
fn write_boolean_row(
    state: &AppState,
    kitsu_id: &str,
    mode: &str,
    available: bool,
    provider: Option<crate::scraper::provider::ProviderId>,
    versions: Vec<crate::commands::play_native_versions::VersionInfo>,
) {
    // Status unknown at this call site (play / download success /
    // failure paths). Use ongoing TTL — the next detail-page probe
    // will overwrite this row anyway, since check_availability's
    // self-heal kicks in for rows with episode_count=None. The
    // provider is kept: the row is the affinity the next play starts
    // from, and a boolean stamp that dropped it would send the next
    // episode back to a provider that may not carry the show.
    write_cache_full(
        state,
        kitsu_id,
        mode,
        None,
        &AvailabilityResponse {
            available,
            episode_count: None,
            extra_episodes: Vec::new(),
            episode_count_approximate: false,
            gate_refused: false,
            unsettled: None,
            provider,
            versions,
        },
    );
}

/// Pick the positive cache TTL based on Kitsu's airing status.
/// Returns the short (24h) TTL for shows likely to get new episodes
/// soon, and the long (30d) TTL only for definitively finished
/// shows. Unknown status falls back to the short TTL — better to
/// re-probe than to serve a stale cap.
fn positive_ttl_for(status: Option<&str>) -> u64 {
    match status {
        Some("finished") => AVAILABILITY_TTL_FINISHED_SECS,
        _ => AVAILABILITY_TTL_ONGOING_SECS,
    }
}

/// Pick the negative cache TTL. A flat 7-day negative is right for
/// finished shows (catalog adds are rare) but wrong around premieres:
/// a show probed unavailable the day before airing would stay hidden
/// from home/search for up to a week AFTER its first episode lands.
/// With the airing schedule available, expire the negative shortly
/// after the next scheduled airing instead:
///
///   - finished          → 7 days (unchanged);
///   - premiere known    → time until `next_airing_at` + 3h grace,
///     clamped to [1h, 7d] (a passed timestamp means the airing row
///     is stale or the episode just dropped — re-probe within 1h);
///   - not finished, no
///     schedule known    → 24h, same as the ongoing positive TTL.
fn negative_ttl_for(status: Option<&str>, next_airing_at: Option<u64>, now_epoch_s: u64) -> u64 {
    use crate::commands::availability_ttl::{FLOOR_SECS, NEGATIVE_GRACE_SECS as GRACE_SECS};
    if status == Some("finished") {
        return AVAILABILITY_TTL_NEGATIVE_SECS;
    }
    // The schedule stretch is only valid PRE-premiere: for a current
    // show, next_airing_at points at the FOLLOWING episode, and
    // stretching the negative to it would hide a catalog-lagged show
    // for almost a week after episode 1 appears. Current/unknown rows cap at the ongoing window.
    let pre_premiere = matches!(status, Some("unreleased" | "tba" | "upcoming"));
    let scheduled = match next_airing_at {
        // Already aired (or the airing row is stale) — re-probe soon.
        Some(at) if at <= now_epoch_s => FLOOR_SECS,
        Some(at) => {
            (at - now_epoch_s + GRACE_SECS).clamp(FLOOR_SECS, AVAILABILITY_TTL_NEGATIVE_SECS)
        }
        None => return AVAILABILITY_TTL_ONGOING_SECS,
    };
    if pre_premiere {
        scheduled
    } else {
        scheduled.min(AVAILABILITY_TTL_ONGOING_SECS)
    }
}

async fn seed_airing_for_negative(
    state: &AppState,
    kitsu_id: &str,
    available: bool,
    status: Option<&str>,
) {
    seed_airing_for_negative_with_base(state, kitsu_id, available, status, None).await;
}

/// [`seed_airing_for_negative`] with the AniList endpoint override
/// exposed for tests. Production passes `None` via the wrapper.
async fn seed_airing_for_negative_with_base(
    state: &AppState,
    kitsu_id: &str,
    available: bool,
    status: Option<&str>,
    anilist_base: Option<&str>,
) {
    if available || !matches!(status, Some("unreleased" | "tba" | "upcoming")) {
        return;
    }
    // Errors degrade to the schedule-less fallback TTL — same outcome
    // as before the seed existed.
    let _ = crate::meta::anilist_gate::in_background(
        crate::commands::airing::airing_get_with_anilist_base(state, kitsu_id, anilist_base),
    )
    .await;
}

/// Best-effort read of the show's next scheduled airing from the
/// airing cache (`commands::airing` writes it on detail-page visits,
/// [`seed_airing_for_negative`] on pre-premiere negative probes).
/// Deliberately cache-only — a missing row just means the
/// status-based fallback in [`negative_ttl_for`] applies.
fn cached_next_airing_at(state: &AppState, kitsu_id: &str) -> Option<u64> {
    cached_airing(state, kitsu_id)?.next_airing_at
}

/// The show's cached airing row, read as [`cached_next_airing_at`] reads it.
fn cached_airing(
    state: &AppState,
    kitsu_id: &str,
) -> Option<crate::meta::anilist_airing::AiringStatus> {
    let key = format!("airing:v2:{kitsu_id}");
    let body = meta_cache_get(&state.cache_pool, &key).ok().flatten()?;
    serde_json::from_str(&body).ok()
}

/// Same as [`write_cache`] but lets the caller supply the episode
/// count + extras when it knows. Used by `check_availability` after
/// running the play picker; everything else stays on the simpler
/// entry point.
pub fn write_cache_full(
    state: &AppState,
    kitsu_id: &str,
    mode: &str,
    status: Option<&str>,
    body: &AvailabilityResponse,
) {
    if kitsu_id.is_empty() {
        return;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let ttl = availability_row_ttl(state, kitsu_id, status, body, now);
    put_availability_row(state, kitsu_id, mode, status, body, ttl);
}

/// The window a row written now gets, from the show's status and the
/// cached schedule.
fn availability_row_ttl(
    state: &AppState,
    kitsu_id: &str,
    status: Option<&str>,
    body: &AvailabilityResponse,
    now: u64,
) -> u64 {
    if body.available && status == Some("finished") {
        positive_ttl_for(status)
    } else if body.available {
        // Still airing: the count goes stale at the next drop — or
        // already has, when the schedule's aired count is past it.
        use crate::commands::availability_ttl::{bounded_by_next_airing, next_airing_for_count};
        let airing = cached_airing(state, kitsu_id);
        let next = airing.as_ref().and_then(|a| a.next_airing_at);
        let aired = airing.as_ref().and_then(|a| a.aired);
        bounded_by_next_airing(
            positive_ttl_for(status),
            next_airing_for_count(next, body.episode_count, aired, now),
            now,
        )
    } else {
        negative_ttl_for(status, cached_next_airing_at(state, kitsu_id), now)
    }
}

/// Store a row under the window [`availability_row_ttl`] gave it.
fn put_availability_row(
    state: &AppState,
    kitsu_id: &str,
    mode: &str,
    status: Option<&str>,
    body: &AvailabilityResponse,
    ttl: u64,
) {
    let key = cache_key(kitsu_id, mode);
    let Ok(serialized) = serde_json::to_string(body) else {
        return;
    };
    if meta_cache_put(&state.cache_pool, &key, &serialized, ttl).is_err() {
        return;
    }
    if body.available {
        // Counted so a negative out at this moment can tell, when
        // it comes to write, that this proof landed meanwhile —
        // even where it put the same bytes back.
        state.availability_refreshes.note_positive(&key);
    }
    // A schedule put after this write read none has scanned for rows
    // to cut before this one existed; re-read it and cut here, so
    // whichever of the two writes lands second applies the cut. A
    // finished show's window ignores the schedule by design.
    if status != Some("finished") {
        recut_after_put(state, kitsu_id, &key);
    }
}

/// Apply the cached schedule to the row just put at `key`.
fn recut_after_put(state: &AppState, kitsu_id: &str, key: &str) {
    let Some(airing) = cached_airing(state, kitsu_id) else {
        return;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    crate::commands::availability_reschedule::cut_row(
        &state.cache_pool,
        key,
        airing.next_airing_at,
        airing.aired,
        now,
    );
}

/// Cached-only batch lookup. Returns `cached[id] = available` for
/// every id with a fresh cache entry; missing entries are absent
/// from the map so the caller can render them while waiting.
pub fn batch_cached(state: &AppState, args: &AvailabilityBatchArgs) -> AvailabilityBatchResponse {
    let mode = if args.mode == "dub" { "dub" } else { "sub" };
    let mut cached = HashMap::with_capacity(args.kitsu_ids.len());
    let mut playable_episode_counts = HashMap::new();
    for id in &args.kitsu_ids {
        if id.is_empty() {
            continue;
        }
        let key = cache_key(id, mode);
        if let Ok(Some(body)) = meta_cache_get(&state.cache_pool, &key) {
            if let Ok(parsed) = serde_json::from_str::<AvailabilityResponse>(&body) {
                // A row nobody walked stands behind, or a negative its
                // provider cannot back now, is not served: the card
                // renders, and the page's probe re-asks.
                if !row_is_walked(state, &parsed)
                    || (!parsed.available && !negative_row_is_backed(state, &parsed))
                {
                    continue;
                }
                cached.insert(id.clone(), parsed.available);
                // Only surface a count when the probe found the show
                // AND the cache body carries one. Negative-cached rows
                // and pre-v2 legacy rows yield no entry — the home
                // card's cap will fall back to Kitsu's episode_count.
                if parsed.available {
                    if let Some(count) = parsed.episode_count {
                        playable_episode_counts.insert(id.clone(), count);
                    }
                }
            }
        }
    }
    AvailabilityBatchResponse {
        cached,
        playable_episode_counts,
    }
}

/// Warm the cache for a set of titles, and return once the warm's
/// worker is idle — every row it was handed walked, or left out of a
/// full queue. Each entry carries the data needed to run
/// check_availability (title + alt_titles + mode + kitsu_id). The
/// items join the state's warm queue (see `availability_warm`):
/// entries with a fresh cache row are skipped, the rest are probed
/// one at a time with a 500ms gap between queries so we don't hammer
/// the provider.
///
/// The route hands its items to the queue and returns at once; the
/// next visit to the same list reads the now-populated cache and
/// filters known-unavailable cards.
pub async fn warm(state: std::sync::Arc<AppState>, items: Vec<AvailabilityArgs>) {
    let queue = state.availability_refreshes.warm_queue.clone();
    queue.push(&state, items);
    queue.drained().await;
}

/// The warm entries whose airing row is batch-seeded before they
/// probe: every entry still on air or to come that will actually
/// probe (no fresh availability row). One AniList request per id
/// space — AniList id, or MAL id for a show mapped to MAL alone —
/// covers a rail of up to 50 shows. A pre-premiere probe's per-show seed becomes a cache
/// hit, and a current show's positive row is bounded by its next
/// airing when it is written rather than holding the ongoing day.
pub(super) fn schedule_seed_ids(state: &AppState, items: &[AvailabilityArgs]) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for args in items {
        let mode = if args.mode == "dub" { "dub" } else { "sub" };
        let Some(id) = args.kitsu_id.as_deref().filter(|s| !s.is_empty()) else {
            continue;
        };
        if has_usable_row(state, id, mode) {
            continue;
        }
        if matches!(
            args.status.as_deref(),
            Some("current" | "unreleased" | "tba" | "upcoming")
        ) {
            ids.push(id.to_string());
        }
    }
    ids
}

/// Whether the cache holds a row for `(kitsu_id, mode)` that the
/// readers would serve — fresh, and usable under
/// [`cache_hit_is_usable`]: a count-less or approximate positive
/// re-probes, and so does a negative row whose provider no longer
/// stands behind it.
pub(super) fn has_usable_row(state: &AppState, kitsu_id: &str, mode: &str) -> bool {
    let Ok(Some(body)) = meta_cache_get(&state.cache_pool, &cache_key(kitsu_id, mode)) else {
        return false;
    };
    serde_json::from_str::<AvailabilityResponse>(&body)
        .is_ok_and(|parsed| cache_hit_is_usable(state, &parsed))
}

/// How long the warm loop sleeps after one probe, given its outcome.
/// A rate-limited probe waits out the upstream's advertised window
/// (floored at 1 s so a "0 seconds" answer can't become a hot loop;
/// 10 s when the message carried no number) instead of the 500 ms
/// pacing gap — continuing at full pace would issue doomed requests
/// that deepen the limit while the user's click path competes for
/// the same budget. Every other outcome keeps the half-second
/// cadence. The rate-limited row stays unwritten; a later warm pass
/// re-probes it.
pub(super) fn warm_backoff(outcome: &Result<AvailabilityResponse>) -> std::time::Duration {
    match outcome {
        Err(crate::error::AniError::RateLimited {
            retry_after_secs, ..
        }) => std::time::Duration::from_secs(retry_after_secs.unwrap_or(10).max(1)),
        _ => std::time::Duration::from_millis(500),
    }
}

/// Inputs for `availability_warm` — a batch of titles whose cache
/// rows the backend should probe (and populate) in the background.
/// Drives the home-page hybrid filter that skips already-cached rows
/// and rate-limits the rest at one probe per 500 ms.
#[derive(Debug, Clone, Deserialize)]
pub struct AvailabilityWarmArgs {
    /// Per-title args — same shape as the single-item check.
    pub items: Vec<AvailabilityArgs>,
}

#[cfg(test)]
#[path = "availability_backed_test.rs"]
mod backed_tests;

#[cfg(test)]
#[path = "availability_flight_test.rs"]
mod flight_tests;

#[cfg(test)]
#[path = "availability_stamp_race_test.rs"]
mod stamp_race_tests;

#[cfg(test)]
#[path = "availability_positive_ttl_test.rs"]
mod positive_ttl_tests;

#[cfg(test)]
#[path = "availability_test.rs"]
mod tests;

#[cfg(test)]
#[path = "availability_versions_test.rs"]
mod versions_tests;

#[cfg(test)]
#[path = "availability_subtype_args_test.rs"]
mod subtype_args_tests;

#[cfg(test)]
#[path = "availability_unwalked_test.rs"]
mod unwalked_tests;

#[cfg(test)]
#[path = "availability_former_test.rs"]
mod former_tests;

#[cfg(test)]
#[path = "availability_unread_test.rs"]
mod unread_tests;
