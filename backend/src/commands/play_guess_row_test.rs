//! A play asked from a guessed Continue card says whether it landed on
//! that card's own listing.

use super::*;
use crate::commands::play_native_test_provider::{stitched_recap, STITCHED, STITCHED_SLUG};

fn state(td: &tempfile::TempDir) -> AppState {
    let mut state = super::tests::state_with_proxy_origin();
    state
        .providers
        .set_stub(std::sync::Arc::new(stitched_recap()));
    state.config_path = td.path().join("config.toml");
    state.history_path = td.path().join("history");
    state
}

/// The key the stitched show plays under.
fn played_key() -> String {
    crate::scraper::provider::ShowKey::new(
        crate::provider_set::ProviderSet::TEST_DEFAULT,
        STITCHED_SLUG,
    )
    .to_string()
}

fn args(guess_row: Option<&str>) -> PlayArgs {
    let mut v = serde_json::json!({
        "title": STITCHED,
        "episode": "5",
        "mode": "sub",
        "episode_count": 12,
        "year": 2026,
        "subtype": "ONA",
    });
    if let Some(row) = guess_row {
        v["guess_row"] = serde_json::json!(row);
    }
    serde_json::from_value(v).expect("args")
}

#[tokio::test]
async fn a_guessed_play_that_lands_on_its_cards_listing_says_so() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let resp = play_with_progress(&state, &args(Some(&played_key())), |_| {})
        .await
        .expect("plays");
    assert!(resp.own_listing);
}

#[tokio::test]
async fn a_guessed_play_from_a_bare_row_with_the_listings_words_lands_on_it() {
    // anidb.app's key for the same show: its words, its own id.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let resp = play_with_progress(&state, &args(Some("stitched-show-907")), |_| {})
        .await
        .expect("plays");
    assert!(resp.own_listing);
}

#[tokio::test]
async fn a_guessed_play_that_lands_on_another_listing_says_it_did_not() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let other = crate::scraper::provider::ShowKey::new(
        crate::provider_set::ProviderSet::TEST_DEFAULT,
        "another-show-4",
    )
    .to_string();
    let resp = play_with_progress(&state, &args(Some(&other)), |_| {})
        .await
        .expect("plays");
    assert!(!resp.own_listing);
}

#[tokio::test]
async fn a_play_no_guessed_card_asked_for_lands_on_no_cards_listing() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let resp = play_with_progress(&state, &args(None), |_| {})
        .await
        .expect("plays");
    assert!(!resp.own_listing);
    // And the wire says nothing of it.
    let json = serde_json::to_value(&resp).expect("json");
    assert!(json.get("own_listing").is_none());
}

#[tokio::test]
async fn a_replayed_resolution_says_whether_it_lands_on_the_cards_listing() {
    let upstream = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("HEAD"))
        .respond_with(wiremock::ResponseTemplate::new(200))
        .mount(&upstream)
        .await;
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    std::fs::write(&state.config_path, "cache_resolutions = true\n").expect("config");
    let asked = args(Some(&played_key()));
    play_resolution_cache::put(
        &state.cache_pool,
        &asked.resolution_key(),
        &CachedResolution {
            upstream_url: format!("{}/x/master.m3u8", upstream.uri()),
            referer: String::new(),
            media_kind: MediaKind::Hls,
            show_id: played_key(),
            show_title: STITCHED.into(),
            resolved_slot: Some(5),
            subtitles: Vec::new(),
            kitsu_id: None,
            played: Some("5".into()),
            former_episode: None,
            versions: Vec::new(),
            version: None,
            episode_unverified: false,
        },
    );
    let resp = play_with_progress(&state, &asked, |_| {})
        .await
        .expect("replays");
    assert!(resp.cache_hit);
    assert!(resp.own_listing);
    let elsewhere = args(Some("test-a:another-show-4"));
    let resp = play_with_progress(&state, &elsewhere, |_| {})
        .await
        .expect("replays");
    assert!(resp.cache_hit);
    assert!(!resp.own_listing);
}
