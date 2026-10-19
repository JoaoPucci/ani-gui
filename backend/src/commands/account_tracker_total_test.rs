//! The mark-watched write path against each tracker's own episode
//! total: what `push_progress_via` reads, and what it writes, when the
//! tracker counts a prologue Kitsu does not (Air on AniList: 13 against 12), when
//! the totals agree, when the tracker's entry is shorter than Kitsu's
//! show (Ranma ½ on AniList: 18 against 161), and when the total is unknown;
//! and, on both trackers, an episode past Kitsu's finale, a rewatch's
//! finale and a replay behind the row.

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
    let row = format!(r#"{{"status":"watching","num_episodes_watched":{watched}}}"#);
    mal_server_with_row(total, &row, form, writes).await
}

/// [`mal_server`] with the user's `my_list_status` given whole, so a
/// scenario can hold a rewatching or completed row.
async fn mal_server_with_row(
    total: u32,
    row: &str,
    form: &[&str],
    writes: u64,
) -> wiremock::MockServer {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/21"))
        .and(query_param("fields", "my_list_status,num_episodes"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(format!(
                r#"{{"id":21,"num_episodes":{total},"my_list_status":{row}}}"#
            )),
        )
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

/// The bodies of the PATCHes `server` received.
async fn patch_bodies(server: &wiremock::MockServer) -> Vec<String> {
    server
        .received_requests()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.method.as_str() == "PATCH")
        .map(|r| String::from_utf8_lossy(&r.body).into_owned())
        .collect()
}

#[tokio::test]
async fn mal_the_finale_beside_a_prologue_total_sends_its_number_without_completing() {
    let server = mal_server(13, 11, &["num_watched_episodes=12"], 1).await;
    assert!(push_to_mal(&server, finale(12)).await.is_some());
    let bodies = patch_bodies(&server).await;
    assert!(
        bodies.iter().all(|b| !b.contains("status=")),
        "no status beside a larger tracker total: {bodies:?}"
    );
}

#[tokio::test]
async fn mal_the_finale_sends_its_own_number_when_the_totals_agree() {
    let server = mal_server(12, 11, &["num_watched_episodes=12", "status=completed"], 1).await;
    assert!(push_to_mal(&server, finale(12)).await.is_some());
}

#[tokio::test]
async fn mal_an_episode_past_a_shorter_entry_writes_nothing_once_the_entry_is_full() {
    // Ranma ½'s shape (AniList's 18-episode entry for Kitsu's 161),
    // served here by the MAL mock: watching Kitsu's 30 with the row
    // already at 18 — nothing is sent, and nothing past 18 ever is.
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

// The rows the play page reaches and cannot decide: what reaches each
// tracker depends on that tracker's total and row, which only this
// path reads. The renderer's half — which episode, and whether it
// carries `completed` — is pinned through the real play page in
// `frontend/tests/acceptance/play-tracker-sync.test.ts`.

#[tokio::test]
async fn mal_an_episode_past_kitsus_finale_is_clamped_to_the_total_without_completing() {
    // The provider lists a 13th episode of a show Kitsu and MAL count as
    // 12: the fan-out sends 13 with no status, and MAL is sent 12.
    let server = mal_server(12, 11, &["num_watched_episodes=12"], 1).await;
    assert!(push_to_mal(&server, episode(13)).await.is_some());
    let bodies = patch_bodies(&server).await;
    assert!(
        bodies.iter().all(|b| !b.contains("status=")),
        "no status past Kitsu's finale: {bodies:?}"
    );
}

#[tokio::test]
async fn mal_an_episode_past_kitsus_finale_writes_nothing_once_the_row_is_full() {
    let server = mal_server(12, 12, &[], 0).await;
    assert!(push_to_mal(&server, episode(13)).await.is_none());
}

#[tokio::test]
async fn mal_the_finale_completes_when_the_total_is_unknown() {
    let server = mal_server(0, 11, &["num_watched_episodes=12", "status=completed"], 1).await;
    assert!(push_to_mal(&server, finale(12)).await.is_some());
}

#[tokio::test]
async fn mal_a_rewatchs_finale_sends_its_progress_and_keeps_the_rewatch() {
    let row = r#"{"status":"watching","is_rewatching":true,"num_episodes_watched":11}"#;
    let server = mal_server_with_row(12, row, &["num_watched_episodes=12"], 1).await;
    assert!(push_to_mal(&server, finale(12)).await.is_some());
    let bodies = patch_bodies(&server).await;
    assert!(
        bodies
            .iter()
            .all(|b| !b.contains("status=") && !b.contains("is_rewatching=")),
        "a rewatch keeps its status: {bodies:?}"
    );
}

#[tokio::test]
async fn mal_a_replay_behind_the_row_writes_nothing() {
    let row = r#"{"status":"completed","num_episodes_watched":12}"#;
    let server = mal_server_with_row(12, row, &[], 0).await;
    assert!(push_to_mal(&server, episode(3)).await.is_none());
}

/// An AniList server whose media 21 has `episodes` (JSON: a number or
/// `null`) and the user's `entry` (JSON: a row or `null`), answering
/// every save.
async fn anilist_server(episodes: &str, entry: &str) -> wiremock::MockServer {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .and(body_string_contains("mediaListEntry"))
        .and(body_string_contains("episodes"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(format!(
                r#"{{"data":{{"Media":{{"episodes":{episodes},"mediaListEntry":{entry}}}}}}}"#
            )),
        )
        .expect(1)
        .mount(&server)
        .await;
    wiremock::Mock::given(method("POST"))
        .and(body_string_contains("SaveMediaListEntry"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"SaveMediaListEntry":{"mediaId":21,"status":"CURRENT","progress":12,"score":0,"updatedAt":1,"repeat":0,"media":{"idMal":21,"title":{"romaji":"Air","english":null,"userPreferred":"Air"}}}}}"#,
        ))
        .mount(&server)
        .await;
    server
}

async fn push_to_anilist(server: &wiremock::MockServer, update: EntryUpdate) -> Option<ListEntry> {
    let provider = crate::meta::anilist_user::AniListProvider::with_bases(
        reqwest::Client::new(),
        server.uri(),
        "http://unused-token".to_string(),
    );
    let state = state_with_kitsu("http://unused-kitsu");
    push_progress_via(
        &state,
        ProviderKind::AniList,
        &provider,
        &test_tokens(),
        ProviderMediaId(21),
        update,
    )
    .await
    .expect("push ok")
}

/// The `variables` of every save `server` received.
async fn anilist_saves(server: &wiremock::MockServer) -> Vec<serde_json::Value> {
    server
        .received_requests()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|r| serde_json::from_slice::<serde_json::Value>(&r.body).ok())
        .filter(|b| {
            b["query"]
                .as_str()
                .is_some_and(|q| q.contains("SaveMediaListEntry"))
        })
        .map(|b| b["variables"].clone())
        .collect()
}

