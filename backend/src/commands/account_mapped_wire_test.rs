//! The mapped write-back against each tracker's real provider, served
//! by wiremock: the requests AniList and MyAnimeList receive, in order,
//! for Ranma ½'s handover to Nettou-hen and the Attack on Titan Final
//! Chapters' second special.

use std::path::PathBuf;
use std::sync::Arc;

use super::*;
use crate::account::episode_map::{EpisodeMap, Segment, Shape};
use crate::account::provider::{ProviderKind, Tokens};
use crate::account::status::ListStatus;
use crate::app::AppState;
use crate::cache::meta_cache_put;
use crate::commands::tracker_map::map_key;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn state() -> Arc<AppState> {
    Arc::new(AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: PathBuf::from("/tmp/ani-gui/history"),
        providers: crate::provider_set::ProviderSet::for_tests(),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(
            reqwest::Client::new(),
            "http://unused-kitsu",
        ),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    })
}

fn tokens() -> Tokens {
    Tokens {
        access_token: "t".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    }
}

fn seed(state: &AppState, kind: ProviderKind, kitsu_id: &str, map: &EpisodeMap) {
    meta_cache_put(
        &state.cache_pool,
        &map_key(
            kind,
            kitsu_id,
            Some(match kitsu_id {
                "186" => (210, Some(18)),
                _ => (51_535, Some(2)),
            }),
        ),
        &serde_json::to_string(map).expect("serialise"),
        3600,
    )
    .expect("seed");
}

fn segment(entry: u32, total: u32, from: u32, to: u32, lead: u32) -> Segment {
    Segment {
        entry,
        entry_total: total,
        entry_finished: true,
        kitsu_from: from,
        kitsu_to: to,
        lead,
        extras_after: vec![],
    }
}

fn save_reply(media: u32, status: &str, progress: u32) -> String {
    format!(
        r#"{{"data":{{"SaveMediaListEntry":{{"mediaId":{media},"status":"{status}","progress":{progress},"score":0,"updatedAt":1,"repeat":0,"media":{{"idMal":null,"title":{{"romaji":"Ranma","english":null,"userPreferred":"Ranma"}}}}}}}}}}"#
    )
}

/// The GraphQL bodies AniList received that name `marker`, in order.
async fn bodies(server: &MockServer, marker: &str) -> Vec<String> {
    server
        .received_requests()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| String::from_utf8_lossy(&r.body).into_owned())
        .filter(|b| b.contains(marker))
        .collect()
}

#[tokio::test]
async fn anilist_ranma_s_nineteenth_episode_completes_210_then_starts_nettou_hen() {
    let state = state();
    seed(
        &state,
        ProviderKind::AniList,
        "186",
        &EpisodeMap {
            shape: Shape::Chain,
            segments: vec![
                segment(210, 18, 1, 18, 0),
                segment(149_939, 143, 19, 161, 0),
            ],
        },
    );
    let server = MockServer::start().await;
    mount_cache_owner(&server).await;
    Mock::given(method("POST"))
        .and(body_string_contains("query Tracked"))
        .and(body_string_contains("\"mediaId\":210"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"Media":{"episodes":18,"mediaListEntry":{"status":"CURRENT","progress":16}}}}"#,
        ))
        // Once for the total the map is checked against, once more under
        // its own lock when it is brought to 18: each entry is read where
        // it is written.
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_string_contains("query Tracked"))
        .and(body_string_contains("\"mediaId\":149939"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"Media":{"episodes":143,"mediaListEntry":null}}}"#),
        )
        // Read to see the write starts its segment, and again after the
        // entry before it is brought up.
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_string_contains("SaveMediaListEntry"))
        .and(body_string_contains("\"mediaId\":210"))
        .respond_with(ResponseTemplate::new(200).set_body_string(save_reply(210, "COMPLETED", 18)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_string_contains("SaveMediaListEntry"))
        .and(body_string_contains("\"mediaId\":149939"))
        .respond_with(ResponseTemplate::new(200).set_body_string(save_reply(149_939, "CURRENT", 1)))
        .expect(1)
        .mount(&server)
        .await;
    let provider = crate::meta::anilist_user::AniListProvider::with_bases(
        reqwest::Client::new(),
        server.uri(),
        "http://unused-token".to_string(),
    );
    let tokens = tokens();
    let ask = MapAsk {
        state: &state,
        kind: ProviderKind::AniList,
        provider: &provider,
        tokens: &tokens,
        anilist_base: None,
    };
    let update = EntryUpdate {
        progress_episodes: Some(19),
        ..Default::default()
    };
    let entry = push_progress_mapped(&ask, "186", Some(ProviderMediaId(210)), update)
        .await
        .expect("push ok")
        .expect("written");
    assert_eq!(entry.media_id, ProviderMediaId(149_939));
    assert_eq!(entry.progress_episodes, 1);
    let saves = bodies(&server, "SaveMediaListEntry").await;
    assert_eq!(saves.len(), 2, "{saves:?}");
    assert!(saves[0].contains("\"mediaId\":210"));
    assert!(saves[0].contains("\"status\":\"COMPLETED\""));
    assert!(saves[0].contains("\"progress\":18"));
    assert!(saves[1].contains("\"mediaId\":149939"));
    assert!(saves[1].contains("\"progress\":1"));
    assert!(saves[1].contains("\"status\":\"CURRENT\""));
}

