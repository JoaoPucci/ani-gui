//! A row is served only on the word of a provider the app walks.
//!
//! An availability row is one provider's verdict: its catalogue, its
//! cap, its numbering. Once that provider is no longer walked, nothing
//! stands behind the row — its negative hides a show the walked
//! providers may carry, and its positive gates the strip with a cap
//! counted in another catalogue. A row that names no provider at all
//! predates attribution and is nobody's verdict either. Both re-probe.
//! Mounted by `#[path]` beside the availability tests and borrowing
//! their state builder and helpers.

use super::tests::{cache_only_state, check_with};
use super::*;
use crate::scraper::provider::{BrowseHit, EpisodeRef, ProviderId};
use crate::scraper::table_provider::TableProvider;
use std::sync::Arc;

/// Rows written as raw JSON, as an earlier build left them: they mean
/// the same whether or not this build still has a name for the
/// provider they cite.
fn seed(state: &AppState, kitsu_id: &str, body: &str) {
    meta_cache_put(&state.cache_pool, &cache_key(kitsu_id, "sub"), body, 3600).expect("seed row");
}

/// The walked provider: carries the show, twelve episodes, sub.
fn walked() -> Arc<TableProvider> {
    let rows: Vec<EpisodeRef> = (1..=12)
        .map(|n| EpisodeRef {
            id: 7000 + u64::from(n),
            number: n,
            number2: None,
            former_tag: None,
            title: None,
        })
        .collect();
    let mut table = TableProvider::new()
        .answering_as(ProviderId::Hianime)
        .search_default(vec![BrowseHit {
            slug: "the-show-7".into(),
            title: "The Show".into(),
            kind: None,
            jname: None,
        }])
        .listing(7, rows);
    for n in 1..=12u64 {
        table = table.mode(7000 + n, "sub", true);
    }
    Arc::new(table)
}

fn state_walking_hianime(td: &tempfile::TempDir) -> AppState {
    let mut state = cache_only_state(td);
    state.providers.set_order(vec![ProviderId::Hianime]);
    // Seen answering, so its own rows would stand.
    super::tests::close_breaker(state.providers.gate(ProviderId::Hianime));
    state
}

fn args(kitsu_id: &str) -> AvailabilityArgs {
    serde_json::from_value(serde_json::json!({
        "title": "The Show",
        "mode": "sub",
        "kitsu_id": kitsu_id,
        "episode_count": 12
    }))
    .expect("args")
}

const UNWALKED_POSITIVE: &str =
    r#"{"available":true,"episode_count":1160,"extra_episodes":[],"provider":"anidb"}"#;
const UNWALKED_NEGATIVE: &str = r#"{"available":false,"extra_episodes":[],"provider":"anidb"}"#;
const UNATTRIBUTED_POSITIVE: &str =
    r#"{"available":true,"episode_count":1160,"extra_episodes":[]}"#;
const UNATTRIBUTED_NEGATIVE: &str = r#"{"available":false,"extra_episodes":[]}"#;

#[tokio::test]
async fn a_row_from_a_provider_the_app_does_not_walk_re_probes() {
    for (id, body) in [("901", UNWALKED_POSITIVE), ("902", UNWALKED_NEGATIVE)] {
        let td = tempfile::tempdir().expect("td");
        let state = state_walking_hianime(&td);
        seed(&state, id, body);
        let provider = walked();
        let got = check_with(&state, &args(id), &provider)
            .await
            .expect("the walked provider answers");
        assert!(
            !provider.asked().is_empty(),
            "{body}: the row was served instead of re-probed"
        );
        assert!(
            got.available,
            "{body}: the walked provider carries the show"
        );
        assert_eq!(
            got.episode_count,
            Some(12),
            "{body}: its cap, not the row's"
        );
        assert_eq!(got.provider, Some(ProviderId::Hianime));
    }
}

