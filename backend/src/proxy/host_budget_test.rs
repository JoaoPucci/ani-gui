//! The host budget's pure core and its admission, mounted by `#[path]`
//! beside the module.

use super::*;
use proptest::prelude::*;

#[test]
fn a_burst_is_served_without_waiting_and_the_next_request_waits_a_refill() {
    let now = Instant::now();
    let mut bucket = Bucket::full(SEGMENT_BURST, now);
    for _ in 0..SEGMENT_BURST {
        assert_eq!(take(&mut bucket, now, SEGMENT_BURST, SEGMENT_REFILL), None);
    }
    let wait = take(&mut bucket, now, SEGMENT_BURST, SEGMENT_REFILL).expect("the burst is spent");
    assert!(
        wait >= SEGMENT_REFILL && wait <= SEGMENT_REFILL + Duration::from_millis(2),
        "{wait:?}"
    );
}

#[test]
fn waiting_the_returned_time_yields_a_token() {
    let now = Instant::now();
    let mut bucket = Bucket::full(SEGMENT_BURST, now);
    for _ in 0..SEGMENT_BURST {
        take(&mut bucket, now, SEGMENT_BURST, SEGMENT_REFILL);
    }
    let wait = take(&mut bucket, now, SEGMENT_BURST, SEGMENT_REFILL).expect("spent");
    assert_eq!(
        take(&mut bucket, now + wait, SEGMENT_BURST, SEGMENT_REFILL),
        None,
        "the token promised for then is there"
    );
}

#[test]
fn an_idle_bucket_refills_to_the_burst_and_no_further() {
    let now = Instant::now();
    let mut bucket = Bucket::full(SEGMENT_BURST, now);
    for _ in 0..SEGMENT_BURST {
        take(&mut bucket, now, SEGMENT_BURST, SEGMENT_REFILL);
    }
    let later = now + Duration::from_secs(600);
    for _ in 0..SEGMENT_BURST {
        assert_eq!(take(&mut bucket, later, SEGMENT_BURST, SEGMENT_REFILL), None);
    }
    assert!(
        take(&mut bucket, later, SEGMENT_BURST, SEGMENT_REFILL).is_some(),
        "ten idle minutes buy one burst, not more"
    );
}

proptest! {
    #[test]
    fn tokens_stay_within_the_burst_and_no_wait_exceeds_a_refill(
        gaps in proptest::collection::vec(0u64..4000, 1..200)
    ) {
        let mut now = Instant::now();
        let mut bucket = Bucket::full(SEGMENT_BURST, now);
        for gap in gaps {
            now += Duration::from_millis(gap);
            let wait = take(&mut bucket, now, SEGMENT_BURST, SEGMENT_REFILL);
            prop_assert!(bucket.tokens() >= 0.0);
            prop_assert!(bucket.tokens() <= f64::from(SEGMENT_BURST));
            if let Some(wait) = wait {
                prop_assert!(wait <= SEGMENT_REFILL + Duration::from_millis(1));
            }
        }
    }
}

#[tokio::test(start_paused = true)]
async fn the_budget_admits_a_burst_at_once_and_the_next_after_a_refill() {
    let budget = HostBudget::new(3, Duration::from_secs(1));
    let start = Instant::now();
    for _ in 0..3 {
        budget.admit("cdn.example:443").await;
    }
    assert_eq!(Instant::now(), start, "the burst waits for nothing");
    budget.admit("cdn.example:443").await;
    assert!(
        Instant::now() - start >= Duration::from_secs(1),
        "the fourth waited a refill"
    );
    // Another host has a burst of its own.
    let before = Instant::now();
    budget.admit("other.example:443").await;
    assert_eq!(Instant::now(), before);
}

#[test]
fn the_key_is_host_and_port() {
    let a = Url::parse("https://cdn.example/a/seg.ts").expect("url");
    let b = Url::parse("https://cdn.example:8443/b/seg.ts").expect("url");
    let c = Url::parse("http://127.0.0.1:4001/seg.ts").expect("url");
    let d = Url::parse("http://127.0.0.1:4002/seg.ts").expect("url");
    assert_eq!(host_key(&a), "cdn.example:443");
    assert_ne!(host_key(&a), host_key(&b));
    assert_ne!(host_key(&c), host_key(&d), "two servers on one machine are two budgets");
}
