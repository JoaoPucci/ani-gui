use crate::commands::play_native_resolve::NativeError;
use crate::commands::play_native_walk::pick_native_walk;
use crate::error::AniError;
use crate::scraper::table_provider::{Answer, TableProvider};

use super::super::play_native_test_provider::{scripted, Pool, THE_SHOW};
use crate::commands::play_native::PickedShow;

// ---- pick_native_walk: the availability probes' pick half, driven
// directly so each verdict arm is pinned where it lives rather than
// only through the probes that consume it. ----

async fn walk(
    provider: &TableProvider,
    title: &str,
    alts: &[String],
    expected: Option<u32>,
) -> std::result::Result<PickedShow, NativeError> {
    pick_native_walk(provider, title, alts, expected, None, None).await
}

#[tokio::test]
async fn the_pick_walk_reports_the_all_clean_miss() {
    // Every alias answered a genuine no-results page: the one verdict
    // that proves absence, and the only one availability may persist.
    let provider = scripted(&[]);
    let ne = walk(
        &provider,
        "unknown show",
        &["other name".to_string()],
        Some(3),
    )
    .await
    .expect_err("nothing matches");
    assert!(ne.clean_miss, "all-clean absence is the persistable miss");
    assert!(matches!(ne.error, AniError::NoResults));
    assert!(
        ne.failed_at.is_none(),
        "no attempt failed, nothing to stamp"
    );
    assert_eq!(provider.asked().len(), 2, "both aliases were asked");
}

#[tokio::test]
async fn the_pick_walk_recovers_through_a_later_alias() {
    // The canonical title lands a pool the pick rejects on episode
    // distance (seven listed against three expected). That rejection
    // is a verdict about THAT pool, so the walk moves to the fallback
    // alias and picks the real show there.
    let provider = scripted(&[
        ("wrong cour", Pool::Hits(&[("wrong-cour-99", "Wrong Cour")])),
        ("the show", THE_SHOW),
    ]);
    let picked = walk(&provider, "wrong cour", &["the show".to_string()], Some(3))
        .await
        .expect("the alias carries the real show");
    assert_eq!(picked.hit.slug, "the-show-77");
}

#[tokio::test]
async fn the_pick_walk_stops_typed_on_an_interstitial() {
    // A challenge page is the provider refusing the CLIENT, not
    // answering about the show — pressing later aliases through it
    // would hammer a blocking provider. The walk stops typed.
    let provider = scripted(&[("blocked", Pool::Blocked)]);
    let ne = walk(
        &provider,
        "blocked show",
        &["never asked".to_string()],
        Some(3),
    )
    .await
    .expect_err("the block stops the walk");
    assert!(matches!(ne.error, AniError::Upstream { .. }));
    assert!(!ne.clean_miss);
    assert_eq!(
        provider.asked().len(),
        1,
        "the walk must not press later aliases through a block"
    );
}

#[tokio::test]
async fn the_pick_walk_marks_answered_dead_ends_non_persistable() {
    // A pool whose every candidate answers not-found (stale slugs) is
    // dead candidates on a healthy provider: NoResults, but NOT the
    // persistable clean miss — the next probe should look again.
    let provider = scripted(&[("ghost", Pool::Hits(&[("ghost-1", "Ghost Entry")]))]);
    let ne = walk(&provider, "ghost", &[], Some(3))
        .await
        .expect_err("dead candidates resolve nothing");
    assert!(matches!(ne.error, AniError::NoResults));
    assert!(
        !ne.clean_miss,
        "an answered dead end proves nothing about the show's absence"
    );
}

#[tokio::test]
async fn the_pick_walk_aggregates_transport_deaths_with_a_stamp() {
    // One alias died on transport, another answered clean: the clean
    // page cannot prove absence past weather, so the aggregate is the
    // transient Network verdict — stamped with the provider's own
    // per-attempt start, exactly as the resolve walk stamps its.
    let stamp = tokio::time::Instant::now();
    let provider = TableProvider::new()
        .hits("dead alias", Answer::fails(|| AniError::Network))
        .stamped(stamp);
    let ne = pick_native_walk(
        &provider,
        "dead alias",
        &["clean alias".to_string()],
        None,
        None,
        None,
    )
    .await
    .expect_err("weather is not a verdict");
    assert!(matches!(ne.error, AniError::Network));
    assert!(!ne.clean_miss);
    assert_eq!(
        ne.failed_at,
        Some(stamp),
        "the stamp is the provider's per-attempt start"
    );
}

#[tokio::test]
async fn the_pick_walk_stops_typed_on_a_gate_refusal() {
    // The gate's refusal is its OWN answer: the walk surfaces it
    // typed and does not spend the alias list against a closed gate.
    let provider = TableProvider::new().search_default(Answer::fails(|| AniError::GateRefused));
    let ne = pick_native_walk(
        &provider,
        "anything",
        &["something else".to_string()],
        None,
        None,
        None,
    )
    .await
    .expect_err("the refusal stops the walk");
    assert!(matches!(ne.error, AniError::GateRefused));
    assert!(!ne.clean_miss);
    assert_eq!(
        provider.asked().len(),
        1,
        "one refused admit; later aliases never asked"
    );
}
