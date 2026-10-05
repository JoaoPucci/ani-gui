//! Tests for `crate::commands::availability_reschedule`.

use super::*;
use crate::commands::availability::{cache_key, AvailabilityResponse};
use crate::commands::availability_ttl::{FLOOR_SECS, GRACE_SECS, NEGATIVE_GRACE_SECS};

const HOUR: u64 = 60 * 60;
const DAY: u64 = 24 * HOUR;

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn body(available: bool) -> String {
    serde_json::to_string(&AvailabilityResponse {
        available,
        episode_count: available.then_some(7),
        extra_episodes: Vec::new(),
        episode_count_approximate: false,
        gate_refused: false,
        provider: None,
    })
    .expect("serializes")
}

/// A row as it stands: `age` seconds old, `ttl` total.
fn seed(pool: &SqlitePool, key: &str, body: &str, age: u64, ttl: u64) {
    let conn = pool.get().expect("conn");
    conn.execute(
        "INSERT OR REPLACE INTO meta_cache(key, body, fetched_at, ttl_seconds) \
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            key,
            body,
            i64::try_from(now() - age).expect("fits"),
            i64::try_from(ttl).expect("fits")
        ],
    )
    .expect("seed");
}

fn ttl_of(pool: &SqlitePool, key: &str) -> u64 {
    let conn = pool.get().expect("conn");
    let ttl: i64 = conn
        .query_row(
            "SELECT ttl_seconds FROM meta_cache WHERE key = ?1",
            [key],
            |r| r.get(0),
        )
        .expect("row");
    u64::try_from(ttl).expect("non-negative")
}

fn pool() -> SqlitePool {
    crate::cache::open_in_memory().expect("pool")
}

#[test]
fn a_positive_row_written_before_the_schedule_is_cut_in_both_modes() {
    let pool = pool();
    for mode in ["sub", "dub"] {
        seed(&pool, &cache_key("weekly", mode), &body(true), HOUR, DAY);
    }
    let t = now();
    cut_rows_at_next_airing(&pool, "weekly", Some(t + 2 * HOUR), t);
    for mode in ["sub", "dub"] {
        // An hour old, then two hours to the drop plus the grace.
        let ttl = ttl_of(&pool, &cache_key("weekly", mode));
        assert!(
            ttl.abs_diff(HOUR + 2 * HOUR + GRACE_SECS) <= 2,
            "{mode}: {ttl}"
        );
    }
}

#[test]
fn a_passed_airing_leaves_the_row_the_floor() {
    let pool = pool();
    let key = cache_key("stale", "sub");
    seed(&pool, &key, &body(true), HOUR, DAY);
    let t = now();
    cut_rows_at_next_airing(&pool, "stale", Some(t - HOUR), t);
    assert!(ttl_of(&pool, &key).abs_diff(HOUR + FLOOR_SECS) <= 2);
}

#[test]
fn a_row_already_inside_the_cut_is_never_lengthened() {
    let pool = pool();
    let key = cache_key("soon", "sub");
    seed(&pool, &key, &body(true), 0, 2 * HOUR);
    let t = now();
    cut_rows_at_next_airing(&pool, "soon", Some(t + 5 * HOUR), t);
    assert_eq!(ttl_of(&pool, &key), 2 * HOUR);
}

#[test]
fn rows_the_cut_does_not_concern_are_left_alone() {
    let pool = pool();
    let finished = cache_key("done", "sub");
    let expired = cache_key("old", "sub");
    let other = cache_key("other", "sub");
    seed(&pool, &finished, &body(true), HOUR, 30 * DAY);
    seed(&pool, &expired, &body(true), 2 * DAY, DAY);
    seed(&pool, &other, &body(true), HOUR, DAY);
    let t = now();
    // A finished show has no next airing.
    cut_rows_at_next_airing(&pool, "done", None, t);
    cut_rows_at_next_airing(&pool, "old", Some(t + 2 * HOUR), t);
    assert_eq!(ttl_of(&pool, &finished), 30 * DAY);
    assert_eq!(ttl_of(&pool, &expired), DAY);
    assert_eq!(ttl_of(&pool, &other), DAY);
}

