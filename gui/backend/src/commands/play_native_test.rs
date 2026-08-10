use super::*;
use crate::error::AniError;
use crate::scraper::anidb::{AnidbClient, AnidbFetch, EpisodeRef, FetchResponse};

/// A fetch whose episodes endpoint answers per numeric id from a
/// canned table; every other route 404s.
struct EpisodesTable(&'static [(u64, u32)]);

#[async_trait::async_trait]
impl AnidbFetch for EpisodesTable {
    async fn get(&self, url: &str) -> crate::error::Result<FetchResponse> {
        for (id, count) in self.0 {
            if url.contains(&format!("/api/frontend/anime/{id}/episodes")) {
                let rows: Vec<String> = (1..=*count)
                    .map(|n| format!("{{\"id\":{},\"number\":{}}}", id * 1000 + u64::from(n), n))
                    .collect();
                return Ok(FetchResponse {
                    status: 200,
                    body: format!("{{\"episodes\":[{}]}}", rows.join(",")),
                });
            }
        }
        Ok(FetchResponse {
            status: 404,
            body: String::new(),
        })
    }
}

fn hit(slug: &str, title: &str) -> BrowseHit {
    BrowseHit {
        slug: slug.into(),
        title: title.into(),
        kind: None,
    }
}

fn typed_hit(slug: &str, title: &str, kind: &str) -> BrowseHit {
    BrowseHit {
        slug: slug.into(),
        title: title.into(),
        kind: Some(kind.into()),
    }
}

#[test]
fn threshold_is_a_floor_of_three_with_proportional_slack() {
    assert_eq!(ep_count_threshold(1), 3);
    assert_eq!(ep_count_threshold(24), 3);
    assert_eq!(ep_count_threshold(1100), 110);
}

#[tokio::test]
async fn expected_count_picks_the_closest_probed_candidate() {
    // Movie (1 ep) vs series (26 eps): expected 26 must pick the
    // series even though the movie ranks first.
    let client = AnidbClient::new(EpisodesTable(&[(11, 1), (22, 26)]));
    let hits = [
        hit("gintama-movie-11", "Gintama: The Movie"),
        hit("gintama-22", "Gintama"),
    ];
    let picked = pick_candidate(&client, &hits, Some(26), "Gintama", None, None)
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "gintama-22");
    assert_eq!(picked.episodes.len(), 26);
}

#[tokio::test]
async fn best_distance_beyond_threshold_is_rejected_not_guessed() {
    // Only sibling available is 6 episodes away from an expected 1 —
    // the Wing-for-1979 shape. A silent pick plays the wrong show.
    let client = AnidbClient::new(EpisodesTable(&[(33, 7)]));
    let hits = [hit("wrong-sibling-33", "Some Sibling")];
    let err = pick_candidate(&client, &hits, Some(1), "The Real Movie", None, None)
        .await
        .expect_err("rejected");
    assert!(matches!(err, AniError::NoResults));
}

#[tokio::test]
async fn ties_prefer_the_exact_title_match() {
    // Two candidates at equal distance; the one named exactly like
    // the search wins regardless of order.
    let client = AnidbClient::new(EpisodesTable(&[(44, 12), (55, 12)]));
    let hits = [
        hit("other-cut-44", "Other Cut"),
        hit("the-show-55", "The Show"),
    ];
    let picked = pick_candidate(&client, &hits, Some(12), "the show", None, None)
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "the-show-55");
}

#[tokio::test]
async fn unknown_expected_count_prefers_exact_title_then_first() {
    let client = AnidbClient::new(EpisodesTable(&[(66, 3), (77, 8)]));
    let hits = [hit("first-66", "First"), hit("wanted-77", "Wanted")];
    let picked = pick_candidate(&client, &hits, None, "wanted", None, None)
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "wanted-77");

    let picked = pick_candidate(&client, &hits, None, "no such title", None, None)
        .await
        .expect("falls back to first");
    assert_eq!(picked.hit.slug, "first-66");
}

