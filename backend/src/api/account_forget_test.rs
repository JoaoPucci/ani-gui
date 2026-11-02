//! `DELETE /api/account/entry/:provider/cached`: clearing the cached row
//! of a show the tracker's live read found absent. It asks the tracker
//! nothing — it carries no bearer to ask with — so a removal on a
//! tracker that never had the row spends none of its allowance and
//! cannot fail on it. A child of the account route tests, for their
//! state helpers.

use super::*;

use crate::account::cache;
use crate::account::provider::{ListEntry, ProviderKind, ProviderMediaId};
use crate::account::status::ListStatus;

/// Kitsu `/anime/12` → MAL id 21.
async fn kitsu_mapping_12_to_mal_21() -> wiremock::MockServer {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"id":"12","type":"anime"},"included":[{"id":"1","type":"mappings","attributes":{"externalSite":"myanimelist/anime","externalId":"21"}}]}"#,
        ))
        .mount(&kitsu)
        .await;
    kitsu
}

fn planned(id: u32) -> ListEntry {
    ListEntry {
        provider: ProviderKind::MyAnimeList,
        media_id: ProviderMediaId(id),
        mal_id: Some(id),
        status: ListStatus::Planning,
        progress_episodes: 0,
        score_0_to_100: None,
        updated_at_epoch_s: 1,
        title: String::new(),
    }
}

fn cached_ids(state: &AppState, user: &str) -> Vec<u32> {
    cache::list_entries(&state.cache_pool, ProviderKind::MyAnimeList, user)
        .unwrap()
        .iter()
        .map(|e| e.media_id.0)
        .collect()
}

fn forget(secret: Option<&str>) -> Request<Body> {
    let mut req = Request::builder()
        .method("DELETE")
        .uri("/api/account/entry/mal/cached?kitsu_id=12&user_id=4242");
    if let Some(secret) = secret {
        req = req.header("x-ani-gui-internal-secret", secret);
    }
    req.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn answers_without_a_bearer_and_clears_nothing_no_absent_read_justifies() {
    // The clear removes a row only on the evidence of a live read that
    // found the show absent (the command tests drive that pairing). With
    // no such read, it answers and leaves every row as it is.
    let kitsu = kitsu_mapping_12_to_mal_21().await;
    let td = TempDir::new().unwrap();
    let state = test_state_with_kitsu(&td, &kitsu.uri());
    let kind = ProviderKind::MyAnimeList;
    cache::write_entries(&state.cache_pool, kind, "4242", &[planned(21), planned(22)]).unwrap();
    cache::write_entries(&state.cache_pool, kind, "someone-else", &[planned(21)]).unwrap();
    let r = router()
        .with_state(state.clone())
        .oneshot(forget(Some("dead")))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);
    assert_eq!(cached_ids(&state, "4242"), vec![21, 22]);
    assert_eq!(cached_ids(&state, "someone-else"), vec![21]);
}

#[tokio::test]
async fn refuses_without_the_internal_secret() {
    let kitsu = kitsu_mapping_12_to_mal_21().await;
    let td = TempDir::new().unwrap();
    let state = test_state_with_kitsu(&td, &kitsu.uri());
    cache::write_entries(
        &state.cache_pool,
        ProviderKind::MyAnimeList,
        "4242",
        &[planned(21)],
    )
    .unwrap();
    for secret in [None, Some("wrong-value")] {
        let r = router()
            .with_state(state.clone())
            .oneshot(forget(secret))
            .await
            .unwrap();
        assert!(!r.status().is_success(), "{secret:?} was let through");
    }
    assert_eq!(cached_ids(&state, "4242"), vec![21]);
}

#[tokio::test]
async fn an_unmappable_show_clears_nothing() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"id":"12","type":"anime"},"included":[]}"#),
        )
        .mount(&kitsu)
        .await;
    let td = TempDir::new().unwrap();
    let state = test_state_with_kitsu(&td, &kitsu.uri());
    cache::write_entries(
        &state.cache_pool,
        ProviderKind::MyAnimeList,
        "4242",
        &[planned(21)],
    )
    .unwrap();
    let r = router()
        .with_state(state.clone())
        .oneshot(forget(Some("dead")))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);
    assert_eq!(cached_ids(&state, "4242"), vec![21]);
}