#[tokio::test]
async fn a_row_that_names_no_provider_re_probes() {
    for (id, body) in [
        ("903", UNATTRIBUTED_POSITIVE),
        ("904", UNATTRIBUTED_NEGATIVE),
    ] {
        let td = tempfile::tempdir().expect("td");
        let state = state_walking_hianime(&td);
        seed(&state, id, body);
        let provider = walked();
        let got = check_with(&state, &args(id), &provider)
            .await
            .expect("the walked provider answers");
        assert!(
            !provider.asked().is_empty(),
            "{body}: the row was served instead of re-probed"
        );
        assert_eq!(
            got.episode_count,
            Some(12),
            "{body}: the walked provider's cap"
        );
    }
}

#[test]
fn the_lists_treat_such_rows_as_unknown() {
    let td = tempfile::tempdir().expect("td");
    let state = state_walking_hianime(&td);
    seed(&state, "905", UNWALKED_POSITIVE);
    seed(&state, "906", UNWALKED_NEGATIVE);
    seed(&state, "907", UNATTRIBUTED_POSITIVE);
    seed(&state, "908", UNATTRIBUTED_NEGATIVE);
    let listed = batch_cached(
        &state,
        &AvailabilityBatchArgs {
            kitsu_ids: ["905", "906", "907", "908"].map(String::from).to_vec(),
            mode: "sub".into(),
        },
    );
    assert!(
        listed.cached.is_empty(),
        "no card hidden or shown on a row nobody walked stands behind: {:?}",
        listed.cached
    );
    assert!(
        listed.playable_episode_counts.is_empty(),
        "and no cap counted in another catalogue: {:?}",
        listed.playable_episode_counts
    );
}

#[test]
fn a_walked_providers_own_rows_still_serve() {
    let td = tempfile::tempdir().expect("td");
    let state = state_walking_hianime(&td);
    seed(
        &state,
        "909",
        r#"{"available":true,"episode_count":12,"extra_episodes":[],"provider":"hianime"}"#,
    );
    seed(
        &state,
        "910",
        r#"{"available":false,"extra_episodes":[],"provider":"hianime"}"#,
    );
    let listed = batch_cached(
        &state,
        &AvailabilityBatchArgs {
            kitsu_ids: ["909", "910"].map(String::from).to_vec(),
            mode: "sub".into(),
        },
    );
    assert_eq!(listed.cached.get("909"), Some(&true));
    assert_eq!(listed.cached.get("910"), Some(&false));
    assert_eq!(listed.playable_episode_counts.get("909"), Some(&12));
}

/// A replay reaches a show whose positive row a provider no longer
/// walked wrote, or that names none. That row is never served, so it
/// is not one the replay can be counted against: the replay writes the
/// walked provider's own row, count-less, as it would where no row
/// stood. Left in place, the unwalked row would keep the strip from
/// reading a verdict for the show however often it played.
#[tokio::test]
async fn a_replay_over_a_positive_row_nobody_walked_writes_the_walked_providers_row() {
    for (id, body) in [("911", UNWALKED_POSITIVE), ("912", UNATTRIBUTED_POSITIVE)] {
        let td = tempfile::tempdir().expect("td");
        let state = state_walking_hianime(&td);
        seed(&state, id, body);
        let at_start = RowAtStart::read(&state, Some(id), "sub").await;

        stamp_after_cache_hit(
            &state,
            Some(id),
            "sub",
            &at_start,
            ProviderId::Hianime,
            &[],
            None,
        )
        .await;

        let stored = meta_cache_get(&state.cache_pool, &cache_key(id, "sub"))
            .expect("read")
            .expect("a row");
        let row: AvailabilityResponse = serde_json::from_str(&stored).expect("row");
        assert_eq!(row.provider, Some(ProviderId::Hianime), "{body}");
        assert!(row.available, "{body}");
        assert_eq!(
            row.episode_count, None,
            "{body}: not the other catalogue's cap"
        );
    }
}
