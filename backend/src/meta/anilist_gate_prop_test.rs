//! Property coverage for the gate's ledger, beside the pure
//! scheduling in `anilist_gate.rs`.

use std::time::Duration;

use tokio::time::Instant;

use super::{Class, Ledger, Pace, ANILIST};

/// Every request sent the moment the ledger allows it, earliest first:
/// the most any scheduler built on [`Ledger::ready_at`] can send.
/// Returns each send's instant and class, in order.
fn schedule(pace: &Pace, base: Instant, arrivals: &[(u64, bool)]) -> Vec<(Instant, Class)> {
    let mut pending: Vec<(Instant, Class)> = arrivals
        .iter()
        .map(|&(ms, background)| {
            let class = if background {
                Class::Background
            } else {
                Class::Interactive
            };
            (base + Duration::from_millis(ms), class)
        })
        .collect();
    let mut ledger = Ledger::default();
    let mut sent = Vec::new();
    while !pending.is_empty() {
        let (i, at) = pending
            .iter()
            .enumerate()
            .map(|(i, &(arrived, class))| (i, ledger.ready_at(pace, class, arrived)))
            .min_by_key(|&(_, at)| at)
            .expect("pending is not empty");
        let (_, class) = pending.remove(i);
        ledger.record_send(pace, at);
        sent.push((at, class));
    }
    sent
}

/// A pace small enough for a property run to fill its windows.
fn small_pace(capacity: usize, reserve: usize, window_ms: u64, gap_ms: u64) -> Pace {
    Pace {
        window: Duration::from_millis(window_ms),
        capacity,
        reserve,
        min_gap: Duration::from_millis(gap_ms),
        ..ANILIST
    }
}

proptest::proptest! {
    /// However requests arrive, no window ever holds more than the
    /// capacity, no window ending at a background send holds more than
    /// the capacity less the reserve, and no two sends are closer than
    /// the gap.
    #[test]
    fn no_window_ever_holds_more_than_the_pace_allows(
        capacity in 1usize..12,
        reserve in 0usize..6,
        window_ms in 1u64..5_000,
        gap_ms in 0u64..300,
        mut arrivals in proptest::collection::vec((0u64..20_000, proptest::bool::ANY), 0..60),
    ) {
        arrivals.sort_unstable();
        let pace = small_pace(capacity, reserve, window_ms, gap_ms);
        let base = Instant::now();
        let sent = schedule(&pace, base, &arrivals);
        proptest::prop_assert_eq!(sent.len(), arrivals.len());
        for (i, &(at, class)) in sent.iter().enumerate() {
            let in_window = sent[..=i]
                .iter()
                .filter(|&&(s, _)| s + pace.window > at)
                .count();
            proptest::prop_assert!(in_window <= capacity, "{in_window} sends in one window, capacity {capacity}");
            if class == Class::Background {
                let cap = capacity.saturating_sub(reserve).max(1);
                proptest::prop_assert!(in_window <= cap, "a background send made {in_window} in one window, allowed {cap}");
            }
            if i > 0 {
                proptest::prop_assert!(at - sent[i - 1].0 >= pace.min_gap);
            }
        }
    }

    /// AniList's own pace keeps every minute under the 30 requests the
    /// degraded API allows, whatever the burst.
    #[test]
    fn anilist_pace_keeps_every_minute_under_thirty(
        mut arrivals in proptest::collection::vec((0u64..180_000, proptest::bool::ANY), 0..120),
    ) {
        arrivals.sort_unstable();
        let base = Instant::now();
        let sent = schedule(&ANILIST, base, &arrivals);
        for &(at, _) in &sent {
            let in_minute = sent
                .iter()
                .filter(|&&(s, _)| s <= at && s + Duration::from_secs(60) > at)
                .count();
            proptest::prop_assert!(in_minute < 30, "{in_minute} requests in one minute");
        }
    }

    /// Once a cool-down is recorded, nothing of either class may leave
    /// before it ends, and a shorter cool-down recorded later does not
    /// cut it short.
    #[test]
    fn nothing_leaves_inside_a_cool_down(
        until_ms in 0u64..120_000,
        later_ms in 0u64..120_000,
        now_ms in 0u64..120_000,
        background in proptest::bool::ANY,
        sends in proptest::collection::vec(0u64..120_000, 0..30),
    ) {
        let base = Instant::now();
        let mut ledger = Ledger::default();
        let mut sends = sends;
        sends.sort_unstable();
        for ms in sends {
            ledger.record_send(&ANILIST, base + Duration::from_millis(ms));
        }
        let until = base + Duration::from_millis(until_ms);
        ledger.cool_down(until);
        ledger.cool_down(base + Duration::from_millis(later_ms));
        let class = if background { Class::Background } else { Class::Interactive };
        let now = base + Duration::from_millis(now_ms);
        let at = ledger.ready_at(&ANILIST, class, now);
        proptest::prop_assert!(at >= until);
        proptest::prop_assert!(at >= now);
    }
}
