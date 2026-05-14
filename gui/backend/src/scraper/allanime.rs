//! allanime search GraphQL — see `super` for the architectural rationale.

use serde::Deserialize;
use url::Url;

use crate::error::{AniError, Result};

/// One candidate row from allanime's search response. Mirrors the
/// fields ani-cli pulls in `search_anime` (`_id`, `name`,
/// `availableEpisodes`) plus `airedStart` for the year tie-break the
/// disambiguator runs alongside ep-count.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Candidate {
    /// allanime's internal show id.
    #[serde(rename = "_id")]
    pub id: String,
    /// Display name (the same one ani-cli would show in fzf).
    pub name: String,
    /// Episode counts per translation type. Sub is what `ani-cli`'s
    /// default mode reads; dub is also exposed for `--dub` plays.
    #[serde(default, rename = "availableEpisodes")]
    pub available_episodes: AvailableEpisodes,
    /// Premier-date object from allmanga. We only consume `year`, but
    /// keep the wrapper so the deserialiser can grow other fields
    /// later (month/day/hour/minute also come down the wire).
    /// `None` when allmanga omits airedStart — happens on older shows
    /// and on stub entries where the metadata never got populated.
    #[serde(default, rename = "airedStart")]
    pub aired_start: Option<AiredStart>,
    /// allmanga's own format tag: `"TV"`, `"Movie"`, `"OVA"`,
    /// `"Special"`, `"ONA"`, etc. Used by the picker to hard-reject
    /// a same-year 1-ep OVA/Movie/Special when Kitsu's expected
    /// implies a multi-ep TV series. `None` when allmanga returns
    /// `null` for the field (common on partially-imported rows).
    #[serde(default, rename = "type")]
    pub show_type: Option<String>,
    /// Planned total episode count from allmanga (BigInt on the wire,
    /// arrives as a string for compatibility — see the custom
    /// deserialiser). Distinct from [`AvailableEpisodes`], which is
    /// the *released-so-far* count. The picker compares this to
    /// Kitsu's expected to confirm same-show identity even when
    /// fewer eps are out yet (the "real airing show, week 1" case).
    /// `None` when allmanga returns `null`.
    #[serde(
        default,
        rename = "episodeCount",
        deserialize_with = "deserialize_bigint_u32"
    )]
    pub episode_count: Option<u32>,
    /// allmanga's airing status — `"Releasing"`, `"Finished"`,
    /// `"Upcoming"`. Currently unused by the picker but pulled so the
    /// disambiguator can grow status-aware heuristics without another
    /// schema bump on the search query. `None` when allmanga omits.
    #[serde(default)]
    pub status: Option<String>,
}

/// Custom deserialiser for allmanga's `episodeCount` field. The
/// schema exposes it as `BigInt`, which the JSON layer renders as a
/// string (`"28"`) or `null`. Plain `Option<u32>` would fail to parse
/// the string form, so we accept either shape.
fn deserialize_bigint_u32<'de, D>(d: D) -> std::result::Result<Option<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wire {
        Str(String),
        Num(u64),
    }
    Ok(Option::<Wire>::deserialize(d)?.and_then(|w| match w {
        Wire::Str(s) => s.parse().ok(),
        Wire::Num(n) => u32::try_from(n).ok(),
    }))
}

/// `airedStart` object on allmanga's `Show` type. Flat fields, no
/// sub-selection (the GraphQL schema explicitly rejects subfields).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct AiredStart {
    /// Calendar year of the show's premiere. `None` when allmanga
    /// has the airedStart object but the year slot is null.
    #[serde(default)]
    pub year: Option<u32>,
}

impl AiredStart {
    /// Convenience accessor that flattens `Option<AiredStart>`'s nested
    /// `year` field to a single `Option<u32>` for the picker's filter
    /// predicate.
    #[must_use]
    pub fn year_value(this: Option<&Self>) -> Option<u32> {
        this.and_then(|a| a.year)
    }
}

/// `availableEpisodes` object from allanime's response. Both fields
/// default to 0 when allanime omits them (rare but possible).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct AvailableEpisodes {
    /// Subtitled-episode count.
    #[serde(default)]
    pub sub: u32,
    /// Dubbed-episode count.
    #[serde(default)]
    pub dub: u32,
}

impl AvailableEpisodes {
    /// Episode count to score against Kitsu's `episode_count`. Picks
    /// the dub count when the caller asked for dub playback, else sub.
    #[must_use]
    pub fn for_mode(&self, mode: &str) -> u32 {
        if mode == "dub" {
            self.dub
        } else {
            self.sub
        }
    }
}

/// Pick the 1-based index of the candidate whose episode count for
/// `mode` is closest to `expected`, with an exact-name-match tie-break
/// against `search_title` when several candidates land on the same
/// minimum distance.
///
/// Tie-break rationale: a Kitsu-routed play call carries the user's
/// chosen show title (e.g. `"Gintama."`), and allanime's "Gintama."
/// search returns several 12-episode shows (Ginpachi-sensei at pos 0,
/// Shirogane no Tamashii-hen at pos 9, and the actual Gintama. at pos
/// 12). The episode-count signal can't disambiguate them — but the
/// allanime `name` of the right show is literally the same as the
/// search title. Empty `search_title` preserves the older positional
/// tie-break for non-Kitsu-routed callers.
///
/// Returns `None` when the input is empty. The base distance pick
/// always wins over the tie-break, so a closer non-exact match beats
/// a more-distant exact-name match.
///
/// Returns a 1-based index because ani-cli's `-S` flag is 1-based.
#[must_use]
pub fn pick_by_ep_count(
    candidates: &[Candidate],
    expected: u32,
    mode: &str,
    search_title: &str,
) -> Option<usize> {
    if candidates.is_empty() {
        return None;
    }
    let mut best_idx = 0usize;
    let mut best_dist = u32::MAX;
    for (i, c) in candidates.iter().enumerate() {
        let got = c.available_episodes.for_mode(mode);
        let dist = got.abs_diff(expected);
        if dist < best_dist {
            best_idx = i;
            best_dist = dist;
        }
    }
    // Exact-name tie-break: among the candidates that landed on
    // `best_dist`, prefer one whose name (case-insensitive, trimmed)
    // equals the search title. Falls through to `best_idx` when no
    // exact name match exists, preserving allanime's positional
    // ranking.
    let needle = search_title.trim().to_lowercase();
    if !needle.is_empty() {
        for (i, c) in candidates.iter().enumerate() {
            if c.available_episodes.for_mode(mode).abs_diff(expected) != best_dist {
                continue;
            }
            if c.name.trim().to_lowercase() == needle {
                return Some(i + 1);
            }
        }
    }
    Some(best_idx + 1)
}

