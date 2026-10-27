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
            versions: Vec::new(),
            version: None,
            episode_unverified: false,
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
        versions: Vec::new(),
        version: None,
        episode_unverified: false,
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
    // slot 14 ("12.75"). An earlier version's watch of the second left
    // the show's display stamp on slot 14. Replaying the first from the
    // cache records "12.5" itself, and it reads back so, whatever the
    // stamp names.
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
    let watch = crate::commands::play_cache::cached_watch(&cached, "12.5");
    crate::commands::play_native_record::record_watch(&state, &watch, None).await;
    assert_eq!(listed_as(&state), "12.5");
}

#[tokio::test]
async fn a_cached_watch_overtaken_by_another_leaves_that_watchs_row() {
    // A mark-watched of the cached slot-13 recap began before the
    // slot-14 recap ("12.75") was watched: it records nothing, and the
    // row stays the one that watch left.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let begun = crate::history::guard::epoch(&state.history_path);
    let later = crate::commands::play_native_record::Watch {
        show_id: stitched_key(),
        title: "Stitched Show".into(),
        ep_no: "12.75".into(),
    };
    crate::commands::play_native_record::record_watch(&state, &later, None).await;
    let cached = first_recap_row("http://x");
    let watch = crate::commands::play_cache::cached_watch(&cached, "12.5");
    crate::commands::play_native_record::record_watch_requested_at(&state, &watch, None, begun)
        .await;
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

/// A listing the app has at hand for `show_key`, heard now: episodes 1
/// to 12, with a row tagged by each of `tags` among them.
fn hear_listing(state: &AppState, show_key: &str, tags: &[&str]) {
    let mut episodes: Vec<crate::scraper::provider::EpisodeRef> = (1..=12)
        .map(|n| crate::scraper::provider::EpisodeRef {
            id: u64::from(n),
            number: n,
            number2: None,
            former_tag: None,
            title: None,
        })
        .collect();
    for (i, tag) in tags.iter().enumerate() {
        episodes.push(crate::scraper::provider::EpisodeRef {
            id: 100 + i as u64,
            number: 13 + i as u32,
            number2: Some((*tag).to_string()),
            former_tag: None,
            title: None,
        });
    }
    crate::commands::listing_tags::record_listing(
        state,
        show_key,
        &episodes,
        crate::commands::listing_tags::Begun::now().picked(),
    );
}

/// Replay the slot-13 recap from a row that keeps the former number
/// "24.5", after `heard` (if any) — the show keys and tags of the
/// listings the app had at hand since — and answer the replay's
/// session.
async fn replay_after(heard: &[(&str, &[&str])]) -> CreateSessionResponse {
    let upstream = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("HEAD"))
        .respond_with(wiremock::ResponseTemplate::new(200))
        .mount(&upstream)
        .await;
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    std::fs::write(&state.config_path, "cache_resolutions = true\n").expect("config");
    for (show_key, tags) in heard {
        hear_listing(&state, show_key, tags);
    }
    let asked = args("12.5");
    let mut row = first_recap_row(&upstream.uri());
    row.former_episode = Some("24.5".into());
    play_resolution_cache::put(&state.cache_pool, &key_for(&asked, "12.5"), &row);
    let resp = play_with_progress(&state, &asked, |_| {})
        .await
        .expect("replays");
    assert!(resp.cache_hit);
    resp
}

#[tokio::test]
async fn a_replay_names_the_former_number_its_row_keeps() {
    // The listing the app last had at hand carries no "24.5": a position
    // kept under it is the recap's, and the replay names it.
    let resp = replay_after(&[(&stitched_key(), &["12.5", "12.75"])]).await;
    assert_eq!(resp.former_episode.as_deref(), Some("24.5"));
}

#[tokio::test]
async fn a_replay_names_no_former_number_the_listing_has_come_to_carry() {
    // Since the row was written, a listing at hand has come to tag
    // another episode "24.5": a position under it may be that one's.
    let resp = replay_after(&[
        (&stitched_key(), &["12.5"]),
        (&stitched_key(), &["12.5", "24.5"]),
    ])
    .await;
    assert_eq!(resp.former_episode, None);
}

