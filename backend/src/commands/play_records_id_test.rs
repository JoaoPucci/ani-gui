//! A play asked from a Kitsu page records the page's id on the history
//! row it writes when it resolves, wherever the cache alone can answer
//! the cour guard — so a row whose mark-watched never arrives (the
//! renderer closed or reloaded first, or the request failed) still
//! names the show it was played as.
//!
//! The resolve is not the watch: it records no watch moment, and a
//! watch waiting to be recorded still records after it. Where only a
//! Kitsu read could judge the pairing, or the cache refuses it, the
//! id waits for the watch as before. A guessed Continue session's
//! resolve names its guess apart from the page (`guess_id`, beside
//! `guess_row`), and records it only when the play landed on the
//! card's own listing — under the same cache-only check.

use super::*;
use crate::commands::play_native_test_provider::{
    cumulative_opening_recap_season, stitched_recap, CUMULATIVE_S2, CUMULATIVE_S2_SLUG, STITCHED,
    STITCHED_SLUG,
};
use crate::meta::kitsu::KitsuAnimeRef;

fn state(td: &tempfile::TempDir) -> AppState {
    let mut state = super::tests::state_with_proxy_origin();
    state
        .providers
        .set_stub(std::sync::Arc::new(stitched_recap()));
    state.config_path = td.path().join("config.toml");
    state.history_path = td.path().join("history");
    state
}

fn key(slug: &str) -> String {
    crate::scraper::provider::ShowKey::new(crate::provider_set::ProviderSet::TEST_DEFAULT, slug)
        .to_string()
}

/// A play of the stitched show's episode 5, from the Kitsu page
/// `kitsu_id`, opened from the guessed card of `guess_row` when one.
fn stitched(kitsu_id: Option<&str>, guess_row: Option<&str>) -> PlayArgs {
    let mut v = serde_json::json!({
        "title": STITCHED,
        "episode": "5",
        "mode": "sub",
        "episode_count": 12,
        "year": 2026,
        "subtype": "ONA",
    });
    if let Some(id) = kitsu_id {
        v["kitsu_id"] = serde_json::json!(id);
    }
    if let Some(row) = guess_row {
        v["guess_row"] = serde_json::json!(row);
    }
    serde_json::from_value(v).expect("args")
}

/// A play of "Cumulative Show 2nd Season" — a title with cour
/// evidence — from the Kitsu page 77.
fn season_two() -> PlayArgs {
    serde_json::from_value(serde_json::json!({
        "title": CUMULATIVE_S2,
        "episode": "1",
        "mode": "sub",
        "episode_count": 2,
        "year": 2026,
        "kitsu_id": "77",
    }))
    .expect("args")
}

fn row(state: &AppState, show_id: &str) -> crate::history::HistoryEntry {
    crate::history::read_all(&state.history_path)
        .expect("rows")
        .into_iter()
        .find(|r| r.id == show_id)
        .expect("the play wrote its row")
}

/// Seed Kitsu's detail of `id` in the cache, under `slug`.
fn cache_detail(state: &AppState, id: &str, slug: &str) {
    let detail = KitsuAnimeRef {
        id: id.into(),
        canonical_title: "Cumulative Show".into(),
        titles: std::collections::HashMap::new(),
        abbreviated_titles: Vec::new(),
        slug: Some(slug.into()),
        synopsis: None,
        start_date: None,
        end_date: None,
        episode_count: Some(2),
        average_rating: None,
        subtype: Some("TV".into()),
        status: Some("finished".into()),
        age_rating: None,
        popularity_rank: None,
        poster_image: None,
        anilist_banner: None,
        cover_image: None,
    };
    crate::cache::meta_cache_put(
        &state.cache_pool,
        &crate::commands::kitsu::anime_detail_key(id),
        &serde_json::to_string(&detail).expect("ser"),
        3600,
    )
    .expect("seed detail");
}

/// What the season-two play leaves on its row's id, with the cache
/// holding Kitsu's detail of 77 under `slug` when one.
async fn season_two_records(slug: Option<&str>) -> Option<String> {
    let td = tempfile::tempdir().expect("td");
    let mut state = state(&td);
    state
        .providers
        .set_stub(std::sync::Arc::new(cumulative_opening_recap_season()));
    if let Some(slug) = slug {
        cache_detail(&state, "77", slug);
    }
    play_with_progress(&state, &season_two(), |_| {})
        .await
        .expect("plays");
    row(&state, &key(CUMULATIVE_S2_SLUG)).kitsu_id
}

#[tokio::test]
async fn a_play_from_a_page_records_its_id_before_any_mark_watched() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    play_with_progress(&state, &stitched(Some("45412"), None), |_| {})
        .await
        .expect("plays");
    let row = row(&state, &key(STITCHED_SLUG));
    assert_eq!(row.kitsu_id.as_deref(), Some("45412"));
    assert_eq!(row.watched_at, None, "a resolve is not a watch");
}

