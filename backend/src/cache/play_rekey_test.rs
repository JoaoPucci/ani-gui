//! Tests for `crate::cache::play_rekey`. Sibling file loaded via
//! `#[path]`; the `_test.rs` suffix keeps it out of the module's CRAP
//! roll-up (`tools/crap-score.mjs`).

use super::*;
use crate::commands::play_native_versions::VersionKind;
use crate::commands::play_resolution_cache::cache_key_for_version;

/// The key a v28 build wrote a play row under, quality and all.
fn old_key(
    title: &str,
    mode: &str,
    quality: &str,
    episode: &str,
    (year, count, subtype): (Option<u32>, Option<u32>, Option<&str>),
    version: Option<VersionKind>,
) -> String {
    let dash = |v: Option<u32>| v.map_or_else(|| "-".to_string(), |v| v.to_string());
    let key = format!(
        "play:v28:{title}:{mode}:{quality}:{episode}:{}:{}:{}",
        dash(year),
        dash(count),
        subtype.unwrap_or("-")
    );
    match version {
        None => key,
        Some(v) => format!("{key}:{}", v.as_str()),
    }
}

/// The key this build reads for the same request.
fn new_key(
    title: &str,
    mode: &str,
    episode: &str,
    (year, count, subtype): (Option<u32>, Option<u32>, Option<&str>),
    version: Option<VersionKind>,
) -> String {
    cache_key_for_version(title, mode, episode, year, count, subtype, version)
}

#[allow(clippy::type_complexity)]
fn request() -> impl proptest::strategy::Strategy<
    Value = (
        String,
        String,
        String,
        (Option<u32>, Option<u32>, Option<String>),
        Option<VersionKind>,
    ),
> {
    (
        // Titles may carry colons (Stone Ocean Part 2 does), and may
        // even end in a word another axis uses.
        proptest::prop_oneof![
            "[a-zA-Z0-9 :'-]{0,16}",
            proptest::strategy::Just("Show: sub:best".to_string()),
            proptest::strategy::Just("Standard".to_string()),
        ],
        proptest::prop_oneof![
            proptest::strategy::Just("sub".to_string()),
            proptest::strategy::Just("dub".to_string()),
        ],
        "[0-9]{1,4}(\\.[0-9]{1,2})?",
        (
            proptest::option::of(0u32..3000),
            proptest::option::of(0u32..5000),
            proptest::option::of(proptest::prop_oneof![
                proptest::strategy::Just("TV".to_string()),
                proptest::strategy::Just("movie".to_string()),
                proptest::strategy::Just("OVA".to_string()),
                proptest::strategy::Just("ONA".to_string()),
                proptest::strategy::Just("special".to_string()),
                proptest::strategy::Just("music".to_string()),
            ]),
        ),
        proptest::option::of(proptest::prop_oneof![
            proptest::strategy::Just(VersionKind::Standard),
            proptest::strategy::Just(VersionKind::Uncensored),
        ]),
    )
}

proptest::proptest! {
    /// A row kept for `best` held the master, which is what every play
    /// asks for now: it moves to the key the same request reads.
    #[test]
    fn a_best_row_moves_to_the_key_its_request_reads(r in request()) {
        let (title, mode, episode, (y, e, s), version) = r;
        let axes = (y, e, s.as_deref());
        proptest::prop_assert_eq!(
            rekeyed(&old_key(&title, &mode, "best", &episode, axes, version)),
            Some(new_key(&title, &mode, &episode, axes, version))
        );
    }

    /// A row kept for any other quality holds one rendition, which the
    /// player could not choose a level of: it is not carried, and the
    /// launch's reclaim deletes it.
    #[test]
    fn a_row_for_another_quality_is_not_carried(
        r in request(),
        quality in proptest::prop_oneof![
            proptest::strategy::Just("worst".to_string()),
            proptest::strategy::Just("1080".to_string()),
            proptest::strategy::Just("720".to_string()),
            proptest::strategy::Just("480".to_string()),
        ],
    ) {
        let (title, mode, episode, (y, e, s), version) = r;
        let axes = (y, e, s.as_deref());
        proptest::prop_assert_eq!(
            rekeyed(&old_key(&title, &mode, &quality, &episode, axes, version)),
            None
        );
    }

    /// A key of this build's own family is never moved.
    #[test]
    fn a_current_key_is_left_alone(r in request()) {
        let (title, mode, episode, (y, e, s), version) = r;
        proptest::prop_assert_eq!(
            rekeyed(&new_key(&title, &mode, &episode, (y, e, s.as_deref()), version)),
            None
        );
    }
}

#[test]
fn keys_that_are_not_a_play_row_are_left_alone() {
    for key in [
        "",
        "play:v28:",
        "play:v28:Show",
        "play:v28:Show:sub:best",
        "play:v28:Show:raw:best:1:-:-:-",
        "play:v27:Show:sub:best:1:-:-:-",
        "availability:v25:12:sub",
    ] {
        assert_eq!(rekeyed(key), None, "{key:?}");
    }
}

const TTL: i64 = 7 * 24 * 3600;

fn put(conn: &Connection, key: &str, body: &str, at: i64) {
    conn.execute(
        "INSERT OR REPLACE INTO meta_cache(key, body, fetched_at, ttl_seconds) \
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![key, body, at, TTL],
    )
    .unwrap();
}