#[tokio::test]
async fn mal_the_final_chapters_second_special_sends_two_completed() {
    let state = state();
    seed(
        &state,
        ProviderKind::MyAnimeList,
        "47132",
        &EpisodeMap {
            shape: Shape::SharedEntry,
            segments: vec![segment(51_535, 2, 1, 1, 1)],
        },
    );
    let server = MockServer::start().await;
    mount_cache_owner(&server).await;
    Mock::given(method("GET"))
        .and(path("/anime/51535"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"id":51535,"num_episodes":2,"my_list_status":{"status":"watching","num_episodes_watched":1}}"#,
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/anime/51535/my_list_status"))
        .and(body_string_contains("num_watched_episodes=2"))
        .and(body_string_contains("status=completed"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"status":"completed","num_episodes_watched":2,"is_rewatching":false}"#,
        ))
        .expect(1)
        .mount(&server)
        .await;
    let provider = crate::meta::mal_user::MalProvider::with_bases(
        reqwest::Client::new(),
        server.uri(),
        "http://unused-token".to_string(),
        crate::meta::mal_user::MalRefreshState::new(),
    );
    let tokens = tokens();
    let ask = MapAsk {
        state: &state,
        kind: ProviderKind::MyAnimeList,
        provider: &provider,
        tokens: &tokens,
        anilist_base: None,
    };
    // Kitsu's 47132 is one episode, a finished show: the fan-out sends
    // its finale as 1, completed.
    let update = EntryUpdate {
        status: Some(ListStatus::Completed),
        progress_episodes: Some(1),
        ..Default::default()
    };
    let entry = push_progress_mapped(&ask, "47132", Some(ProviderMediaId(51_535)), update)
        .await
        .expect("push ok")
        .expect("written");
    assert_eq!(entry.progress_episodes, 2);
    assert_eq!(entry.status, ListStatus::Completed);
}

