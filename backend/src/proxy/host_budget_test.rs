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
        assert_eq!(
            take(&mut bucket, later, SEGMENT_BURST, SEGMENT_REFILL),
            None
        );
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
    assert_ne!(
        host_key(&c),
        host_key(&d),
        "two servers on one machine are two budgets"
    );
}

proptest! {
    #[test]
    fn the_key_is_the_url_host_and_its_port_and_nothing_else(
        // Labels without hyphens: the url crate refuses some hyphenated
        // ones (an `xn--` prefix) as invalid international names.
        host in "[a-z][a-z0-9]{0,12}(\\.[a-z][a-z0-9]{0,12}){0,3}",
        port in 1u16..=65535,
        https in proptest::bool::ANY,
        path in "/[a-z0-9/._-]{0,40}",
    ) {
        let scheme = if https { "https" } else { "http" };
        let explicit = Url::parse(&format!("{scheme}://{host}:{port}{path}")).expect("url");
        prop_assert_eq!(host_key(&explicit), format!("{host}:{port}"));
        let implied = Url::parse(&format!("{scheme}://{host}{path}")).expect("url");
        prop_assert_eq!(host_key(&implied), format!("{host}:{}", if https { 443 } else { 80 }));
        let other_path = Url::parse(&format!("{scheme}://{host}:{port}/elsewhere")).expect("url");
        prop_assert_eq!(host_key(&explicit), host_key(&other_path));
    }
}

/// A request that finds the burst spent waits for the next token, and
/// one arriving just as that token matures — before the waiter's own
/// sleep is over — must not take it: waiters are served in the order
/// they arrived, so a few requests arriving at once beside a download
/// cannot keep taking the tokens ahead of the player's request that
/// has waited longest.
#[tokio::test(start_paused = true)]
async fn a_token_goes_to_the_waiter_that_has_waited_for_it() {
    let budget = Arc::new(HostBudget::new(1, Duration::from_millis(500)));
    budget.admit("cdn.example:443").await;
    let order = Arc::new(Mutex::new(Vec::new()));
    let waiter = |i: u8| {
        let budget = Arc::clone(&budget);
        let order = Arc::clone(&order);
        tokio::spawn(async move {
            budget.admit("cdn.example:443").await;
            order.lock().unwrap_or_else(|e| e.into_inner()).push(i);
        })
    };
    let first = waiter(0);
    // The token matures at the refill; the first waiter wakes a shade
    // after it, and the second arrives in between.
    tokio::time::sleep(Duration::from_millis(500)).await;
    let second = waiter(1);
    first.await.expect("first waiter");
    second.await.expect("second waiter");
    assert_eq!(
        *order.lock().unwrap_or_else(|e| e.into_inner()),
        vec![0, 1],
        "the token went to the request that waited for it"
    );
}

/// A background fetch never waits in the host's line: it takes a
/// token only when no one is waiting for one, so a player's request
/// that arrives after it is served first.
#[tokio::test(start_paused = true)]
async fn a_background_fetch_never_waits_ahead_of_the_player() {
    let budget = Arc::new(HostBudget::new(1, Duration::from_millis(500)));
    budget.admit("cdn.example:443").await;
    let order = Arc::new(Mutex::new(Vec::new()));
    let background = {
        let (budget, order) = (Arc::clone(&budget), Arc::clone(&order));
        tokio::spawn(async move {
            budget.admit_background("cdn.example:443").await;
            order
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push("background");
        })
    };
    tokio::time::sleep(Duration::from_millis(10)).await;
    let player = {
        let (budget, order) = (Arc::clone(&budget), Arc::clone(&order));
        tokio::spawn(async move {
            budget.admit("cdn.example:443").await;
            order
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push("player");
        })
    };
    player.await.expect("player");
    background.await.expect("background");
    assert_eq!(
        *order.lock().unwrap_or_else(|e| e.into_inner()),
        vec!["player", "background"],
        "the player's request went first"
    );
}

/// With no one waiting, a background fetch waits for its token like
/// any other and is admitted.
#[tokio::test(start_paused = true)]
async fn a_background_fetch_alone_is_admitted_at_the_next_token() {
    let budget = HostBudget::new(1, Duration::from_millis(500));
    budget.admit("cdn.example:443").await;
    let start = tokio::time::Instant::now();
    budget.admit_background("cdn.example:443").await;
    let waited = start.elapsed();
    assert!(
        waited >= Duration::from_millis(500) && waited < Duration::from_millis(1000),
        "{waited:?}"
    );
}