#[tokio::test]
async fn probe_errors_skip_the_candidate_instead_of_aborting() {
    // First candidate's episodes endpoint 404s (transient or gone);
    // the second still wins.
    let client = AnidbClient::new(EpisodesTable(&[(88, 13)]));
    let hits = [hit("dead-99", "Dead"), hit("alive-88", "Alive")];
    let picked = pick_candidate(&client, &hits, Some(13), "Alive", None, None)
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "alive-88");
}

#[tokio::test]
async fn probing_stops_at_the_bound() {
    // Six candidates, the only in-threshold one is past the bound —
    // the pick fails rather than probing forever.
    let client = AnidbClient::new(EpisodesTable(&[
        (1, 100),
        (2, 100),
        (3, 100),
        (4, 100),
        (5, 100),
        (6, 12),
    ]));
    let hits = [
        hit("a-1", "A"),
        hit("b-2", "B"),
        hit("c-3", "C"),
        hit("d-4", "D"),
        hit("e-5", "E"),
        hit("f-6", "F"),
    ];
    let err = pick_candidate(&client, &hits, Some(12), "F", None, None)
        .await
        .expect_err("bounded");
    assert!(matches!(err, AniError::NoResults));
}

#[tokio::test]
async fn an_all_not_found_pool_is_an_answered_dead_end() {
    // Every considered candidate's episodes probe was ANSWERED
    // not-found (stale slugs): the pool is dead but the provider is
    // healthy. Classifying this as Network let three such resolves
    // open the global breaker on a provider that answered every
    // request. The verdict is the not-found-shaped upstream status —
    // still never the persistable clean miss, which the resolve walk
    // keeps guarding.
    //
    // This replaces all_probes_failing_is_transient_not_absence: its
    // fake answered 404s, so it was pinning the answered case to the
    // transport verdict. The transport case keeps its own pin below.
    let client = AnidbClient::new(EpisodesTable(&[]));
    let hits = [hit("a-1", "A"), hit("b-2", "B")];
    let err = pick_candidate(&client, &hits, Some(12), "A", None, None)
        .await
        .expect_err("dead pool");
    assert!(
        matches!(err, AniError::Upstream { status: 404 }),
        "got {err:?}"
    );
}

/// A fetch whose probes die on transport — no answer at all.
struct TransportDeadEpisodes;

#[async_trait::async_trait]
impl AnidbFetch for TransportDeadEpisodes {
    async fn get(&self, url: &str) -> crate::error::Result<FetchResponse> {
        if url.contains("/episodes") {
            return Err(AniError::Network);
        }
        Ok(FetchResponse {
            status: 404,
            body: String::new(),
        })
    }
}

#[tokio::test]
async fn all_probes_dying_on_transport_stays_transient() {
    // Nothing was learned about the show when the probes never got
    // an answer: the verdict must be the transient Network so the
    // caller records distress and never persists absence.
    let client = AnidbClient::new(TransportDeadEpisodes);
    let hits = [hit("a-1", "A"), hit("b-2", "B")];
    let err = pick_candidate(&client, &hits, Some(12), "A", None, None)
        .await
        .expect_err("transient");
    assert!(matches!(err, AniError::Network), "got {err:?}");
}

/// One candidate's probe dies on transport while the other answers —
/// with a count far outside tolerance.
struct MixedEpisodes;

#[async_trait::async_trait]
impl AnidbFetch for MixedEpisodes {
    async fn get(&self, url: &str) -> crate::error::Result<FetchResponse> {
        if url.contains("/api/frontend/anime/11/episodes") {
            return Err(AniError::Network);
        }
        if url.contains("/api/frontend/anime/22/episodes") {
            let rows: Vec<String> = (1..=7u32)
                .map(|n| format!("{{\"id\":{},\"number\":{}}}", 22_000 + n, n))
                .collect();
            return Ok(FetchResponse {
                status: 200,
                body: format!("{{\"episodes\":[{}]}}", rows.join(",")),
            });
        }
        Ok(FetchResponse {
            status: 404,
            body: String::new(),
        })
    }
}