#[tokio::test]
async fn a_replay_names_no_former_number_when_no_listing_is_known() {
    // Nothing records the listing: whether "24.5" names another episode
    // is unknown, and an unknown is not replayed as a fact.
    assert_eq!(replay_after(&[]).await.former_episode, None);
    // Another show's listing — another version's, another provider's —
    // says nothing of this one's.
    let other = crate::scraper::provider::ShowKey::new(
        crate::provider_set::ProviderSet::TEST_DEFAULT,
        "stitched-show-uncut-5",
    )
    .to_string();
    assert_eq!(
        replay_after(&[(&other, &["12.5"])]).await.former_episode,
        None
    );
}

#[tokio::test]
async fn a_replay_after_a_play_whose_listing_carries_its_former_number_names_none() {
    // The row's resolve found the opening recap's former number "2.5"
    // free. Since, the provider's listing tags another recap "2.5", and a
    // play of episode 5 had that listing at hand; the replay of the
    // opening recap names no former number.
    let upstream = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("HEAD"))
        .respond_with(wiremock::ResponseTemplate::new(200))
        .mount(&upstream)
        .await;
    let td = tempfile::tempdir().expect("td");
    let mut state = state(&td);
    std::fs::write(&state.config_path, "cache_resolutions = true\n").expect("config");
    let asked = args("1.5");
    let mut row = first_recap_row(&upstream.uri());
    row.resolved_slot = Some(2);
    row.played = Some("1.5".into());
    row.former_episode = Some("2.5".into());
    play_resolution_cache::put(&state.cache_pool, &key_for(&asked, "1.5"), &row);
    state.providers.set_stub(std::sync::Arc::new(
        crate::commands::play_native_test_provider::stitched_shared_former(),
    ));
    play_with_progress(&state, &args("5"), |_| {})
        .await
        .expect("plays episode 5");
    let resp = play_with_progress(&state, &asked, |_| {})
        .await
        .expect("replays");
    assert!(resp.cache_hit);
    assert_eq!(resp.former_episode, None);
}

#[tokio::test]
async fn a_click_replaying_its_prefetch_names_the_former_number_the_resolve_found() {
    // The page's prefetch resolves the recap, its former number "2.5"
    // with it, and leaves the cached row; the click replays the row, and
    // the listing the prefetch had at hand still leaves "2.5" free.
    let upstream = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("HEAD"))
        .respond_with(wiremock::ResponseTemplate::new(200))
        .mount(&upstream)
        .await;
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    std::fs::write(&state.config_path, "cache_resolutions = true\n").expect("config");
    let mut prefetch = args("1.5");
    prefetch.prefetch = true;
    play_with_progress(&state, &prefetch, |_| {})
        .await
        .expect("prefetches");
    let key = key_for(&prefetch, "1.5");
    let mut row = play_resolution_cache::get(&state.cache_pool, &key)
        .expect("readable")
        .expect("kept");
    // The stub's stream is not served over HTTP; point the row at one
    // that answers the replay's check.
    row.upstream_url = format!("{}/x/master.m3u8", upstream.uri());
    play_resolution_cache::put(&state.cache_pool, &key, &row);
    let resp = play_with_progress(&state, &args("1.5"), |_| {})
        .await
        .expect("replays");
    assert!(resp.cache_hit);
    assert_eq!(resp.former_episode.as_deref(), Some("2.5"));
}