#[tokio::test]
async fn mal_the_final_chapters_first_special_sends_one_and_no_status() {
    let state = state();
    seed(
        &state,
        ProviderKind::MyAnimeList,
        "46038",
        &EpisodeMap {
            shape: Shape::SharedEntry,
            segments: vec![segment(51_535, 2, 1, 1, 0)],
        },
    );
    let server = MockServer::start().await;
    mount_cache_owner(&server).await;
    Mock::given(method("GET"))
        .and(path("/anime/51535"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"id":51535,"num_episodes":2}"#),
        )
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/anime/51535/my_list_status"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"status":"watching","num_episodes_watched":1,"is_rewatching":false}"#,
        ))
        .expect(1)
        .mount(&server)
        .await;
    let provider = crate::meta::mal_user::MalProvider::with_bases(
        reqwest::Client::new(),
        server.uri(),
        "http://unused-token".to_string(),
        crate::meta::mal_user::MalRefreshState::new(),
    );
    let tokens = tokens();
    let ask = MapAsk {
        state: &state,
        kind: ProviderKind::MyAnimeList,
        provider: &provider,
        tokens: &tokens,
        anilist_base: None,
    };
    let update = EntryUpdate {
        status: Some(ListStatus::Completed),
        progress_episodes: Some(1),
        ..Default::default()
    };
    push_progress_mapped(&ask, "46038", Some(ProviderMediaId(51_535)), update)
        .await
        .expect("push ok");
    let patches: Vec<String> = server
        .received_requests()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.method.as_str() == "PATCH")
        .map(|r| String::from_utf8_lossy(&r.body).into_owned())
        .collect();
    assert_eq!(patches.len(), 1);
    assert!(patches[0].contains("num_watched_episodes=1"), "{patches:?}");
    // The Kitsu entry's finale is not the MAL entry's: watching, not
    // completed (the row was absent, so the write promotes it).
    assert!(patches[0].contains("status=watching"), "{patches:?}");
}

/// MAL keeps "finished" in Completed: its rewatching row is a Watching
/// one with `is_rewatching` set. A write the map files under an entry
/// being rewatched closes the rewatch where it reaches the entry's
/// total, on each path the map writes by: back at Completed, and the
/// entry's count of finished rewatches up by one.
/// The `fields` the write-back's MAL read asks for.
const MAL_TRACKED_FIELDS: &str =
    "my_list_status{status,num_episodes_watched,is_rewatching,num_times_rewatched},num_episodes";

async fn mal_get(server: &MockServer, media: u32, body: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/anime/{media}")))
        .and(wiremock::matchers::query_param(
            "fields",
            MAL_TRACKED_FIELDS,
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(body.to_string()))
        .mount(server)
        .await;
}

/// The PATCH bodies MAL received for `media`, in order.
async fn mal_patches(server: &MockServer, media: u32) -> Vec<String> {
    let at = format!("/anime/{media}/my_list_status");
    server
        .received_requests()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.method.as_str() == "PATCH" && r.url.path() == at)
        .map(|r| String::from_utf8_lossy(&r.body).into_owned())
        .collect()
}

async fn mal_push(
    state: &Arc<AppState>,
    server: &MockServer,
    kitsu_id: &str,
    native: u32,
    update: EntryUpdate,
) {
    Mock::given(method("PATCH"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"status":"completed","num_episodes_watched":1,"is_rewatching":false}"#,
        ))
        .mount(server)
        .await;
    let provider = crate::meta::mal_user::MalProvider::with_bases(
        reqwest::Client::new(),
        server.uri(),
        "http://unused-token".to_string(),
        crate::meta::mal_user::MalRefreshState::new(),
    );
    let tokens = tokens();
    let ask = MapAsk {
        state,
        kind: ProviderKind::MyAnimeList,
        provider: &provider,
        tokens: &tokens,
        anilist_base: None,
    };
    push_progress_mapped(&ask, kitsu_id, Some(ProviderMediaId(native)), update)
        .await
        .expect("push ok");
}

fn mal_ranma(state: &AppState) {
    seed(
        state,
        ProviderKind::MyAnimeList,
        "186",
        &EpisodeMap {
            shape: Shape::Chain,
            segments: vec![
                segment(210, 18, 1, 18, 0),
                segment(149_939, 143, 19, 161, 0),
            ],
        },
    );
}

fn closes_the_rewatch(patch: &str, count: u32) -> bool {
    patch.contains("status=completed")
        && patch.contains("is_rewatching=false")
        && patch.contains(&format!("num_times_rewatched={count}"))
}

