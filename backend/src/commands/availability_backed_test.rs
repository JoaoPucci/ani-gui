//! Whether a negative row's provider can stand behind it — the read
//! rule over the gate's states, half-open included. Mounted by
//! `#[path]` beside the availability tests and borrowing their state
//! builder and gate helpers.

use super::tests::{
    cache_only_state, close_breaker, open_breaker, pause_provider, stub_hianime_sub_only,
};
use super::*;

fn negative_row(provider: Option<crate::scraper::provider::ProviderId>) -> AvailabilityResponse {
    AvailabilityResponse {
        available: false,
        episode_count: None,
        extra_episodes: Vec::new(),
        episode_count_approximate: false,
        gate_refused: false,
        provider,
    }
}

/// A provider stands behind its own negative only once it has been
/// seen answering: past the cooldown the breaker refuses nobody but
/// is half-open, and the row served then would short-circuit the very
/// probe whose trial could fail over — hiding, for the rest of the
/// row's life, a show only the fallback carries. A success closes the
/// breaker and the row stands again. The providers ahead are still
/// asked whether they refuse: a half-open primary does not, so the
/// fallback's negative stops standing and the next look reprobes,
/// which is the trial. An advertised pause ends at its window, since
/// the upstream itself named that moment.
#[tokio::test(start_paused = true)]
async fn a_negative_row_is_backed_by_its_provider_only_once_it_has_recovered() {
    use crate::scraper::provider::ProviderId;
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state.provider_order = vec![ProviderId::Anidb, ProviderId::Hianime];
    let primary = negative_row(Some(ProviderId::Anidb));
    let unattributed = negative_row(None);
    let fallback = negative_row(Some(ProviderId::Hianime));

    assert!(
        negative_row_is_backed(&state, &primary),
        "a fresh primary answers"
    );
    assert!(
        !negative_row_is_backed(&state, &fallback),
        "so the fallback's row does not stand"
    );

    open_breaker(&state.anidb_gate);
    assert!(
        !negative_row_is_backed(&state, &primary),
        "an open primary stands behind nothing"
    );
    assert!(!negative_row_is_backed(&state, &unattributed));
    assert!(
        negative_row_is_backed(&state, &fallback),
        "the fallback's row stands through the outage"
    );

    tokio::time::advance(
        crate::scraper::gate::BREAKER_COOLDOWN + std::time::Duration::from_millis(1),
    )
    .await;
    assert!(
        !negative_row_is_backed(&state, &primary),
        "past the cooldown the primary is half-open: nothing has answered, nothing is backed"
    );
    assert!(!negative_row_is_backed(&state, &unattributed));
    assert!(
        !negative_row_is_backed(&state, &fallback),
        "and the half-open primary no longer refuses, so the fallback's row yields to the trial"
    );

    close_breaker(&state.anidb_gate);
    assert!(
        negative_row_is_backed(&state, &primary),
        "a success is the recovery"
    );
    assert!(!negative_row_is_backed(&state, &fallback));

    pause_provider(&state.anidb_gate);
    assert!(
        !negative_row_is_backed(&state, &primary),
        "a running pause is not answering"
    );
    assert!(negative_row_is_backed(&state, &fallback));
    tokio::time::advance(std::time::Duration::from_secs(120) + std::time::Duration::from_millis(1))
        .await;
    assert!(
        negative_row_is_backed(&state, &primary),
        "the pause ends at the window the upstream named"
    );
    assert!(!negative_row_is_backed(&state, &fallback));
}

/// The primary's negative, written before its outage, is not served
/// merely because the cooldown elapsed: the lists do not hide the
/// show, and the page's probe runs — and fails over to the fallback
/// that carries it. Once the primary has answered again, the row is
/// served as before.
#[tokio::test]
async fn a_primary_negative_is_not_served_past_the_cooldown_until_the_primary_answers() {
    use crate::scraper::provider::ProviderId;
    let hianime = stub_hianime_sub_only().await;
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state.provider_order = vec![ProviderId::Anidb, ProviderId::Hianime];
    state.hianime_base = Some(hianime.uri());
    write_cache(&state, "571", "sub", false, Some(ProviderId::Anidb));
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Fallback Show",
        "mode": "sub",
        "kitsu_id": "571",
        "episode_count": 2
    }))
    .expect("args");
    let batch = AvailabilityBatchArgs {
        kitsu_ids: vec!["571".into()],
        mode: "sub".into(),
    };
    assert_eq!(
        batch_cached(&state, &batch).cached.get("571"),
        Some(&false),
        "the primary's own negative is served while the primary answers"
    );

    open_breaker(&state.anidb_gate);
    tokio::time::pause();
    tokio::time::advance(
        crate::scraper::gate::BREAKER_COOLDOWN + std::time::Duration::from_millis(1),
    )
    .await;
    tokio::time::resume();
    assert!(
        !batch_cached(&state, &batch).cached.contains_key("571"),
        "past the cooldown the primary has answered nothing, so its negative is not served"
    );
    let got = check_availability_with_base(&state, &args, Some("http://127.0.0.1:1"))
        .await
        .expect("the probe ran and the fallback answered");
    assert!(
        got.available,
        "the probe ran past the still-down primary and the fallback carries the show"
    );
    assert!(
        !hianime
            .received_requests()
            .await
            .expect("recorded")
            .is_empty(),
        "the fallback was asked"
    );

    // Recovery observed: the primary's negative stands again.
    let td2 = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td2);
    state.provider_order = vec![ProviderId::Anidb, ProviderId::Hianime];
    state.hianime_base = Some(hianime.uri());
    write_cache(&state, "572", "sub", false, Some(ProviderId::Anidb));
    open_breaker(&state.anidb_gate);
    tokio::time::pause();
    tokio::time::advance(
        crate::scraper::gate::BREAKER_COOLDOWN + std::time::Duration::from_millis(1),
    )
    .await;
    tokio::time::resume();
    close_breaker(&state.anidb_gate);
    let batch = AvailabilityBatchArgs {
        kitsu_ids: vec!["572".into()],
        mode: "sub".into(),
    };
    assert_eq!(
        batch_cached(&state, &batch).cached.get("572"),
        Some(&false),
        "a success closed the breaker, and the primary stands behind its row again"
    );
    let asked_before = hianime.received_requests().await.expect("recorded").len();
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Fallback Show",
        "mode": "sub",
        "kitsu_id": "572",
        "episode_count": 2
    }))
    .expect("args");
    let got = check_availability_with_base(&state, &args, Some("http://127.0.0.1:1"))
        .await
        .expect("served from the row");
    assert!(!got.available, "the row is the answer");
    assert_eq!(
        asked_before,
        hianime.received_requests().await.expect("recorded").len(),
        "a cache hit asks the fallback nothing"
    );
}