#[tokio::test]
async fn anilist_the_finale_beside_a_prologue_total_sends_its_number_without_completing() {
    let server = anilist_server("13", r#"{"status":"CURRENT","progress":11}"#).await;
    let entry = push_to_anilist(&server, finale(12)).await.expect("written");
    assert_eq!(entry.progress_episodes, 12);
    assert_eq!(
        anilist_saves(&server).await,
        vec![serde_json::json!({"mediaId": 21, "progress": 12})]
    );
}

#[tokio::test]
async fn anilist_the_finale_completes_when_the_totals_agree() {
    let server = anilist_server("12", r#"{"status":"CURRENT","progress":11}"#).await;
    assert!(push_to_anilist(&server, finale(12)).await.is_some());
    assert_eq!(
        anilist_saves(&server).await,
        vec![serde_json::json!({"mediaId": 21, "status": "COMPLETED", "progress": 12})]
    );
}

#[tokio::test]
async fn anilist_the_finale_completes_when_the_total_is_unknown() {
    let server = anilist_server("null", r#"{"status":"CURRENT","progress":11}"#).await;
    assert!(push_to_anilist(&server, finale(12)).await.is_some());
    assert_eq!(
        anilist_saves(&server).await,
        vec![serde_json::json!({"mediaId": 21, "status": "COMPLETED", "progress": 12})]
    );
}

#[tokio::test]
async fn anilist_an_episode_past_kitsus_finale_is_clamped_to_the_total_without_completing() {
    let server = anilist_server("12", r#"{"status":"CURRENT","progress":11}"#).await;
    assert!(push_to_anilist(&server, episode(13)).await.is_some());
    assert_eq!(
        anilist_saves(&server).await,
        vec![serde_json::json!({"mediaId": 21, "progress": 12})]
    );
}

#[tokio::test]
async fn anilist_an_episode_past_kitsus_finale_writes_nothing_once_the_row_is_full() {
    let server = anilist_server("12", r#"{"status":"CURRENT","progress":12}"#).await;
    assert!(push_to_anilist(&server, episode(13)).await.is_none());
    assert!(anilist_saves(&server).await.is_empty());
}

#[tokio::test]
async fn anilist_a_rewatchs_finale_sends_its_progress_and_keeps_the_rewatch() {
    let server = anilist_server("12", r#"{"status":"REPEATING","progress":11}"#).await;
    assert!(push_to_anilist(&server, finale(12)).await.is_some());
    assert_eq!(
        anilist_saves(&server).await,
        vec![serde_json::json!({"mediaId": 21, "progress": 12})]
    );
}

#[tokio::test]
async fn anilist_a_replay_behind_the_row_writes_nothing() {
    let server = anilist_server("12", r#"{"status":"COMPLETED","progress":12}"#).await;
    assert!(push_to_anilist(&server, episode(3)).await.is_none());
    assert!(anilist_saves(&server).await.is_empty());
}
