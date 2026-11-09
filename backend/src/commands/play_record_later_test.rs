//! A play whose caller records the watch itself, once its player has
//! opened on the stream (`record_later`): the player's switch to the
//! show the user picked for a card. Its resolve writes no history row,
//! on a fresh resolve or a replay, so a viewer who leaves before the
//! player opens, or a player that cannot open, leaves the history as it
//! was. It is still a user waiting, not a warm: it runs at interactive
//! priority.

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

fn args(record_later: bool) -> PlayArgs {
    serde_json::from_value(serde_json::json!({
        "title": STITCHED,
        "episode": "5",
        "mode": "sub",
        "episode_count": 12,
        "year": 2026,
        "subtype": "ONA",
        "kitsu_id": "77",
        "record_later": record_later,
    }))
    .expect("args")
}

fn row(state: &AppState) -> Option<crate::history::HistoryEntry> {
    let key = crate::scraper::provider::ShowKey::new(
        crate::provider_set::ProviderSet::TEST_DEFAULT,
        STITCHED_SLUG,
    )
    .to_string();
    crate::commands::history::history_list(state)
        .expect("list")
        .into_iter()
        .find(|e| e.id == key)
}

#[tokio::test]
async fn a_fresh_resolve_recorded_later_writes_no_history_row() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    play_with_progress(&state, &args(true), |_| {})
        .await
        .expect("plays");
    assert_eq!(row(&state), None);
    // The same play recorded on resolving writes it, as before.
    play_with_progress(&state, &args(false), |_| {})
        .await
        .expect("plays");
    assert!(row(&state).is_some());
}

#[tokio::test]
async fn a_replay_recorded_later_writes_no_history_row() {
    let upstream = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("HEAD"))
        .respond_with(wiremock::ResponseTemplate::new(200))
        .mount(&upstream)
        .await;
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    std::fs::write(&state.config_path, "cache_resolutions = true\n").expect("config");
    let asked = args(true);
    play_with_progress(&state, &asked, |_| {})
        .await
        .expect("resolves");
    let key = play_resolution_cache::cache_key(
        &asked.title,
        &asked.mode,
        "best",
        "5",
        asked.year,
        asked.episode_count,
        asked.subtype.as_deref(),
    );
    let mut cached = play_resolution_cache::get(&state.cache_pool, &key)
        .expect("readable")
        .expect("kept");
    // The stub's stream is not served over HTTP; point the row at one
    // that answers the replay's check.
    cached.upstream_url = format!("{}/x/master.m3u8", upstream.uri());
    play_resolution_cache::put(&state.cache_pool, &key, &cached);
    let resp = play_with_progress(&state, &asked, |_| {})
        .await
        .expect("replays");
    assert!(resp.cache_hit);
    assert_eq!(row(&state), None);
}

#[test]
fn a_play_recorded_later_is_still_a_user_waiting() {
    assert!(matches!(
        scrape_priority(&args(true)),
        crate::scraper::gate::ScrapePriority::Interactive
    ));
}

#[test]
fn the_play_url_carries_it_as_the_renderer_sends_it() {
    // The renderer's SSE GET sends `record_later=1`; absent, a play
    // writes its history as it always did.
    let asked: PlayArgs =
        serde_urlencoded::from_str("title=X&episode=1&mode=sub&record_later=1").expect("parses");
    assert!(asked.record_later);
    assert!(!asked.writes_history());
    let plain: PlayArgs = serde_urlencoded::from_str("title=X&episode=1&mode=sub").expect("parses");
    assert!(!plain.record_later);
    assert!(plain.writes_history());
}

proptest::proptest! {
    /// A play writes its history unless it is a warm or its caller
    /// records it later, and only a warm is background traffic.
    #[test]
    fn history_and_priority_each_follow_their_own_flag(
        prefetch in proptest::bool::ANY,
        record_later in proptest::bool::ANY,
        episode in "[0-9]{1,4}",
    ) {
        let asked: PlayArgs = serde_json::from_value(serde_json::json!({
            "title": "X",
            "episode": episode,
            "mode": "sub",
            "prefetch": prefetch,
            "record_later": record_later,
        }))
        .expect("args");
        proptest::prop_assert_eq!(asked.writes_history(), !prefetch && !record_later);
        let background = matches!(
            scrape_priority(&asked),
            crate::scraper::gate::ScrapePriority::Background
        );
        proptest::prop_assert_eq!(background, prefetch);
    }
}