fn row(conn: &Connection, key: &str) -> Option<(String, i64, i64)> {
    conn.query_row(
        "SELECT body, fetched_at, ttl_seconds FROM meta_cache WHERE key = ?1",
        [key],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .ok()
}

fn plain() -> (Option<u32>, Option<u32>, Option<&'static str>) {
    (None, Some(12), Some("TV"))
}

#[test]
fn opening_the_cache_carries_a_best_row_with_its_moment_and_lifetime() {
    assert_ne!(
        new_key("Show", "sub", "3", plain(), None),
        old_key("Show", "sub", "best", "3", plain(), None),
        "the row moves to a key of its own"
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    {
        let mut conn = Connection::open(&path).unwrap();
        crate::cache::schema::run_migrations(&mut conn).unwrap();
        put(
            &conn,
            &old_key("Show", "sub", "best", "3", plain(), None),
            "master",
            1_000,
        );
        put(
            &conn,
            &old_key("Show", "sub", "720", "4", plain(), None),
            "rendition",
            1_000,
        );
    }
    let pool = crate::cache::open_pool(&path).unwrap();
    let conn = pool.get().unwrap();
    assert_eq!(
        row(&conn, &new_key("Show", "sub", "3", plain(), None)),
        Some(("master".to_string(), 1_000, TTL))
    );
    assert_eq!(
        row(&conn, &new_key("Show", "sub", "4", plain(), None)),
        None,
        "a rendition's row is not an answer for a play"
    );
}

#[test]
fn a_row_this_build_wrote_is_kept_over_an_older_one() {
    let pool = crate::cache::open_in_memory().unwrap();
    let mut conn = pool.get().unwrap();
    put(
        &conn,
        &old_key("Show", "dub", "best", "1", plain(), None),
        "older",
        1_000,
    );
    put(
        &conn,
        &new_key("Show", "dub", "1", plain(), None),
        "fresh",
        2_000,
    );
    carry_play_rows(&mut conn).unwrap();
    assert_eq!(
        row(&conn, &new_key("Show", "dub", "1", plain(), None)),
        Some(("fresh".to_string(), 2_000, TTL))
    );
}

#[test]
fn a_version_row_is_carried_under_its_version() {
    let pool = crate::cache::open_in_memory().unwrap();
    let mut conn = pool.get().unwrap();
    let uncensored = Some(VersionKind::Uncensored);
    put(
        &conn,
        &old_key("Show", "sub", "best", "2", plain(), uncensored),
        "uncut",
        1_000,
    );
    carry_play_rows(&mut conn).unwrap();
    assert_eq!(
        row(&conn, &new_key("Show", "sub", "2", plain(), uncensored)),
        Some(("uncut".to_string(), 1_000, TTL))
    );
    assert_eq!(
        row(&conn, &new_key("Show", "sub", "2", plain(), None)),
        None
    );
}

#[test]
fn a_row_that_cannot_be_read_is_left_and_the_cache_still_opens() {
    // Carrying is an optimisation: a row SQLite holds in a shape the
    // read refuses (a blob body) is skipped, the rows beside it are
    // carried, and opening the cache does not fail over it.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    {
        let mut conn = Connection::open(&path).unwrap();
        crate::cache::schema::run_migrations(&mut conn).unwrap();
        conn.execute(
            "INSERT INTO meta_cache(key, body, fetched_at, ttl_seconds) VALUES (?1, X'00ff', 1000, ?2)",
            rusqlite::params![old_key("Odd", "sub", "best", "1", plain(), None), TTL],
        )
        .unwrap();
        put(
            &conn,
            &old_key("Show", "sub", "best", "5", plain(), None),
            "master",
            1_000,
        );
    }
    let pool = crate::cache::open_pool(&path).expect("the cache opens");
    let conn = pool.get().unwrap();
    assert_eq!(
        row(&conn, &new_key("Show", "sub", "5", plain(), None)),
        Some(("master".to_string(), 1_000, TTL))
    );
}

#[test]
fn a_carry_sqlite_refuses_rolls_back_and_the_cache_still_opens() {
    // A write the carry cannot make fails the whole pass: nothing is
    // carried, the rows stay where they were for the reclaim, and
    // opening the cache goes on, as it does when the reclaim is refused.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    let old = old_key("Show", "sub", "best", "6", plain(), None);
    {
        let mut conn = Connection::open(&path).unwrap();
        crate::cache::schema::run_migrations(&mut conn).unwrap();
        put(&conn, &old, "master", 1_000);
        conn.execute_batch(
            "CREATE TRIGGER refuse_carry BEFORE INSERT ON meta_cache \
             WHEN substr(NEW.key, 1, 9) = 'play:v29:' \
             BEGIN SELECT RAISE(ABORT, 'refused'); END;",
        )
        .unwrap();
    }
    let pool = crate::cache::open_pool(&path).expect("the cache opens");
    let conn = pool.get().unwrap();
    assert_eq!(
        row(&conn, &new_key("Show", "sub", "6", plain(), None)),
        None
    );
    assert_eq!(row(&conn, &old), Some(("master".to_string(), 1_000, TTL)));
}
