use super::*;
use crate::scraper::anidb::{BrowseHit, EpisodeRef, FetchResponse};

/// Serves the full chain — languages, embed, master — for exactly
/// one episode id; every other id 404s. Resolving proves which row
/// the matcher chased.
struct OnlyEpisode(u64);

#[async_trait::async_trait]
impl crate::scraper::anidb::AnidbFetch for OnlyEpisode {
    async fn get(&self, url: &str) -> crate::error::Result<FetchResponse> {
        if url.contains(&format!("/api/frontend/episode/{}/languages", self.0)) {
            return Ok(FetchResponse {
                status: 200,
                body: r#"{"languages":[{"code":"jpn","embed_url":"https://cdn.example/e/x"}]}"#
                    .into(),
            });
        }
        if url.contains("/e/x") {
            return Ok(FetchResponse {
                status: 200,
                body: "player.setup({ file: 'https://cdn.example/x/master.m3u8' });".into(),
            });
        }
        if url.contains("/x/master.m3u8") {
            return Ok(FetchResponse {
                status: 200,
                body: "#EXTM3U\n".into(),
            });
        }
        Ok(FetchResponse {
            status: 404,
            body: String::new(),
        })
    }
}

fn ep(id: u64, number: u32, tag: Option<&str>) -> EpisodeRef {
    EpisodeRef {
        id,
        number,
        number2: tag.map(str::to_string),
    }
}

fn show(episodes: Vec<EpisodeRef>) -> PickedShow {
    PickedShow {
        hit: BrowseHit {
            slug: "the-show-77".into(),
            title: "The Show".into(),
            kind: None,
        },
        episodes,
    }
}

#[tokio::test]
async fn a_tagged_continuation_maps_per_entry_numbers() {
    // The cumulative numbering lives in the tags while slots restart
    // at 1: per-entry request 1 is display 41.
    let picked = show(vec![ep(10, 1, Some("41")), ep(11, 2, Some("42"))]);
    let client = crate::scraper::anidb::AnidbClient::new(OnlyEpisode(10));
    let url = resolve_episode(&client, &picked, "1", "sub", "best")
        .await
        .expect("per-entry 1 is display 41");
    assert_eq!(url, "https://cdn.example/x/master.m3u8");
}

#[tokio::test]
async fn a_fractional_request_maps_through_the_offset_too() {
    // The strip advertises per-entry "1.5" for the provider's
    // "41.5" recap; the click must translate back the same way the
    // advertisement translated forward.
    let picked = show(vec![
        ep(10, 1, Some("41")),
        ep(12, 2, Some("41.5")),
        ep(11, 3, Some("42")),
    ]);
    let client = crate::scraper::anidb::AnidbClient::new(OnlyEpisode(12));
    let url = resolve_episode(&client, &picked, "1.5", "sub", "best")
        .await
        .expect("per-entry 1.5 is display 41.5");
    assert_eq!(url, "https://cdn.example/x/master.m3u8");
}

#[tokio::test]
async fn a_zero_padded_integer_tag_still_answers_its_episode() {
    // The provider can file the tag in a non-canonical string form:
    // "041" is display episode 41 to the offset and the cap, which
    // parse it numerically — the matcher's byte comparison against
    // the constructed "41" is the one place it dies.
    let picked = show(vec![ep(10, 1, Some("041")), ep(11, 2, Some("042"))]);
    let client = crate::scraper::anidb::AnidbClient::new(OnlyEpisode(10));
    let url = resolve_episode(&client, &picked, "1", "sub", "best")
        .await
        .expect("per-entry 1 is display 041 = 41");
    assert_eq!(url, "https://cdn.example/x/master.m3u8");
}

#[tokio::test]
async fn a_trailing_zero_fraction_matches_the_normalized_click() {
    // Availability advertises "3.50" verbatim; the frontend parses
    // extras numerically and the click comes back as "3.5". The
    // same row answered numerically for the cap and the extras —
    // it must answer the click too.
    let picked = show(vec![
        ep(1, 1, None),
        ep(2, 2, None),
        ep(3, 3, None),
        ep(4, 4, Some("3.50")),
    ]);
    let client = crate::scraper::anidb::AnidbClient::new(OnlyEpisode(4));
    let url = resolve_episode(&client, &picked, "3.5", "sub", "best")
        .await
        .expect("the normalized click finds the padded tag");
    assert_eq!(url, "https://cdn.example/x/master.m3u8");
}

