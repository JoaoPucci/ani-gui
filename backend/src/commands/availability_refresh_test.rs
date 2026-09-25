//! The write-ordering rules of the availability cache, stated as
//! examples and as properties. Mounted by `#[path]` beside the
//! module, so its complexity is counted as the tests' and not the
//! module's.

use super::*;
use crate::commands::availability::cache_key;

#[tokio::test]
async fn a_write_runs_while_the_row_is_still_held() {
    // The guard has to outlive the write, not just precede it.
    // `hold_if_still_ours(..).await.is_some()` reads as a check and
    // is really a drop: the row is free again by the time the write
    // runs, and on the multi-thread runtime production uses, a
    // refresh can take it and land its row in that gap — after
    // which the stale write replaces it for the row's whole TTL.
    //
    // So the write goes inside, where it cannot be separated from
    // the permission to make it.
    let refreshes = AvailabilityRefreshes::new();
    let key = cache_key("kid-w", "sub");
    let started_at = refreshes.generation(&key);

    let held_during_write = with_row_if_ours(&refreshes, &key, started_at, false, || {
        refreshes.for_row(&key).try_lock().is_err()
    })
    .await;

    assert_eq!(
        held_during_write,
        Some(true),
        "the row must still be locked while the write runs"
    );
    // And released afterwards, or the next writer for this show
    // would wait forever.
    assert!(refreshes.for_row(&key).try_lock().is_ok());
}

#[test]
fn an_ordinary_lookup_yields_to_a_refresh_that_landed_while_it_was_out() {
    let refreshes = AvailabilityRefreshes::new();
    let key = cache_key("kid-1", "sub");
    let started_at = refreshes.generation(&key);

    // The user clicked a dimmed tile and the cache-bypassing
    // lookup came back first, writing the fresh row.
    refreshes.bump(&key);

    // This one has been out since before that. Its answer is
    // older, and it read through the cache to get it — writing it
    // now puts the stale count back for the whole TTL, so the next
    // page visit restores the gate the refresh just cleared.
    assert!(!may_write_cache(&refreshes, &key, started_at, false));
}

#[test]
fn a_refresh_writes_even_when_another_refresh_landed_first() {
    let refreshes = AvailabilityRefreshes::new();
    let key = cache_key("kid-1", "sub");
    let started_at = refreshes.generation(&key);
    refreshes.bump(&key);

    // Both skipped the cache, so neither is the stale one — last
    // write wins is the right rule between them.
    assert!(may_write_cache(&refreshes, &key, started_at, true));
}

#[test]
fn an_undisturbed_lookup_still_writes() {
    let refreshes = AvailabilityRefreshes::new();
    let key = cache_key("kid-1", "sub");
    let started_at = refreshes.generation(&key);

    assert!(may_write_cache(&refreshes, &key, started_at, false));
}

proptest::proptest! {
    // The rule `may_write_cache` applies, stated over every shape
    // the two inputs can take rather than the four the examples
    // above pin:
    //
    //   • A refresh writes unconditionally. It skipped the cache,
    //     so no reading it could be holding is the stale one, and
    //     between two refreshes last-write-wins is correct.
    //   • An ordinary lookup writes exactly when no refresh has
    //     landed since it went out — never on a count that moved,
    //     always on one that did not.
    //
    // The predicate decides whether a cached cap outlives its own
    // correction, so "some refresh, some lookup, some interleaving"
    // is the honest domain to state it over.
    #[test]
    fn a_refresh_writes_whatever_happened_while_it_was_out(
        landed in 0usize..8,
    ) {
        let refreshes = AvailabilityRefreshes::new();
        let key = cache_key("kid-p", "sub");
        let started_at = refreshes.generation(&key);
        for _ in 0..landed {
            refreshes.bump(&key);
        }

        proptest::prop_assert!(may_write_cache(&refreshes, &key, started_at, true));
    }

    #[test]
    fn a_lookup_writes_exactly_when_no_refresh_landed(
        landed in 0usize..8,
    ) {
        let refreshes = AvailabilityRefreshes::new();
        let key = cache_key("kid-p", "sub");
        let started_at = refreshes.generation(&key);
        for _ in 0..landed {
            refreshes.bump(&key);
        }

        proptest::prop_assert_eq!(
            may_write_cache(&refreshes, &key, started_at, false),
            landed == 0
        );
    }

    #[test]
    fn only_a_refresh_of_this_very_row_takes_a_lookup_s_turn(
        id in "[a-z0-9]{1,6}",
        other_id in "[a-z0-9]{1,6}",
        noise in 0usize..6,
        on_this_row in proptest::bool::ANY,
    ) {
        // sub and dub are separate catalogues, and separate shows
        // are separate rows, so a refresh of one settles nothing
        // about any other. Whatever else lands, the answer tracks
        // this row and only this row.
        let refreshes = AvailabilityRefreshes::new();
        let key = cache_key(&id, "sub");
        let started_at = refreshes.generation(&key);
        for _ in 0..noise {
            refreshes.bump(&cache_key(&id, "dub"));
            refreshes.bump(&cache_key(&other_id, "dub"));
        }
        if on_this_row {
            refreshes.bump(&key);
        }

        proptest::prop_assert_eq!(
            may_write_cache(&refreshes, &key, started_at, false),
            !on_this_row
        );
    }
}

#[test]
fn generations_do_not_leak_between_shows_or_modes() {
    let refreshes = AvailabilityRefreshes::new();
    let sub = cache_key("kid-1", "sub");
    let dub = cache_key("kid-1", "dub");
    let other = cache_key("kid-2", "sub");
    let sub_started = refreshes.generation(&sub);

    refreshes.bump(&dub);
    refreshes.bump(&other);

    // A refresh for the dub catalogue, or for a different show,
    // says nothing about this row.
    assert!(may_write_cache(&refreshes, &sub, sub_started, false));
}
