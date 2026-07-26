//! Tests for `crate::commands::account_edit`. Extracted via `#[path]`
//! so the test ccn doesn't pile onto the command file's CRAP budget —
//! per `project_crap_inline_test_gotcha`.
//!
//! The happy paths drive the real `update_entry`/`current_entry`/
//! `delete_entry`/`me` round-trips against a wiremock-backed MAL
//! provider injected into the `*_via` seams, so the explicit-edit
//! command bodies are covered without touching the live host.

use std::sync::Arc;

use super::{get_entry, get_entry_via, remove_entry_via, set_entry, set_entry_via};
use crate::account::provider::{EntryUpdate, ProviderKind, Tokens};
use crate::account::status::ListStatus;
use crate::app::AppState;
use crate::meta::mal_user::{MalProvider, MalRefreshState};

/// `AppState` whose Kitsu client points at `kitsu_uri` (a wiremock
/// server) so id resolution can be exercised.
fn state_with_kitsu(kitsu_uri: &str) -> Arc<AppState> {
    use std::path::PathBuf;
    Arc::new(AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        ani_cli_path: PathBuf::from("/tmp/ani-cli"),
        bash_path: None,
        bundled_bin: None,
        history_path: PathBuf::from("/tmp/ani-cli/ani-hsts"),
        scraper_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), kitsu_uri),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
    })
}

/// A MAL provider pointed at a wiremock server.
fn mal_provider(api_uri: &str) -> MalProvider {
    MalProvider::with_bases(
        reqwest::Client::new(),
        api_uri.to_string(),
        "http://unused-token".to_string(),
        MalRefreshState::new(),
    )
}

fn test_tokens() -> Tokens {
    Tokens {
        access_token: "t".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    }
}

/// Kitsu `/anime/12` → MAL id 21 (the happy-path mapping).
const KITSU_MAL_MAPPING_BODY: &str = r#"{
    "data": { "id": "12", "type": "anime", "attributes": { "canonicalTitle": "One Piece" } },
    "included": [{
        "id": "1175",
        "type": "mappings",
        "attributes": { "externalSite": "myanimelist/anime", "externalId": "21" }
    }]
}"#;

async fn mappable_kitsu() -> wiremock::MockServer {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(KITSU_MAL_MAPPING_BODY))
        .mount(&kitsu)
        .await;
    kitsu
}

/// Kitsu `/anime/999` with empty `included` → no MAL mapping → the
/// explicit commands short-circuit before any upstream provider call.
async fn unmappable_kitsu() -> wiremock::MockServer {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/999"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"id":"999","type":"anime"},"included":[]}"#),
        )
        .mount(&kitsu)
        .await;
    kitsu
}