#[tokio::test]
async fn a_rejected_pool_with_failed_probes_stays_transient() {
    // Candidate 11's probe died on transport; candidate 22 answered
    // seven episodes against an expected 24 — outside tolerance, so
    // the pool is rejected. But the dead candidate may have been the
    // right show: the rejection is only a clean verdict when every
    // candidate got to answer, and a NoResults here rides the walk
    // into a persistable clean miss that hides the show for the
    // negative TTL. Weather stays weather.
    let client = AnidbClient::new(MixedEpisodes);
    let hits = [hit("the-show-11", "The Show"), hit("the-spinoff-22", "S")];
    let err = pick_candidate(&client, &hits, Some(24), "the show", None, None)
        .await
        .expect_err("an out-of-tolerance survivor beside a dead probe");
    assert!(matches!(err, AniError::Network), "got {err:?}");
}

#[tokio::test]
async fn empty_hits_are_no_results() {
    let client = AnidbClient::new(EpisodesTable(&[]));
    let err = pick_candidate(&client, &[], Some(12), "x", None, None)
        .await
        .expect_err("empty");
    assert!(matches!(err, AniError::NoResults));
}

/// Episodes plus premiere years, served the way the provider does:
/// counts from the episodes API, years off each slug's detail page.
/// A `None` year answers the detail route with a 404 — the soft-miss
/// shape a page without a season link produces.
struct YearTable(&'static [(u64, u32, Option<u32>)]);

#[async_trait::async_trait]
impl AnidbFetch for YearTable {
    async fn get(&self, url: &str) -> crate::error::Result<FetchResponse> {
        for (id, count, year) in self.0 {
            if url.contains(&format!("/api/frontend/anime/{id}/episodes")) {
                let rows: Vec<String> = (1..=*count)
                    .map(|n| format!("{{\"id\":{},\"number\":{}}}", id * 1000 + u64::from(n), n))
                    .collect();
                return Ok(FetchResponse {
                    status: 200,
                    body: format!("{{\"episodes\":[{}]}}", rows.join(",")),
                });
            }
            if !url.contains("/api/") && url.ends_with(&format!("-{id}")) {
                return match year {
                    Some(y) => Ok(FetchResponse {
                        status: 200,
                        body: format!("<a href=\"/browse?season=fall&year={y}\">Fall {y}</a>"),
                    }),
                    None => Ok(FetchResponse {
                        status: 404,
                        body: String::new(),
                    }),
                };
            }
        }
        Ok(FetchResponse {
            status: 404,
            body: String::new(),
        })
    }
}

#[tokio::test]
async fn a_year_match_beats_count_tied_cour_siblings() {
    // The TYBW shape that mispicked live: three finished cours at
    // 13/14/13 episodes against an expected 13, while the airing
    // fourth part has only aired 2. Count alone lands on Part 1; the
    // year names the part — only the 2026 candidate is the clicked
    // show, and its short count is what airing looks like.
    let client = AnidbClient::new(YearTable(&[
        (675, 13, Some(2022)),
        (676, 14, Some(2023)),
        (677, 13, Some(2024)),
        (6378, 2, Some(2026)),
    ]));
    let hits = [
        hit("tybw-675", "TYBW"),
        hit("tybw-the-conflict-676", "TYBW - The Conflict"),
        hit("tybw-the-separation-677", "TYBW - The Separation"),
        hit("tybw-the-calamity-6378", "TYBW - The Calamity"),
    ];
    let picked = pick_candidate(
        &client,
        &hits,
        Some(13),
        "tybw kashin-tan",
        Some(2026),
        None,
    )
    .await
    .expect("picked");
    assert_eq!(picked.hit.slug, "tybw-the-calamity-6378");
}

#[tokio::test]
async fn year_mismatches_are_excluded_before_scoring() {
    // Gundam 1979 vs Wing 1995: both TV-length and both within the
    // count tolerance, so the mismatched sibling can win on distance
    // or order. The year is an identity filter, not a late tie-break.
    let client = AnidbClient::new(YearTable(&[(2, 45, Some(1995)), (1, 43, Some(1979))]));
    // Neither title matches the search term exactly, so without the
    // year the count tie falls to provider order — the 1995 sibling.
    let hits = [
        hit("mobile-suit-gundam-wing-2", "Mobile Suit Gundam Wing"),
        hit("mobile-suit-gundam-1", "Mobile Suit Gundam"),
    ];
    let picked = pick_candidate(
        &client,
        &hits,
        Some(44),
        "kidou senshi gundam",
        Some(1979),
        None,
    )
    .await
    .expect("picked");
    assert_eq!(picked.hit.slug, "mobile-suit-gundam-1");
}

#[tokio::test]
async fn unknown_years_pass_the_identity_filter() {
    // A detail page that 404s or names no season must not exclude its
    // candidate — the year is a soft hint, and a provider hiccup on
    // one page must not hide the right show.
    let client = AnidbClient::new(YearTable(&[(2, 12, Some(1990)), (1, 14, None)]));
    let hits = [hit("old-2", "Old"), hit("mystery-1", "Mystery")];
    let picked = pick_candidate(&client, &hits, Some(12), "unrelated", Some(2020), None)
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "mystery-1");
}