#[tokio::test]
async fn a_prefetch_after_a_watched_recap_leaves_the_row_reading_as_the_recap() {
    // The recap is watched — the row records "0.5" itself — then the
    // player prefetches episode 1, a displaced row of its own. The
    // prefetch records no watch, and the row still reads as the recap.
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

fn cumulative_ask(episode: &str) -> PlayArgs {
    serde_json::from_value(serde_json::json!({
        "title": crate::commands::play_native_test_provider::CUMULATIVE,
        "episode": episode,
        "mode": "sub",
        "episode_count": 2,
        "year": 2026,
    }))
    .expect("args")
}

fn cumulative_listed(state: &AppState) -> String {
    let key = crate::scraper::provider::ShowKey::new(
        crate::provider_set::ProviderSet::TEST_DEFAULT,
        crate::commands::play_native_test_provider::CUMULATIVE_SLUG,
    )
    .to_string();
    crate::commands::history::history_list(state)
        .expect("list")
        .into_iter()
        .find(|e| e.id == key)
        .expect("row")
        .ep_no
}

#[tokio::test]
async fn a_fresh_play_overtaken_by_a_later_watch_leaves_that_watchs_stamp() {
    // The recap's resolve began before episode 1 was watched and lands
    // after it: its row is refused, and so must its stamp be.
    use crate::commands::play_native_test_provider::{cumulative_opening_recap, CUMULATIVE};
    let td = tempfile::tempdir().expect("td");
    let mut state = state(&td);
    let table = cumulative_opening_recap();
    state
        .providers
        .set_stub(std::sync::Arc::new(cumulative_opening_recap()));
    let begun_before = crate::history::guard::Asked::now(&state.history_path, None);
    play_with_progress(&state, &cumulative_ask("1"), |_| {})
        .await
        .expect("plays episode 1");
    // The player marks episode 1 watched: a watch the recap's resolve,
    // begun before it, gives way to.
    let cached = play_resolution_cache::get(&state.cache_pool, &key_for(&cumulative_ask("1"), "1"))
        .expect("readable")
        .expect("kept");
    let watch = crate::commands::play_cache::cached_watch(&cached, "1");
    crate::commands::play_native_record::record_watch(&state, &watch, None).await;
    let recap = crate::commands::play_native_resolve::resolve_native(
        &table,
        crate::commands::play_native_resolve::NativeResolveRequest {
            title: CUMULATIVE,
            alt_titles: &[],
            episode: "0.5",
            mode: "sub",
            quality: "best",
            expected_count: Some(2),
            year: Some(2026),
            subtype: None,
            version: None,
            stamps_beside: None,
            tail: None,
            heard: None,
        },
        &mut |_| {},
    )
    .await
    .expect("resolves the recap");
    crate::commands::play_native_record::record_fresh_watch(&state, &recap, "0.5", begun_before);
    assert_eq!(cumulative_listed(&state), "1");
}

#[tokio::test]
async fn a_handoff_that_fails_to_start_leaves_the_watched_stamp() {
    let td = tempfile::tempdir().expect("td");
    let mut state = state(&td);
    state.providers.set_stub(std::sync::Arc::new(
        crate::commands::play_native_test_provider::cumulative_opening_recap(),
    ));
    play_with_progress(&state, &cumulative_ask("0.5"), |_| {})
        .await
        .expect("plays the recap");
    std::fs::write(
        &state.config_path,
        "external_player = \"/nonexistent/player\"\n",
    )
    .expect("config");
    let failed =
        crate::commands::play_external_command::play_external(&state, &cumulative_ask("1")).await;
    assert!(failed.is_err(), "the player never started");
    assert_eq!(cumulative_listed(&state), "0.5");
}

#[cfg(unix)]
#[tokio::test]
async fn a_handoff_whose_player_started_puts_the_pair_its_row_reads_through() {
    use std::os::unix::fs::PermissionsExt;
    let td = tempfile::tempdir().expect("td");
    let mut state = state(&td);
    state.providers.set_stub(std::sync::Arc::new(
        crate::commands::play_native_test_provider::cumulative_opening_recap(),
    ));
    let player = td.path().join("player");
    std::fs::write(&player, "#!/bin/sh\nexit 0\n").expect("stage player");
    let mut perms = std::fs::metadata(&player).expect("meta").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&player, perms).expect("chmod");
    std::fs::write(
        &state.config_path,
        format!(
            "external_player = {}\n",
            crate::stub_file::toml_quote(&player)
        ),
    )
    .expect("config");
    crate::commands::play_external_command::play_external(&state, &cumulative_ask("0.5"))
        .await
        .expect("the player started");
    assert_eq!(cumulative_listed(&state), "0.5");
}

use crate::commands::play_native_test_provider::{
    cumulative_opening_recap_season, CUMULATIVE_S2, CUMULATIVE_S2_SLUG,
};

