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