/// A download running unpaced against a host — yt-dlp's sixteen
/// fragments at a time with nothing playing — sends its requests past
/// the budget, but the host counts them all the same. A player that
/// starts beside it gets what those requests leave of the burst, not a
/// full one: twenty more on top of sixteen is the refusal the budget
/// exists to prevent.
#[tokio::test(start_paused = true)]
async fn a_player_starting_beside_an_unpaced_download_gets_only_what_it_leaves() {
    let budget = HostBudget::fresh();
    let _run = budget.unpaced_run(16);
    for i in 0..4 {
        tokio::time::timeout(Duration::from_millis(1), budget.admit("cdn.example:443"))
            .await
            .unwrap_or_else(|_| panic!("request {i} is within what the download left"));
    }
    assert!(
        tokio::time::timeout(SEGMENT_REFILL / 2, budget.admit("cdn.example:443"))
            .await
            .is_err(),
        "the fifth waits for the refill"
    );
}

/// However many unpaced downloads run, the hold never takes a host's
/// last token: a fresh host still admits the player's first request.
#[tokio::test(start_paused = true)]
async fn the_players_first_request_is_admitted_beside_any_number_of_unpaced_downloads() {
    let budget = HostBudget::fresh();
    let _a = budget.unpaced_run(16);
    let _b = budget.unpaced_run(16);
    tokio::time::timeout(Duration::from_millis(1), budget.admit("cdn.example:443"))
        .await
        .expect("the first request is admitted");
}

/// A request the app makes without waiting for a token — a cached
/// row's liveness check — is spent from the host's budget all the same.
#[test]
fn a_request_the_budget_does_not_pace_is_spent_from_it() {
    let budget = HostBudget::fresh();
    budget.spend("cdn.example:443", 3);
    assert_eq!(budget.on_hand("cdn.example:443"), Some(17.0));
    budget.spend("cdn.example:443", 40);
    assert_eq!(budget.on_hand("cdn.example:443"), Some(0.0));
}

/// An unpaced download's fragments go wherever its playlists send
/// them — a segment host, an edge a redirect lands on — which the app
/// does not see. So the hold covers every host the player fetches
/// from while the download runs, not only the stream's own.
#[tokio::test(start_paused = true)]
async fn the_hold_covers_every_host_while_an_unpaced_download_runs() {
    let budget = HostBudget::fresh();
    let _run = budget.unpaced_run(16);
    for host in ["cdn.example:443", "edge.example:443"] {
        for _ in 0..4 {
            tokio::time::timeout(Duration::from_millis(1), budget.admit(host))
                .await
                .expect("within what the download left");
        }
        assert!(
            tokio::time::timeout(SEGMENT_REFILL / 2, budget.admit(host))
                .await
                .is_err(),
            "{host}: the fifth waits"
        );
    }
}

/// When the download ends, what its requests in flight left is where
/// a host's tokens stand — not that, less them again: a player that
/// already took what the hold left keeps the rest.
#[tokio::test(start_paused = true)]
async fn an_unpaced_download_ending_does_not_charge_its_requests_twice() {
    let budget = HostBudget::fresh();
    let run = budget.unpaced_run(16);
    budget.admit("cdn.example:443").await;
    drop(run);
    let left = budget.on_hand("cdn.example:443").expect("taken");
    assert!((left - 3.0).abs() < 1e-6, "{left}");
}

/// A host the player first fetches from just after an unpaced download
/// ended starts where the download left every host, and refills from
/// the moment it ended.
#[tokio::test(start_paused = true)]
async fn a_host_first_fetched_from_after_an_unpaced_download_starts_where_it_left_off() {
    let budget = HostBudget::fresh();
    drop(budget.unpaced_run(16));
    for _ in 0..4 {
        tokio::time::timeout(Duration::from_millis(1), budget.admit("seg.example:443"))
            .await
            .expect("within what the download left");
    }
    assert!(
        tokio::time::timeout(SEGMENT_REFILL / 2, budget.admit("seg.example:443"))
            .await
            .is_err(),
        "the fifth waits"
    );
}

/// A host fetched from before an unpaced download ran, and not since,
/// is held when the download ends like any other: it does not keep the
/// full burst the download's requests have spent.
#[tokio::test(start_paused = true)]
async fn a_host_fetched_from_before_an_unpaced_download_is_held_when_it_ends() {
    let budget = HostBudget::fresh();
    budget.admit("cdn.example:443").await;
    drop(budget.unpaced_run(16));
    let left = budget.on_hand("cdn.example:443").expect("fetched from");
    assert!((left - 4.0).abs() < 1e-6, "{left}");
}

