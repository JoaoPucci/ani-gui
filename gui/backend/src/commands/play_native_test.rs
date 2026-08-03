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
    let picked = pick_candidate(&client, &hits, Some(26), "Gintama", None)
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
    let err = pick_candidate(&client, &hits, Some(1), "The Real Movie", None)
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
    let picked = pick_candidate(&client, &hits, Some(12), "the show", None)
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "the-show-55");
}

#[tokio::test]
async fn unknown_expected_count_prefers_exact_title_then_first() {
    let client = AnidbClient::new(EpisodesTable(&[(66, 3), (77, 8)]));
    let hits = [hit("first-66", "First"), hit("wanted-77", "Wanted")];
    let picked = pick_candidate(&client, &hits, None, "wanted", None)
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "wanted-77");

    let picked = pick_candidate(&client, &hits, None, "no such title", None)
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
    let picked = pick_candidate(&client, &hits, Some(13), "Alive", None)
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
    let err = pick_candidate(&client, &hits, Some(12), "F", None)
        .await
        .expect_err("bounded");
    assert!(matches!(err, AniError::NoResults));
}

#[tokio::test]
async fn all_probes_failing_is_transient_not_absence() {
    // Every considered candidate's episodes fetch failed: nothing was
    // learned about the show, so the verdict must be the transient
    // Network — a NoResults here would flow into the persistable
    // clean-miss path and hide a real show behind the negative TTL.
    let client = AnidbClient::new(EpisodesTable(&[]));
    let hits = [hit("a-1", "A"), hit("b-2", "B")];
    let err = pick_candidate(&client, &hits, Some(12), "A", None)
        .await
        .expect_err("transient");
    assert!(matches!(err, AniError::Network));
}

#[tokio::test]
async fn empty_hits_are_no_results() {
    let client = AnidbClient::new(EpisodesTable(&[]));
    let err = pick_candidate(&client, &[], Some(12), "x", None)
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
    let picked = pick_candidate(&client, &hits, Some(13), "tybw kashin-tan", Some(2026))
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
    let picked = pick_candidate(&client, &hits, Some(44), "kidou senshi gundam", Some(1979))
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
    let picked = pick_candidate(&client, &hits, Some(12), "unrelated", Some(2020))
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "mystery-1");
}

#[tokio::test]
async fn an_all_mismatched_pool_falls_back_to_count() {
    // When every known year disagrees with Kitsu's, the year data is
    // suspect — count keeps deciding rather than emptying the pool.
    let client = AnidbClient::new(YearTable(&[(1, 12, Some(2000)), (2, 30, Some(2001))]));
    let hits = [hit("a-1", "A"), hit("b-2", "B")];
    let picked = pick_candidate(&client, &hits, Some(12), "a", Some(2026))
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "a-1");
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
    let picked = pick_candidate(&client, &hits, None, "tybw kashin-tan", Some(2026))
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "tybw-the-calamity-6378");
}

// Silence the unused import when EpisodeRef isn't referenced directly.
#[allow(dead_code)]
fn _types(_: EpisodeRef) {}
