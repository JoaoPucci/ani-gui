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
                    body: format!("[{}]", rows.join(",")),
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
    let picked = pick_candidate(&client, &hits, Some(26), "Gintama")
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
    let err = pick_candidate(&client, &hits, Some(1), "The Real Movie")
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
    let picked = pick_candidate(&client, &hits, Some(12), "the show")
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "the-show-55");
}

#[tokio::test]
async fn unknown_expected_count_prefers_exact_title_then_first() {
    let client = AnidbClient::new(EpisodesTable(&[(66, 3), (77, 8)]));
    let hits = [hit("first-66", "First"), hit("wanted-77", "Wanted")];
    let picked = pick_candidate(&client, &hits, None, "wanted")
        .await
        .expect("picked");
    assert_eq!(picked.hit.slug, "wanted-77");

    let picked = pick_candidate(&client, &hits, None, "no such title")
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
    let picked = pick_candidate(&client, &hits, Some(13), "Alive")
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
    let err = pick_candidate(&client, &hits, Some(12), "F")
        .await
        .expect_err("bounded");
    assert!(matches!(err, AniError::NoResults));
}

#[tokio::test]
async fn empty_hits_are_no_results() {
    let client = AnidbClient::new(EpisodesTable(&[]));
    let err = pick_candidate(&client, &[], Some(12), "x")
        .await
        .expect_err("empty");
    assert!(matches!(err, AniError::NoResults));
}

// Silence the unused import when EpisodeRef isn't referenced directly.
#[allow(dead_code)]
fn _types(_: EpisodeRef) {}
