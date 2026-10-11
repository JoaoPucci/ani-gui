//! Tests for `crate::commands::play_resolution_cache`. Extracted via
//! `#[path]` so the module's complexity stays out of
//! `play_resolution_cache.rs`'s CRAP count.

use super::*;
use crate::cache::open_in_memory;

fn pool() -> SqlitePool {
    open_in_memory().expect("in-memory pool")
}

/// One full argument tuple for [`cache_key`], over its real
/// domains: titles may carry colons (Stone Ocean Part 2 does),
/// every other field is colon-free by construction — the enums,
/// digit episodes, and Kitsu subtypes the callers pass.
#[allow(clippy::type_complexity)]
fn axis() -> impl proptest::strategy::Strategy<
    Value = (
        String,
        String,
        String,
        String,
        Option<u32>,
        Option<u32>,
        Option<String>,
    ),
> {
    (
        "[a-zA-Z0-9 :'-]{0,16}",
        proptest::prop_oneof![
            proptest::strategy::Just("sub".to_string()),
            proptest::strategy::Just("dub".to_string()),
        ],
        proptest::prop_oneof![
            proptest::strategy::Just("best".to_string()),
            proptest::strategy::Just("worst".to_string()),
            proptest::strategy::Just("720".to_string()),
            proptest::strategy::Just("1080".to_string()),
        ],
        "[0-9]{1,4}(\\.[0-9])?",
        proptest::option::of(0u32..3000),
        proptest::option::of(0u32..5000),
        proptest::option::of(proptest::prop_oneof![
            proptest::strategy::Just("TV".to_string()),
            proptest::strategy::Just("movie".to_string()),
            proptest::strategy::Just("OVA".to_string()),
            proptest::strategy::Just("ONA".to_string()),
            proptest::strategy::Just("special".to_string()),
        ]),
    )
}

proptest::proptest! {
    /// Determinism and axis separation: two keys agree exactly
    /// when every axis agrees. Colons in the title cannot forge
    /// another tuple's key — every non-title field is colon-free,
    /// so keys with differing colon counts differ as strings and
    /// keys with equal counts align positionally.
    #[test]
    fn keys_are_deterministic_and_separate_every_axis(a in axis(), b in axis()) {
        let key = |x: &(
            String,
            String,
            String,
            String,
            Option<u32>,
            Option<u32>,
            Option<String>,
        )| {
            cache_key(&x.0, &x.1, &x.2, &x.3, x.4, x.5, x.6.as_deref())
        };
        proptest::prop_assert_eq!(key(&a), key(&a));
        proptest::prop_assert_eq!(key(&a) == key(&b), a == b);
    }
}

fn sample_resolution() -> CachedResolution {
    CachedResolution {
        upstream_url:
            "https://video.wixstatic.com/video/3d2d69_c12bd6c53e234420b3ae3d3b4c5b526f/1080p/mp4/file.mp4"
                .into(),
        referer: "https://allmanga.to".into(),
        media_kind: MediaKind::Mp4,
        show_id: "vDTSJHSpYnrkZnAvG".into(),
        show_title: "Naruto: Shippuuden (500 episodes)".into(),
        resolved_slot: None,
        subtitles: Vec::new(),
        kitsu_id: None,
    }
}

#[test]
fn cache_key_is_deterministic_for_the_same_inputs() {
    let a = cache_key("One Piece", "sub", "best", "1", None, None, None);
    let b = cache_key("One Piece", "sub", "best", "1", None, None, None);
    assert_eq!(a, b);
}

#[test]
fn cache_key_separates_subtypes() {
    // Subtype changes candidate selection (the movie-vs-special
    // disproof), so it is a key axis: two one-video entries with
    // identical other axes but different Kitsu formats must not
    // share a row, or the second request serves the first entry's
    // HEAD-valid stream without the disproof ever running.
    assert_ne!(
        cache_key(
            "Konoha Gakuen",
            "sub",
            "best",
            "1",
            None,
            Some(1),
            Some("special")
        ),
        cache_key(
            "Konoha Gakuen",
            "sub",
            "best",
            "1",
            None,
            Some(1),
            Some("movie")
        ),
    );
    assert_ne!(
        cache_key(
            "Konoha Gakuen",
            "sub",
            "best",
            "1",
            None,
            Some(1),
            Some("special")
        ),
        cache_key("Konoha Gakuen", "sub", "best", "1", None, Some(1), None),
    );
}