/// A Kitsu that holds every lookup it is asked ([`crate::held`]), so a
/// watch's mapping lookup stays out until the test lets it go.
async fn kitsu_that_holds() -> (wiremock::MockServer, crate::held::Gate) {
    let mock = wiremock::MockServer::start().await;
    let (hold, gate) = crate::held::gate();
    wiremock::Mock::given(wiremock::matchers::any())
        .respond_with(crate::held::Held::new(
            hold,
            wiremock::ResponseTemplate::new(200),
        ))
        .mount(&mock)
        .await;
    (mock, gate)
}

/// What the row of the show `slug` keys reads as once `recording` has
/// its Kitsu lookup out — `kitsu` asked — and while it still has: the
/// lookup is held, so the recording cannot have finished, and the read
/// is taken with nothing left of the watch to wait for but Kitsu. A
/// recording that ends without asking Kitsu fails the test, as does
/// one whose row is not there by then.
///
/// No clock decides it: the read may queue behind other histories'
/// writes for as long as the machine takes, and still reads what the
/// watch put down before it asked Kitsu.
async fn reads_while_kitsu_is_out<T: std::fmt::Debug>(
    state: &AppState,
    slug: &str,
    kitsu: &crate::held::Gate,
    recording: &mut tokio::task::JoinHandle<T>,
) -> String {
    tokio::select! {
        () = kitsu.asked() => {}
        ended = &mut *recording => panic!("the recording ended without asking Kitsu: {ended:?}"),
    }
    let key = crate::scraper::provider::ShowKey::new(
        crate::provider_set::ProviderSet::TEST_DEFAULT,
        slug,
    )
    .to_string();
    let read = crate::commands::history::history_list(state)
        .expect("list")
        .into_iter()
        .find(|e| e.id == key)
        .map_or_else(|| String::from("<no row>"), |row| row.ep_no);
    assert!(!recording.is_finished(), "the lookup is still out");
    recording.abort();
    kitsu.release();
    read
}

#[tokio::test]
async fn a_mark_watched_reads_as_the_recap_while_its_kitsu_lookup_is_out() {
    // The recap was resolved by a prefetch alone, so the show has no
    // display stamp. The player marks it watched with the page's Kitsu
    // id, and the mapping lookup that follows the row is held: a
    // Continue card loaded meanwhile reads the row as the recap, the
    // number it records.
    use tower::ServiceExt as _;
    let (mock, kitsu) = kitsu_that_holds().await;
    let td = tempfile::tempdir().expect("td");
    let mut state = state(&td);
    state.providers.set_stub(std::sync::Arc::new(
        crate::commands::play_native_test_provider::cumulative_opening_recap_season(),
    ));
    state.kitsu = crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), mock.uri());
    let mut prefetch = cumulative_ask("0.5");
    prefetch.title = CUMULATIVE_S2.into();
    prefetch.prefetch = true;
    play_with_progress(&state, &prefetch, |_| {})
        .await
        .expect("prefetches the recap");
    let router = crate::api::build_api_router(std::sync::Arc::new(state.clone()));
    let body = serde_json::json!({
        "title": CUMULATIVE_S2,
        "episode": "0.5",
        "mode": "sub",
        "episode_count": 2,
        "year": 2026,
        "kitsu_id": "77",
    });
    let mut marking = tokio::spawn(async move {
        router
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/play/mark-watched")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&body).expect("json"),
                    ))
                    .expect("req"),
            )
            .await
            .expect("oneshot")
            .status()
    });
    let read = reads_while_kitsu_is_out(&state, CUMULATIVE_S2_SLUG, &kitsu, &mut marking).await;
    assert_eq!(read, "0.5");
}

