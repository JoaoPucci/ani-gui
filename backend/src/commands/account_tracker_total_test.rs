//! The mark-watched write path against each tracker's own episode
//! total: what `push_progress_via` reads, and what it writes, when the
//! tracker counts a prologue Kitsu does not (Air: 13 against 12), when
//! the totals agree, when the tracker's entry is shorter than Kitsu's
//! show (Ranma ½: 18 against 161), and when the total is unknown.

use super::*;
use crate::account::provider::EntryUpdate;
use wiremock::matchers::{body_string_contains, method, path, query_param};

fn finale(ep: u32) -> EntryUpdate {
    EntryUpdate {
        status: Some(ListStatus::Completed),
        progress_episodes: Some(ep),
        ..Default::default()
    }
}

fn episode(ep: u32) -> EntryUpdate {
    EntryUpdate {
        progress_episodes: Some(ep),
        ..Default::default()
    }
}

/// A MAL server whose anime 21 has `total` episodes and a watching row
/// at `watched`, expecting `writes` PATCHes carrying every `form` pair.
async fn mal_server(total: u32, watched: u32, form: &[&str], writes: u64) -> wiremock::MockServer {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/21"))
        .and(query_param("fields", "my_list_status,num_episodes"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(format!(
            r#"{{"id":21,"num_episodes":{total},"my_list_status":{{"status":"watching","num_episodes_watched":{watched}}}}}"#
        )))
        .expect(1)
        .mount(&server)
        .await;
    let mut patch = wiremock::Mock::given(method("PATCH")).and(path("/anime/21/my_list_status"));
    for pair in form {
        patch = patch.and(body_string_contains(*pair));
    }
    patch
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"status":"completed","num_episodes_watched":13,"is_rewatching":false}"#,
        ))
        .expect(writes)
        .mount(&server)
        .await;
    server
}

async fn push_to_mal(server: &wiremock::MockServer, update: EntryUpdate) -> Option<ListEntry> {
    let state = state_with_kitsu("http://unused-kitsu");
    push_progress_via(
        &state,
        ProviderKind::MyAnimeList,
        &mal_provider(&server.uri()),
        &test_tokens(),
        ProviderMediaId(21),
        update,
    )
    .await
    .expect("push ok")
}

#[tokio::test]
async fn mal_the_finale_completes_a_tracker_that_counts_a_prologue() {
    let server = mal_server(13, 11, &["num_watched_episodes=13", "status=completed"], 1).await;
    assert!(push_to_mal(&server, finale(12)).await.is_some());
}

#[tokio::test]
async fn mal_the_finale_sends_its_own_number_when_the_totals_agree() {
    let server = mal_server(12, 11, &["num_watched_episodes=12", "status=completed"], 1).await;
    assert!(push_to_mal(&server, finale(12)).await.is_some());
}

#[tokio::test]
async fn mal_an_episode_past_a_shorter_entry_writes_nothing_once_the_entry_is_full() {
    // Ranma ½: watching Kitsu's 30 against MAL's 18-episode entry, the
    // row already at 18 — nothing is sent, and nothing past 18 ever is.
    let server = mal_server(18, 18, &[], 0).await;
    assert!(push_to_mal(&server, episode(30)).await.is_none());
}

#[tokio::test]
async fn mal_an_episode_past_a_shorter_entry_is_clamped_to_it() {
    let server = mal_server(18, 17, &["num_watched_episodes=18"], 1).await;
    assert!(push_to_mal(&server, episode(30)).await.is_some());
}

#[tokio::test]
async fn mal_an_unknown_total_sends_the_episode_as_is() {
    let server = mal_server(0, 4, &["num_watched_episodes=5"], 1).await;
    assert!(push_to_mal(&server, episode(5)).await.is_some());
}

#[tokio::test]
async fn anilist_the_finale_completes_a_tracker_that_counts_a_prologue() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .and(body_string_contains("mediaListEntry"))
        .and(body_string_contains("episodes"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"Media":{"episodes":13,"mediaListEntry":{"status":"CURRENT","progress":11}}}}"#,
        ))
        .expect(1)
        .mount(&server)
        .await;
    wiremock::Mock::given(method("POST"))
        .and(body_string_contains("SaveMediaListEntry"))
        .and(body_string_contains("\"progress\":13"))
        .and(body_string_contains("\"status\":\"COMPLETED\""))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"SaveMediaListEntry":{"mediaId":21,"status":"COMPLETED","progress":13,"score":0,"updatedAt":1,"repeat":0,"media":{"idMal":21,"title":{"romaji":"Air","english":null,"userPreferred":"Air"}}}}}"#,
        ))
        .expect(1)
        .mount(&server)
        .await;
    let provider = crate::meta::anilist_user::AniListProvider::with_bases(
        reqwest::Client::new(),
        server.uri(),
        "http://unused-token".to_string(),
    );
    let state = state_with_kitsu("http://unused-kitsu");
    let entry = push_progress_via(
        &state,
        ProviderKind::AniList,
        &provider,
        &test_tokens(),
        ProviderMediaId(21),
        finale(12),
    )
    .await
    .expect("push ok")
    .expect("written");
    assert_eq!(entry.progress_episodes, 13);
}