#[tokio::test]
async fn an_all_mismatched_pool_is_rejected_so_the_walk_moves_on() {
    // Token-matching searches surface pools that contain the show
    // nowhere: the full romaji "Tai-Ari deshita..." returned four
    // decades-old shows, one of them count-tied, and the old
    // count-decides fallback opened it. Every candidate carrying a
    // known year far from Kitsu's is positive evidence the pool is
    // wrong — reject it and let the walk try the next alias, which
    // is where the real show turns up. (A provider-side markup
    // change can't trip this: unparseable years read as unknown and
    // unknown years never exclude.)
    let client = AnidbClient::new(YearTable(&[(1, 12, Some(2000)), (2, 30, Some(2001))]));
    let hits = [hit("a-1", "A"), hit("b-2", "B")];
    let err = pick_candidate(&client, &hits, Some(12), "a", Some(2026), None)
        .await
        .expect_err("pool rejected");
    assert!(matches!(err, AniError::NoResults));
}

#[tokio::test]
async fn a_lone_candidate_with_a_mismatched_year_is_rejected() {
    // The identity filter applies even when there is nothing to
    // discriminate between: one count-tied candidate from the wrong
    // decade is still the wrong show.
    let client = AnidbClient::new(YearTable(&[(1, 12, Some(2001))]));
    let hits = [hit("old-show-1", "Old Show")];
    let err = pick_candidate(&client, &hits, Some(12), "new show", Some(2022), None)
        .await
        .expect_err("rejected");
    assert!(matches!(err, AniError::NoResults));
}

#[tokio::test]
async fn a_lone_candidate_with_a_matching_year_wins() {
    let client = AnidbClient::new(YearTable(&[(1, 12, Some(2022))]));
    let hits = [hit("new-show-1", "New Show")];
    let picked = pick_candidate(&client, &hits, Some(12), "new show", Some(2022), None)
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "new-show-1");
}

#[tokio::test]
async fn a_lone_airing_part_with_a_confirmed_year_survives_the_count_gap() {
    // An alias can return the airing part alone. Its aired count sits
    // far under Kitsu's whole-season count, but its detail year
    // matches — the same airing-part evidence that rescues it inside
    // a sibling pool must rescue it here, or exactly the
    // currently-airing show becomes unresolvable through that alias.
    let client = AnidbClient::new(YearTable(&[(6378, 2, Some(2026))]));
    let hits = [hit("tybw-the-calamity-6378", "TYBW - The Calamity")];
    let picked = pick_candidate(
        &client,
        &hits,
        Some(13),
        "tybw kashin-tan",
        Some(2026),
        None,
    )
    .await
    .expect("picked");
    assert_eq!(picked.hit.slug, "tybw-the-calamity-6378");
}

#[tokio::test]
async fn year_filters_apply_without_an_episode_count() {
    // Kitsu's count is often null while a part is airing — exactly
    // when the cour siblings collide. With no count, the year filter
    // plus the provider's own ranking must still land on the right
    // part instead of the first sibling.
    let client = AnidbClient::new(YearTable(&[(675, 13, Some(2022)), (6378, 2, Some(2026))]));
    let hits = [
        hit("tybw-675", "TYBW"),
        hit("tybw-the-calamity-6378", "TYBW - The Calamity"),
    ];
    let picked = pick_candidate(&client, &hits, None, "tybw kashin-tan", Some(2026), None)
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "tybw-the-calamity-6378");
}

