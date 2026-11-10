//! The route a Continue row's guess asks which of its hits the row
//! rules out by.

use super::tests::{body_string, test_app_state};
use super::*;
use axum::body::Body;
use axum::http::Request;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

async fn post(body: serde_json::Value) -> (StatusCode, String) {
    let td = TempDir::new().expect("tempdir");
    let router = build_api_router(Arc::new(test_app_state(&td)));
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/history/guess/ruled-out")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .expect("req"),
        )
        .await
        .expect("oneshot");
    let status = response.status();
    (status, body_string(response).await)
}

#[tokio::test]
async fn the_route_answers_the_ids_the_row_rules_out() {
    // hianime's "Jujutsu Kaisen (TV)" at its last episode, against the
    // special Kitsu's search returned for it and the series, written as
    // the renderer holds them: the special is out.
    let (status, body) = post(serde_json::json!({
        "key": "hianime:jujutsu-kaisen-237",
        "title": "Jujutsu Kaisen (TV)",
        "episode": 24,
        "hits": [
            {
                "id": "43886",
                "canonical_title": "Jujutsu Kaisen New Year's Special",
                "titles": { "en": "Jujutsu Kaisen New Year's Special" },
                "abbreviated_titles": [],
                "slug": "jujutsu-kaisen-new-year-s-special",
                "episode_count": 1,
                "subtype": "special"
            },
            {
                "id": "42765",
                "canonical_title": "Jujutsu Kaisen",
                "titles": { "en": "Jujutsu Kaisen" },
                "abbreviated_titles": [],
                "slug": "jujutsu-kaisen",
                "episode_count": 24,
                "subtype": "TV"
            }
        ]
    }))
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ids: Vec<String> = serde_json::from_str(&body).expect("ids");
    assert_eq!(ids, vec!["43886"]);
}

#[tokio::test]
async fn a_hit_missing_its_optional_fields_is_read_as_carrying_none() {
    // A hit from a cached answer written before Kitsu's titles were
    // kept carries no `titles` or `abbreviated_titles`; it is judged on
    // what it carries rather than refused as malformed.
    let (status, body) = post(serde_json::json!({
        "key": "hianime:black-clover-949",
        "title": "Black Clover",
        "episode": 100,
        "hits": [
            { "id": "12625", "canonical_title": "Black Clover: Jump Festa 2016 Special", "episode_count": 1 },
            { "id": "13209", "canonical_title": "Black Clover" }
        ]
    }))
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ids: Vec<String> = serde_json::from_str(&body).expect("ids");
    assert_eq!(ids, vec!["12625"]);
}

#[tokio::test]
async fn a_row_a_retired_provider_wrote_is_never_past_a_hit() {
    // anidb.app listed a later cour on the franchise's numbering, and a
    // row it wrote with no numbering stamp still speaks that number:
    // episode 55 of a cour Kitsu counts 10 episodes. Only a row a
    // provider the app walks wrote, which numbers each listing from its
    // first episode, says how far into the hit it got.
    let later_cour = serde_json::json!({
        "id": "7",
        "canonical_title": "Show Part 2",
        "slug": "show-part-2",
        "episode_count": 10,
        "subtype": "TV"
    });
    let (status, body) = post(serde_json::json!({
        "key": "show-part-2-3687",
        "title": "Show Part 2",
        "episode": 55,
        "hits": [later_cour.clone()]
    }))
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ids: Vec<String> = serde_json::from_str(&body).expect("ids");
    assert!(
        ids.is_empty(),
        "a bare key's episode rules nothing out: {ids:?}"
    );
    // The same row from a walked provider is past it.
    let (_, body) = post(serde_json::json!({
        "key": "hianime:show-part-2-1",
        "title": "Show Part 2",
        "episode": 55,
        "hits": [later_cour]
    }))
    .await;
    let ids: Vec<String> = serde_json::from_str(&body).expect("ids");
    assert_eq!(ids, vec!["7"]);
}

#[tokio::test]
async fn the_route_reads_the_season_the_show_key_names() {
    // hianime's "Jujutsu Kaisen: Specials" is
    // `jujutsu-kaisen-season-2-recaps`: its key names the second season
    // and its title none, so the New Year's Special Kitsu's search
    // returns for the title is out and the second season's recap is not.
    let (status, body) = post(serde_json::json!({
        "key": "hianime:jujutsu-kaisen-season-2-recaps-3513",
        "title": "Jujutsu Kaisen: Specials",
        "episode": 1,
        "hits": [
            {
                "id": "43886",
                "canonical_title": "Jujutsu Kaisen New Year's Special",
                "titles": { "en": "Jujutsu Kaisen New Year's Special" },
                "abbreviated_titles": ["Jujutsu Kaisen (TV) Recap"],
                "slug": "jujutsu-kaisen-new-year-s-special",
                "episode_count": 1,
                "subtype": "special"
            },
            {
                "id": "47880",
                "canonical_title": "Jujutsu Kaisen 2nd Season Recap",
                "titles": { "en": "Jujutsu Kaisen Season 2 Recap" },
                "abbreviated_titles": ["Jujutsu Kaisen Season 2 + Movie Recap"],
                "slug": "jujutsu-kaisen-2nd-season-recap",
                "episode_count": 2,
                "subtype": "special"
            }
        ]
    }))
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ids: Vec<String> = serde_json::from_str(&body).expect("ids");
    assert_eq!(ids, vec!["43886"]);
}
