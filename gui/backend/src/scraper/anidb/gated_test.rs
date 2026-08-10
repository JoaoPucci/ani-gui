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
    assert!(matches!(err, AniError::Network), "got {err:?}");
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

#[tokio::test]
async fn no_gate_is_a_passthrough() {
    let (inner, calls) = counting();
    let fetch = GatedFetch::new(inner, None, ScrapePriority::Background);
    fetch.get("https://provider.test/x").await.expect("through");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
