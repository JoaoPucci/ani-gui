use super::*;
use crate::error::AniError;
use crate::scraper::gate::ScrapeOutcome;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Inner fetch that only counts how often the transport actually ran.
struct Counting {
    calls: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl AnidbFetch for Counting {
    async fn get(&self, _url: &str) -> crate::error::Result<FetchResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(FetchResponse {
            status: 200,
            body: "ok".into(),
        })
    }
}

fn counting() -> (Counting, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    (
        Counting {
            calls: calls.clone(),
        },
        calls,
    )
}

/// A gate whose breaker is open: three fresh consecutive failures.
fn open_gate() -> ScraperGate {
    let gate = ScraperGate::new();
    for _ in 0..crate::scraper::gate::FAILURE_THRESHOLD {
        gate.record(ScrapeOutcome::Failure, tokio::time::Instant::now());
    }
    gate
}

#[tokio::test]
async fn an_open_breaker_refuses_a_background_fetch_before_the_transport() {
    // The per-request contract: a background provider request that
    // the gate refuses must never reach the transport — otherwise a
    // background resolve's candidate probes and episode chain ride a
    // single pre-flight admit and burst past the breaker.
    let (inner, calls) = counting();
    let gate = open_gate();
    let fetch = GatedFetch::new(inner, Some(&gate), ScrapePriority::Background);
    let err = fetch
        .get("https://provider.test/x")
        .await
        .expect_err("refused");
    // The refusal keeps its identity: mapped to Network it would be
    // recorded as an upstream failure, and each background warmup
    // would refresh the open breaker's cooldown without ever
    // contacting the provider — half-open recovery never arrives.
    assert!(matches!(err, AniError::GateRefused), "got {err:?}");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "transport ran while refused"
    );
}

#[tokio::test]
async fn an_open_breaker_still_admits_interactive_fetches() {
    // Interactive requests ignore the breaker by the gate's own
    // contract — a user click must not be refused by background
    // weather.
    let (inner, calls) = counting();
    let gate = open_gate();
    let fetch = GatedFetch::new(inner, Some(&gate), ScrapePriority::Interactive);
    fetch
        .get("https://provider.test/x")
        .await
        .expect("admitted");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn a_healthy_gate_admits_each_background_fetch() {
    // Paced, not refused: with the breaker closed every request
    // takes its own slot and reaches the transport. Paused time
    // fast-forwards the 500 ms spacing.
    let (inner, calls) = counting();
    let gate = ScraperGate::new();
    let fetch = GatedFetch::new(inner, Some(&gate), ScrapePriority::Background);
    fetch.get("https://provider.test/a").await.expect("first");
    fetch.get("https://provider.test/b").await.expect("second");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test(start_paused = true)]
async fn a_recovered_breaker_lets_a_background_chain_run() {
    // The half-open trial admits the chain's first fetch; without
    // the sanction carried to the next fetch, the same resolve's
    // second request saw an outstanding trial, was refused before
    // the transport, and the chain died mid-walk — recording a
    // failure that re-opened the breaker on its own refusal.
    let (inner, calls) = counting();
    let gate = open_gate();
    tokio::time::sleep(crate::scraper::gate::BREAKER_COOLDOWN + std::time::Duration::from_secs(1))
        .await;
    let fetch = GatedFetch::new(inner, Some(&gate), ScrapePriority::Background);
    fetch
        .get("https://provider.test/search")
        .await
        .expect("the trial fetch runs");
    fetch
        .get("https://provider.test/episodes")
        .await
        .expect("the chained fetch rides the sanction");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn no_gate_is_a_passthrough() {
    let (inner, calls) = counting();
    let fetch = GatedFetch::new(inner, None, ScrapePriority::Background);
    fetch.get("https://provider.test/x").await.expect("through");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn the_stamp_is_the_post_admission_attempt_start() {
    // The orchestrator records breaker outcomes with the resolve
    // chain's start time, but the gate's stale filters read the
    // timestamp as when the request that PRODUCED the outcome began:
    // a long chain that observes a fresh 429 after another resolve
    // recorded recovery gets its evidence discarded as pre-recovery.
    // The transport therefore stamps each attempt's own start —
    // taken after admission, so time spent queueing in the paced
    // background slot never backdates the evidence.
    let (inner, _calls) = counting();
    let gate = ScraperGate::new();
    let fetch = GatedFetch::new(inner, Some(&gate), ScrapePriority::Background);
    assert!(fetch.last_attempt_at().is_none());
    fetch.get("http://s/one").await.expect("first");
    let first = fetch.last_attempt_at().expect("first attempt stamped");
    let before_second = tokio::time::Instant::now();
    fetch.get("http://s/two").await.expect("second");
    let second = fetch.last_attempt_at().expect("second attempt stamped");
    assert!(second > first, "each attempt re-stamps");
    assert!(
        second >= before_second + crate::scraper::gate::BACKGROUND_INTERVAL,
        "the stamp postdates the paced admission wait, not the call"
    );
}