#[tokio::test]
async fn a_replay_from_a_page_records_its_id_before_any_mark_watched() {
    let upstream = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("HEAD"))
        .respond_with(wiremock::ResponseTemplate::new(200))
        .mount(&upstream)
        .await;
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    std::fs::write(&state.config_path, "cache_resolutions = true\n").expect("config");
    let asked = stitched(Some("45412"), None);
    play_resolution_cache::put(
        &state.cache_pool,
        &asked.resolution_key(),
        &CachedResolution {
            upstream_url: format!("{}/x/master.m3u8", upstream.uri()),
            referer: String::new(),
            media_kind: MediaKind::Hls,
            show_id: key(STITCHED_SLUG),
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
    let row = row(&state, &key(STITCHED_SLUG));
    assert_eq!(row.kitsu_id.as_deref(), Some("45412"));
    assert_eq!(row.watched_at, None, "a replay is not a watch");
}

#[tokio::test]
async fn a_play_with_no_page_records_no_id() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    play_with_progress(&state, &stitched(None, None), |_| {})
        .await
        .expect("plays");
    assert_eq!(row(&state, &key(STITCHED_SLUG)).kitsu_id, None);
}

#[tokio::test]
async fn a_cour_title_whose_page_the_cache_accepts_records_its_id() {
    assert_eq!(
        season_two_records(Some("cumulative-show-season-2"))
            .await
            .as_deref(),
        Some("77")
    );
}

#[tokio::test]
async fn a_cour_title_the_cache_cannot_judge_leaves_the_id_to_the_watch() {
    // Only a Kitsu read could say whether page 77 is this cour; the
    // resolve makes none, and the row waits for mark-watched's guard.
    assert_eq!(season_two_records(None).await, None);
}

#[tokio::test]
async fn a_cour_title_whose_page_the_cache_refuses_records_no_id() {
    assert_eq!(season_two_records(Some("cumulative-show")).await, None);
}

#[tokio::test]
async fn a_guessed_play_on_its_cards_listing_records_the_guess() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let own = key(STITCHED_SLUG);
    play_with_progress(&state, &stitched(Some("45412"), Some(&own)), |_| {})
        .await
        .expect("plays");
    assert_eq!(row(&state, &own).kitsu_id.as_deref(), Some("45412"));
}

#[tokio::test]
async fn a_guessed_play_that_lands_elsewhere_records_no_id() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let other = key("another-show-4");
    play_with_progress(&state, &stitched(Some("45412"), Some(&other)), |_| {})
        .await
        .expect("plays");
    assert_eq!(row(&state, &key(STITCHED_SLUG)).kitsu_id, None);
}

#[tokio::test]
async fn a_play_that_records_its_id_does_not_drop_a_watch_waiting_on_it() {
    // A watch of the show asked for before the play resolved — the
    // previous episode's mark-watched, say — is still recorded: the
    // id the resolve put down is not another watch.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let requested = crate::history::guard::epoch(&state.history_path);
    play_with_progress(&state, &stitched(Some("45412"), None), |_| {})
        .await
        .expect("plays");
    let watch = crate::commands::play_native_record::Watch {
        show_id: key(STITCHED_SLUG),
        title: STITCHED.into(),
        ep_no: "4".into(),
    };
    crate::commands::play_native_record::record_watch_requested_at(
        &state,
        &watch,
        Some("45412"),
        requested,
    )
    .await;
    let row = row(&state, &key(STITCHED_SLUG));
    assert_eq!(row.ep_no, "4");
    assert!(row.watched_at.is_some(), "the watch is recorded");
    assert_eq!(row.kitsu_id.as_deref(), Some("45412"));
}

#[tokio::test]
async fn a_row_keeps_its_id_when_the_resolve_cannot_judge_the_page() {
    // The row's earlier watch recorded 76; this play's page 77 waits
    // for its own watch, and the resolve leaves 76 where it is.
    let td = tempfile::tempdir().expect("td");
    let mut state = state(&td);
    state
        .providers
        .set_stub(std::sync::Arc::new(cumulative_opening_recap_season()));
    crate::history::upsert_and_write(
        &state.history_path,
        crate::history::HistoryEntry {
            ep_no: "1".into(),
            id: key(CUMULATIVE_S2_SLUG),
            title: CUMULATIVE_S2.into(),
            watched_at: Some(1),
            kitsu_id: Some("76".into()),
            own_number: true,
        },
    )
    .expect("seed row");
    play_with_progress(&state, &season_two(), |_| {})
        .await
        .expect("plays");
    assert_eq!(
        row(&state, &key(CUMULATIVE_S2_SLUG)).kitsu_id.as_deref(),
        Some("76")
    );
}

