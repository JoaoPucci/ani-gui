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
    // A cached resolution records that its resolve played "1.5" — here
    // a row an earlier build kept under the request "2.5" — and the
    // replay's session names that.
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
            former_episode: None,
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

fn key_for(asked: &PlayArgs, episode: &str) -> String {
    play_resolution_cache::cache_key(
        &asked.title,
        &asked.mode,
        "best",
        episode,
        asked.year,
        asked.episode_count,
        asked.subtype.as_deref(),
    )
}

#[tokio::test]
async fn a_resolve_by_a_former_tag_is_kept_under_the_episode_it_played() {
    // Kept under the request "2.5", the recap's stream would answer a
    // later request for "2.5" — another row's, should the listing tag
    // one so — for as long as the row lives. It is the episode "1.5".
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let asked = args("2.5");
    play_with_progress(&state, &asked, |_| {})
        .await
        .expect("plays");
    let under = |ep: &str| {
        play_resolution_cache::get(&state.cache_pool, &key_for(&asked, ep)).expect("readable")
    };
    assert!(under("2.5").is_none(), "nothing under the former tag");
    let kept = under("1.5").expect("kept under the episode played");
    assert_eq!(kept.played.as_deref(), Some("1.5"));
}

#[tokio::test]
async fn a_cumulative_listings_opening_recap_asked_for_by_its_former_number_plays_as_itself() {
    // An earlier version advertised the opening recap as "12.5"; it is
    // "0.5" now. A request by the old number plays the recap, and the
    // session names "0.5" and the "12.5" a position from then is under.
    use crate::commands::play_native_test_provider::{cumulative_opening_recap, CUMULATIVE};
    let td = tempfile::tempdir().expect("td");
    let mut state = state(&td);
    state
        .providers
        .set_stub(std::sync::Arc::new(cumulative_opening_recap()));
    let asked: PlayArgs = serde_json::from_value(serde_json::json!({
        "title": CUMULATIVE,
        "episode": "12.5",
        "mode": "sub",
        "episode_count": 2,
        "year": 2026,
        "kitsu_id": "88",
    }))
    .expect("args");
    let resp = play_with_progress(&state, &asked, |_| {})
        .await
        .expect("plays the recap");
    assert_eq!(resp.episode.as_deref(), Some("0.5"));
    assert_eq!(resp.former_episode.as_deref(), Some("12.5"));
}

/// The stitched show's key under the test provider.
fn stitched_key() -> String {
    crate::scraper::provider::ShowKey::new(
        crate::provider_set::ProviderSet::TEST_DEFAULT,
        STITCHED_SLUG,
    )
    .to_string()
}

/// The cached resolution a fresh play of the first of two recaps left:
/// slot 13, played as "12.5".
fn first_recap_row(upstream: &str) -> CachedResolution {
    CachedResolution {
        upstream_url: format!("{upstream}/x/master.m3u8"),
        referer: String::new(),
        media_kind: MediaKind::Hls,
        show_id: stitched_key(),
        show_title: STITCHED.into(),
        resolved_slot: Some(13),
        subtitles: Vec::new(),
        kitsu_id: None,
        played: Some("12.5".into()),
        former_episode: None,
    }
}

fn listed_as(state: &AppState) -> String {
    crate::commands::history::history_list(state)
        .expect("list")
        .into_iter()
        .find(|e| e.id == stitched_key())
        .expect("row")
        .ep_no
}

#[tokio::test]
async fn a_replay_of_one_recap_after_another_reads_back_as_the_one_replayed() {
    // Two recaps sit between episodes 12 and 13: slot 13 ("12.5") and
    // slot 14 ("12.75"). The second was played last, so the show's one
    // display stamp names slot 14. Replaying the first from the cache
    // writes slot 13; it must read back as "12.5", not as the slot.
    let upstream = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("HEAD"))
        .respond_with(wiremock::ResponseTemplate::new(200))
        .mount(&upstream)
        .await;
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    std::fs::write(&state.config_path, "cache_resolutions = true\n").expect("config");
    crate::commands::numbering_offset::put_display(&state, &stitched_key(), 0, 14, "12.75");
    let asked = args("12.5");
    play_resolution_cache::put(
        &state.cache_pool,
        &key_for(&asked, "12.5"),
        &first_recap_row(&upstream.uri()),
    );
    let resp = play_with_progress(&state, &asked, |_| {})
        .await
        .expect("replays");
    assert!(resp.cache_hit);
    assert_eq!(listed_as(&state), "12.5");
}

#[tokio::test]
async fn a_mark_watched_of_one_recap_after_another_reads_back_as_the_one_watched() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    crate::commands::numbering_offset::put_display(&state, &stitched_key(), 0, 14, "12.75");
    let cached = first_recap_row("http://x");
    let watch = crate::commands::play_cache::cached_watch(&state, &cached, "12.5");
    crate::commands::play_native_record::record_watch(&state, &watch, None).await;
    crate::commands::play_cache::restamp_recorded(&state, &cached);
    assert_eq!(listed_as(&state), "12.5");
}

