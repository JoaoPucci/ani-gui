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
