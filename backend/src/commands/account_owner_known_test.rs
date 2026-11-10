//! Tracker writes for a bearer whose user is already known: the write
//! asks the tracker nothing about the account, so a refused lookup
//! cannot hold it — until a disconnect forgets the bearer, when the
//! write goes back to asking first.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{
    mal_patch_answer, mal_provider, mappable_kitsu, remove_entry_via, set_entry_via,
    state_with_kitsu, test_tokens,
};
use crate::account::provider::{EntryUpdate, ListEntry, ProviderKind, ProviderMediaId};
use crate::account::status::ListStatus;
use crate::app::AppState;
use crate::error::AniError;

/// Which write a case makes.
#[derive(Clone, Copy, Debug)]
enum Write {
    Save,
    Removal,
    Progress,
}

fn planned(kind: ProviderKind) -> ListEntry {
    ListEntry {
        provider: kind,
        media_id: ProviderMediaId(21),
        mal_id: Some(21),
        status: ListStatus::Planning,
        progress_episodes: 0,
        score_0_to_100: None,
        updated_at_epoch_s: 1,
        title: "One Piece".into(),
    }
}

async fn make(
    write: Write,
    state: &Arc<AppState>,
    provider: &crate::meta::mal_user::MalProvider,
) -> crate::error::Result<()> {
    let kind = ProviderKind::MyAnimeList;
    let tokens = test_tokens();
    let update = EntryUpdate {
        status: Some(ListStatus::Watching),
        progress_episodes: Some(3),
        ..Default::default()
    };
    match write {
        Write::Progress => crate::commands::account::write_through(
            state,
            kind,
            provider,
            &tokens,
            ProviderMediaId(21),
            update,
            state.account_write_locks.epochs.current(kind),
        )
        .await
        .map(|_| ()),
        Write::Removal => remove_entry_via(state, kind, provider, &tokens, "12")
            .await
            .map(|_| ()),
        Write::Save => set_entry_via(state, kind, provider, &tokens, "12", update)
            .await
            .map(|_| ()),
    }
}

/// MAL 21 cached at Plan to Watch for user 4242. The account lookup is
/// answered once — learning the bearer — and refused with a 429 from
/// then on. With `disconnected`, the account disconnects after that
/// first answer (and the row is cached again, as a reconnect's pull
/// would). Then `write` is made.
async fn known_bearer_then_refused_lookup(write: Write, disconnected: bool) {
    let kitsu = mappable_kitsu().await;
    let mal = MockServer::start().await;
    let state = state_with_kitsu(&kitsu.uri());
    let kind = ProviderKind::MyAnimeList;
    let removal = matches!(write, Write::Removal);
    let mutated = Arc::new(AtomicBool::new(false));
    let wrote = mutated.clone();
    Mock::given(method(if removal { "DELETE" } else { "PATCH" }))
        .and(path("/anime/21/my_list_status"))
        .respond_with(move |_: &wiremock::Request| {
            wrote.store(true, Ordering::SeqCst);
            if removal {
                ResponseTemplate::new(200)
            } else {
                mal_patch_answer()
            }
        })
        .mount(&mal)
        .await;
    Mock::given(method("GET"))
        .and(path("/users/@me"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"id":4242,"name":"shiro"}"#))
        .up_to_n_times(1)
        .mount(&mal)
        .await;
    Mock::given(method("GET"))
        .and(path("/users/@me"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "30"))
        .mount(&mal)
        .await;
    let provider = mal_provider(&mal.uri());
    let tokens = test_tokens();
    let owners = &state.account_write_locks.owners;
    assert_eq!(
        owners.owner(kind, &provider, &tokens).await.expect("owner"),
        "4242"
    );
    if disconnected {
        crate::commands::account::clear_provider_cache(&state, kind).expect("disconnect");
    }
    crate::account::cache::write_entries(&state.cache_pool, kind, "4242", &[planned(kind)])
        .unwrap();
    let result = make(write, &state, &provider).await;
    let rows = crate::account::cache::list_entries(&state.cache_pool, kind, "4242").unwrap();
    let row = rows.iter().find(|r| r.media_id.0 == 21);
    if disconnected {
        assert!(
            matches!(result, Err(AniError::RateLimited { .. })),
            "{write:?}: a forgotten bearer's refused lookup must hold the write: {result:?}"
        );
        assert!(
            !mutated.load(Ordering::SeqCst),
            "{write:?}: the tracker changed without a cache owner"
        );
        assert_eq!(row.map(|r| r.status), Some(ListStatus::Planning));
        return;
    }
    result.unwrap_or_else(|e| panic!("{write:?}: a known bearer's write was held: {e:?}"));
    assert!(
        mutated.load(Ordering::SeqCst),
        "{write:?}: nothing reached the tracker"
    );
    match (removal, row) {
        (true, None) => {}
        (false, Some(row)) => assert_eq!(
            (row.status, row.progress_episodes),
            (ListStatus::Watching, 3),
            "{write:?}: the cache missed the write"
        ),
        (_, row) => panic!("{write:?}: the cached row is wrong: {row:?}"),
    }
}

#[tokio::test]
async fn a_removal_for_a_known_bearer_goes_out_and_drops_its_row_when_the_lookup_is_refused() {
    known_bearer_then_refused_lookup(Write::Removal, false).await;
}
#[tokio::test]
async fn progress_for_a_known_bearer_goes_out_and_files_its_row_when_the_lookup_is_refused() {
    known_bearer_then_refused_lookup(Write::Progress, false).await;
}
#[tokio::test]
async fn a_save_after_a_disconnect_waits_for_the_owner_again() {
    known_bearer_then_refused_lookup(Write::Save, true).await;
}
#[tokio::test]
async fn a_removal_after_a_disconnect_waits_for_the_owner_again() {
    known_bearer_then_refused_lookup(Write::Removal, true).await;
}
#[tokio::test]
async fn progress_after_a_disconnect_waits_for_the_owner_again() {
    known_bearer_then_refused_lookup(Write::Progress, true).await;
}

/// Kitsu `/anime/12` → AniList 21 directly (no MAL mapping).
const KITSU_ANILIST_MAPPING_BODY: &str = r#"{
    "data": { "id": "12", "type": "anime", "attributes": { "canonicalTitle": "One Piece" } },
    "included": [{
        "id": "1176",
        "type": "mappings",
        "attributes": { "externalSite": "anilist/anime", "externalId": "21" }
    }]
}"#;

