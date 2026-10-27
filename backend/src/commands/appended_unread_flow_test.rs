//! Every caller of [`super`] when no reading reaches the rows past
//! Noragami's count: Kitsu unreachable, nothing of its evidence
//! cached, and no reading kept. The listing's rows 13 and 14 may be
//! its OADs or two more episodes, and nothing says which; so no caller
//! stamps the listing's fourteen as the show's count, and none lets
//! what it could not read write over a count that stands.

use super::*;

/// The availability row as it stands, cap, extras and all.
fn row(
    state: &crate::app::AppState,
) -> Option<crate::commands::availability::AvailabilityResponse> {
    crate::cache::meta_cache_get(&state.cache_pool, &cache_key(KITSU, "sub"))
        .expect("read")
        .map(|body| serde_json::from_str(&body).expect("row"))
}

/// The row a probe proved, with the evidence it was proved on and the
/// reading it kept both gone: what a play a week on, Kitsu unreachable,
/// finds.
async fn a_count_proved_then_its_evidence_gone(state: &crate::app::AppState) {
    seed_kitsu(state, "finished");
    check_availability(state, &availability_args())
        .await
        .expect("answered");
    assert_eq!(stamped(state), (Some(12), oads()));
    crate::cache::meta_cache_delete(&state.cache_pool, &format!("airing:v2:{KITSU}"))
        .expect("airing row gone");
    crate::cache::meta_cache_delete(&state.cache_pool, &format!("appended:v2:{KITSU}"))
        .expect("kept reading gone");
}

#[tokio::test]
async fn a_probe_that_cannot_read_the_rows_past_the_count_gives_no_count() {
    let td = tempfile::tempdir().expect("td");
    let provider = hianime();
    let state = state_over(&td, &provider);
    let got = check_availability(&state, &availability_args())
        .await
        .expect("answered");
    assert!(got.available, "the show plays, whatever its count");
    assert_eq!(got.episode_count, None, "{got:?}");
    assert!(got.extra_episodes.is_empty());
    let stood = row(&state).expect("a row: the show is there");
    assert!(stood.available);
    assert_eq!(stood.episode_count, None, "{stood:?}");
}

#[tokio::test]
async fn a_play_that_cannot_read_the_rows_past_the_count_stamps_no_count() {
    let td = tempfile::tempdir().expect("td");
    let provider = hianime();
    let state = state_over(&td, &provider);
    crate::commands::play::play_with_progress(&state, &play_args("5"), |_| {})
        .await
        .expect("episode 5 plays");
    let stood = row(&state).expect("a row");
    assert!(stood.available);
    assert_eq!(stood.episode_count, None, "{stood:?}");
}

#[tokio::test]
async fn a_play_that_cannot_read_the_rows_past_the_count_keeps_the_count_that_stands() {
    let td = tempfile::tempdir().expect("td");
    let provider = hianime();
    let state = state_over(&td, &provider);
    a_count_proved_then_its_evidence_gone(&state).await;
    crate::commands::play::play_with_progress(&state, &play_args("5"), |_| {})
        .await
        .expect("episode 5 plays");
    assert_eq!(stamped(&state), (Some(12), oads()));
}

#[tokio::test]
async fn a_handoff_that_cannot_read_the_rows_past_the_count_keeps_the_count_that_stands() {
    let td = tempfile::tempdir().expect("td");
    let provider = hianime();
    let state = state_over(&td, &provider);
    a_count_proved_then_its_evidence_gone(&state).await;
    let args = play_args("5");
    let asked = crate::history::guard::Asked::now(&state.history_path, args.kitsu_id.as_deref());
    crate::commands::play_handoff::resolve_launch_args(&state, &args, asked)
        .await
        .expect("episode 5 resolves");
    assert_eq!(stamped(&state), (Some(12), oads()));
}

