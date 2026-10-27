//! One walk per availability row: lookups of the same row share the
//! walk already out for it, and the warm hands its items to one
//! worker through a queue that holds each row once.
//!
//! Every scenario holds the provider's requests on a gate, so the
//! walk is outstanding for exactly as long as the test needs it to
//! be — a second caller, a caller leaving, a warm pushed again — and
//! is then let through.
//!
//! Mounted by `#[path]` beside the availability tests and borrowing
//! their state builder.

use super::tests::cache_only_state;
use super::*;
use crate::commands::availability_flight::{check_shared, UNWATCHED_MAX};
use crate::commands::availability_warm::WARM_QUEUE_MAX;
use crate::scraper::provider::{BrowseHit, ProviderId};
use crate::scraper::table_provider::{listing_of, TableProvider};
use std::sync::Arc;

/// A provider whose every search finds `flight-show-7`, two episodes
/// long and streaming in sub; with `hold`, every request waits on it.
fn carrying(hold: Option<crate::held::Hold>) -> Arc<TableProvider> {
    let table = TableProvider::new()
        .answering_as(ProviderId::TestA)
        .search_default(vec![BrowseHit {
            slug: "flight-show-7".to_string(),
            title: "Flight Show".to_string(),
            kind: None,
            jname: None,
        }])
        .listing(7, listing_of(7, 2))
        .modes_default(true);
    Arc::new(match hold {
        Some(hold) => table.holding(hold),
        None => table,
    })
}

/// A state walking `table` alone.
fn state_with(td: &tempfile::TempDir, table: &Arc<TableProvider>) -> Arc<AppState> {
    let mut state = cache_only_state(td);
    state.providers.set_stub(Arc::clone(table));
    Arc::new(state)
}

/// The lookup of kitsu id `id` in sub, interactive unless
/// `background`.
fn lookup(id: &str, background: bool) -> AvailabilityArgs {
    serde_json::from_value(serde_json::json!({
        "title": "Flight Show",
        "mode": "sub",
        "kitsu_id": id,
        "episode_count": 2,
        "status": "finished",
        "background": background
    }))
    .expect("args")
}

/// How many requests one walk of the show sends, measured on a
/// provider of its own.
async fn one_walk() -> usize {
    let table = carrying(None);
    let td = tempfile::tempdir().expect("td");
    let state = state_with(&td, &table);
    check_shared(&state, lookup("700", false))
        .await
        .expect("the walk answers");
    let sent = table.asked().len();
    assert!(sent > 0, "a walk sends requests");
    sent
}

fn spawn_lookup(
    state: &Arc<AppState>,
    args: AvailabilityArgs,
) -> tokio::task::JoinHandle<crate::error::Result<AvailabilityResponse>> {
    let state = Arc::clone(state);
    tokio::spawn(async move { check_shared(&state, args).await })
}

fn row(id: &str) -> String {
    cache_key(id, "sub")
}

#[tokio::test]
async fn two_lookups_of_one_row_share_one_walk() {
    let expected = one_walk().await;
    let (hold, gate) = crate::held::gate();
    let table = carrying(Some(hold));
    let td = tempfile::tempdir().expect("td");
    let state = state_with(&td, &table);

    let first = spawn_lookup(&state, lookup("700", false));
    gate.asked().await;
    let second = spawn_lookup(&state, lookup("700", false));
    let flights = state.availability_refreshes.flights.clone();
    crate::held::until("the second lookup waits on the first's walk", || {
        flights.waiting(&row("700")) == Some(2)
    })
    .await;
    gate.release();

    let a = first.await.expect("joins").expect("answers");
    let b = second.await.expect("joins").expect("answers");
    assert!(a.available && b.available, "both callers hear the verdict");
    assert_eq!(a.episode_count, b.episode_count);
    assert_eq!(table.asked().len(), expected, "one walk, not two");
    assert_eq!(flights.live(), 0, "the finished walk leaves nothing behind");
}