// Silence the unused import when EpisodeRef isn't referenced directly.
#[allow(dead_code)]
fn _types(_: EpisodeRef) {}

#[tokio::test]
async fn a_countless_pick_prefers_the_year_confirmed_candidate() {
    // Kitsu's count is null while a show airs — exactly when the
    // provider's token search pollutes the pool. Positional order
    // must lose to a candidate whose detail year matches Kitsu's.
    let client = AnidbClient::new(YearTable(&[(1, 1, None), (2, 4, Some(2026))]));
    let hits = [
        hit("some-movie-1", "Some Movie"),
        hit("the-show-2", "The Show"),
    ];
    let picked = pick_candidate(&client, &hits, None, "unrelated words", Some(2026), None)
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "the-show-2");
}

#[tokio::test]
async fn a_countless_garbage_pool_with_only_unknown_survivors_is_rejected() {
    // The live Tai-Ari shape: the full romaji title token-matched
    // four unrelated shows; the year filter excluded the three with
    // known decades-off years, leaving one movie whose detail page
    // names no season. With no count to vouch for it, a pool the
    // year evidence already showed to be mostly wrong must not be
    // won by the one candidate with no identity evidence at all —
    // rejecting it lets the walk's next alias find the real entry.
    let client = AnidbClient::new(YearTable(&[
        (1, 1, Some(1991)),
        (2, 4, Some(1975)),
        (3, 12, Some(2001)),
        (4, 1, None),
    ]));
    let hits = [
        hit("old-movie-1", "Old Movie"),
        hit("old-robot-2", "Old Robot"),
        hit("old-maids-3", "Old Maids"),
        hit("unknown-movie-4", "Unknown Movie"),
    ];
    let err = pick_candidate(&client, &hits, None, "the real show", Some(2026), None)
        .await
        .expect_err("pool rejected");
    assert!(matches!(err, AniError::NoResults));
}

#[tokio::test]
async fn a_countless_clean_pool_with_an_unknown_year_still_picks_first() {
    // The year stays a soft hint when it disproved nothing: a pool
    // with no exclusions and an unknown-year first hit keeps the
    // provider's own ranking, so pages without season links don't
    // become unresolvable.
    let client = AnidbClient::new(YearTable(&[(1, 12, None)]));
    let hits = [hit("plain-show-1", "Plain Show")];
    let picked = pick_candidate(&client, &hits, None, "unrelated", Some(2026), None)
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "plain-show-1");
}

#[tokio::test]
async fn an_off_by_one_year_carries_no_identity_for_the_rescue() {
    // The live Ninjaboy mispick: a Jan-2025 movie in Tai-Ari's
    // token-garbage pool sat within the year filter's ±1 tolerance
    // (the December-premiere allowance), so the airing-part rescue
    // treated it as identity-confirmed and picked a 1-episode movie
    // for a 12-episode show. Off-by-one keeps a candidate in the
    // pool but must not vouch for it: with the far-off siblings
    // excluded and no exact-year survivor, the pool is rejected so
    // the next alias finds the real entry.
    let client = AnidbClient::new(YearTable(&[
        (1, 1, Some(1991)),
        (2, 12, Some(2001)),
        (3, 1, Some(2025)),
    ]));
    let hits = [
        hit("old-movie-1", "Old Movie"),
        hit("old-maids-2", "Old Maids"),
        hit("boundary-movie-3", "Boundary Movie"),
    ];
    let err = pick_candidate(&client, &hits, Some(12), "the real show", Some(2026), None)
        .await
        .expect_err("no exact-year survivor to rescue");
    assert!(matches!(err, AniError::NoResults));
}

#[tokio::test]
async fn an_off_by_one_year_still_competes_on_count() {
    // The tolerance keeps its original job: a season-boundary entry
    // (Kitsu says 2025, the provider files the winter premiere under
    // 2026) whose count agrees must keep winning outright.
    let client = AnidbClient::new(YearTable(&[(1, 12, Some(2026))]));
    let hits = [hit("boundary-show-1", "Boundary Show")];
    let picked = pick_candidate(&client, &hits, Some(12), "boundary show", Some(2025), None)
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "boundary-show-1");
}