#[test]
fn cache_key_differs_across_each_axis() {
    let base = cache_key("One Piece", "sub", "best", "1", None, None, None);
    assert_ne!(
        cache_key("Naruto", "sub", "best", "1", None, None, None),
        base
    );
    assert_ne!(
        cache_key("One Piece", "dub", "best", "1", None, None, None),
        base
    );
    assert_ne!(
        cache_key("One Piece", "sub", "1080", "1", None, None, None),
        base
    );
    assert_ne!(
        cache_key("One Piece", "sub", "best", "2", None, None, None),
        base
    );
    // Year + ep-count axes — different Kitsu entries sharing a
    // title must map to different keys so the first resolve
    // doesn't poison the row for the other entry.
    assert_ne!(
        cache_key(
            "Mobile Suit Gundam",
            "sub",
            "best",
            "1",
            Some(1979),
            Some(43),
            None
        ),
        cache_key(
            "Mobile Suit Gundam",
            "sub",
            "best",
            "1",
            Some(1995),
            Some(49),
            None
        ),
    );
    // Just-year-different is enough — Codex's concern was two
    // Kitsu entries with same title differing by year. Episode
    // count is the secondary discriminator; pin both axes
    // independently so a regression on either drops a test.
    assert_ne!(
        cache_key("Show", "sub", "best", "1", Some(2020), None, None),
        cache_key("Show", "sub", "best", "1", Some(2021), None, None),
    );
    assert_ne!(
        cache_key("Show", "sub", "best", "1", None, Some(12), None),
        cache_key("Show", "sub", "best", "1", None, Some(13), None),
    );
}

#[test]
fn cache_key_includes_schema_version_so_v0_entries_are_unreachable() {
    // Bump SCHEMA when CachedResolution gains a field consumers
    // depend on; old un-versioned-or-older-versioned entries
    // become misses on first access. This test pins the prefix
    // shape so a typo in SCHEMA doesn't silently produce keys
    // that collide with the prior version.
    let k = cache_key("X", "sub", "best", "1", None, None, None);
    assert!(k.starts_with("play:v15:"), "got {k}");
}

#[test]
fn a_row_under_the_schema_before_this_one_is_not_served() {
    // A row's `show_id` is a show key now — printed bare for
    // anidb and `<provider>:<slug>` for anyone else — and every
    // reader of a served row parses it that way, taking a bare
    // id for anidb's. The schema before this one wrote the slug
    // alone, whichever provider it came from, so a bare id in
    // one of those rows says nothing about whose it is; served
    // under the new reading it would write history, stamps,
    // mappings and title matches into anidb's namespace and
    // alias a real anidb row. The schema segment is what
    // retires them: the same request now builds a key those
    // rows do not sit under, so the walk runs again and writes
    // the qualified identity.
    let pool = pool();
    let current = cache_key(
        "Cowboy Bebop",
        "sub",
        "best",
        "1",
        Some(1998),
        Some(26),
        Some("TV"),
    );
    let previous = current.replacen(&format!("play:{SCHEMA}:"), "play:v13:", 1);
    assert_ne!(
        previous, current,
        "the schema segment must have moved past the one that wrote bare show ids"
    );
    let body = serde_json::to_string(&sample_resolution()).expect("serialize");
    meta_cache_put(&pool, &previous, &body, 60).unwrap();
    assert!(
        get(&pool, &current).expect("ok").is_none(),
        "a row keyed under the previous schema must not answer the current key"
    );
}