/// A request spent without waiting while an unpaced download runs
/// stays spent: the hold applies first, and the spend comes off what
/// it leaves.
#[test]
fn a_spend_beside_an_unpaced_download_comes_off_what_the_hold_leaves() {
    let budget = HostBudget::fresh();
    let _run = budget.unpaced_run(16);
    budget.spend("cdn.example:443", 2);
    let left = budget.on_hand("cdn.example:443").expect("spent");
    assert!((left - 2.0).abs() < 1e-3, "{left}");
}

/// Two unpaced downloads running together hold every host to what
/// their requests in flight leave between them, and the first to end
/// leaves every host there — a host fetched from before, and one not
/// fetched from yet — not where its own requests alone would have.
#[tokio::test(start_paused = true)]
async fn overlapping_unpaced_downloads_leave_every_host_where_both_held_it() {
    let budget = HostBudget::fresh();
    budget.admit("cdn.example:443").await;
    let first = budget.unpaced_run(8);
    let second = budget.unpaced_run(8);
    drop(first);
    drop(second);
    let left = budget.on_hand("cdn.example:443").expect("fetched from");
    assert!((left - 4.0).abs() < 1e-6, "fetched from before: {left}");
    for _ in 0..4 {
        tokio::time::timeout(Duration::from_millis(1), budget.admit("seg.example:443"))
            .await
            .expect("within what the downloads left");
    }
    assert!(
        tokio::time::timeout(SEGMENT_REFILL / 2, budget.admit("seg.example:443"))
            .await
            .is_err(),
        "a host not fetched from yet: the fifth waits"
    );
}

#[test]
fn background_traffic_leaves_the_reserve_in_the_bucket() {
    // Background traffic takes what the player is not using, never the
    // last tokens: with the reserve still in the bucket it waits, and
    // it waits exactly until one more token than the reserve is there.
    let now = Instant::now();
    let mut bucket = Bucket::full(SEGMENT_BURST, now);
    for _ in 0..(SEGMENT_BURST - BACKGROUND_RESERVE) {
        assert_eq!(
            take_leaving(
                &mut bucket,
                now,
                SEGMENT_BURST,
                SEGMENT_REFILL,
                BACKGROUND_RESERVE
            ),
            None
        );
    }
    let wait = take_leaving(
        &mut bucket,
        now,
        SEGMENT_BURST,
        SEGMENT_REFILL,
        BACKGROUND_RESERVE,
    )
    .expect("the reserve is not background traffic's");
    assert!(
        wait >= SEGMENT_REFILL && wait <= SEGMENT_REFILL + Duration::from_millis(2),
        "{wait:?}"
    );
    // The player still has the reserve.
    for _ in 0..BACKGROUND_RESERVE {
        assert_eq!(take(&mut bucket, now, SEGMENT_BURST, SEGMENT_REFILL), None);
    }
}

proptest! {
    /// Whatever the bucket holds and however long since it was last
    /// topped up, a background take that succeeds leaves at least the
    /// reserve behind.
    #[test]
    fn a_background_take_never_dips_into_the_reserve(
        spent in 0u32..=SEGMENT_BURST,
        elapsed_ms in 0u64..60_000,
        reserve in 0u32..SEGMENT_BURST,
    ) {
        let start = Instant::now();
        let mut bucket = Bucket::full(SEGMENT_BURST, start);
        for _ in 0..spent {
            let _ = take(&mut bucket, start, SEGMENT_BURST, SEGMENT_REFILL);
        }
        let now = start + Duration::from_millis(elapsed_ms);
        if take_leaving(&mut bucket, now, SEGMENT_BURST, SEGMENT_REFILL, reserve).is_none() {
            prop_assert!(bucket.tokens() >= f64::from(reserve));
        }
    }
}

/// The budget the app builds keeps the reserve: background traffic
/// takes the burst down to it, and the player's request after that is
/// served at once.
#[tokio::test(start_paused = true)]
async fn the_apps_budget_keeps_the_players_reserve_from_background_traffic() {
    let budget = HostBudget::fresh();
    for _ in 0..(SEGMENT_BURST - BACKGROUND_RESERVE) {
        budget.admit_background("cdn.example:443").await;
    }
    let start = tokio::time::Instant::now();
    for _ in 0..BACKGROUND_RESERVE {
        budget.admit("cdn.example:443").await;
    }
    assert_eq!(
        tokio::time::Instant::now(),
        start,
        "the reserve was the player's"
    );
}