#[test]
fn no_schedule_leaves_the_row_alone() {
    let pool = pool();
    let key = cache_key("ona", "sub");
    seed(&pool, &key, &body(true), HOUR, DAY);
    let t = now();
    cut_rows_at_next_airing(&pool, "ona", None, t);
    assert_eq!(ttl_of(&pool, &key), DAY);
}

#[test]
fn a_missing_row_is_not_created() {
    let pool = pool();
    let t = now();
    cut_rows_at_next_airing(&pool, "none", Some(t + HOUR), t);
    assert_eq!(
        crate::cache::meta_cache_get(&pool, &cache_key("none", "sub")).expect("read"),
        None
    );
}

#[test]
fn a_cut_row_still_serves_its_body() {
    let pool = pool();
    let key = cache_key("weekly", "sub");
    seed(&pool, &key, &body(true), HOUR, DAY);
    let t = now();
    cut_rows_at_next_airing(&pool, "weekly", Some(t + 2 * HOUR), t);
    assert_eq!(
        crate::cache::meta_cache_get(&pool, &key).expect("read"),
        Some(body(true))
    );
}

// --- negative rows ----------------------------------------------------

#[test]
fn a_negative_row_written_before_the_schedule_is_cut_in_both_modes() {
    let pool = pool();
    for mode in ["sub", "dub"] {
        seed(&pool, &cache_key("lagged", mode), &body(false), HOUR, DAY);
    }
    let t = now();
    cut_rows_at_next_airing(&pool, "lagged", Some(t + 2 * HOUR), t);
    for mode in ["sub", "dub"] {
        // An hour old, then two hours to the drop plus the negative grace.
        let ttl = ttl_of(&pool, &cache_key("lagged", mode));
        assert!(
            ttl.abs_diff(HOUR + 2 * HOUR + NEGATIVE_GRACE_SECS) <= 2,
            "{mode}: {ttl}"
        );
    }
}

#[test]
fn a_negative_row_with_a_passed_airing_is_left_the_floor() {
    let pool = pool();
    let key = cache_key("lagged", "sub");
    seed(&pool, &key, &body(false), HOUR, DAY);
    let t = now();
    cut_rows_at_next_airing(&pool, "lagged", Some(t - HOUR), t);
    assert!(ttl_of(&pool, &key).abs_diff(HOUR + FLOOR_SECS) <= 2);
}

#[test]
fn negative_rows_the_cut_does_not_concern_are_left_alone() {
    let pool = pool();
    let finished = cache_key("gone", "sub");
    let expired = cache_key("stale-neg", "sub");
    let inside = cache_key("soon-neg", "sub");
    seed(&pool, &finished, &body(false), HOUR, 7 * DAY);
    seed(&pool, &expired, &body(false), 2 * DAY, DAY);
    seed(&pool, &inside, &body(false), 0, 2 * HOUR);
    let t = now();
    // A finished show has no next airing.
    cut_rows_at_next_airing(&pool, "gone", None, t);
    for id in ["stale-neg", "soon-neg"] {
        cut_rows_at_next_airing(&pool, id, Some(t + 5 * HOUR), t);
    }
    assert_eq!(ttl_of(&pool, &finished), 7 * DAY);
    assert_eq!(ttl_of(&pool, &expired), DAY);
    assert_eq!(ttl_of(&pool, &inside), 2 * HOUR);
    let none = cache_key("ona-neg", "sub");
    seed(&pool, &none, &body(false), HOUR, DAY);
    cut_rows_at_next_airing(&pool, "ona-neg", None, t);
    assert_eq!(ttl_of(&pool, &none), DAY);
}

#[test]
fn a_premiere_moved_earlier_cuts_the_negative_its_old_date_sized() {
    let pool = pool();
    let key = cache_key("moved", "sub");
    seed(&pool, &key, &body(false), HOUR, 5 * DAY);
    let t = now();
    cut_rows_at_next_airing(&pool, "moved", Some(t + 2 * HOUR), t);
    let ttl = ttl_of(&pool, &key);
    assert!(
        ttl.abs_diff(HOUR + 2 * HOUR + NEGATIVE_GRACE_SECS) <= 2,
        "{ttl}"
    );
}