#[tokio::test]
async fn a_handoffs_watch_reads_as_the_recap_while_its_kitsu_lookup_is_out() {
    // The watch a handoff records from its fresh resolve, with the
    // page's Kitsu id: its row, the recap's own number, is written
    // before the lookup, not after it.
    let (mock, kitsu) = kitsu_that_holds().await;
    let td = tempfile::tempdir().expect("td");
    let mut state = state(&td);
    state.kitsu = crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), mock.uri());
    let table = cumulative_opening_recap_season();
    let recap = crate::commands::play_native_resolve::resolve_native(
        &table,
        crate::commands::play_native_resolve::NativeResolveRequest {
            title: CUMULATIVE_S2,
            alt_titles: &[],
            episode: "0.5",
            mode: "sub",
            quality: "best",
            expected_count: Some(2),
            year: Some(2026),
            subtype: None,
            version: None,
            stamps_beside: None,
            tail: None,
            heard: None,
        },
        &mut |_| {},
    )
    .await
    .expect("resolves the recap");
    let watch = crate::commands::play_native_record::Watch::of(&recap, "0.5");
    let mut recording = {
        let state = state.clone();
        tokio::spawn(async move {
            crate::commands::play_native_record::record_watch(&state, &watch, Some("77")).await;
        })
    };
    let read = reads_while_kitsu_is_out(&state, CUMULATIVE_S2_SLUG, &kitsu, &mut recording).await;
    assert_eq!(read, "0.5");
}

#[tokio::test]
async fn a_fresh_play_of_a_stamped_recap_keeps_the_tag_the_stamp_replaced() {
    // The row from before holds slot 2, stamped (2, "2.5") — the
    // stitched recap's former tag. Its fresh play puts the stamp right,
    // (2, "1.5"), and keeps "2.5" beside it: a position kept under the
    // former number is found by it.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let row = crate::history::parse(&format!("2\t{}\tStitched Show\n", stitched_key()))
        .pop()
        .expect("a row");
    crate::history::upsert_and_write(&state.history_path, row).expect("write");
    crate::commands::numbering_offset::put_display(&state, &stitched_key(), 0, 2, "2.5");
    play_with_progress(&state, &args("2.5"), |_| {})
        .await
        .expect("plays the recap");
    assert_eq!(listed_as(&state), "1.5");
    assert_eq!(
        crate::commands::numbering_offset::former_stamp(&state, &stitched_key()),
        Some((2, "2.5".to_string()))
    );
}

/// A play whose numbering repair stands behind the rows it caches.
async fn rows_a_play_caches_behind_its_repair(prefetch: bool, stamp_after: (u32, &str)) {
    // An upgraded profile: a history row's stamp names the stitched
    // recap's slot 2 by its former tag "2.5". A play of episode 5 pays
    // for the listing, which puts the stamp right — before the rows
    // it caches land, or a probe taking the fresh availability row
    // never fetches the listing that would repair it.
    use crate::commands::play_native_test_provider::{
        assert_rows_landed_after_the_repair, stamps_as_rows_land,
    };
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    crate::commands::numbering_offset::put_display(&state, &stitched_key(), 0, 2, "2.5");
    let mut asked = args("5");
    asked.prefetch = prefetch;
    let (_observing, seen) =
        stamps_as_rows_land(&state, &stitched_key(), &["availability:", "play:"]);
    play_with_progress(&state, &asked, |_| {})
        .await
        .expect("plays");
    let after = crate::commands::numbering_offset::display_stamp(&state, &stitched_key());
    // A watch records the app's own number and moves no display
    // stamp, so a play leaves the repaired one as a prefetch does.
    assert_eq!(after, Some((stamp_after.0, stamp_after.1.to_string())));
    assert_rows_landed_after_the_repair(&seen, &["availability:", "play:"], &after);
}

#[tokio::test]
async fn a_play_puts_the_numbering_right_before_it_caches_its_rows() {
    rows_a_play_caches_behind_its_repair(false, (2, "1.5")).await;
}

#[tokio::test]
async fn a_prefetch_puts_the_numbering_right_before_it_caches_its_rows() {
    rows_a_play_caches_behind_its_repair(true, (2, "1.5")).await;
}