#[cfg(unix)]
#[tokio::test]
async fn a_download_that_cannot_read_the_rows_past_the_count_keeps_the_count_that_stands() {
    let td = tempfile::tempdir().expect("td");
    let provider = hianime();
    let state = state_over(&td, &provider);
    a_count_proved_then_its_evidence_gone(&state).await;
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    recording_tool(bin.path(), &dest.path().join("calls.log"));
    crate::commands::download::download_with_tools(
        &state,
        &download_args("5", dest.path()),
        &bin.path().display().to_string(),
        |_| {},
    )
    .await
    .expect("downloaded");
    assert_eq!(stamped(&state), (Some(12), oads()));
}

#[cfg(unix)]
#[tokio::test]
async fn a_range_download_that_cannot_read_the_rows_past_the_count_keeps_the_count_that_stands() {
    let td = tempfile::tempdir().expect("td");
    let provider = hianime();
    let state = state_over(&td, &provider);
    a_count_proved_then_its_evidence_gone(&state).await;
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    recording_tool(bin.path(), &dest.path().join("calls.log"));
    crate::commands::download::download_with_tools(
        &state,
        &download_args("4-5", dest.path()),
        &bin.path().display().to_string(),
        |_| {},
    )
    .await
    .expect("downloaded");
    assert_eq!(stamped(&state), (Some(12), oads()));
}

/// Row 13 of a listing no reading reached is Noragami's first OAD or
/// its thirteenth episode, unknown which. The session that plays it
/// says so, as does the replay row it leaves, so the renderer moves
/// no tracker on it; an episode within the count, a row a proved
/// reading tagged, and a row a reading refused to tag are no such
/// rows.
#[tokio::test]
async fn a_play_of_a_row_past_an_unread_count_says_it_is_unverified() {
    let td = tempfile::tempdir().expect("td");
    let provider = hianime();
    let state = state_over(&td, &provider);
    let row_of = |state: &crate::app::AppState, ep: &str| {
        crate::commands::play_resolution_cache::get(
            &state.cache_pool,
            &play_args(ep).resolution_key(),
        )
        .expect("read")
        .expect("a replay row")
    };
    let past = crate::commands::play::play_with_progress(&state, &play_args("13"), |_| {})
        .await
        .expect("row 13 plays");
    assert!(past.episode_unverified, "{past:?}");
    assert!(row_of(&state, "13").episode_unverified);
    let within = crate::commands::play::play_with_progress(&state, &play_args("5"), |_| {})
        .await
        .expect("episode 5 plays");
    assert!(!within.episode_unverified);
    assert!(!row_of(&state, "5").episode_unverified);

    let td = tempfile::tempdir().expect("td");
    let state = state_over(&td, &provider);
    seed_kitsu(&state, "finished");
    let proved = crate::commands::play::play_with_progress(&state, &play_args("12.1"), |_| {})
        .await
        .expect("the OAD plays");
    assert!(!proved.episode_unverified);

    let td = tempfile::tempdir().expect("td");
    let state = state_over(&td, &provider);
    seed_kitsu(&state, "current");
    let refused = crate::commands::play::play_with_progress(&state, &play_args("13"), |_| {})
        .await
        .expect("row 13 plays");
    assert!(!refused.episode_unverified, "a refusal is an answer");
}

/// A replay serves the session the row it stands on describes,
/// unverified episode and all.
#[tokio::test]
async fn a_replay_of_an_unverified_row_says_it_is_unverified() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("HEAD"))
        .respond_with(wiremock::ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let state = crate::commands::play::tests::state_with_proxy_origin();
    let cached = crate::commands::play_resolution_cache::CachedResolution {
        episode_unverified: true,
        ..crate::commands::play::tests::cached_walked(
            format!("{}/video.mp4", server.uri()),
            String::new(),
            crate::proxy::token::MediaKind::Mp4,
        )
    };
    let resp = crate::commands::play_cache::try_serve_cached(&state, &cached)
        .await
        .expect("hit");
    assert!(resp.episode_unverified);
}
