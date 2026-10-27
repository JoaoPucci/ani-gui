//! The mark-watched write path against each tracker's own episode
//! total: what `push_progress_via` reads, and what it writes, when the
//! tracker counts a prologue Kitsu does not (Air on AniList: 13 against 12), when
//! the totals agree, when the tracker's entry is shorter than Kitsu's
//! show (Ranma ½'s AniList shape, 18 against 161, served by the MAL
//! mock only), and when the total is unknown;
//! and, on both trackers, an episode past Kitsu's finale, a rewatch's
//! finale and a replay behind the row.
//!
//! This is the identity write: the one a show's episode map leaves as
//! Kitsu numbers it. Where the map files the episodes under other
//! entries or numbers — Ranma ½'s two AniList entries among them — the
//! write is `account_mapped`'s, which decides each entry's completion
//! from the map and does not read the renderer's `completed`; it is
//! covered in `account_mapped_test.rs`.

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
        .and(query_param(
            "fields",
            "my_list_status{status,num_episodes_watched,is_rewatching,num_times_rewatched},num_episodes",
        ))
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
// tracker depends on that tracker's total and row, which only the
// backend reads: this path for an identity write, `account_mapped`
// (`account_mapped_test.rs`, `account_mapped_wire_test.rs`) for a
// mapped one. The renderer's half — which episode, and whether it
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

/// MAL has no rewatch status of its own: a rewatch is a Watching row
/// with `is_rewatching` set, and "finished" lives in Completed. A
/// finished show's finale on a row being rewatched closes the rewatch:
/// the row goes back to Completed, out of the rewatch, and the count of
/// finished rewatches goes up by one — read first, never lowered.
#[tokio::test]
async fn mal_a_rewatchs_finale_closes_it_and_counts_it() {
    let row = r#"{"status":"watching","is_rewatching":true,"num_episodes_watched":11,"num_times_rewatched":2}"#;
    let server = mal_server_with_row(
        12,
        row,
        &[
            "num_watched_episodes=12",
            "status=completed",
            "is_rewatching=false",
            "num_times_rewatched=3",
        ],
        1,
    )
    .await;
    assert!(push_to_mal(&server, finale(12)).await.is_some());
}

/// The same at the finale already counted: a rewatching row at the
/// finale's number, watched again, is closed and counted too.
#[tokio::test]
async fn mal_a_rewatch_at_its_finale_already_is_closed_and_counted() {
    let row = r#"{"status":"watching","is_rewatching":true,"num_episodes_watched":12,"num_times_rewatched":0}"#;
    let server = mal_server_with_row(
        12,
        row,
        &[
            "status=completed",
            "is_rewatching=false",
            "num_times_rewatched=1",
        ],
        1,
    )
    .await;
    assert!(push_to_mal(&server, finale(12)).await.is_some());
}

/// A count the read did not carry is unknown, and an unknown count is
/// never written over: the rewatch is closed without one.
#[tokio::test]
async fn mal_a_rewatch_with_no_count_read_is_closed_without_one() {
    let row = r#"{"status":"watching","is_rewatching":true,"num_episodes_watched":11}"#;
    let server =
        mal_server_with_row(12, row, &["status=completed", "is_rewatching=false"], 1).await;
    assert!(push_to_mal(&server, finale(12)).await.is_some());
    let bodies = patch_bodies(&server).await;
    assert!(
        bodies.iter().all(|b| !b.contains("num_times_rewatched")),
        "{bodies:?}"
    );
}

/// The finale replayed on a row the rewatch already closed counts
/// nothing again.
#[tokio::test]
async fn mal_the_finale_replayed_after_a_closed_rewatch_counts_nothing() {
    let row = r#"{"status":"completed","num_episodes_watched":12,"num_times_rewatched":3}"#;
    let server = mal_server_with_row(12, row, &[], 1).await;
    push_to_mal(&server, finale(12)).await;
    let bodies = patch_bodies(&server).await;
    assert!(
        bodies.iter().all(|b| !b.contains("num_times_rewatched")),
        "{bodies:?}"
    );
}