#[tokio::test]
async fn mal_the_resolved_entry_s_mapped_finale_closes_its_rewatch() {
    let state = state();
    seed(
        &state,
        ProviderKind::MyAnimeList,
        "47132",
        &EpisodeMap {
            shape: Shape::SharedEntry,
            segments: vec![segment(51_535, 2, 1, 1, 1)],
        },
    );
    let server = MockServer::start().await;
    mount_cache_owner(&server).await;
    mal_get(
        &server,
        51_535,
        r#"{"id":51535,"num_episodes":2,"my_list_status":{"status":"watching","is_rewatching":true,"num_episodes_watched":1,"num_times_rewatched":4}}"#,
    )
    .await;
    let update = EntryUpdate {
        status: Some(ListStatus::Completed),
        progress_episodes: Some(1),
        ..Default::default()
    };
    mal_push(&state, &server, "47132", 51_535, update).await;
    let patches = mal_patches(&server, 51_535).await;
    assert_eq!(patches.len(), 1, "{patches:?}");
    assert!(patches[0].contains("num_watched_episodes=2"), "{patches:?}");
    assert!(closes_the_rewatch(&patches[0], 5), "{patches:?}");
}

#[tokio::test]
async fn mal_an_earlier_entry_rewatched_to_its_total_is_closed() {
    let state = state();
    mal_ranma(&state);
    let server = MockServer::start().await;
    mount_cache_owner(&server).await;
    mal_get(
        &server,
        210,
        r#"{"id":210,"num_episodes":18,"my_list_status":{"status":"watching","is_rewatching":true,"num_episodes_watched":18,"num_times_rewatched":1}}"#,
    )
    .await;
    mal_get(&server, 149_939, r#"{"id":149939,"num_episodes":143}"#).await;
    let update = EntryUpdate {
        progress_episodes: Some(19),
        ..Default::default()
    };
    mal_push(&state, &server, "186", 210, update).await;
    let earlier = mal_patches(&server, 210).await;
    assert_eq!(earlier.len(), 1, "{earlier:?}");
    assert!(closes_the_rewatch(&earlier[0], 2), "{earlier:?}");
    let later = mal_patches(&server, 149_939).await;
    assert_eq!(later.len(), 1, "{later:?}");
    assert!(later[0].contains("num_watched_episodes=1"), "{later:?}");
}

#[tokio::test]
async fn mal_a_later_entry_s_finale_closes_its_rewatch() {
    let state = state();
    mal_ranma(&state);
    let server = MockServer::start().await;
    mount_cache_owner(&server).await;
    mal_get(
        &server,
        210,
        r#"{"id":210,"num_episodes":18,"my_list_status":{"status":"completed","num_episodes_watched":18}}"#,
    )
    .await;
    mal_get(
        &server,
        149_939,
        r#"{"id":149939,"num_episodes":143,"my_list_status":{"status":"watching","is_rewatching":true,"num_episodes_watched":142,"num_times_rewatched":0}}"#,
    )
    .await;
    let update = EntryUpdate {
        status: Some(ListStatus::Completed),
        progress_episodes: Some(161),
        ..Default::default()
    };
    mal_push(&state, &server, "186", 210, update).await;
    assert!(mal_patches(&server, 210).await.is_empty());
    let later = mal_patches(&server, 149_939).await;
    assert_eq!(later.len(), 1, "{later:?}");
    assert!(later[0].contains("num_watched_episodes=143"), "{later:?}");
    assert!(closes_the_rewatch(&later[0], 1), "{later:?}");
}

// A real mutation resolves its cache owner before writing. These suites
// exercise the episode/status payload, so answer that prerequisite too.
async fn mount_cache_owner(server: &wiremock::MockServer) {
    use wiremock::matchers::{body_string_contains, method, path};
    wiremock::Mock::given(method("GET"))
        .and(path("/users/@me"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(r#"{"id":4242,"name":"tester"}"#),
        )
        .mount(server)
        .await;
    wiremock::Mock::given(method("POST")).and(body_string_contains("Viewer"))
        .respond_with(wiremock::ResponseTemplate::new(200)
            .set_body_string(r#"{"data":{"Viewer":{"id":4242,"name":"tester","avatar":{"large":null},"statistics":{"anime":{"count":1,"meanScore":0}}}}}"#))
        .mount(server).await;
}