#[test]
fn a_watch_that_recorded_another_slot_leaves_the_stamp_where_it_is() {
    // The row holds slot 14 ("12.75"): a mark-watched of the cached
    // slot-13 recap that recorded nothing — overtaken by that watch —
    // must not move the stamp off the row.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let row = crate::history::parse(&format!("14\t{}\tStitched Show\n", stitched_key()))
        .pop()
        .expect("a row");
    crate::history::upsert_and_write(&state.history_path, row).expect("write");
    crate::commands::numbering_offset::put_display(&state, &stitched_key(), 0, 14, "12.75");
    crate::commands::play_cache::restamp_recorded(&state, &first_recap_row("http://x"));
    assert_eq!(listed_as(&state), "12.75");
}

#[tokio::test]
async fn a_resolve_keeps_the_former_number_with_its_cached_row() {
    // A prefetch's session is never opened, so its former number has to
    // outlive it for the replay that the click is.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let mut asked = args("1.5");
    asked.prefetch = true;
    play_with_progress(&state, &asked, |_| {})
        .await
        .expect("plays");
    let kept = play_resolution_cache::get(&state.cache_pool, &key_for(&asked, "1.5"))
        .expect("readable")
        .expect("kept");
    assert_eq!(kept.former_episode.as_deref(), Some("2.5"));
}

#[tokio::test]
async fn a_replay_names_the_former_number_its_row_keeps() {
    let upstream = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("HEAD"))
        .respond_with(wiremock::ResponseTemplate::new(200))
        .mount(&upstream)
        .await;
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    std::fs::write(&state.config_path, "cache_resolutions = true\n").expect("config");
    let asked = args("12.5");
    let mut row = first_recap_row(&upstream.uri());
    row.former_episode = Some("24.5".into());
    play_resolution_cache::put(&state.cache_pool, &key_for(&asked, "12.5"), &row);
    let resp = play_with_progress(&state, &asked, |_| {})
        .await
        .expect("replays");
    assert!(resp.cache_hit);
    assert_eq!(resp.former_episode.as_deref(), Some("24.5"));
}

#[tokio::test]
async fn a_prefetch_after_a_watched_recap_leaves_the_row_reading_as_the_recap() {
    // The recap is watched — the row holds its slot, the stamp names it
    // "0.5" — then the player prefetches episode 1, a displaced row of
    // its own. The prefetch records no watch, and the row still reads
    // as the recap.
    use crate::commands::play_native_test_provider::{
        cumulative_opening_recap, CUMULATIVE, CUMULATIVE_SLUG,
    };
    let td = tempfile::tempdir().expect("td");
    let mut state = state(&td);
    state
        .providers
        .set_stub(std::sync::Arc::new(cumulative_opening_recap()));
    let ask = |episode: &str, prefetch: bool| -> PlayArgs {
        serde_json::from_value(serde_json::json!({
            "title": CUMULATIVE,
            "episode": episode,
            "mode": "sub",
            "episode_count": 2,
            "year": 2026,
            "prefetch": prefetch,
        }))
        .expect("args")
    };
    play_with_progress(&state, &ask("0.5", false), |_| {})
        .await
        .expect("plays the recap");
    play_with_progress(&state, &ask("1", true), |_| {})
        .await
        .expect("prefetches episode 1");
    let key = crate::scraper::provider::ShowKey::new(
        crate::provider_set::ProviderSet::TEST_DEFAULT,
        CUMULATIVE_SLUG,
    )
    .to_string();
    let listed = crate::commands::history::history_list(&state)
        .expect("list")
        .into_iter()
        .find(|e| e.id == key)
        .expect("row")
        .ep_no;
    assert_eq!(listed, "0.5");
}

#[tokio::test]
async fn a_watch_marked_on_a_prefetched_recap_reads_back_as_the_recap() {
    // A click that shared the page's prefetch of the recap records its
    // watch through mark-watched: the row then reads as the recap,
    // though only the prefetch resolved it.
    use crate::commands::play_native_test_provider::{
        cumulative_opening_recap, CUMULATIVE, CUMULATIVE_SLUG,
    };
    use tower::ServiceExt as _;
    let td = tempfile::tempdir().expect("td");
    let mut state = state(&td);
    state
        .providers
        .set_stub(std::sync::Arc::new(cumulative_opening_recap()));
    let asked = serde_json::json!({
        "title": CUMULATIVE,
        "episode": "0.5",
        "mode": "sub",
        "episode_count": 2,
        "year": 2026,
    });
    let mut prefetch: PlayArgs = serde_json::from_value(asked.clone()).expect("args");
    prefetch.prefetch = true;
    play_with_progress(&state, &prefetch, |_| {})
        .await
        .expect("prefetches the recap");
    let router = crate::api::build_api_router(std::sync::Arc::new(state.clone()));
    let response = router
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/play/mark-watched")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&asked).expect("json"),
                ))
                .expect("req"),
        )
        .await
        .expect("oneshot");
    assert!(response.status().is_success());
    let key = crate::scraper::provider::ShowKey::new(
        crate::provider_set::ProviderSet::TEST_DEFAULT,
        CUMULATIVE_SLUG,
    )
    .to_string();
    let listed = crate::commands::history::history_list(&state)
        .expect("list")
        .into_iter()
        .find(|e| e.id == key)
        .expect("row")
        .ep_no;
    assert_eq!(listed, "0.5");
}