#[tokio::test]
async fn a_handoff_puts_the_numbering_right_before_it_caches_availability() {
    use crate::commands::play_native_test_provider::{
        assert_rows_landed_after_the_repair, stamps_as_rows_land,
    };
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    crate::commands::numbering_offset::put_display(&state, &stitched_key(), 0, 2, "2.5");
    let (_observing, seen) = stamps_as_rows_land(&state, &stitched_key(), &["availability:"]);
    let asked = crate::history::guard::Asked::now(&state.history_path, Some("77"));
    crate::commands::play_handoff::resolve_launch_args(&state, &args("5"), asked)
        .await
        .expect("resolves");
    let after = crate::commands::numbering_offset::display_stamp(&state, &stitched_key());
    assert_eq!(
        after,
        Some((2, "1.5".to_string())),
        "the handoff repaired the stamp"
    );
    assert_rows_landed_after_the_repair(&seen, &["availability:"], &after);
}

// ── the external player's window names the episode played ──────────

#[tokio::test]
async fn a_handoff_by_a_former_tag_names_the_episode_it_plays_in_the_window() {
    // History records the recap as "1.5", the number it has now; the
    // player's window says the same, not the "2.5" it was asked by.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let asked = crate::history::guard::Asked::now(&state.history_path, None);
    let (launch, watch) =
        crate::commands::play_handoff::resolve_launch_args(&state, &args("2.5"), asked)
            .await
            .expect("resolves");
    assert_eq!(watch.ep_no, "1.5");
    assert_eq!(launch.title.as_deref(), Some("Stitched Show · ep 1.5"));
}

#[tokio::test]
async fn a_handoff_of_an_ordinary_episode_names_it_as_asked() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let asked = crate::history::guard::Asked::now(&state.history_path, None);
    let (launch, watch) =
        crate::commands::play_handoff::resolve_launch_args(&state, &args("5"), asked)
            .await
            .expect("resolves");
    assert_eq!(watch.ep_no, "5");
    assert_eq!(launch.title.as_deref(), Some("Stitched Show · ep 5"));
}

#[tokio::test]
async fn a_cached_handoff_names_the_episode_its_resolve_played_in_the_window() {
    // The row's resolve played "12.5"; it is replayed for a request by
    // another number. The window and the watch both name "12.5".
    let upstream = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("HEAD"))
        .respond_with(wiremock::ResponseTemplate::new(200))
        .mount(&upstream)
        .await;
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let asked = args("24.5");
    play_resolution_cache::put(
        &state.cache_pool,
        &key_for(&asked, "24.5"),
        &first_recap_row(&upstream.uri()),
    );
    let cfg = crate::config::Config {
        cache_resolutions: true,
        ..crate::config::Config::default()
    };
    let (launch, watch) = try_launch_args_from_cache(&state, &asked, &cfg)
        .await
        .expect("replays");
    assert_eq!(watch.ep_no, "12.5");
    assert_eq!(launch.title.as_deref(), Some("Stitched Show · ep 12.5"));
}

#[tokio::test]
async fn a_replay_after_a_failed_play_whose_listing_carries_its_former_number_names_none() {
    // The listing last recorded leaves "2.5" free. A play of an episode
    // the provider does not list fails — but the listing it picked tags
    // another recap "2.5" now, and the replay that follows names no
    // former number.
    let upstream = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("HEAD"))
        .respond_with(wiremock::ResponseTemplate::new(200))
        .mount(&upstream)
        .await;
    let td = tempfile::tempdir().expect("td");
    let mut state = state(&td);
    std::fs::write(&state.config_path, "cache_resolutions = true\n").expect("config");
    hear_listing(&state, &stitched_key(), &["1.5"]);
    let asked = args("1.5");
    let mut row = first_recap_row(&upstream.uri());
    row.resolved_slot = Some(2);
    row.played = Some("1.5".into());
    row.former_episode = Some("2.5".into());
    play_resolution_cache::put(&state.cache_pool, &key_for(&asked, "1.5"), &row);
    state.providers.set_stub(std::sync::Arc::new(
        crate::commands::play_native_test_provider::stitched_shared_former(),
    ));
    assert!(
        play_with_progress(&state, &args("40"), |_| {})
            .await
            .is_err(),
        "episode 40 is not listed"
    );
    let resp = play_with_progress(&state, &asked, |_| {})
        .await
        .expect("replays");
    assert!(resp.cache_hit);
    assert_eq!(resp.former_episode, None);
}

