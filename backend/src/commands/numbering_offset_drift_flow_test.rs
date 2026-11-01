//! A history row from before rows recorded the app's own number, past a
//! recap, whose display stamp a warm moved to another row: the next
//! listing a probe or a resolve fetches for its show puts it right, and
//! Continue Watching names the episode the user watched.

use crate::app::AppState;
use crate::commands::play_native_test_provider::{stitched_recap, STITCHED, STITCHED_SLUG};
use crate::scraper::provider::ShowKey;

fn key() -> String {
    ShowKey::new(
        crate::provider_set::ProviderSet::TEST_DEFAULT,
        STITCHED_SLUG,
    )
    .to_string()
}

fn state(td: &tempfile::TempDir) -> AppState {
    let mut state = crate::commands::play::tests::state_with_proxy_origin();
    state
        .providers
        .set_stub(std::sync::Arc::new(stitched_recap()));
    state.config_path = td.path().join("config.toml");
    state.history_path = td.path().join("history");
    state
}

/// The stitched show's episode 5 watched before the marker — slot 6,
/// past the recap in slot 2 — and then a warm of episode 2 (slot 3),
/// which moved the show's one stamp off the row.
fn seed_drifted_row(state: &AppState) {
    let row = crate::history::parse(&format!("6\t{}\t{STITCHED}\n", key()))
        .pop()
        .expect("a row");
    crate::history::upsert_and_write(&state.history_path, row).expect("write");
    crate::commands::numbering_offset::put_display(state, &key(), 0, 3, "2");
}

fn read_back(state: &AppState) -> String {
    crate::commands::history::history_list(state)
        .expect("read")
        .into_iter()
        .find(|e| e.id == key())
        .expect("the row")
        .ep_no
}

fn probe_args() -> crate::commands::availability::AvailabilityArgs {
    crate::commands::availability::AvailabilityArgs {
        title: STITCHED.into(),
        mode: "sub".into(),
        alt_titles: Vec::new(),
        episode_count: Some(12),
        year: Some(2026),
        subtype: Some("ONA".into()),
        kitsu_id: Some("77".into()),
        status: None,
        background: true,
        bypass_cache: false,
    }
}

fn warm(episode: &str) -> crate::commands::play::PlayArgs {
    serde_json::from_value(serde_json::json!({
        "title": STITCHED,
        "episode": episode,
        "mode": "sub",
        "episode_count": 12,
        "year": 2026,
        "subtype": "ONA",
        "kitsu_id": "77",
        "prefetch": true,
    }))
    .expect("args")
}

#[test]
fn a_drifted_row_reads_one_episode_high_until_a_listing_puts_it_right() {
    // The defect itself, before anything fetches a listing.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_drifted_row(&state);
    assert_eq!(read_back(&state), "6");
}

#[tokio::test]
async fn a_probe_of_the_show_puts_a_drifted_row_right() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_drifted_row(&state);
    let got = crate::commands::availability::check_availability(&state, &probe_args())
        .await
        .expect("probed");
    assert!(got.available);
    assert_eq!(read_back(&state), "5");
    assert_eq!(
        crate::commands::numbering_offset::display_stamp(&state, &key()),
        Some((6, "5".to_string()))
    );
    // The Continue card re-reads the row as it is clicked.
    assert_eq!(
        crate::commands::history_number::history_number(&state, &key())
            .expect("read")
            .map(|n| n.ep_no),
        Some("5".to_string())
    );
}

#[tokio::test]
async fn a_warm_of_another_episode_puts_a_drifted_row_right() {
    // The very kind of resolve that moved the stamp now puts it back:
    // a warm records no row, and the row it reads is the watch's.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_drifted_row(&state);
    crate::commands::play::play_with_progress(&state, &warm("8"), |_| {})
        .await
        .expect("warms");
    assert_eq!(read_back(&state), "5");
}

#[tokio::test]
async fn a_row_in_the_app_s_own_numbering_is_not_restamped() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let row = crate::history::parse(&format!("6\t{}\t{STITCHED}\tep:own\n", key()))
        .pop()
        .expect("a row");
    assert!(row.own_number);
    crate::history::upsert_and_write(&state.history_path, row).expect("write");
    crate::commands::numbering_offset::put_display(&state, &key(), 0, 3, "2");
    crate::commands::availability::check_availability(&state, &probe_args())
        .await
        .expect("probed");
    assert_eq!(read_back(&state), "6");
    assert_eq!(
        crate::commands::numbering_offset::display_stamp(&state, &key()),
        Some((3, "2".to_string()))
    );
}

#[tokio::test]
async fn a_row_the_listing_cannot_prove_is_left_as_it_reads() {
    // No stamp at all: nothing says the listing the row was written
    // against had the recap where this one has it.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let row = crate::history::parse(&format!("6\t{}\t{STITCHED}\n", key()))
        .pop()
        .expect("a row");
    crate::history::upsert_and_write(&state.history_path, row).expect("write");
    crate::commands::numbering_offset::put(&state, &key(), 0);
    crate::commands::availability::check_availability(&state, &probe_args())
        .await
        .expect("probed");
    assert_eq!(read_back(&state), "6");
    assert_eq!(
        crate::commands::numbering_offset::display_stamp(&state, &key()),
        None
    );
}