proptest::proptest! {
    /// Numeric identity over padded forms: leading zeros on the
    /// integer part and trailing zeros after the fraction never
    /// change what a tag matches, in either direction; genuinely
    /// nonnumeric tags keep exact equality.
    #[test]
    fn tag_matching_ignores_numeric_padding(
        n in 0u32..100_000,
        frac in 0u32..10,
        lead in 0usize..3,
        tail in 0usize..3,
    ) {
        let canonical_int = format!("{n}");
        let padded_int = format!("{}{n}", "0".repeat(lead));
        proptest::prop_assert!(tag_matches(&padded_int, &canonical_int));
        proptest::prop_assert!(tag_matches(&canonical_int, &padded_int));
        let canonical = format!("{n}.{frac}");
        let padded = format!("{}{n}.{frac}{}", "0".repeat(lead), "0".repeat(tail));
        proptest::prop_assert!(tag_matches(&padded, &canonical));
        proptest::prop_assert!(tag_matches(&canonical, &padded));
        proptest::prop_assert!(tag_matches("SP", "SP"));
        proptest::prop_assert!(!tag_matches("SP", "sp"));
        proptest::prop_assert!(!tag_matches(&canonical, &format!("{}.{}", n + 1, frac)));
    }
}

#[tokio::test]
async fn an_integer_request_skips_the_recap_in_its_slot() {
    // The captured provider shape: a recap occupies integer slot 4
    // (number: 4) while its displayed identity is "3.5" (number2),
    // and the true episode 4 sits one slot later under tag "4". A
    // request for episode 4 matched by slot plays the recap; the
    // display tag is the row's identity, for integers as much as
    // for decimals.
    let picked = show(vec![
        ep(1, 1, None),
        ep(2, 2, None),
        ep(3, 3, None),
        ep(4, 4, Some("3.5")),
        ep(5, 5, Some("4")),
    ]);
    let client = crate::scraper::anidb::AnidbClient::new(OnlyEpisode(5));
    let url = resolve_episode(&client, &picked, "4", "sub", "best")
        .await
        .expect("the tag-4 row carries episode 4");
    assert_eq!(url, "https://cdn.example/x/master.m3u8");
}

#[tokio::test]
async fn a_recap_slot_without_its_true_episode_is_a_dead_end() {
    // When the listing ends on the recap, a request for the slot's
    // integer must dead-end rather than serve the recap's stream —
    // the show has no episode 4, whatever slot the recap sits in.
    let picked = show(vec![
        ep(1, 1, None),
        ep(2, 2, None),
        ep(3, 3, None),
        ep(4, 4, Some("3.5")),
    ]);
    let client = crate::scraper::anidb::AnidbClient::new(OnlyEpisode(4));
    let ne = resolve_episode(&client, &picked, "4", "sub", "best")
        .await
        .expect_err("the recap must not answer for episode 4");
    assert!(
        matches!(ne.error, AniError::NoResults),
        "expected the dead end, got {:?}",
        ne.error
    );
    assert!(!ne.clean_miss);
}

#[tokio::test]
async fn an_integer_display_tag_still_matches_through_the_offset() {
    // Continuation cours re-display their cumulative numbering in
    // tags; the per-entry request maps through the offset into the
    // same display space the tags live in.
    let picked = show(vec![ep(10, 41, None), ep(11, 42, Some("42"))]);
    let client = crate::scraper::anidb::AnidbClient::new(OnlyEpisode(11));
    let url = resolve_episode(&client, &picked, "2", "sub", "best")
        .await
        .expect("per-entry 2 is display 42");
    assert_eq!(url, "https://cdn.example/x/master.m3u8");
}

proptest::proptest! {
    /// The chain-failure decision table over every error shape: a
    /// provider block or gate refusal stops the walk with the error
    /// intact, answered dead ends (NoResults, non-block upstream
    /// statuses) move to the next alias, and everything else stays
    /// transient.
    #[test]
    fn chain_failures_classify_by_the_decision_table(
        kind in 0u8..6,
        status in 100u16..600,
    ) {
        let error = match kind {
            0 => AniError::Upstream { status },
            1 => AniError::GateRefused,
            2 => AniError::NoResults,
            3 => AniError::Network,
            4 => AniError::Timeout,
            _ => AniError::RateLimited {
                retry_after_secs: None,
            },
        };
        let stops = error.is_provider_block() || matches!(error, AniError::GateRefused);
        let dead_end =
            !stops && matches!(error, AniError::NoResults | AniError::Upstream { .. });
        let ne = NativeError {
            error,
            clean_miss: false,
            failed_at: None,
        };
        match classify_chain_failure(ne) {
            ChainOutcome::Stop(kept) => {
                proptest::prop_assert!(stops, "only blocks and refusals stop the walk");
                proptest::prop_assert!(
                    kept.error.is_provider_block()
                        || matches!(kept.error, AniError::GateRefused),
                    "the stop keeps the error's identity"
                );
            }
            ChainOutcome::DeadEnd => proptest::prop_assert!(dead_end),
            ChainOutcome::Transient => proptest::prop_assert!(!stops && !dead_end),
        }
    }
}
