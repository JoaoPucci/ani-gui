//! Which row an unread count leaves standing: only a count it could
//! have been read against. A count from another provider's listing is
//! no count of the one that served, and an approximate one is no count
//! the read rule serves; neither stands in for the answer the stamp
//! could not read.

use super::tests::cache_only_state;
use super::*;
use crate::scraper::provider::ProviderId;

const ID: &str = "7881";
const MODE: &str = "sub";

fn walking_both(td: &tempfile::TempDir) -> AppState {
    let mut state = cache_only_state(td);
    state
        .providers
        .set_order(vec![ProviderId::TestA, ProviderId::TestB]);
    state
}

fn seed(state: &AppState, provider: ProviderId, approximate: bool) {
    write_cache_full(
        state,
        ID,
        MODE,
        None,
        &AvailabilityResponse {
            available: true,
            episode_count: Some(12),
            extra_episodes: vec!["12.1".into(), "12.2".into()],
            episode_count_approximate: approximate,
            gate_refused: false,
            unsettled: None,
            provider: Some(provider),
            versions: Vec::new(),
        },
    );
}

fn row_now(state: &AppState) -> AvailabilityResponse {
    let body = meta_cache_get(&state.cache_pool, &cache_key(ID, MODE))
        .expect("read")
        .expect("a row");
    serde_json::from_str(&body).expect("row")
}

async fn stamp_unread(state: &AppState, served_by: ProviderId) {
    let at_start = RowAtStart::read(state, Some(ID), MODE).await;
    stamp_after_native(
        state,
        Some(ID),
        MODE,
        &at_start,
        ResolveVerdict::served(served_by, Some(14), &[]).with_count_unread(true),
    )
    .await;
}

#[tokio::test]
async fn an_unread_count_leaves_the_same_providers_count_standing() {
    let td = tempfile::tempdir().expect("td");
    let state = walking_both(&td);
    seed(&state, ProviderId::TestA, false);
    stamp_unread(&state, ProviderId::TestA).await;
    let row = row_now(&state);
    assert_eq!(row.episode_count, Some(12));
    assert_eq!(row.provider, Some(ProviderId::TestA));
}

/// The row names the provider that served, and the next operation for
/// the show starts there; another provider's count says nothing of its
/// listing.
#[tokio::test]
async fn an_unread_count_does_not_keep_another_providers_count() {
    let td = tempfile::tempdir().expect("td");
    let state = walking_both(&td);
    seed(&state, ProviderId::TestB, false);
    stamp_unread(&state, ProviderId::TestA).await;
    let row = row_now(&state);
    assert_eq!(row.provider, Some(ProviderId::TestA), "{row:?}");
    assert_eq!(row.episode_count, None, "{row:?}");
    assert!(row.extra_episodes.is_empty());
}

#[tokio::test]
async fn an_unread_count_does_not_keep_an_approximate_count() {
    let td = tempfile::tempdir().expect("td");
    let state = walking_both(&td);
    seed(&state, ProviderId::TestA, true);
    stamp_unread(&state, ProviderId::TestA).await;
    let row = row_now(&state);
    assert_eq!(row.episode_count, None, "{row:?}");
    assert!(!row.episode_count_approximate);
}