/// Mid-series, a rewatch is left as it is: its progress alone, no count.
#[tokio::test]
async fn mal_a_rewatch_mid_series_counts_nothing() {
    let row = r#"{"status":"watching","is_rewatching":true,"num_episodes_watched":4,"num_times_rewatched":2}"#;
    let server = mal_server_with_row(12, row, &["num_watched_episodes=5"], 1).await;
    assert!(push_to_mal(&server, episode(5)).await.is_some());
    let bodies = patch_bodies(&server).await;
    assert!(
        bodies
            .iter()
            .all(|b| !b.contains("num_times_rewatched") && !b.contains("status=")),
        "{bodies:?}"
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

// ── what the MAL rewatch change must not move ──────────────────────────

#[tokio::test]
async fn anilist_a_rewatch_mid_series_sends_its_progress_alone() {
    let server = anilist_server("12", r#"{"status":"REPEATING","progress":5}"#).await;
    assert!(push_to_anilist(&server, episode(6)).await.is_some());
    assert_eq!(
        anilist_saves(&server).await,
        vec![serde_json::json!({"mediaId": 21, "progress": 6})]
    );
}

#[tokio::test]
async fn anilist_a_planning_row_is_promoted_to_watching() {
    let server = anilist_server("12", r#"{"status":"PLANNING","progress":0}"#).await;
    assert!(push_to_anilist(&server, episode(1)).await.is_some());
    assert_eq!(
        anilist_saves(&server).await,
        vec![serde_json::json!({"mediaId": 21, "status": "CURRENT", "progress": 1})]
    );
}

#[tokio::test]
async fn mal_a_planning_row_is_promoted_to_watching() {
    let row = r#"{"status":"plan_to_watch","num_episodes_watched":0}"#;
    let server = mal_server_with_row(
        12,
        row,
        &[
            "status=watching",
            "is_rewatching=false",
            "num_watched_episodes=1",
        ],
        1,
    )
    .await;
    assert!(push_to_mal(&server, episode(1)).await.is_some());
}

/// A row MAL already reads as rewatching takes a mid-series episode's
/// progress alone: only a finished show's finale completes it.
#[tokio::test]
async fn mal_a_rewatching_row_mid_series_sends_its_progress_alone() {
    let row = r#"{"status":"watching","is_rewatching":true,"num_episodes_watched":5}"#;
    let server = mal_server_with_row(12, row, &["num_watched_episodes=6"], 1).await;
    assert!(push_to_mal(&server, episode(6)).await.is_some());
    let bodies = patch_bodies(&server).await;
    assert!(
        bodies
            .iter()
            .all(|b| !b.contains("status=") && !b.contains("is_rewatching=")),
        "mid-series, the row's status is left alone: {bodies:?}"
    );
}

/// A replay behind a row MAL reads as rewatching writes nothing, as it
/// does behind any row.
#[tokio::test]
async fn mal_a_replay_behind_a_rewatching_row_writes_nothing() {
    let row = r#"{"status":"watching","is_rewatching":true,"num_episodes_watched":8}"#;
    let server = mal_server_with_row(12, row, &[], 0).await;
    assert!(push_to_mal(&server, episode(3)).await.is_none());
}

/// The write-back's own write with nothing in it makes no request
/// either, and is a no-op.
#[tokio::test]
async fn an_empty_write_back_sends_no_request() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("PATCH"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"status":"watching","num_episodes_watched":0,"is_rewatching":false}"#,
        ))
        .expect(0)
        .mount(&server)
        .await;
    let state = state_with_kitsu("http://unused-kitsu");
    let got = push_progress_via(
        &state,
        ProviderKind::MyAnimeList,
        &mal_provider(&server.uri()),
        &test_tokens(),
        ProviderMediaId(21),
        EntryUpdate::default(),
    )
    .await
    .expect("a no-op, not a failure");
    assert!(got.is_none());
}