/// The listing tags recorded for the stitched show.
fn recorded(state: &AppState) -> Option<Vec<String>> {
    crate::commands::listing_tags::get(state, &stitched_key())
}

#[tokio::test]
async fn a_handoff_that_fails_records_the_listing_it_picked() {
    // Episode 40 is not listed, so the hand-off fails after the pick;
    // the listing it had in hand, which tags a recap "2.5", is on record.
    let td = tempfile::tempdir().expect("td");
    let mut state = state(&td);
    state.providers.set_stub(std::sync::Arc::new(
        crate::commands::play_native_test_provider::stitched_shared_former(),
    ));
    let asked = crate::history::guard::Asked::now(&state.history_path, None);
    assert!(
        crate::commands::play_handoff::resolve_launch_args(&state, &args("40"), asked)
            .await
            .is_err()
    );
    let tags = recorded(&state).expect("recorded");
    assert!(tags.iter().any(|t| t == "2.5"), "{tags:?}");
}

#[cfg(unix)]
#[tokio::test]
async fn a_download_that_fails_records_the_listing_it_picked() {
    use std::os::unix::fs::PermissionsExt;
    let td = tempfile::tempdir().expect("td");
    let mut state = state(&td);
    state.providers.set_stub(std::sync::Arc::new(
        crate::commands::play_native_test_provider::stitched_shared_former(),
    ));
    // A download tool to find, so the download walks the provider.
    let bin = tempfile::tempdir().expect("bin");
    let tool = bin.path().join("yt-dlp");
    std::fs::write(&tool, "#!/bin/sh\nexit 0\n").expect("tool");
    let mut perms = std::fs::metadata(&tool).expect("meta").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&tool, perms).expect("chmod");
    let dest = tempfile::tempdir().expect("dest");
    let download: crate::commands::download::DownloadArgs =
        serde_json::from_value(serde_json::json!({
            "title": STITCHED,
            "episode": "40",
            "mode": "sub",
            "episode_count": 12,
            "year": 2026,
            "subtype": "ONA",
            "download_dir": dest.path(),
        }))
        .expect("args");
    assert!(crate::commands::download::download_with_tools(
        &state,
        &download,
        &bin.path().display().to_string(),
        |_| {},
    )
    .await
    .is_err());
    let tags = recorded(&state).expect("recorded");
    assert!(tags.iter().any(|t| t == "2.5"), "{tags:?}");
}

#[tokio::test]
async fn a_play_of_a_pick_with_two_versions_records_both_listings() {
    use crate::scraper::provider::BrowseHit;
    use crate::scraper::table_provider::{listing_of, TableProvider};
    let card = |slug: &str, title: &str| BrowseHit {
        slug: slug.into(),
        title: title.into(),
        kind: Some("TV".into()),
        jname: None,
    };
    let provider = TableProvider::new()
        .search_default(vec![
            card("the-show-1", "The Show"),
            card("the-show-2", "The Show (Uncensored)"),
        ])
        .listing(1, listing_of(1, 3))
        .listing(2, listing_of(2, 3))
        .year(1, Some(2020))
        .year(2, Some(2020))
        .stream(
            2001,
            "sub",
            crate::commands::play_native_test_provider::MASTER,
        )
        .mode(2001, "sub", true);
    let td = tempfile::tempdir().expect("td");
    let mut state = state(&td);
    state.providers.set_stub(std::sync::Arc::new(provider));
    let asked: PlayArgs = serde_json::from_value(serde_json::json!({
        "title": "The Show", "episode": "1", "mode": "sub",
        "episode_count": 3, "year": 2020, "subtype": "TV",
    }))
    .expect("args");
    play_with_progress(&state, &asked, |_| {})
        .await
        .expect("plays");
    for slug in ["the-show-1", "the-show-2"] {
        let key = crate::scraper::provider::ShowKey::new(
            crate::provider_set::ProviderSet::TEST_DEFAULT,
            slug,
        )
        .to_string();
        assert!(
            crate::commands::listing_tags::get(&state, &key).is_some(),
            "{slug}'s listing is recorded"
        );
    }
}