#[tokio::test]
async fn a_replay_whose_page_the_cache_refuses_records_no_id() {
    let upstream = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("HEAD"))
        .respond_with(wiremock::ResponseTemplate::new(200))
        .mount(&upstream)
        .await;
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    std::fs::write(&state.config_path, "cache_resolutions = true\n").expect("config");
    cache_detail(&state, "77", "cumulative-show");
    let asked = season_two();
    play_resolution_cache::put(
        &state.cache_pool,
        &asked.resolution_key(),
        &CachedResolution {
            upstream_url: format!("{}/x/master.m3u8", upstream.uri()),
            referer: String::new(),
            media_kind: MediaKind::Hls,
            show_id: key(CUMULATIVE_S2_SLUG),
            show_title: CUMULATIVE_S2.into(),
            resolved_slot: Some(2),
            subtitles: Vec::new(),
            kitsu_id: None,
            played: Some("1".into()),
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
    assert_eq!(row(&state, &key(CUMULATIVE_S2_SLUG)).kitsu_id, None);
}

/// A play of the stitched show's episode 5 that a guessed Continue card
/// asked for: the card's row `guess_row` and its guess `guess_id`, sent
/// as the renderer sends a guessed resolve — with no page id.
fn guessed(guess_id: Option<&str>, guess_row: Option<&str>) -> PlayArgs {
    let mut v = serde_json::json!({
        "title": STITCHED,
        "episode": "5",
        "mode": "sub",
        "episode_count": 12,
        "year": 2026,
        "subtype": "ONA",
    });
    if let Some(id) = guess_id {
        v["guess_id"] = serde_json::json!(id);
    }
    if let Some(row) = guess_row {
        v["guess_row"] = serde_json::json!(row);
    }
    serde_json::from_value(v).expect("args")
}

#[tokio::test]
async fn a_guessed_resolve_on_its_cards_listing_records_the_guess() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let own = key(STITCHED_SLUG);
    let resp = play_with_progress(&state, &guessed(Some("45412"), Some(&own)), |_| {})
        .await
        .expect("plays");
    assert!(resp.own_listing);
    let row = row(&state, &own);
    assert_eq!(row.kitsu_id.as_deref(), Some("45412"));
    assert_eq!(row.watched_at, None, "a resolve is not a watch");
}

#[tokio::test]
async fn a_guessed_replay_on_its_cards_listing_records_the_guess() {
    let upstream = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("HEAD"))
        .respond_with(wiremock::ResponseTemplate::new(200))
        .mount(&upstream)
        .await;
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    std::fs::write(&state.config_path, "cache_resolutions = true\n").expect("config");
    let own = key(STITCHED_SLUG);
    let asked = guessed(Some("45412"), Some(&own));
    play_resolution_cache::put(
        &state.cache_pool,
        &asked.resolution_key(),
        &CachedResolution {
            upstream_url: format!("{}/x/master.m3u8", upstream.uri()),
            referer: String::new(),
            media_kind: MediaKind::Hls,
            show_id: own.clone(),
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
    assert_eq!(row(&state, &own).kitsu_id.as_deref(), Some("45412"));
}

#[tokio::test]
async fn a_guessed_resolve_that_lands_elsewhere_records_no_guess() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let other = key("another-show-4");
    play_with_progress(&state, &guessed(Some("45412"), Some(&other)), |_| {})
        .await
        .expect("plays");
    assert_eq!(row(&state, &key(STITCHED_SLUG)).kitsu_id, None);
}

#[tokio::test]
async fn a_guess_no_card_row_names_is_not_recorded() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    play_with_progress(&state, &guessed(Some("45412"), None), |_| {})
        .await
        .expect("plays");
    assert_eq!(row(&state, &key(STITCHED_SLUG)).kitsu_id, None);
}

#[tokio::test]
async fn a_guess_the_cache_refuses_is_not_recorded_on_its_own_listing() {
    let td = tempfile::tempdir().expect("td");
    let mut state = state(&td);
    state
        .providers
        .set_stub(std::sync::Arc::new(cumulative_opening_recap_season()));
    cache_detail(&state, "77", "cumulative-show");
    let own = key(CUMULATIVE_S2_SLUG);
    let args: PlayArgs = serde_json::from_value(serde_json::json!({
        "title": CUMULATIVE_S2,
        "episode": "1",
        "mode": "sub",
        "episode_count": 2,
        "year": 2026,
        "guess_row": own,
        "guess_id": "77",
    }))
    .expect("args");
    let resp = play_with_progress(&state, &args, |_| {})
        .await
        .expect("plays");
    assert!(resp.own_listing);
    assert_eq!(row(&state, &own).kitsu_id, None);
}