#[test]
fn a_row_resolved_before_split_entries_were_stitched_is_not_served() {
    // A show the provider splits in two (Steel Ball Run's premiere
    // and its 2nd Stage) was resolved against one part alone, so
    // its rows hold streams for the wrong episode — Kitsu's
    // episode 1 cached as the 2nd Stage's first. Nothing in a row
    // says which picks were split, so the whole schema retires.
    let pool = pool();
    let current = cache_key(
        "Show",
        "sub",
        "best",
        "1",
        Some(2026),
        Some(12),
        Some("ONA"),
    );
    let previous = current.replacen(&format!("play:{SCHEMA}:"), "play:v14:", 1);
    assert_ne!(previous, current, "the schema must have moved past v14");
    let body = serde_json::to_string(&sample_resolution()).expect("serialize");
    meta_cache_put(&pool, &previous, &body, 60).unwrap();
    assert!(get(&pool, &current).expect("ok").is_none());
}

#[test]
fn cache_key_emits_dash_placeholder_for_missing_year_and_eps() {
    // Legacy callers and ongoing shows have year=None /
    // episode_count=None. The key must stay well-formed (no
    // adjacent colons) so the SQLite text doesn't drift across
    // None/Some shapes — `-` is the chosen placeholder.
    let k = cache_key("Show", "sub", "best", "1", None, None, None);
    assert!(k.ends_with(":-:-"), "got {k}");
}

#[test]
fn put_then_get_round_trips_the_resolution() {
    let pool = pool();
    let key = cache_key("Stone Ocean", "sub", "best", "1", None, None, None);
    put(&pool, &key, &sample_resolution());
    let got = get(&pool, &key).expect("ok").expect("hit");
    assert_eq!(got, sample_resolution());
}

#[test]
fn get_returns_none_on_miss() {
    let pool = pool();
    let got = get(&pool, "play:v4:Nope:sub:best:1:-:-").expect("ok");
    assert!(got.is_none());
}

#[test]
fn evict_removes_a_row_so_subsequent_get_misses() {
    let pool = pool();
    let key = cache_key("Stone Ocean", "sub", "best", "1", None, None, None);
    put(&pool, &key, &sample_resolution());
    assert!(get(&pool, &key).expect("ok").is_some());
    evict(&pool, &key);
    assert!(
        get(&pool, &key).expect("ok").is_none(),
        "evict() must wipe the row, not just expire it"
    );
}

#[test]
fn evict_is_idempotent_on_missing_key() {
    let pool = pool();
    // Eviction by frontend feedback may race the natural
    // eviction-on-HEAD-fail in the backend. Both callers should
    // be safe to invoke even when the row is already gone.
    evict(&pool, "play:v4:Never:Cached:best:1:-:-");
    assert!(get(&pool, "play:v4:Never:Cached:best:1:-:-")
        .expect("ok")
        .is_none());
}

#[test]
fn get_parses_legacy_rows_missing_show_id_and_title() {
    // Rows written before the show_id/show_title fields existed
    // must still deserialize — serde_default fills in empty strings
    // and the cache-hit path skips the history-write when those
    // are blank. Without this, the bump to v2 of CachedResolution
    // would silently invalidate every row.
    let pool = pool();
    let key = "play:v4:Legacy:sub:best:1:-:-";
    let legacy =
        r#"{"upstream_url":"https://x/y.mp4","referer":"","subtitle_url":null,"media_kind":"mp4"}"#;
    meta_cache_put(&pool, key, legacy, 60).unwrap();
    let got = get(&pool, key).expect("ok").expect("hit");
    assert_eq!(got.show_id, "");
    assert_eq!(got.show_title, "");
    assert_eq!(got.upstream_url, "https://x/y.mp4");
}

#[test]
fn get_treats_corrupt_payload_as_miss() {
    // A migrated payload from a future version, or an externally
    // edited row, shouldn't permanently mask the show — the play
    // flow should resolve afresh and overwrite the row.
    let pool = pool();
    let key = "play:v4:Garbage:sub:best:1:-:-";
    meta_cache_put(&pool, key, "{ not valid json", 60).unwrap();
    assert!(get(&pool, key).expect("ok").is_none());
}