/// Layered allmanga picker: year filter → ep-count threshold →
/// exact-name tie-break. Same return shape as [`pick_by_ep_count`]
/// (1-based index, `None` for "no good match") but with two extra
/// rejection paths that stop the disambiguator from silently picking
/// a sibling when the show isn't on allmanga (the
/// Mobile-Suit-Gundam-1979-vs-Wing repro).
///
///   1. If `expected_year` is `Some` AND at least one candidate has a
///      `aired_start.year` within ±1 of it, restrict the working pool
///      to those candidates. Otherwise the full pool is used (so
///      shows where allmanga omits the year still resolve).
///   2. Pick by ep-count distance within the working pool.
///   3. Reject the pick when `best_dist > max(3, expected * 10%)`.
///      Long-running shows get proportional slack (One Piece-shaped
///      drift of 60+ episodes against an `expected` near 1100 still
///      lands inside the 110-ep tolerance); short shows get a hard
///      floor of 3 so sibling-distance-6 picks like Wing-for-1979
///      can't slip through.
///   4. Exact-name tie-break stays — among candidates at `best_dist`
///      in the working pool, prefer one whose name matches
///      `search_title` (case-insensitive, trimmed).
#[must_use]
pub fn pick_by_ep_count_v2(
    candidates: &[Candidate],
    expected: u32,
    expected_year: Option<u32>,
    mode: &str,
    search_title: &str,
) -> Option<usize> {
    if candidates.is_empty() {
        return None;
    }
    // 1) Year filter — pool is indices into the full slice so the
    //    1-based return value still points at the right candidate.
    //
    //    When expected_year is Some, build the pool by preference:
    //      1. If any candidate is dated and in ±1 of expected_year,
    //         the pool is JUST those — a positive year signal beats
    //         an unknown one, so dated wrong-year siblings AND
    //         undated entries both drop out.
    //      2. Else if any candidate has a year (so we know they're
    //         all wrong-year, no undated either) → return None.
    //         Year disproves the match; ep-count fallback would
    //         re-admit a known-wrong sibling.
    //      3. Else (all undated, or no candidate had any year value)
    //         → keep all and degrade to pure ep-count + threshold.
    // `year_filtered` is true when the pool below was narrowed to
    // candidates whose aired-year matched `expected_year` (i.e. we
    // hit the in-range branch). The threshold step downstream
    // relaxes its upper bound in that case so a partial-season
    // candidate (allmanga has fewer eps released than Kitsu's
    // planned total) still survives — year already disambiguated
    // wrong-show siblings, so the distance check is no longer the
    // safety net it is in the no-year path. Codex P2 #3236... .
    let (pool, year_filtered): (Vec<usize>, bool) = match expected_year {
        Some(want) => {
            let in_range: Vec<usize> = (0..candidates.len())
                .filter(|&i| {
                    AiredStart::year_value(candidates[i].aired_start.as_ref())
                        .is_some_and(|y| y.abs_diff(want) <= 1)
                })
                .collect();
            if !in_range.is_empty() {
                (in_range, true)
            } else {
                let any_has_year = (0..candidates.len())
                    .any(|i| AiredStart::year_value(candidates[i].aired_start.as_ref()).is_some());
                let any_undated = (0..candidates.len())
                    .any(|i| AiredStart::year_value(candidates[i].aired_start.as_ref()).is_none());
                if any_has_year && !any_undated {
                    return None;
                }
                // Fall back: include undated candidates (and any
                // wrong-year ones, since the wrong-year-only branch
                // already returned None above). In practice this is
                // the "all undated" or "mixed wrong-year + undated"
                // shape — the undated ones become the working pool.
                // year_filtered stays false: we landed here BECAUSE
                // the year signal couldn't narrow the pool.
                let undated: Vec<usize> = (0..candidates.len())
                    .filter(|&i| {
                        AiredStart::year_value(candidates[i].aired_start.as_ref()).is_none()
                    })
                    .collect();
                (undated, false)
            }
        }
        None => ((0..candidates.len()).collect(), false),
    };
    if pool.is_empty() {
        return None;
    }

    // 2) Same-show identity filter on the pool. Codex P2 #3242661503
    //    introduced these checks; Codex P2 #3243194264 surfaced that
    //    they MUST run before the ep-count scoring step. A same-year
    //    OVA with available=12 (movie/special bundle) would otherwise
    //    win best_i by distance, fail this filter, and reject the
    //    whole pool — even with a real TV show sitting one slot away
    //    with available=1 (week 1 of airing).
    //
    //    Two hard-rejects per candidate:
    //    - Format mismatch: when planned-count is unknown, the
    //      `type` field alone speaks — OVA/Movie/Special against a
    //      multi-ep Kitsu expected is the wrong show. Skipped when
    //      `expected <= 1` so legit OVA/Movie Kitsu entries resolve.
    //    - Planned-count divergence: when allmanga's own
    //      `episodeCount` is far from Kitsu's expected, it's a
    //      different show regardless of release progress.
    let tolerance = std::cmp::max(3, expected / 10);
    let pool: Vec<usize> = pool
        .into_iter()
        .filter(|&i| {
            let c = &candidates[i];
            if expected > 1
                && c.episode_count.is_none()
                && matches!(c.show_type.as_deref(), Some("OVA" | "Movie" | "Special"))
            {
                return false;
            }
            if c.episode_count
                .is_some_and(|p| p.abs_diff(expected) > tolerance)
            {
                return false;
            }
            true
        })
        .collect();
    if pool.is_empty() {
        return None;
    }

    // 3) Acceptability filter. Each candidate must independently
    //    satisfy the available-eps threshold (or the partial-season
    //    relaxation) — same predicate the previous round applied only
    //    to best_i, now applied per-candidate. Codex P2 #3243312178
    //    surfaced that the old shape let the exact-name tie-break
    //    return a same-distance stub whose null-fallback gate would
    //    have rejected it on its own. Scoping both the scoring and
    //    the tie-break to candidates that pass standalone closes
    //    that gap.
    //
    //    Relaxation signals when `dist > tolerance` and the
    //    candidate is in the partial-release direction:
    //    - strong: planned-count Some (step 2 invariant guarantees
    //      Some implies within tolerance).
    //    - medium: year-filtered + TV/ONA format.
    //    - null fallback: year-filtered + planned None + type None +
    //      at least 1/4 of expected eps available (the gate from
    //      Codex P2 #3236031635 — kept for shows where allmanga
    //      returns nulls for both type and episodeCount).
    let pool: Vec<usize> = pool
        .into_iter()
        .filter(|&i| {
            let c = &candidates[i];
            let got = c.available_episodes.for_mode(mode);
            let dist = got.abs_diff(expected);
            if dist <= tolerance {
                return true;
            }
            if got >= expected {
                return false;
            }
            if c.episode_count.is_some() {
                return true;
            }
            if year_filtered && matches!(c.show_type.as_deref(), Some("TV" | "ONA")) {
                return true;
            }
            year_filtered && c.show_type.is_none() && got.saturating_mul(4) >= expected
        })
        .collect();
    if pool.is_empty() {
        return None;
    }

    // 4) Ep-count pick within the acceptable pool.
    let mut best_i = pool[0];
    let mut best_dist = u32::MAX;
    for &i in &pool {
        let got = candidates[i].available_episodes.for_mode(mode);
        let dist = got.abs_diff(expected);
        if dist < best_dist {
            best_i = i;
            best_dist = dist;
        }
    }

    // 5) Exact-name tie-break, scoped to the acceptable pool +
    //    min-distance bucket. Candidates outside `pool` have already
    //    been rejected on their own merits and must not re-enter via
    //    a name match.
    let needle = search_title.trim().to_lowercase();
    if !needle.is_empty() {
        for &i in &pool {
            let dist = candidates[i]
                .available_episodes
                .for_mode(mode)
                .abs_diff(expected);
            if dist != best_dist {
                continue;
            }
            if candidates[i].name.trim().to_lowercase() == needle {
                return Some(i + 1);
            }
        }
    }
    Some(best_i + 1)
}

const ALLANIME_API: &str = "https://api.allanime.day";
const ALLANIME_REFERER: &str = "https://allmanga.to";

