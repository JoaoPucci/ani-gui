//! The retry bookkeeping: one ticket per item while it waits, a new one
//! after it ended, a bounded memory of the ended, and the step a retry
//! takes after each run.

use std::time::{Duration, Instant};

use super::registry::{
    after_run, Registry, Standing, Step, MAX_PENDING, MAX_RETRIES, ON_SCREEN_GRACE, REMEMBERED,
};

fn now() -> Instant {
    Instant::now()
}

#[test]
fn an_item_held_twice_waits_on_one_ticket() {
    let mut r = Registry::default();
    let (a, fresh_a) = r.hold("thumbs:10083", now()).expect("room");
    let (b, fresh_b) = r.hold("thumbs:10083", now()).expect("room");
    assert_eq!(a, b);
    assert!(fresh_a);
    assert!(!fresh_b, "the second hold joins the first retry");
    assert_eq!(r.pending("thumbs:10083"), Some(a));
    assert_eq!(r.standing(a), Standing::Pending);
}

#[test]
fn different_items_wait_on_different_tickets() {
    let mut r = Registry::default();
    let (a, _) = r.hold("thumbs:1", now()).expect("room");
    let (b, _) = r.hold("thumbs:2", now()).expect("room");
    assert_ne!(a, b);
}

#[test]
fn an_item_held_after_its_retry_ended_is_a_new_item() {
    let mut r = Registry::default();
    let (a, _) = r.hold("detail:1", now()).expect("room");
    r.end("detail:1", a, true);
    assert_eq!(r.standing(a), Standing::Ended(true));
    assert_eq!(r.pending("detail:1"), None);
    let (b, fresh) = r.hold("detail:1", now()).expect("room");
    assert!(fresh);
    assert_ne!(a, b);
}

#[test]
fn an_unissued_ticket_has_ended_unlanded() {
    let r = Registry::default();
    assert_eq!(r.standing(7), Standing::Ended(false));
}

#[test]
fn ending_a_ticket_the_key_no_longer_holds_changes_nothing() {
    let mut r = Registry::default();
    let (a, _) = r.hold("k", now()).expect("room");
    r.end("k", a, false);
    let (b, _) = r.hold("k", now()).expect("room");
    r.end("k", a, true);
    assert_eq!(r.standing(b), Standing::Pending);
    assert_eq!(r.standing(a), Standing::Ended(false));
}

#[test]
fn the_oldest_ended_tickets_are_forgotten_past_the_bound() {
    let mut r = Registry::default();
    let mut tickets = Vec::new();
    for i in 0..=REMEMBERED {
        let key = format!("k{i}");
        let (t, _) = r.hold(&key, now()).expect("room");
        r.end(&key, t, true);
        tickets.push(t);
    }
    assert_eq!(r.standing(tickets[0]), Standing::Ended(false));
    assert_eq!(r.standing(tickets[REMEMBERED]), Standing::Ended(true));
}

#[test]
fn a_retry_held_back_again_runs_once_per_pause_up_to_its_bound() {
    for attempt in 1..MAX_RETRIES {
        assert_eq!(after_run(false, true, attempt), Step::Again);
    }
    assert_eq!(after_run(false, true, MAX_RETRIES), Step::GiveUp);
    assert_eq!(after_run(false, false, 1), Step::GiveUp);
    assert_eq!(after_run(true, true, 1), Step::Landed);
}

#[test]
fn a_new_ticket_is_on_screen_until_its_grace_passes_unwatched() {
    let mut r = Registry::default();
    let t0 = Instant::now();
    let (t, _) = r.hold("k", t0).expect("room");
    assert!(r.on_screen(t, t0));
    assert!(!r.on_screen(t, t0 + ON_SCREEN_GRACE));
}

#[test]
fn a_waiting_page_keeps_its_ticket_on_screen_and_a_release_takes_it_off() {
    let mut r = Registry::default();
    let t0 = Instant::now();
    let (t, _) = r.hold("k", t0).expect("room");
    let era = r.watch(t, t0);
    let later = t0 + ON_SCREEN_GRACE * 10;
    assert!(r.on_screen(t, later), "a page still waits");
    r.unwatch(t, era, later);
    assert!(
        r.on_screen(t, later + Duration::from_secs(1)),
        "between two waits"
    );
    r.release(t);
    assert!(!r.on_screen(t, later + Duration::from_secs(1)));
    r.watch(t, later);
    assert!(r.on_screen(t, later), "a page waits on it again");
}

/// A page that closes while its wait is still open: the renderer drops
/// the wait, but the backend's handler may hold it up to 20 s longer.
/// The release takes the fill off screen at once, so the page now shown
/// goes first.
#[test]
fn a_release_takes_a_fill_off_screen_while_its_wait_is_still_open() {
    let mut r = Registry::default();
    let t0 = Instant::now();
    let (left, _) = r.hold("left", t0).expect("room");
    r.watch(left, t0);
    let later = t0 + ON_SCREEN_GRACE * 2;
    r.release(left);
    assert!(!r.on_screen(left, later), "its page has gone");
    let (shown, _) = r.hold("shown", later).expect("room");
    r.watch(shown, later);
    assert!(r.may_go(shown, later));
    assert!(!r.may_go(left, later), "the page on screen goes first");
}

#[test]
fn a_fill_no_page_shows_goes_only_while_no_fill_on_screen_waits() {
    let mut r = Registry::default();
    let t0 = Instant::now();
    let (left, _) = r.hold("left", t0).expect("room");
    r.release(left);
    assert!(r.may_go(left, t0), "nothing on screen waits");
    let (shown, _) = r.hold("shown", t0).expect("room");
    r.watch(shown, t0);
    assert!(!r.may_go(left, t0), "a fill on screen goes first");
    assert!(r.may_go(shown, t0));
    r.end("shown", shown, true);
    assert!(r.may_go(left, t0));
}