/// Mount MAL `/users/@me` so the folded cache write-through resolves an
/// owner id (4242) and the upsert/delete-row line runs.
async fn mount_mal_me(server: &wiremock::MockServer) {
    use wiremock::matchers::{method, path};
    wiremock::Mock::given(method("GET"))
        .and(path("/users/@me"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(r#"{"id":4242,"name":"shiro"}"#),
        )
        .mount(server)
        .await;
}

// ─── Skip-path: unmappable show short-circuits before any write ──────

#[tokio::test]
async fn set_entry_skips_unmappable_show_without_writing() {
    // Exercises the public wrapper: provider_for_kind builds the real MAL
    // provider, but resolve_native_media_id returns None first, so
    // update_entry is never reached (a stray write would surface as
    // Network/Upstream, not Ok(None)).
    let kitsu = unmappable_kitsu().await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = set_entry(
        &state,
        ProviderKind::MyAnimeList,
        &test_tokens(),
        "999",
        EntryUpdate {
            progress_episodes: Some(3),
            ..Default::default()
        },
    )
    .await
    .expect("set ok");
    assert!(got.is_none(), "unmappable show must short-circuit to None");
}

#[tokio::test]
async fn get_entry_returns_none_for_unmappable_show() {
    let kitsu = unmappable_kitsu().await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = get_entry(&state, ProviderKind::MyAnimeList, &test_tokens(), "999")
        .await
        .expect("get ok");
    assert!(got.is_none(), "unmappable show has no current entry");
}

// ─── Happy-path: injected wiremock provider covers the round-trips ───

#[tokio::test]
async fn get_entry_via_reads_live_current_entry() {
    use wiremock::matchers::{method, path, query_param};
    let kitsu = mappable_kitsu().await;
    let mal = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/21"))
        .and(query_param("fields", "my_list_status"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"id":21,"my_list_status":{"status":"watching","num_episodes_watched":5}}"#,
        ))
        .mount(&mal)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = mal_provider(&mal.uri());
    let got = get_entry_via(
        &state,
        ProviderKind::MyAnimeList,
        &provider,
        &test_tokens(),
        "12",
    )
    .await
    .expect("get ok")
    .expect("on the list");
    assert_eq!(got.status, ListStatus::Watching);
    assert_eq!(got.progress_episodes, 5);
}

#[tokio::test]
async fn get_entry_via_waits_for_an_in_flight_write_on_the_same_show() {
    // Codex P2 #3427461062: a mark-watched sync holds the per-show lock
    // while it reconciles and writes the newer progress. The editor's seed
    // read must take the SAME lock, or it can read a value mid-overwrite —
    // and a later explicit Save would then force-write that stale value back
    // over the just-synced higher one. Proven here by holding the show lock
    // (as the in-flight sync would) and asserting the read can't complete
    // until it's released.
    use std::time::Duration;
    use wiremock::matchers::{method, path, query_param};
    let kitsu = mappable_kitsu().await;
    let mal = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/21"))
        .and(query_param("fields", "my_list_status"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"id":21,"my_list_status":{"status":"watching","num_episodes_watched":5}}"#,
        ))
        .mount(&mal)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = mal_provider(&mal.uri());
    let tokens = test_tokens();
    // Hold the per-show write lock keyed on the Kitsu id ("12"), as a
    // mark-watched sync does before it resolves the native id. The seed
    // read must take the SAME lock before its own resolve, so it blocks.
    let show_lock = state
        .account_write_locks
        .for_show(ProviderKind::MyAnimeList, "12");
    let guard = show_lock.lock().await;
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_millis(200)) => {}
        _ = get_entry_via(&state, ProviderKind::MyAnimeList, &provider, &tokens, "12") => {
            panic!("get_entry_via read the entry while the show's write lock was held");
        }
    }
    // Releasing the lock lets the queued read settle and return the entry.
    drop(guard);
    let got = get_entry_via(&state, ProviderKind::MyAnimeList, &provider, &tokens, "12")
        .await
        .expect("get ok")
        .expect("on the list");
    assert_eq!(got.progress_episodes, 5);
}

#[tokio::test]
async fn set_entry_via_writes_explicit_lower_progress_verbatim() {
    // The editor can correct an over-count downward. set_entry_via does
    // NOT read current first (no monotonic reconcile), so the lower value
    // is PATCHed as-is; the returned entry echoes it and the forced cache
    // write-through (via the mocked /users/@me) records it.
    use wiremock::matchers::{body_string_contains, method, path};
    let kitsu = mappable_kitsu().await;
    let mal = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("PATCH"))
        .and(path("/anime/21/my_list_status"))
        .and(body_string_contains("num_watched_episodes=3"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"status":"watching","num_episodes_watched":3,"is_rewatching":false}"#,
        ))
        .mount(&mal)
        .await;
    mount_mal_me(&mal).await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = mal_provider(&mal.uri());
    let entry = set_entry_via(
        &state,
        ProviderKind::MyAnimeList,
        &provider,
        &test_tokens(),
        "12",
        EntryUpdate {
            progress_episodes: Some(3),
            ..Default::default()
        },
    )
    .await
    .expect("set ok")
    .expect("mapped + written");
    assert_eq!(entry.progress_episodes, 3, "explicit lower value written");
}