#[tokio::test]
async fn a_movie_badge_is_disproof_against_a_multi_episode_expectation() {
    // The browse card names its format. A Movie cannot be the
    // 12-episode series the caller expects, however well its count
    // or position scores — the same class of mispick the allanime
    // picker's type filter used to catch.
    let client = AnidbClient::new(EpisodesTable(&[(1, 12), (2, 12)]));
    let hits = [
        typed_hit("recap-movie-1", "Recap Movie", "Movie"),
        typed_hit("the-series-2", "The Series", "TV"),
    ];
    let picked = pick_candidate(&client, &hits, Some(12), "unrelated", None, None)
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "the-series-2");
}

#[tokio::test]
async fn a_pool_of_only_movies_under_a_series_expectation_is_rejected() {
    let client = AnidbClient::new(EpisodesTable(&[(1, 12)]));
    let hits = [typed_hit("recap-movie-1", "Recap Movie", "Movie")];
    let err = pick_candidate(&client, &hits, Some(12), "the series", None, None)
        .await
        .expect_err("a movie cannot satisfy a series expectation");
    assert!(matches!(err, AniError::NoResults));
}

#[tokio::test]
async fn a_single_video_expectation_keeps_movie_candidates() {
    // Kitsu movies expect one episode — the badge agrees, no
    // disproof. Unknown badges never exclude either way.
    let client = AnidbClient::new(EpisodesTable(&[(1, 1)]));
    let hits = [typed_hit("the-movie-1", "The Movie", "Movie")];
    let picked = pick_candidate(&client, &hits, Some(1), "the movie", None, None)
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "the-movie-1");
}

#[tokio::test]
async fn a_movie_badge_is_disproof_against_a_special_subtype() {
    // The Konoha Gakuen Den shape, live: Kitsu's entry is a 1-episode
    // SPECIAL the provider doesn't carry, and the pool holds the
    // franchise's movies — count-tied at 1 and within the year
    // tolerance. Kitsu's subtype is the signal that disproves them:
    // a Movie cannot be the clicked Special, whatever its count or
    // year. With every candidate disproven the pool rejects, the
    // walk exhausts cleanly, and the frontend gets its "isn't on the
    // streaming source" overlay instead of the wrong film.
    let client = AnidbClient::new(YearTable(&[(1, 1, Some(2007)), (2, 500, Some(2007))]));
    let hits = [
        typed_hit("franchise-movie-1-1", "Franchise Movie 1", "Movie"),
        typed_hit("franchise-3687-2", "Franchise", "TV"),
    ];
    let err = pick_candidate(
        &client,
        &hits,
        Some(1),
        "franchise side story",
        Some(2008),
        Some("special"),
    )
    .await
    .expect_err("no candidate can be the special");
    assert!(matches!(err, AniError::NoResults));
}

#[tokio::test]
async fn a_non_movie_badge_is_disproof_against_a_movie_subtype() {
    // The inverse of the special-vs-movie shape: Kitsu says the
    // clicked entry IS a movie, and a one-episode Special/TV/OVA
    // card with the same title and year ties on every remaining
    // signal. A known non-movie badge cannot be the movie; unknown
    // badges stay soft.
    let client = AnidbClient::new(EpisodesTable(&[(1, 1), (2, 1)]));
    let hits = [
        typed_hit("franchise-special-1", "The Title", "Special"),
        typed_hit("the-movie-2", "The Title", "Movie"),
    ];
    let picked = pick_candidate(&client, &hits, Some(1), "unrelated", None, Some("movie"))
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "the-movie-2");
}

#[tokio::test]
async fn an_unknown_badge_survives_a_movie_subtype() {
    // A card without a format badge stays eligible — the filter
    // works on positive disproof only, so a provider markup change
    // degrades to badge-blind picking instead of emptying pools.
    let client = AnidbClient::new(EpisodesTable(&[(1, 1)]));
    let hits = [hit("plain-1", "Plain")];
    let picked = pick_candidate(&client, &hits, Some(1), "plain", None, Some("movie"))
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "plain-1");
}

#[tokio::test]
async fn a_movie_subtype_keeps_movie_candidates() {
    let client = AnidbClient::new(EpisodesTable(&[(1, 1)]));
    let hits = [typed_hit("the-movie-1", "The Movie", "Movie")];
    let picked = pick_candidate(&client, &hits, Some(1), "the movie", None, Some("movie"))
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "the-movie-1");
}