#[tokio::test]
async fn the_caller_still_waiting_gets_its_answer_when_the_first_leaves() {
    let expected = one_walk().await;
    let (hold, gate) = crate::held::gate();
    let table = carrying(Some(hold));
    let td = tempfile::tempdir().expect("td");
    let state = state_with(&td, &table);
    let flights = state.availability_refreshes.flights.clone();

    let first = spawn_lookup(&state, lookup("700", false));
    gate.asked().await;
    let second = spawn_lookup(&state, lookup("700", false));
    crate::held::until("the second lookup joins", || {
        flights.waiting(&row("700")) == Some(2)
    })
    .await;
    first.abort();
    let _ = first.await;
    crate::held::until("the first caller's leaving is counted", || {
        flights.waiting(&row("700")) == Some(1)
    })
    .await;
    gate.release();

    let got = second
        .await
        .expect("joins")
        .expect("the walk answered the caller still waiting");
    assert!(got.available);
    assert_eq!(table.asked().len(), expected, "the walk was not restarted");
    assert_eq!(flights.live(), 0);
}

#[tokio::test]
async fn an_interactive_walk_nobody_waits_on_any_more_ends() {
    let (hold, gate) = crate::held::gate();
    let table = carrying(Some(hold));
    let td = tempfile::tempdir().expect("td");
    let state = state_with(&td, &table);
    let flights = state.availability_refreshes.flights.clone();

    let only = spawn_lookup(&state, lookup("700", false));
    gate.asked().await;
    only.abort();
    let _ = only.await;
    crate::held::until("the walk nobody waits on is gone", || flights.live() == 0).await;
    let sent = table.asked().len();
    gate.release();
    tokio::task::yield_now().await;
    assert_eq!(
        table.asked().len(),
        sent,
        "the ended walk sends nothing more once the provider answers"
    );
}

#[tokio::test]
async fn a_background_walk_its_caller_left_runs_on_and_the_next_caller_joins_it() {
    let expected = one_walk().await;
    let (hold, gate) = crate::held::gate();
    let table = carrying(Some(hold));
    let td = tempfile::tempdir().expect("td");
    let state = state_with(&td, &table);
    let flights = state.availability_refreshes.flights.clone();

    // A Continue Watching probe, and the page it was asked for left.
    let left = spawn_lookup(&state, lookup("700", true));
    gate.asked().await;
    left.abort();
    let _ = left.await;
    crate::held::until("the caller's leaving is counted", || {
        flights.waiting(&row("700")) == Some(0)
    })
    .await;
    assert_eq!(flights.live(), 1, "the walk runs on with nobody waiting");

    // The page mounts again and asks for the same row.
    let back = spawn_lookup(&state, lookup("700", true));
    crate::held::until("the returning caller joins the walk", || {
        flights.waiting(&row("700")) == Some(1)
    })
    .await;
    gate.release();
    let got = back.await.expect("joins").expect("answers");
    assert!(got.available);
    assert_eq!(table.asked().len(), expected, "one walk across both mounts");
    assert!(
        meta_cache_get(&state.cache_pool, &row("700"))
            .expect("cache read")
            .is_some(),
        "the walk's verdict is cached"
    );
}

#[tokio::test]
async fn background_walks_nobody_waits_on_are_bounded() {
    let (hold, gate) = crate::held::gate();
    let table = carrying(Some(hold));
    let td = tempfile::tempdir().expect("td");
    let state = state_with(&td, &table);
    let flights = state.availability_refreshes.flights.clone();

    for n in 0..=UNWATCHED_MAX {
        let id = format!("{}", 800 + n);
        let caller = spawn_lookup(&state, lookup(&id, true));
        crate::held::until("the walk is out", || flights.waiting(&row(&id)) == Some(1)).await;
        caller.abort();
        let _ = caller.await;
        crate::held::until("the caller's leaving is counted", || {
            flights.waiting(&row(&id)) != Some(1)
        })
        .await;
    }
    assert_eq!(
        flights.live(),
        UNWATCHED_MAX,
        "past the bound, a walk nobody waits on ends"
    );
    drop(gate);
}

#[tokio::test]
async fn an_interactive_lookup_does_not_wait_behind_a_background_walk() {
    let expected = one_walk().await;
    let (hold, gate) = crate::held::gate();
    let table = carrying(Some(hold));
    let td = tempfile::tempdir().expect("td");
    let state = state_with(&td, &table);
    let flights = state.availability_refreshes.flights.clone();

    let warm = spawn_lookup(&state, lookup("700", true));
    gate.asked().await;
    let sent_by_background = table.asked().len();
    let click = spawn_lookup(&state, lookup("700", false));
    crate::held::until(
        "the background caller moves onto the interactive walk",
        || {
            flights.interactive(&row("700")) == Some(true)
                && flights.waiting(&row("700")) == Some(2)
        },
    )
    .await;
    gate.release();
    let a = warm.await.expect("joins").expect("answers");
    let b = click.await.expect("joins").expect("answers");
    assert!(a.available && b.available);
    assert_eq!(
        table.asked().len(),
        sent_by_background + expected,
        "the background walk stopped where it was; the interactive one ran whole"
    );
    assert_eq!(flights.live(), 0);
}