/// Counts the tokens a player that keeps one request waiting at all
/// times and `background` concurrent background fetchers take at one
/// host over `span`.
async fn shares(budget: Arc<HostBudget>, background: usize, span: Duration) -> (usize, usize) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let player_tokens = Arc::new(AtomicUsize::new(0));
    let background_tokens = Arc::new(AtomicUsize::new(0));
    let mut tasks = Vec::new();
    {
        let (budget, count) = (Arc::clone(&budget), Arc::clone(&player_tokens));
        tasks.push(tokio::spawn(async move {
            loop {
                budget.admit("cdn.example:443").await;
                count.fetch_add(1, Ordering::SeqCst);
            }
        }));
    }
    tokio::time::sleep(Duration::from_millis(10)).await;
    for _ in 0..background {
        let (budget, count) = (Arc::clone(&budget), Arc::clone(&background_tokens));
        tasks.push(tokio::spawn(async move {
            loop {
                budget.admit_background("cdn.example:443").await;
                count.fetch_add(1, Ordering::SeqCst);
            }
        }));
    }
    tokio::time::sleep(span).await;
    for task in &tasks {
        task.abort();
    }
    (
        player_tokens.load(Ordering::SeqCst),
        background_tokens.load(Ordering::SeqCst),
    )
}

/// A player filling its buffer asks back to back, so the line is never
/// empty, and background traffic that only took unused tokens took
/// none for as long as the buffer filled — a download beside a player
/// that had just started, or just sought, stood still for a minute.
/// Background traffic waiting beside it is served at the next turn.
#[tokio::test(start_paused = true)]
async fn background_traffic_beside_a_waiting_player_is_served_at_the_next_turn() {
    let budget = Arc::new(HostBudget::new(1, Duration::from_millis(500)));
    budget.admit("cdn.example:443").await;
    let player = {
        let budget = Arc::clone(&budget);
        tokio::spawn(async move {
            loop {
                budget.admit("cdn.example:443").await;
            }
        })
    };
    tokio::time::sleep(Duration::from_millis(10)).await;
    let served = tokio::time::timeout(
        Duration::from_millis(500) * 3,
        budget.admit_background("cdn.example:443"),
    )
    .await;
    player.abort();
    assert!(served.is_ok(), "served after the player's next token");
}

/// While both wait, the player and background traffic take turns, and
/// however many background fetches are in flight they share the one
/// turn: a download running four fragments at once takes no more of
/// the host than one running one.
#[tokio::test(start_paused = true)]
async fn beside_a_waiting_player_background_traffic_takes_every_other_token() {
    for background in [1, 4] {
        let budget = Arc::new(HostBudget::new(1, Duration::from_millis(500)));
        budget.admit("cdn.example:443").await;
        let (player, taken) = shares(budget, background, Duration::from_secs(30)).await;
        assert!(
            taken > 0 && taken.abs_diff(player) <= 2,
            "{background} background fetchers: the player took {player}, background {taken}"
        );
    }
}

/// With no player request waiting, background traffic still leaves the
/// reserve: however many background fetches are in flight, a player
/// asking now and then is served at once.
#[tokio::test(start_paused = true)]
async fn many_background_fetches_leave_the_reserve_to_a_player_asking_now_and_then() {
    let budget = HostBudget::fresh();
    let fetchers: Vec<_> = (0..4)
        .map(|_| {
            let budget = Arc::clone(&budget);
            tokio::spawn(async move {
                loop {
                    budget.admit_background("cdn.example:443").await;
                }
            })
        })
        .collect();
    for request in 0..20 {
        tokio::time::sleep(Duration::from_secs(6)).await;
        let start = tokio::time::Instant::now();
        budget.admit("cdn.example:443").await;
        assert_eq!(
            start.elapsed(),
            Duration::ZERO,
            "the player's request {request} waited"
        );
    }
    for fetcher in &fetchers {
        fetcher.abort();
    }
}

/// A background fetch given up while it waits — its request dropped —
/// leaves no turn behind it: the player goes on at the refill rate.
#[tokio::test(start_paused = true)]
async fn a_background_fetch_given_up_while_waiting_holds_no_turn() {
    let budget = Arc::new(HostBudget::new(1, Duration::from_millis(500)));
    budget.admit("cdn.example:443").await;
    let player = {
        let budget = Arc::clone(&budget);
        tokio::spawn(async move {
            loop {
                budget.admit("cdn.example:443").await;
            }
        })
    };
    tokio::time::sleep(Duration::from_millis(10)).await;
    let _ = tokio::time::timeout(
        Duration::from_millis(700),
        budget.admit_background("cdn.example:443"),
    )
    .await;
    player.abort();
    let start = tokio::time::Instant::now();
    tokio::time::timeout(Duration::from_millis(1100), async {
        budget.admit("cdn.example:443").await;
        budget.admit("cdn.example:443").await;
    })
    .await
    .expect("two refills' worth of player requests in two refills");
    assert!(start.elapsed() >= Duration::from_millis(500));
}