#[tokio::test]
async fn remove_entry_via_deletes_then_drops_cache_row() {
    use wiremock::matchers::{method, path};
    let kitsu = mappable_kitsu().await;
    let mal = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("DELETE"))
        .and(path("/anime/21/my_list_status"))
        .respond_with(wiremock::ResponseTemplate::new(200))
        .mount(&mal)
        .await;
    mount_mal_me(&mal).await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = mal_provider(&mal.uri());
    let removed = remove_entry_via(
        &state,
        ProviderKind::MyAnimeList,
        &provider,
        &test_tokens(),
        "12",
    )
    .await
    .expect("remove ok");
    assert!(removed, "a mapped show is removed");
}

#[tokio::test]
async fn remove_entry_via_treats_a_failed_delete_as_removed_when_the_entry_is_gone() {
    // Codex P2 #3423227862: AniList doesn't report an already-absent row
    // as Upstream{404} — its delete fails at the row-id lookup (a
    // parse/GraphQL error). So the idempotency check can't key off 404;
    // instead, on ANY delete error, confirm with a live read — if the
    // show is no longer on the list, the delete's goal is already met.
    // Modelled here with a generic 500 delete + a current_entry that
    // reports the show absent.
    use wiremock::matchers::{method, path, query_param};
    let kitsu = mappable_kitsu().await;
    let mal = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("DELETE"))
        .and(path("/anime/21/my_list_status"))
        .respond_with(wiremock::ResponseTemplate::new(500))
        .mount(&mal)
        .await;
    // Live read confirms it's gone (no my_list_status → None).
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/21"))
        .and(query_param("fields", "my_list_status"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(r#"{"id":21}"#))
        .mount(&mal)
        .await;
    mount_mal_me(&mal).await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = mal_provider(&mal.uri());
    let removed = remove_entry_via(
        &state,
        ProviderKind::MyAnimeList,
        &provider,
        &test_tokens(),
        "12",
    )
    .await
    .expect("a delete error must not surface when the entry is already gone");
    assert!(removed, "an already-gone entry reports removed");
}

#[tokio::test]
async fn remove_entry_via_propagates_a_delete_error_when_the_entry_is_still_present() {
    // The flip side: a genuine failure (the show is still on the list)
    // must propagate, not be silently swallowed.
    use wiremock::matchers::{method, path, query_param};
    let kitsu = mappable_kitsu().await;
    let mal = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("DELETE"))
        .and(path("/anime/21/my_list_status"))
        .respond_with(wiremock::ResponseTemplate::new(500))
        .mount(&mal)
        .await;
    // Live read shows it's STILL on the list → the failure is real.
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/21"))
        .and(query_param("fields", "my_list_status"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"id":21,"my_list_status":{"status":"watching","num_episodes_watched":3}}"#,
        ))
        .mount(&mal)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = mal_provider(&mal.uri());
    let res = remove_entry_via(
        &state,
        ProviderKind::MyAnimeList,
        &provider,
        &test_tokens(),
        "12",
    )
    .await;
    assert!(res.is_err(), "a real delete failure must propagate");
}

#[tokio::test]
async fn remove_entry_via_treats_a_404_delete_as_already_removed() {
    // Codex P2 #3423108945: the title was already gone upstream (MAL 404 —
    // the user double-clicked Remove, or removed it in another client).
    // The DELETE route is documented idempotent, so once the live read
    // confirms it's absent the call returns true and drops the cache row.
    use wiremock::matchers::{method, path, query_param};
    let kitsu = mappable_kitsu().await;
    let mal = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("DELETE"))
        .and(path("/anime/21/my_list_status"))
        .respond_with(wiremock::ResponseTemplate::new(404))
        .mount(&mal)
        .await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/21"))
        .and(query_param("fields", "my_list_status"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(r#"{"id":21}"#))
        .mount(&mal)
        .await;
    mount_mal_me(&mal).await;
    let state = state_with_kitsu(&kitsu.uri());
    let provider = mal_provider(&mal.uri());
    let removed = remove_entry_via(
        &state,
        ProviderKind::MyAnimeList,
        &provider,
        &test_tokens(),
        "12",
    )
    .await
    .expect("a 404 delete must not surface as an error");
    assert!(removed, "an already-removed title still reports removed");
}