#[tokio::test]
async fn an_anilist_removal_for_a_known_bearer_asks_nothing_about_the_viewer() {
    // AniList's removal needs the user to find the list row. With the
    // bearer known, the removal reuses it: the one Viewer query is the
    // lookup that learned the bearer.
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(ResponseTemplate::new(200).set_body_string(KITSU_ANILIST_MAPPING_BODY))
        .mount(&kitsu)
        .await;
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_string_contains("Viewer"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"Viewer":{"id":4242,"name":"shiro","avatar":{"large":null,"medium":null},"statistics":{"anime":{"count":0,"meanScore":0}}}}}"#,
        ))
        .expect(1)
        .mount(&anilist)
        .await;
    Mock::given(method("POST"))
        .and(body_string_contains("$mediaId: Int!, $userId: Int!"))
        .and(body_string_contains("\"userId\":4242"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"data":{"MediaList":{"id":7777}}}"#),
        )
        .expect(1)
        .mount(&anilist)
        .await;
    Mock::given(method("POST"))
        .and(body_string_contains("DeleteMediaListEntry"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"DeleteMediaListEntry":{"deleted":true}}}"#),
        )
        .expect(1)
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let kind = ProviderKind::AniList;
    let provider = crate::meta::anilist_user::AniListProvider::with_bases(
        reqwest::Client::new(),
        anilist.uri(),
        "http://unused-token".to_owned(),
    );
    let tokens = test_tokens();
    state
        .account_write_locks
        .owners
        .owner(kind, &provider, &tokens)
        .await
        .expect("owner");
    crate::account::cache::write_entries(&state.cache_pool, kind, "4242", &[planned(kind)])
        .unwrap();
    assert!(remove_entry_via(&state, kind, &provider, &tokens, "12")
        .await
        .expect("removed"));
    let rows = crate::account::cache::list_entries(&state.cache_pool, kind, "4242").unwrap();
    assert!(rows.is_empty(), "the removed row stayed cached: {rows:?}");
    // `.expect(1)` on the Viewer query: the removal asked it nothing.
}
