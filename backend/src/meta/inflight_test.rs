//! Lookups under one key at the same time run once; under different
//! keys, or one after another, each runs.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::Inflight;

/// A lookup that counts its runs and answers after `ms`.
fn counted(
    runs: &Arc<AtomicUsize>,
    ms: u64,
    answer: u32,
) -> impl std::future::Future<Output = u32> {
    let runs = Arc::clone(runs);
    async move {
        runs.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(ms)).await;
        answer
    }
}

#[tokio::test]
async fn lookups_under_one_key_at_the_same_time_share_one_run_and_its_answer() {
    let shared = Inflight::<u32>::default();
    let runs = Arc::new(AtomicUsize::new(0));
    let (a, b, c) = tokio::join!(
        shared.run("show-1555".into(), || counted(&runs, 50, 7)),
        shared.run("show-1555".into(), || counted(&runs, 50, 8)),
        shared.run("show-1555".into(), || counted(&runs, 50, 9)),
    );
    assert_eq!(runs.load(Ordering::SeqCst), 1, "one run for one key");
    assert_eq!((a, b, c), (7, 7, 7), "every caller gets the run's answer");
}

#[tokio::test]
async fn lookups_under_different_keys_each_run() {
    let shared = Inflight::<u32>::default();
    let runs = Arc::new(AtomicUsize::new(0));
    let (a, b) = tokio::join!(
        shared.run("show-1555".into(), || counted(&runs, 20, 1)),
        shared.run("show-8272".into(), || counted(&runs, 20, 2)),
    );
    assert_eq!(runs.load(Ordering::SeqCst), 2);
    assert_eq!((a, b), (1, 2));
}

#[tokio::test]
async fn a_lookup_after_the_last_one_answered_runs_again() {
    let shared = Inflight::<u32>::default();
    let runs = Arc::new(AtomicUsize::new(0));
    assert_eq!(shared.run("k".into(), || counted(&runs, 1, 1)).await, 1);
    assert_eq!(shared.run("k".into(), || counted(&runs, 1, 2)).await, 2);
    assert_eq!(
        runs.load(Ordering::SeqCst),
        2,
        "nothing is remembered once answered"
    );
}

#[tokio::test]
async fn a_caller_whose_leader_went_away_unanswered_runs_the_lookup_itself() {
    let shared = Arc::new(Inflight::<u32>::default());
    let runs = Arc::new(AtomicUsize::new(0));
    let leader = {
        let shared = Arc::clone(&shared);
        let runs = Arc::clone(&runs);
        tokio::spawn(async move { shared.run("k".into(), || counted(&runs, 10_000, 1)).await })
    };
    tokio::time::sleep(Duration::from_millis(20)).await;
    let follower = {
        let shared = Arc::clone(&shared);
        let runs = Arc::clone(&runs);
        tokio::spawn(async move { shared.run("k".into(), || counted(&runs, 10, 2)).await })
    };
    tokio::time::sleep(Duration::from_millis(20)).await;
    leader.abort();
    let answer = tokio::time::timeout(Duration::from_secs(2), follower)
        .await
        .expect("the follower is not left waiting")
        .expect("joined");
    assert_eq!(answer, 2, "the follower ran it itself");
    assert_eq!(runs.load(Ordering::SeqCst), 2);
}