/// A fetch whose first episodes probe answers a Cloudflare-shaped
/// refusal and which counts every episodes request it receives.
struct RefusingEpisodes {
    probes: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait::async_trait]
impl AnidbFetch for RefusingEpisodes {
    async fn get(&self, url: &str) -> crate::error::Result<FetchResponse> {
        if url.contains("/episodes") {
            self.probes
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            return Err(AniError::Upstream { status: 403 });
        }
        Ok(FetchResponse {
            status: 404,
            body: String::new(),
        })
    }
}

/// A fetch whose detail pages answer a refusal while the episodes
/// endpoint stays healthy, counting the detail requests it receives.
struct RefusingDetails {
    details: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait::async_trait]
impl AnidbFetch for RefusingDetails {
    async fn get(&self, url: &str) -> crate::error::Result<FetchResponse> {
        if url.contains("/episodes") {
            let rows: Vec<String> = (1..=12)
                .map(|n| format!("{{\"id\":{},\"number\":{}}}", 1000 + n, n))
                .collect();
            return Ok(FetchResponse {
                status: 200,
                body: format!("{{\"episodes\":[{}]}}", rows.join(",")),
            });
        }
        self.details
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(AniError::Upstream { status: 403 })
    }
}

#[tokio::test]
async fn a_detail_probe_refusal_stops_the_pick() {
    // A refusal on a detail page is the provider blocking this
    // client, not a page without a season link: reading it as an
    // unknown year keeps requesting the rest of the pool's detail
    // pages and then selects year-blind through the block. The
    // refusal must surface typed after exactly one detail request.
    let details = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let client = AnidbClient::new(RefusingDetails {
        details: details.clone(),
    });
    let hits = [hit("show-a-1", "Show A"), hit("show-b-2", "Show B")];
    let err = pick_candidate(&client, &hits, Some(12), "Show A", Some(2026), None)
        .await
        .expect_err("refused");
    assert!(matches!(err, AniError::Upstream { .. }), "got {err:?}");
    assert_eq!(
        details.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the pick kept probing detail pages past the refusal"
    );
}

#[tokio::test]
async fn an_upstream_refusal_stops_the_probe_walk() {
    // A refusal mid-probe is the provider blocking, not a candidate
    // missing: continuing to probe the rest of the pool turns one
    // block into a burst of further requests, and the alias walk can
    // then repeat the burst per alias. The refusal must surface
    // immediately, typed, after exactly one probe.
    let probes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let client = AnidbClient::new(RefusingEpisodes {
        probes: probes.clone(),
    });
    let hits = [hit("show-a-1", "Show A"), hit("show-b-2", "Show B")];
    let err = pick_candidate(&client, &hits, Some(12), "Show A", None, None)
        .await
        .expect_err("refused");
    assert!(matches!(err, AniError::Upstream { .. }), "got {err:?}");
    assert_eq!(
        probes.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the walk kept probing past the refusal"
    );
}

#[tokio::test]
async fn format_incompatible_hits_do_not_crowd_the_probe_head() {
    // Five franchise movies ranked ahead of the series: the badge
    // filter costs nothing, so it must run before the bounded probe
    // head is taken. Filtered after, the movies fill all five probe
    // slots, the series at rank six is never considered, and the
    // alias is rejected — ultimately reported absent despite a
    // perfectly valid result.
    let client = AnidbClient::new(EpisodesTable(&[(66, 26)]));
    let hits = [
        typed_hit("film-1-11", "Film 1", "Movie"),
        typed_hit("film-2-22", "Film 2", "Movie"),
        typed_hit("film-3-33", "Film 3", "Movie"),
        typed_hit("film-4-44", "Film 4", "Movie"),
        typed_hit("film-5-55", "Film 5", "Movie"),
        hit("the-series-66", "The Series"),
    ];
    let picked = pick_candidate(&client, &hits, Some(26), "the series", None, None)
        .await
        .expect("the sixth-ranked series is the only compatible hit");
    assert_eq!(picked.hit.slug, "the-series-66");
}
