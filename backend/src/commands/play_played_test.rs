//! The session a play opens names the episode it plays.

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

fn args(episode: &str) -> PlayArgs {
    serde_json::from_value(serde_json::json!({
        "title": STITCHED,
        "episode": episode,
        "mode": "sub",
        "episode_count": 12,
        "year": 2026,
        "subtype": "ONA",
        "kitsu_id": "77",
    }))
    .expect("args")
}

#[tokio::test]
async fn a_request_by_a_former_tag_opens_a_session_named_for_the_episode_it_plays() {
    // A history row from before names the stitched recap "2.5"; it is
    // "1.5" now. The session plays the recap, and says so — the player
    // numbers it, steps from it and keeps its position by "1.5" — and
    // says what it was called, which a position kept then is under.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let resp = play_with_progress(&state, &args("2.5"), |_| {})
        .await
        .expect("plays");
    assert_eq!(resp.episode.as_deref(), Some("1.5"));
    assert_eq!(resp.former_episode.as_deref(), Some("2.5"));
}

#[tokio::test]
async fn a_request_by_a_rows_number_now_opens_a_session_named_for_it() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let resp = play_with_progress(&state, &args("5"), |_| {})
        .await
        .expect("plays");
    assert_eq!(resp.episode.as_deref(), Some("5"));
    assert_eq!(resp.former_episode, None);
}

#[tokio::test]
async fn a_replay_opens_a_session_named_for_the_episode_its_resolve_played() {
    // A cached resolution keyed by the request "2.5" records that its
    // resolve played "1.5"; the replay's session names that.
    let upstream = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("HEAD"))
        .respond_with(wiremock::ResponseTemplate::new(200))
        .mount(&upstream)
        .await;
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    std::fs::write(&state.config_path, "cache_resolutions = true\n").expect("config");
    let asked = args("2.5");
    let key = play_resolution_cache::cache_key(
        &asked.title,
        &asked.mode,
        "best",
        &asked.episode,
        asked.year,
        asked.episode_count,
        asked.subtype.as_deref(),
    );
    play_resolution_cache::put(
        &state.cache_pool,
        &key,
        &CachedResolution {
            upstream_url: format!("{}/x/master.m3u8", upstream.uri()),
            referer: String::new(),
            media_kind: MediaKind::Hls,
            show_id: crate::scraper::provider::ShowKey::new(
                crate::provider_set::ProviderSet::TEST_DEFAULT,
                STITCHED_SLUG,
            )
            .to_string(),
            show_title: STITCHED.into(),
            resolved_slot: Some(2),
            subtitles: Vec::new(),
            kitsu_id: None,
            played: Some("1.5".into()),
        },
    );
    let resp = play_with_progress(&state, &asked, |_| {})
        .await
        .expect("replays");
    assert!(resp.cache_hit);
    assert_eq!(resp.episode.as_deref(), Some("1.5"));
}

#[tokio::test]
async fn a_session_names_no_former_number_another_row_carries_now() {
    // The opening recap was "2.5" before; the listing tags another
    // recap "2.5" now. A position kept under "2.5" may be that one's,
    // so the session names no former number for the opening recap.
    let td = tempfile::tempdir().expect("td");
    let mut state = state(&td);
    state.providers.set_stub(std::sync::Arc::new(
        crate::commands::play_native_test_provider::stitched_shared_former(),
    ));
    let resp = play_with_progress(&state, &args("1.5"), |_| {})
        .await
        .expect("plays");
    assert_eq!(resp.episode.as_deref(), Some("1.5"));
    assert_eq!(resp.former_episode, None);
}