#[test]
fn the_queue_is_bounded() {
    let mut r = Registry::default();
    for i in 0..MAX_PENDING {
        assert!(r.hold(&format!("k{i}"), now()).is_some());
    }
    assert!(r.hold("one-more", now()).is_none());
    assert!(
        r.hold("k0", now()).is_some(),
        "a held item still joins its retry"
    );
}

proptest::proptest! {
    /// Whatever the order of holds and ends, a key waits on at most one
    /// ticket, every ticket is issued once, a pending ticket reads
    /// pending and an ended one reads as it ended.
    #[test]
    fn a_key_waits_on_one_ticket_at_a_time(
        ops in proptest::collection::vec((0u8..4, proptest::bool::ANY, proptest::bool::ANY), 0..64),
    ) {
        let mut r = Registry::default();
        let mut issued = std::collections::HashSet::new();
        for (k, end, landed) in ops {
            let key = format!("k{k}");
            if end {
                if let Some(t) = r.pending(&key) {
                    r.end(&key, t, landed);
                    proptest::prop_assert_eq!(r.standing(t), Standing::Ended(landed));
                    proptest::prop_assert_eq!(r.pending(&key), None);
                }
            } else {
                let before = r.pending(&key);
                let (t, fresh) = r.hold(&key, now()).expect("room");
                proptest::prop_assert_eq!(fresh, before.is_none());
                if fresh {
                    proptest::prop_assert!(issued.insert(t), "ticket {} issued twice", t);
                } else {
                    proptest::prop_assert_eq!(Some(t), before);
                }
                proptest::prop_assert_eq!(r.standing(t), Standing::Pending);
            }
        }
    }

    /// The number of runs a held-back fill gets is bounded whatever the
    /// gate says: run until a step other than `Again`, and that is never
    /// past the bound.
    #[test]
    fn a_retry_runs_at_most_max_retries_times(
        landed_at in proptest::option::of(1u32..10),
        paused in proptest::collection::vec(proptest::bool::ANY, 10),
    ) {
        let mut runs = 0;
        for attempt in 1.. {
            runs += 1;
            let landed = landed_at == Some(attempt);
            match after_run(landed, paused[(attempt as usize - 1) % paused.len()], attempt) {
                Step::Again => {}
                Step::Landed => {
                    proptest::prop_assert!(landed);
                    break;
                }
                Step::GiveUp => {
                    proptest::prop_assert!(!landed);
                    break;
                }
            }
        }
        proptest::prop_assert!(runs <= MAX_RETRIES);
    }

    /// Whatever pages watch, release and come back, a fill no page shows
    /// never ranks ahead of one a page shows.
    #[test]
    fn a_fill_off_screen_never_ranks_ahead_of_one_on_screen(
        ops in proptest::collection::vec((0u8..6, 0u8..3), 0..64),
        at_ms in 0u64..20_000,
    ) {
        let t0 = Instant::now();
        let mut r = Registry::default();
        let mut tickets = Vec::new();
        for k in 0..6 {
            tickets.push(r.hold(&format!("k{k}"), t0).expect("room").0);
        }
        let mut open: Vec<Vec<u64>> = vec![Vec::new(); tickets.len()];
        for (k, op) in ops {
            let t = tickets[k as usize];
            match op {
                0 => open[k as usize].push(r.watch(t, t0)),
                1 => {
                    if let Some(era) = open[k as usize].pop() {
                        r.unwatch(t, era, t0);
                    }
                }
                _ => r.release(t),
            }
        }
        let now = t0 + Duration::from_millis(at_ms);
        let shown: Vec<u64> = tickets.iter().copied().filter(|&t| r.on_screen(t, now)).collect();
        for &t in &tickets {
            let goes = r.may_go(t, now);
            if r.on_screen(t, now) {
                proptest::prop_assert!(goes);
            } else {
                proptest::prop_assert_eq!(goes, shown.is_empty());
            }
        }
    }

    /// Whatever the order of waits starting, waits ending and releases,
    /// a ticket is on screen exactly while a wait that started after the
    /// last release is still open — or, within the grace, one such wait
    /// started or ended (or the ticket was issued) and no release came
    /// since. A wait still open across a release never keeps it there.
    #[test]
    fn on_screen_follows_the_waits_no_release_has_let_go(
        ops in proptest::collection::vec((0u8..3, 0usize..8, 0u64..3_000), 0..64),
    ) {
        let mut now = Instant::now();
        let mut r = Registry::default();
        let (t, _) = r.hold("k", now).expect("room");
        // Each open wait: the era `watch` gave it, and whether a release
        // has let it go since it started.
        let mut open: Vec<(u64, bool)> = Vec::new();
        let mut seen = Some(now);
        for (op, pick, gap_ms) in ops {
            now += Duration::from_millis(gap_ms);
            match op {
                0 => {
                    open.push((r.watch(t, now), false));
                    seen = Some(now);
                }
                1 if !open.is_empty() => {
                    let (era, released) = open.remove(pick % open.len());
                    r.unwatch(t, era, now);
                    if !released {
                        seen = Some(now);
                    }
                }
                1 => {}
                _ => {
                    r.release(t);
                    open.iter_mut().for_each(|w| w.1 = true);
                    seen = None;
                }
            }
            let live = open.iter().any(|&(_, released)| !released);
            let recent = seen.is_some_and(|s| now.saturating_duration_since(s) < ON_SCREEN_GRACE);
            proptest::prop_assert_eq!(r.on_screen(t, now), live || recent);
            proptest::prop_assert_eq!(r.on_screen(t, now + ON_SCREEN_GRACE), live);
        }
    }
}