/// Subset of allanime's `show(_id: …)` response — only the title fields
/// our resolver consumes when bridging from a history-recorded
/// allmanga show_id to a Kitsu entry.
///
/// The `name` field can be a stub (e.g. `"1P"` for One Piece, `"Nato:
/// Shippuuden"` for Naruto Shippuuden) — those are the cases where
/// title-text-search through Kitsu returns zero hits and the home
/// page's Continue Watching card falls through to the bare allmanga
/// label. `english_name` / `native_name` / `alt_names` are the
/// recovery surface.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct ShowMetadata {
    /// Primary catalogue name. Sometimes a stub.
    #[serde(default)]
    pub name: String,
    /// Localised English title. `null` on shows that don't ship one.
    #[serde(default, rename = "englishName")]
    pub english_name: Option<String>,
    /// Romanised native-language title. `null` on non-Japanese shows.
    #[serde(default, rename = "nativeName")]
    pub native_name: Option<String>,
    /// Alternate titles allmanga keeps for fuzzy search. May be empty
    /// or contain non-Latin scripts; callers filter as needed.
    #[serde(default, rename = "altNames")]
    pub alt_names: Vec<String>,
    /// Per-mode list of episode tags allmanga has streamable. Each
    /// entry is the same string ani-cli's `-e` accepts — usually an
    /// integer like `"1160"`, but may include half-episodes like
    /// `"1061.5"` (recaps / specials). The COUNT in
    /// `availableEpisodes` includes these halves, so taking it as
    /// the cap proposes a phantom max episode (One Piece: count=1161
    /// but max integer is 1160 because 1061.5 occupies one slot).
    /// Use [`Self::max_integer_episode`] to get the cap that the
    /// player CTA + episode strip should use.
    #[serde(default, rename = "availableEpisodesDetail")]
    pub available_episodes_detail: AvailableEpisodesDetail,
}

/// `availableEpisodesDetail` object — episode TAG lists per mode.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct AvailableEpisodesDetail {
    /// Episode tags streamable in sub mode. Each entry is a string
    /// `ani-cli`'s `-e` flag accepts (e.g. `"5"`, `"1061.5"`).
    #[serde(default)]
    pub sub: Vec<String>,
    /// Episode tags streamable in dub mode. Same format as
    /// [`Self::sub`]. Often shorter — many shows lack a dub track.
    #[serde(default)]
    pub dub: Vec<String>,
}

impl AvailableEpisodesDetail {
    /// Per-mode episode list.
    #[must_use]
    pub fn for_mode(&self, mode: &str) -> &[String] {
        if mode == "dub" {
            &self.dub
        } else {
            &self.sub
        }
    }
}

impl ShowMetadata {
    /// Highest integer episode number streamable in `mode`, ignoring
    /// half-episode entries (`"1061.5"` etc.). Returns `None` when
    /// the list is empty or contains only non-integer tags.
    ///
    /// allmanga's `availableEpisodes.<mode>` field returns a COUNT
    /// that includes halves, so a show with episodes 1..1160 plus
    /// one `1061.5` reports 1161. Taking that count as the cap
    /// proposes episode 1161 as the next playable, which doesn't
    /// exist. Walking the actual tag list and dropping non-integers
    /// gives the truthful upper bound.
    #[must_use]
    pub fn max_integer_episode(&self, mode: &str) -> Option<u32> {
        self.available_episodes_detail
            .for_mode(mode)
            .iter()
            .filter_map(|tag| tag.parse::<u32>().ok())
            .max()
    }

    /// Ordered list of search terms to feed to a downstream fuzzy
    /// matcher (Kitsu text search). `english_name` first because Kitsu
    /// indexes by transliterated English titles; `native_name` second
    /// for shows whose English release is the alias; `alt_names` last
    /// as a wide net. `name` is intentionally NOT included — it's the
    /// stub that already failed the original search, so retrying it is
    /// a no-op. Empty/whitespace-only strings are skipped.
    #[must_use]
    pub fn search_terms(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for s in std::iter::once(self.english_name.as_deref())
            .chain(std::iter::once(self.native_name.as_deref()))
            .chain(self.alt_names.iter().map(|s| Some(s.as_str())))
            .flatten()
        {
            let trimmed = s.trim();
            if !trimmed.is_empty() && !out.iter().any(|prev| prev == trimmed) {
                out.push(trimmed.to_string());
            }
        }
        out
    }
}

// `availableEpisodesDetail` is a custom scalar (free-form JSON),
// NOT an object — subselecting `{ sub dub }` makes allanime return
// it empty (ani-cli's `episodes_list_gql` agrees: no subselection).
// The serde deserializer reads the embedded JSON object's `sub` /
// `dub` fields directly.
const SHOW_GQL: &str =
    "query Show($showId: String!){ show(_id: $showId){ name englishName nativeName altNames availableEpisodesDetail }}";

/// Fetch allanime's per-show metadata (title aliases) for a given
/// `show_id`. Returns the parsed [`ShowMetadata`] on a 2xx response
/// with the expected shape.
///
/// `base_override` mirrors the `search()` parameter — `None` in prod,
/// `Some(uri)` in tests pointing at wiremock.
///
/// # Errors
/// - [`AniError::Network`] on connection failure.
/// - [`AniError::Upstream`] on non-2xx HTTP.
/// - [`AniError::ParseFailed`] when the JSON body doesn't shape into
///   `{ data: { show: {...} } }`.
pub async fn fetch_show(
    client: &reqwest::Client,
    show_id: &str,
    base_override: Option<&str>,
) -> Result<ShowMetadata> {
    let base = base_override.unwrap_or(ALLANIME_API);
    let url = format!("{base}/api");
    let _ = Url::parse(&url).map_err(|_| AniError::ParseFailed {
        detail: format!("allanime show url: {url}"),
    })?;

    let body = serde_json::json!({
        "variables": { "showId": show_id },
        "query": SHOW_GQL,
    });

    let resp = client
        .post(&url)
        .header("content-type", "application/json")
        .header("referer", ALLANIME_REFERER)
        .json(&body)
        .send()
        .await
        .map_err(|_| AniError::Network)?;
    let status = resp.status();
    if !status.is_success() {
        return Err(AniError::Upstream {
            status: status.as_u16(),
        });
    }

    #[derive(Deserialize)]
    struct Wrap {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        show: Option<ShowMetadata>,
    }
    let parsed: Wrap = resp.json().await.map_err(|e| AniError::ParseFailed {
        detail: format!("allanime show response: {e}"),
    })?;
    Ok(parsed.data.show.unwrap_or_default())
}

/// Replace ASCII space with `+` to match ani-cli's `search_anime`
/// pre-processing (line ~178: `printf '%s' "$1" | sed 's| |+|g'`).
/// Allanime treats `+` as a literal character in the search query,
/// so a clean-spaces query and a plus-joined query return *different*
/// hit lists. Both layers must agree byte-for-byte or our index pick
/// won't line up with what ani-cli sees — Stone Ocean Part 2
/// reproduces this when our scraper saw 11 hits and ani-cli saw 2.
///
/// No further URL-encoding is applied; ani-cli doesn't either, and
/// allanime's GraphQL accepts the field as JSON-stringified text.
#[must_use]
pub fn encode_query_for_allanime(s: &str) -> String {
    s.replace(' ', "+")
}
// `airedStart` is an Object whose subfields the GraphQL schema
// rejects in a sub-selection ("must not have a selection since type
// 'Object' has no subfields") — same shape as `availableEpisodes`.
// Selecting the bare field gets us the whole `{year, month, date,
// hour, minute}` object back, which our `AiredStart` struct trims
// down to the `year` we actually consume.
const SEARCH_GQL: &str = "query( $search: SearchInput $limit: Int $page: Int $translationType: VaildTranslationTypeEnumType $countryOrigin: VaildCountryOriginEnumType ) { shows( search: $search limit: $limit page: $page translationType: $translationType countryOrigin: $countryOrigin ) { edges { _id name availableEpisodes airedStart type status episodeCount __typename } }}";