#[tokio::test]
async fn a_cache_skipping_refresh_walks_on_its_own() {
    let expected = one_walk().await;
    let (hold, gate) = crate::held::gate();
    let table = carrying(Some(hold));
    let td = tempfile::tempdir().expect("td");
    let state = state_with(&td, &table);
    let flights = state.availability_refreshes.flights.clone();

    let page = spawn_lookup(&state, lookup("700", false));
    gate.asked().await;
    let mut refresh_args = lookup("700", false);
    refresh_args.bypass_cache = true;
    let refresh = spawn_lookup(&state, refresh_args);
    crate::held::until("the page's walk is out", || {
        flights.waiting(&row("700")) == Some(1)
    })
    .await;
    gate.release();
    page.await.expect("joins").expect("answers");
    refresh.await.expect("joins").expect("answers");
    assert_eq!(
        table.asked().len(),
        2 * expected,
        "a refresh asks the provider itself rather than taking a walk that read through the cache"
    );
}

#[tokio::test]
async fn warming_one_row_on_every_mount_walks_it_once() {
    let expected = one_walk().await;
    let (hold, gate) = crate::held::gate();
    let table = carrying(Some(hold));
    let td = tempfile::tempdir().expect("td");
    let state = state_with(&td, &table);
    let queue = state.availability_refreshes.warm_queue.clone();

    queue.push(&state, vec![lookup("700", false)]);
    gate.asked().await;
    queue.push(&state, vec![lookup("700", false)]);
    queue.push(&state, vec![lookup("700", false)]);
    assert_eq!(queue.queued(), 0, "a row being warmed is not queued again");
    gate.release();
    warm(Arc::clone(&state), Vec::new()).await;
    assert_eq!(table.asked().len(), expected, "three mounts, one walk");
}

#[tokio::test]
async fn the_warm_walks_one_row_at_a_time() {
    let (hold, gate) = crate::held::gate();
    let table = carrying(Some(hold));
    let td = tempfile::tempdir().expect("td");
    let state = state_with(&td, &table);
    let queue = state.availability_refreshes.warm_queue.clone();
    let flights = state.availability_refreshes.flights.clone();

    queue.push(&state, vec![lookup("700", false)]);
    queue.push(&state, vec![lookup("701", false), lookup("702", false)]);
    gate.asked().await;
    assert_eq!(flights.live(), 1, "one warm walk at a time");
    assert_eq!(queue.queued(), 2, "the rest wait their turn");
    drop(gate);
}

#[tokio::test]
async fn a_lookup_joins_the_row_the_warm_is_walking() {
    let expected = one_walk().await;
    let (hold, gate) = crate::held::gate();
    let table = carrying(Some(hold));
    let td = tempfile::tempdir().expect("td");
    let state = state_with(&td, &table);
    let queue = state.availability_refreshes.warm_queue.clone();
    let flights = state.availability_refreshes.flights.clone();

    queue.push(&state, vec![lookup("700", false)]);
    gate.asked().await;
    let probe = spawn_lookup(&state, lookup("700", true));
    crate::held::until("the probe joins the warm's walk", || {
        flights.waiting(&row("700")) == Some(2)
    })
    .await;
    gate.release();
    probe.await.expect("joins").expect("answers");
    warm(Arc::clone(&state), Vec::new()).await;
    assert_eq!(
        table.asked().len(),
        expected,
        "the warm and the probe, one walk"
    );
}

#[tokio::test]
async fn the_warm_queue_is_bounded() {
    let (hold, gate) = crate::held::gate();
    let table = carrying(Some(hold));
    let td = tempfile::tempdir().expect("td");
    let state = state_with(&td, &table);
    let queue = state.availability_refreshes.warm_queue.clone();

    queue.push(&state, vec![lookup("700", false)]);
    gate.asked().await;
    let more: Vec<AvailabilityArgs> = (0..WARM_QUEUE_MAX + 5)
        .map(|n| lookup(&format!("{}", 1000 + n), false))
        .collect();
    queue.push(&state, more);
    assert_eq!(queue.queued(), WARM_QUEUE_MAX);
    drop(gate);
}