/// Hit allanime's GraphQL `shows.search` endpoint with the same
/// payload ani-cli would send and return the candidate list. `mode`
/// is `"sub"` or `"dub"`; passed through as the `translationType`
/// variable.
///
/// `base_override` lets tests redirect the call at a wiremock server.
/// In prod, callers pass `None`.
///
/// # Errors
/// - [`AniError::Network`] for connection failures
/// - [`AniError::Upstream`] for non-2xx responses
/// - [`AniError::ParseFailed`] when the JSON body doesn't shape into
///   the expected `Candidate` list
pub async fn search(
    client: &reqwest::Client,
    query: &str,
    mode: &str,
    base_override: Option<&str>,
) -> Result<Vec<Candidate>> {
    let base = base_override.unwrap_or(ALLANIME_API);
    let url = format!("{base}/api");
    let _ = Url::parse(&url).map_err(|_| AniError::ParseFailed {
        detail: format!("allanime search url: {url}"),
    })?;

    // Body shape mirrors ani-cli's `search_anime` POST byte-for-byte —
    // including the space→`+` substitution. See encode_query_for_allanime
    // for why; without it our hit list disagrees with ani-cli's and our
    // index pick lands on a candidate ani-cli's `-S N` can't reach.
    let encoded_query = encode_query_for_allanime(query);
    let body = serde_json::json!({
        "variables": {
            "search": {
                "allowAdult": false,
                "allowUnknown": false,
                "query": encoded_query,
            },
            "limit": 40,
            "page": 1,
            "translationType": mode,
            "countryOrigin": "ALL",
        },
        "query": SEARCH_GQL,
    });

    let resp = client
        .post(&url)
        .header("content-type", "application/json")
        .header("referer", ALLANIME_REFERER)
        .json(&body)
        .send()
        .await
        .map_err(|_| AniError::Network)?;
    let status = resp.status();
    if !status.is_success() {
        return Err(AniError::Upstream {
            status: status.as_u16(),
        });
    }

    #[derive(Deserialize)]
    struct Wrap {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        shows: Shows,
    }
    #[derive(Deserialize)]
    struct Shows {
        edges: Vec<Candidate>,
    }
    let parsed: Wrap = resp.json().await.map_err(|e| AniError::ParseFailed {
        detail: format!("allanime search response: {e}"),
    })?;
    Ok(parsed.data.shows.edges)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(id: &str, name: &str, sub: u32) -> Candidate {
        Candidate {
            id: id.into(),
            name: name.into(),
            available_episodes: AvailableEpisodes { sub, dub: 0 },
            ..Default::default()
        }
    }

    fn cand_with_year(id: &str, name: &str, sub: u32, year: Option<u32>) -> Candidate {
        Candidate {
            id: id.into(),
            name: name.into(),
            available_episodes: AvailableEpisodes { sub, dub: 0 },
            aired_start: year.map(|y| AiredStart { year: Some(y) }),
            ..Default::default()
        }
    }

    #[test]
    fn pick_by_ep_count_returns_none_for_empty_input() {
        assert_eq!(pick_by_ep_count(&[], 500, "sub", ""), None);
    }

    #[test]
    fn pick_by_ep_count_chooses_closest_to_expected() {
        // The Naruto: Shippuden repro. allanime's ranking puts the
        // side story first; we prefer the show whose ep_count is
        // closest to Kitsu's 500.
        let cands = vec![
            cand(
                "side-story",
                "Naruto: Shippuuden: Shippuu! Konoha Gakuen Den",
                1,
            ),
            cand("main", "Naruto: Shippuuden", 500),
            cand("ova", "Naruto OVAs", 12),
        ];
        assert_eq!(pick_by_ep_count(&cands, 500, "sub", ""), Some(2));
    }

    #[test]
    fn pick_by_ep_count_returns_one_when_only_one_candidate() {
        let cands = vec![cand("only", "Some Show", 12)];
        assert_eq!(pick_by_ep_count(&cands, 500, "sub", ""), Some(1));
    }

    #[test]
    fn pick_by_ep_count_breaks_ties_in_allanime_order() {
        // Two candidates equidistant from expected — the earlier one
        // wins to preserve allanime's own relevance ranking when the
        // ep_count signal is ambiguous AND no exact name match is
        // present.
        let cands = vec![cand("a", "A", 100), cand("b", "B", 100)];
        assert_eq!(pick_by_ep_count(&cands, 100, "sub", ""), Some(1));
    }

    #[test]
    fn pick_by_ep_count_prefers_exact_name_match_among_ep_ties() {
        // The Gintama./Ginpachi-sensei repro. allmanga's "Gintama."
        // search returns three candidates with eps=12 (Ginpachi-sensei
        // at pos 0, Shirogane no Tamashii at pos 9, Gintama. at pos
        // 12). Kitsu's canonical title for the show the user clicked
        // is "Gintama." — the picker should prefer the exact-name
        // match over allanime's positional first.
        let cands = vec![
            cand("ginpachi", "3-nen Z-gumi Ginpachi-sensei", 12),
            cand("shirogane", "Gintama.: Shirogane no Tamashii-hen", 12),
            cand("gintama-dot", "Gintama.", 12),
        ];
        assert_eq!(pick_by_ep_count(&cands, 12, "sub", "Gintama."), Some(3));
    }

    #[test]
    fn pick_by_ep_count_exact_name_match_is_case_insensitive_and_trimmed() {
        // Allmanga occasionally pads names with trailing whitespace;
        // Kitsu's canonical can differ in case. Normalize both so the
        // match doesn't break on cosmetic drift.
        let cands = vec![
            cand("ginpachi", "3-nen Z-gumi Ginpachi-sensei", 12),
            cand("gintama-dot", "  GINTAMA.  ", 12),
        ];
        assert_eq!(pick_by_ep_count(&cands, 12, "sub", "gintama."), Some(2));
    }

    #[test]
    fn pick_by_ep_count_year_filter_excludes_wrong_year_sibling() {
        // The Mobile Suit Gundam (1979, 43 eps) vs Gundam Wing
        // (1995, 49 eps) repro: allmanga doesn't index the 1979
        // series, so the only candidate close on ep-count is Wing
        // (distance 6). With the user-confirmed year tie-break,
        // Wing's 1995 airing is rejected against Kitsu's 1979 →
        // function returns None (caller surfaces "not on allmanga")
        // instead of silently playing the wrong show.
        let cands = vec![
            cand_with_year("wing", "Mobile Suit Gundam Wing", 49, Some(1995)),
            cand_with_year("seed", "Mobile Suit Gundam SEED", 50, Some(2002)),
        ];
        // expected_year=1979, expected_eps=43, search_title="Mobile
        // Suit Gundam" — no candidate within year±1, so the pool
        // empties out and the picker yields None.
        assert_eq!(
            pick_by_ep_count_v2(&cands, 43, Some(1979), "sub", "Mobile Suit Gundam"),
            None,
            "no allmanga candidate within ±1 year of 1979 → no match",
        );
    }

    #[test]
    fn pick_by_ep_count_v2_rejects_when_years_present_but_none_match() {
        // Closest sibling on ep-count would slip past the threshold
        // (45 eps vs expected 43 → distance 2 inside tolerance 4),
        // but its year (1995) disagrees with Kitsu's (1979) by 16.
        // The current v2 picker falls back to the full pool when no
        // candidate is in the ±1 year band — that's exactly the
        // case Codex flagged where ep-count alone re-admits a known
        // wrong show. With all candidates carrying year info,
        // year-mismatch must be a hard reject (None), not a
        // fallback.
        let cands = vec![
            cand_with_year("wrong-1", "Mobile Suit Gundam Wing", 45, Some(1995)),
            cand_with_year("wrong-2", "Mobile Suit Gundam SEED", 50, Some(2002)),
        ];
        assert_eq!(
            pick_by_ep_count_v2(&cands, 43, Some(1979), "sub", "Mobile Suit Gundam"),
            None,
            "candidates have years but none in ±1 of expected → reject, don't fall back",
        );
    }

    #[test]
    fn pick_by_ep_count_v2_prefers_dated_year_match_over_undated_with_better_ep_count() {
        // Codex P2 #3231422652. Mixed pool with one dated year-match
        // and one undated sibling. Today's filter keeps both, then
        // ep-count distance picks the undated sibling whose count is
        // exact while the year-match drifts by 2. The undated entry
        // can't be disproved on year, but it also can't be CONFIRMED
        // on year — the dated match has a positive signal we should
        // prefer.
        let cands = vec![
            cand_with_year("dated-match", "Show", 49, Some(2020)),
            cand_with_year("undated-exact-eps", "Show: Spinoff", 51, None),
        ];
        assert_eq!(
            pick_by_ep_count_v2(&cands, 51, Some(2020), "sub", "Show"),
            Some(1),
            "dated year-match must beat undated even when undated has closer ep-count",
        );
    }

    #[test]
    fn pick_by_ep_count_v2_keeps_undated_candidates_among_wrong_year_siblings() {
        // Mixed metadata: an undated allmanga entry alongside a
        // wrong-year franchise sibling. The undated candidate can't
        // be disproved on year, so it must stay in the working pool;
        // the wrong-year sibling drops out. ep-count + name tie-break
        // then picks among the survivors. Codex P2 #3231391358 — the
        // previous code rejected the whole pool the moment any dated
        // candidate disagreed, stranding the legitimate undated
        // match.
        let cands = vec![
            cand_with_year("undated", "Some Show", 12, None),
            cand_with_year("wrong-year", "Some Show: Sequel", 12, Some(2030)),
        ];
        assert_eq!(
            pick_by_ep_count_v2(&cands, 12, Some(2005), "sub", "Some Show"),
            Some(1),
            "undated candidate should survive even when a dated sibling has the wrong year",
        );
    }

    #[test]
    fn pick_by_ep_count_v2_still_falls_back_when_no_candidate_has_year() {
        // Old shows where allmanga's airedStart is null all around.
        // The year signal is genuinely missing, not just mismatched,
        // so the picker must degrade to ep-count + tolerance instead
        // of silently rejecting everything.
        let cands = vec![
            cand_with_year("a", "Some Show", 12, None),
            cand_with_year("b", "Some Show 2", 24, None),
        ];
        assert_eq!(
            pick_by_ep_count_v2(&cands, 12, Some(2005), "sub", "Some Show"),
            Some(1),
        );
    }

    #[test]
    fn pick_by_ep_count_year_filter_keeps_matching_year_within_one() {
        // BNHA repro: Kitsu says 13 eps + 2016. allmanga has a
        // 13-ep 2026 spinoff (current picker bites on its exact-ep
        // match) AND the original 14-ep 2016 series (distance 1).
        // Year-filter restricts to the 2016 pool; ep-count within
        // tolerance picks the original.
        let cands = vec![
            cand_with_year("spinoff", "Vigilante: BNHA Illegals S2", 13, Some(2026)),
            cand_with_year("og", "Boku no Hero Academia", 14, Some(2016)),
        ];
        assert_eq!(
            pick_by_ep_count_v2(&cands, 13, Some(2016), "sub", "Boku no Hero Academia"),
            Some(2),
        );
    }

    #[test]
    fn pick_by_ep_count_v2_accepts_partial_season_inside_year_filtered_pool() {
        // Codex P2 #3236... : currently-airing 12-ep show, allmanga
        // only has 4 subbed eps released so far. Year matches
        // (2026). The threshold (max(3, 12/10) = 3) rejects because
        // best_dist = 8 > 3, telling the user "not in catalog" even
        // though eps 1-4 stream fine. Year filter already
        // disambiguated wrong-show siblings — inside that pool, the
        // upper distance check is too strict.
        let cands = vec![cand_with_year("current", "Some Airing Show", 4, Some(2026))];
        assert_eq!(
            pick_by_ep_count_v2(&cands, 12, Some(2026), "sub", "Some Airing Show"),
            Some(1),
            "partial-season candidate inside year-filtered pool should accept",
        );
    }

    #[test]
    fn pick_by_ep_count_v2_rejects_implausibly_small_same_year_candidate() {
        // Codex P2 #3236031635: the year-filtered relaxation must not
        // be a free pass for same-year OVAs/movies/specials. Kitsu
        // says 12-ep series, allmanga's only year-matching hit has
        // 1 episode — almost certainly a 1-ep special, not the
        // currently-airing main series. Returning Some(1) here would
        // hand play/download/availability a wrong-show pick. The
        // partial-season relaxation should only apply when `got` is
        // a plausible fraction of `expected` (at least 1/4), not for
        // every year-matched row.
        let cands = vec![cand_with_year("ova", "Some Show: OVA", 1, Some(2026))];
        assert_eq!(
            pick_by_ep_count_v2(&cands, 12, Some(2026), "sub", "Some Show"),
            None,
            "1-ep same-year hit should not be accepted as a 12-ep show",
        );
    }

    #[test]
    fn pick_by_ep_count_v2_rejects_ova_format_for_multi_ep_series() {
        // Codex P2 #3242661503 partial fix: when allmanga's own `type`
        // tags the candidate as OVA/Movie/Special and Kitsu's expected
        // is multi-ep, the format mismatch is a hard reject. This
        // catches the wrong-show case BEFORE we'd otherwise be tempted
        // to accept a partial-release count.
        let cands = vec![Candidate {
            id: "ova".into(),
            name: "Some Show: OVA".into(),
            available_episodes: AvailableEpisodes { sub: 1, dub: 0 },
            aired_start: Some(AiredStart { year: Some(2026) }),
            show_type: Some("OVA".into()),
            episode_count: Some(1),
            status: Some("Finished".into()),
        }];
        assert_eq!(
            pick_by_ep_count_v2(&cands, 12, Some(2026), "sub", "Some Show"),
            None,
            "OVA-typed candidate must not be accepted as a 12-ep series",
        );
    }

    #[test]
    fn pick_by_ep_count_v2_rejects_when_planned_count_diverges_from_kitsu() {
        // Hard-reject filter: allmanga's own `episodeCount` (planned
        // total, not yet-released count) must agree with Kitsu's
        // `expected` within the same tolerance the available-eps
        // threshold uses. A 1-ep planned show is not a 12-ep series
        // regardless of format tag.
        let cands = vec![Candidate {
            id: "wrong-planned".into(),
            name: "Some Show".into(),
            available_episodes: AvailableEpisodes { sub: 1, dub: 0 },
            aired_start: Some(AiredStart { year: Some(2026) }),
            show_type: None,
            episode_count: Some(1),
            status: None,
        }];
        assert_eq!(
            pick_by_ep_count_v2(&cands, 12, Some(2026), "sub", "Some Show"),
            None,
            "candidate whose planned count diverges from Kitsu's must be rejected",
        );
    }

    #[test]
    fn pick_by_ep_count_v2_tie_break_does_not_pick_unverified_stub() {
        // Codex P2 #3243312178: when the chosen best_i passes the
        // relaxed partial-season threshold via a strong signal
        // (planned-count match), the exact-name tie-break must not
        // silently swap to a same-distance stub that wouldn't pass
        // the threshold on its own. Repro shape: the real TV row
        // has the planned-count match but its name carries a
        // variant suffix; a same-year stub at the same distance
        // exact-matches Kitsu's canonical title but its all-nulls
        // metadata fails the null-fallback's 1/4 gate. The picker
        // must keep the real TV row even though the stub looks more
        // "name-correct" — the stub would have been rejected
        // standalone.
        let cands = vec![
            Candidate {
                id: "real".into(),
                name: "Some Show: Season 1".into(),
                available_episodes: AvailableEpisodes { sub: 1, dub: 0 },
                aired_start: Some(AiredStart { year: Some(2026) }),
                show_type: Some("TV".into()),
                episode_count: Some(12),
                status: Some("Releasing".into()),
            },
            Candidate {
                id: "stub".into(),
                name: "Some Show".into(),
                available_episodes: AvailableEpisodes { sub: 1, dub: 0 },
                aired_start: Some(AiredStart { year: Some(2026) }),
                show_type: None,
                episode_count: None,
                status: None,
            },
        ];
        assert_eq!(
            pick_by_ep_count_v2(&cands, 12, Some(2026), "sub", "Some Show"),
            Some(1),
            "tie-break must not swap to an exact-name stub that fails the partial-season check",
        );
    }

    #[test]
    fn pick_by_ep_count_v2_skips_format_rejected_candidate_to_keep_real_show() {
        // Codex P2 #3243194264: identity filters must drop invalid
        // candidates from the pool BEFORE scoring by ep-count
        // distance, not after. Otherwise a same-year OVA with
        // available=12 (movie/special bundle, OVA series) lands
        // closer to Kitsu's expected=12 than a real TV show in
        // its first week (available=1), wins best_i, fails the
        // format/planned-count check, and rejects the whole pool —
        // leaving the legitimate TV row stranded even though it's
        // right there in the same search result.
        let cands = vec![
            Candidate {
                id: "ova".into(),
                name: "Some Show: OVA Collection".into(),
                available_episodes: AvailableEpisodes { sub: 12, dub: 0 },
                aired_start: Some(AiredStart { year: Some(2026) }),
                show_type: Some("OVA".into()),
                episode_count: Some(1),
                status: Some("Finished".into()),
            },
            Candidate {
                id: "tv".into(),
                name: "Some Show".into(),
                available_episodes: AvailableEpisodes { sub: 1, dub: 0 },
                aired_start: Some(AiredStart { year: Some(2026) }),
                show_type: Some("TV".into()),
                episode_count: Some(12),
                status: Some("Releasing".into()),
            },
        ];
        assert_eq!(
            pick_by_ep_count_v2(&cands, 12, Some(2026), "sub", "Some Show"),
            Some(2),
            "TV row must be picked when the closer-by-distance OVA is identity-rejected",
        );
    }

    #[test]
    fn pick_by_ep_count_v2_accepts_tv_in_early_release_with_matching_planned_count() {
        // Codex P2 #3242661503 main fix: a currently-airing TV show
        // whose planned count agrees with Kitsu (12 vs 12) but has
        // only ep 1 released so far must NOT be rejected. The
        // planned-count match is a strong "same show, mid-release"
        // signal that overrides the upper distance threshold.
        let cands = vec![Candidate {
            id: "airing".into(),
            name: "Some Airing Show".into(),
            available_episodes: AvailableEpisodes { sub: 1, dub: 0 },
            aired_start: Some(AiredStart { year: Some(2026) }),
            show_type: Some("TV".into()),
            episode_count: Some(12),
            status: Some("Releasing".into()),
        }];
        assert_eq!(
            pick_by_ep_count_v2(&cands, 12, Some(2026), "sub", "Some Airing Show"),
            Some(1),
            "week-1 airing show with matching planned count must be accepted",
        );
    }

    #[test]
    fn pick_by_ep_count_v2_keeps_threshold_when_year_filter_did_not_engage() {
        // Guard the conservative path: no year info available (caller
        // passed expected_year=None), so the year filter never
        // narrowed the pool. The threshold still applies, otherwise
        // a 1-ep side-story would re-admit itself for a long-running
        // show.
        let cands = vec![cand_with_year("undated", "Some Show", 4, None)];
        assert_eq!(
            pick_by_ep_count_v2(&cands, 12, None, "sub", "Some Show"),
            None,
            "no year info → upper threshold still rejects far-off candidate",
        );
    }

    #[test]
    fn pick_by_ep_count_v2_threshold_rejects_far_off_only_match() {
        // No year, just ep_count, and the closest candidate is well
        // outside the tolerance window (max(3, expected*10%)).
        // expected=20 → tolerance=3; closest at distance 10 → reject.
        let cands = vec![cand_with_year("far", "Some Other Show", 30, None)];
        assert_eq!(
            pick_by_ep_count_v2(&cands, 20, None, "sub", "Whatever"),
            None
        );
    }

    #[test]
    fn pick_by_ep_count_v2_threshold_scales_with_expected() {
        // Long-running shows: tolerance = 10% of expected, so One-
        // Piece-shaped drift (1100 expected vs 1162 measured = 62)
        // sits well inside 110 and accepts.
        let cands = vec![cand_with_year("op", "1P", 1162, Some(1999))];
        assert_eq!(
            pick_by_ep_count_v2(&cands, 1100, Some(1999), "sub", "One Piece"),
            Some(1),
        );
    }

    #[test]
    fn pick_by_ep_count_v2_year_missing_on_one_side_falls_back_to_ep_count() {
        // Allmanga sometimes omits airedStart (older shows or stub
        // entries). When the candidate's year is unknown but Kitsu's
        // is, we can't apply the filter — fall back to plain ep-count
        // picking so we don't strand legitimate matches.
        let cands = vec![cand_with_year("only", "Mushishi", 26, None)];
        assert_eq!(
            pick_by_ep_count_v2(&cands, 26, Some(2005), "sub", "Mushishi"),
            Some(1),
        );
    }

    #[test]
    fn pick_by_ep_count_ignores_exact_name_when_distance_differs() {
        // Tie-break only applies AMONG min-distance candidates. A
        // distant exact-name match shouldn't beat a closer non-exact
        // candidate — episode-count remains the primary signal.
        let cands = vec![
            cand("close-but-wrong-name", "Other Show", 12),
            cand("exact-but-distant", "Gintama.", 100),
        ];
        assert_eq!(pick_by_ep_count(&cands, 12, "sub", "Gintama."), Some(1));
    }

    #[test]
    fn pick_by_ep_count_empty_title_falls_back_to_positional_tie_break() {
        // Callers without a search title pass "" — the existing
        // positional-first tie-break is preserved so the function
        // stays backward-compatible for any non-Kitsu-routed call.
        let cands = vec![cand("a", "A", 100), cand("b", "B", 100)];
        assert_eq!(pick_by_ep_count(&cands, 100, "sub", ""), Some(1));
    }

    #[test]
    fn pick_by_ep_count_uses_dub_when_mode_is_dub() {
        let cands = vec![
            Candidate {
                id: "a".into(),
                name: "A".into(),
                available_episodes: AvailableEpisodes { sub: 500, dub: 1 },
                ..Default::default()
            },
            Candidate {
                id: "b".into(),
                name: "B".into(),
                available_episodes: AvailableEpisodes { sub: 1, dub: 500 },
                ..Default::default()
            },
        ];
        // Looking for 500 dub-eps: B wins (dub=500), even though A
        // would win for sub.
        assert_eq!(pick_by_ep_count(&cands, 500, "dub", ""), Some(2));
    }

    #[tokio::test]
    async fn search_parses_allanime_graphql_response() {
        // Body shape from a real allanime response. Wiremock returns
        // it; the parser pulls out the edges array verbatim.
        let server = wiremock::MockServer::start().await;
        let body = serde_json::json!({
            "data": {
                "shows": {
                    "edges": [
                        {
                            "_id": "abc",
                            "name": "Naruto: Shippuuden",
                            "availableEpisodes": {"sub": 500, "dub": 209, "raw": 0},
                            "__typename": "Show"
                        },
                        {
                            "_id": "side",
                            "name": "Naruto: Shippuuden: Konoha",
                            "availableEpisodes": {"sub": 1, "dub": 0, "raw": 0},
                            "__typename": "Show"
                        }
                    ]
                }
            }
        });
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/api"))
            .and(wiremock::matchers::header("referer", "https://allmanga.to"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let cands = search(&client, "Naruto: Shippuuden", "sub", Some(&server.uri()))
            .await
            .expect("search ok");
        assert_eq!(cands.len(), 2);
        assert_eq!(cands[0].id, "abc");
        assert_eq!(cands[0].available_episodes.sub, 500);
        assert_eq!(cands[1].available_episodes.sub, 1);
    }

    #[tokio::test]
    async fn search_parses_show_type_status_and_planned_episode_count() {
        // Codex P2 #3242661503 fix: SEARCH_GQL now pulls `type`,
        // `status`, and `episodeCount` so the picker can distinguish
        // a 1-ep OVA (planned=1, type=OVA) from a TV show in week 1
        // of release (planned=12, type=TV, available=1). Nulls in
        // any of the three fields decode to None.
        let server = wiremock::MockServer::start().await;
        let body = serde_json::json!({
            "data": {
                "shows": {
                    "edges": [
                        {
                            "_id": "tv",
                            "name": "Sousou no Frieren",
                            "type": "TV",
                            "status": "Finished",
                            "episodeCount": "28",
                            "availableEpisodes": {"sub": 28, "dub": 0, "raw": 0},
                            "__typename": "Show"
                        },
                        {
                            "_id": "nulls",
                            "name": "Some Show",
                            "type": null,
                            "status": null,
                            "episodeCount": null,
                            "availableEpisodes": {"sub": 12, "dub": 0, "raw": 0},
                            "__typename": "Show"
                        }
                    ]
                }
            }
        });
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/api"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let cands = search(&client, "Frieren", "sub", Some(&server.uri()))
            .await
            .expect("search ok");
        assert_eq!(cands.len(), 2);
        assert_eq!(cands[0].show_type.as_deref(), Some("TV"));
        assert_eq!(cands[0].status.as_deref(), Some("Finished"));
        assert_eq!(cands[0].episode_count, Some(28));
        assert_eq!(cands[1].show_type, None);
        assert_eq!(cands[1].status, None);
        assert_eq!(cands[1].episode_count, None);
    }

    #[test]
    fn encode_query_for_allanime_replaces_spaces_with_plus() {
        // Drift-critical: ani-cli does `printf '%s' "$1" | sed 's| |+|g'`
        // before posting the GraphQL query (line ~178). Allanime treats
        // `+` as a literal character, so a clean-spaces query and a
        // plus-joined query return *different* hit lists. When our
        // scraper search disagrees with ani-cli's own search, our
        // pick_by_ep_count picks an index that ani-cli's `-S N` can't
        // reach (Stone Ocean Part 2 reproduces this — we saw 11 hits
        // and picked 3, ani-cli saw 2 hits and exited because index 3
        // is out of range).
        assert_eq!(
            encode_query_for_allanime("JoJo no Kimyou na Bouken: Stone Ocean Part 2"),
            "JoJo+no+Kimyou+na+Bouken:+Stone+Ocean+Part+2"
        );
        assert_eq!(encode_query_for_allanime(""), "");
        assert_eq!(encode_query_for_allanime("nospace"), "nospace");
        // Multiple consecutive spaces collapse one-to-one (mirrors
        // ani-cli's sed behaviour, which doesn't squeeze).
        assert_eq!(encode_query_for_allanime("a  b"), "a++b");
    }

    #[tokio::test]
    async fn search_returns_upstream_error_on_5xx() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(wiremock::ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let err = search(&client, "x", "sub", Some(&server.uri()))
            .await
            .unwrap_err();
        assert!(matches!(err, AniError::Upstream { status: 503 }));
    }

    // — fetch_show: bridge from cryptic allmanga `name` (e.g. "1P" for
    //   One Piece) to richer englishName/altNames the resolver feeds
    //   to Kitsu's text search.

    #[tokio::test]
    async fn fetch_show_parses_name_english_native_and_alt_names() {
        // Real shape lifted from allanime's response for One Piece
        // (show_id ReooPAxPMsHM4KPMY). `name` is the stub the CLI
        // writes to ani-hsts; the rest are recovery surfaces.
        let server = wiremock::MockServer::start().await;
        let body = serde_json::json!({
            "data": {
                "show": {
                    "name": "1P",
                    "englishName": "One Piece",
                    "nativeName": "ONE PIECE",
                    "altNames": ["One Piece", "海贼王", "ワンピース"]
                }
            }
        });
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/api"))
            .and(wiremock::matchers::header("referer", "https://allmanga.to"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let show = fetch_show(&client, "ReooPAxPMsHM4KPMY", Some(&server.uri()))
            .await
            .expect("fetch ok");
        assert_eq!(show.name, "1P");
        assert_eq!(show.english_name.as_deref(), Some("One Piece"));
        assert_eq!(show.native_name.as_deref(), Some("ONE PIECE"));
        assert_eq!(
            show.alt_names,
            vec![
                "One Piece".to_string(),
                "海贼王".to_string(),
                "ワンピース".to_string()
            ]
        );
    }

    #[test]
    fn max_integer_episode_drops_half_episode_tags() {
        // Real shape from allmanga's response for One Piece — the
        // sub list is integers 1..1160 plus one "1061.5" recap.
        // Total len is 1161, but the playable cap is 1160; using
        // the COUNT (which `availableEpisodes.sub` reports) would
        // propose phantom episode 1161 as next-resumable.
        let mut sub: Vec<String> = (1..=1160).map(|n| n.to_string()).collect();
        sub.insert(900, "1061.5".to_string());
        let m = ShowMetadata {
            available_episodes_detail: AvailableEpisodesDetail {
                sub,
                dub: Vec::new(),
            },
            ..Default::default()
        };
        assert_eq!(m.max_integer_episode("sub"), Some(1160));
    }

    #[test]
    fn max_integer_episode_returns_none_for_empty_or_all_halves() {
        let m = ShowMetadata::default();
        assert_eq!(m.max_integer_episode("sub"), None);

        let m = ShowMetadata {
            available_episodes_detail: AvailableEpisodesDetail {
                sub: vec!["1.5".into(), "2.5".into()],
                dub: Vec::new(),
            },
            ..Default::default()
        };
        assert_eq!(m.max_integer_episode("sub"), None);
    }

    use proptest::strategy::Strategy;

    proptest::proptest! {
        // For any tag list mixing valid integer episode tags with
        // arbitrary "noise" tags (decimals, empty strings, alphabet
        // soup), `max_integer_episode` must:
        //
        //   • return Some(largest integer present), OR
        //   • return None when no integer tag exists.
        //
        // The noise generator deliberately includes "<n>.5" style
        // half-episodes — the One-Piece-1161 regression came from
        // counting those as if they were integers. This property
        // pins the fix.
        #[test]
        fn max_integer_episode_picks_largest_int_and_ignores_noise(
            ints in proptest::collection::vec(0u32..=20_000u32, 0..40),
            noise in proptest::collection::vec(
                proptest::prop_oneof![
                    // Half-episode tags — the actual regression source.
                    (0u32..=20_000u32).prop_map(|n| format!("{n}.5")),
                    // Decimal tags with arbitrary fractional component.
                    proptest::strategy::Just(".5".to_string()),
                    proptest::strategy::Just("".to_string()),
                    proptest::strategy::Just("foo".to_string()),
                    proptest::strategy::Just("12abc".to_string()),
                ],
                0..20,
            ),
        ) {
            let mut sub: Vec<String> = ints.iter().map(|n| n.to_string()).collect();
            sub.extend(noise.iter().cloned());
            let m = ShowMetadata {
                available_episodes_detail: AvailableEpisodesDetail {
                    sub,
                    dub: Vec::new(),
                },
                ..Default::default()
            };
            let got = m.max_integer_episode("sub");
            let expected = ints.iter().max().copied();
            proptest::prop_assert_eq!(got, expected);
        }

        // The mode parameter must be honoured — sub and dub are
        // independent lists. Mixing them up would mis-cap one mode
        // when only the other has episodes.
        #[test]
        fn max_integer_episode_reads_only_the_requested_mode(
            sub_max in 0u32..=10_000u32,
            dub_max in 0u32..=10_000u32,
        ) {
            let m = ShowMetadata {
                available_episodes_detail: AvailableEpisodesDetail {
                    sub: (1..=sub_max).map(|n| n.to_string()).collect(),
                    dub: (1..=dub_max).map(|n| n.to_string()).collect(),
                },
                ..Default::default()
            };
            let want_sub = if sub_max == 0 { None } else { Some(sub_max) };
            let want_dub = if dub_max == 0 { None } else { Some(dub_max) };
            proptest::prop_assert_eq!(m.max_integer_episode("sub"), want_sub);
            proptest::prop_assert_eq!(m.max_integer_episode("dub"), want_dub);
        }

        // Picker invariants for `pick_by_ep_count`:
        //
        //   • Empty input → None.
        //   • Non-empty input → Some(idx) with idx in 1..=len.
        //   • The chosen candidate's distance to `expected` is
        //     ≤ every other candidate's distance.
        //   • Ties resolve to the earliest candidate (preserve
        //     allanime's own ordering when ep_count signal is
        //     ambiguous).
        //
        // Catches regressions in the disambiguator that drives
        // both the play flow's `-S` selection and the availability
        // probe's "is this on allmanga?" verdict.
        #[test]
        fn pick_by_ep_count_returns_index_with_minimum_distance(
            counts in proptest::collection::vec(0u32..=10_000u32, 1..30),
            expected in 0u32..=10_000u32,
        ) {
            let cands: Vec<Candidate> = counts
                .iter()
                .enumerate()
                .map(|(i, &n)| Candidate {
                    id: format!("c{i}"),
                    name: format!("c{i}"),
                    available_episodes: AvailableEpisodes { sub: n, dub: 0 },
                    ..Default::default()
                })
                .collect();
            let pick = pick_by_ep_count(&cands, expected, "sub", "").expect("non-empty");
            proptest::prop_assert!(pick >= 1);
            proptest::prop_assert!(pick <= cands.len());

            let chosen_dist = cands[pick - 1].available_episodes.sub.abs_diff(expected);
            for (i, c) in cands.iter().enumerate() {
                let d = c.available_episodes.sub.abs_diff(expected);
                proptest::prop_assert!(
                    chosen_dist <= d,
                    "picker chose c{} with dist {} but c{} has dist {}",
                    pick - 1,
                    chosen_dist,
                    i,
                    d,
                );
            }
            // Tie-break invariant: every earlier candidate must have
            // a strictly larger distance (otherwise the picker
            // should have chosen them).
            for c in &cands[..pick - 1] {
                let d = c.available_episodes.sub.abs_diff(expected);
                proptest::prop_assert!(
                    d > chosen_dist,
                    "tie-break: earlier candidate had equal distance but wasn't chosen",
                );
            }
        }
    }

    #[tokio::test]
    async fn fetch_show_returns_upstream_error_on_5xx() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(wiremock::ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let err = fetch_show(&client, "x", Some(&server.uri()))
            .await
            .unwrap_err();
        assert!(matches!(err, AniError::Upstream { status: 503 }));
    }

    #[tokio::test]
    async fn fetch_show_handles_null_show_as_empty_metadata() {
        // Allanime returns `data.show: null` for unknown ids. Treat as
        // empty (no aliases to enrich) instead of erroring out — the
        // caller will skip the enrichment and fall through.
        let server = wiremock::MockServer::start().await;
        let body = serde_json::json!({ "data": { "show": null } });
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let show = fetch_show(&client, "missing", Some(&server.uri()))
            .await
            .expect("ok");
        assert_eq!(show.name, "");
        assert_eq!(show.english_name, None);
        assert!(show.alt_names.is_empty());
    }

    #[test]
    fn search_terms_walks_english_then_native_then_alt_names() {
        let show = ShowMetadata {
            name: "1P".into(),
            english_name: Some("One Piece".into()),
            native_name: Some("ONE PIECE".into()),
            alt_names: vec!["One Piece".into(), "海贼王".into()],
            available_episodes_detail: AvailableEpisodesDetail::default(),
        };
        // english_name first, native_name second, then alt_names —
        // dedupe so the duplicate "One Piece" doesn't appear twice.
        // `name` is excluded (it already failed the original search).
        assert_eq!(
            show.search_terms(),
            vec![
                "One Piece".to_string(),
                "ONE PIECE".to_string(),
                "海贼王".to_string()
            ]
        );
    }

    #[test]
    fn search_terms_skips_empty_and_whitespace_strings() {
        let show = ShowMetadata {
            name: "stub".into(),
            english_name: Some("".into()),
            native_name: Some("   ".into()),
            alt_names: vec!["".into(), "Real Title".into()],
            available_episodes_detail: AvailableEpisodesDetail::default(),
        };
        assert_eq!(show.search_terms(), vec!["Real Title".to_string()]);
    }
}
