//! The row the Electron end-to-end specs seed, held to what this
//! cache reads.
//!
//! A spec opens playback the way a click does: the renderer asks
//! `/api/play/stream`, and the backend answers from this cache, which
//! the spec has filled with a row naming a stream it serves itself
//! (`electron/lib/e2e-replay.cjs`). The spec writes the row with its
//! own SQLite connection, so nothing ties what it writes to what the
//! backend reads but `electron/e2e/fixtures/replay-row.json`: the key
//! prefix, the insert, the TTL and the row's fields. Every one of them
//! is checked here, so a change to the schema, the key or the row
//! fails in this crate's tests, naming the fixture, rather than as a
//! spec whose play resolves afresh against a provider it cannot reach.

use super::*;
use crate::cache::open_in_memory;

fn fixture() -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../electron/e2e/fixtures/replay-row.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).expect("the fixture is JSON")
}

fn text<'a>(v: &'a serde_json::Value, field: &str) -> &'a str {
    v[field]
        .as_str()
        .unwrap_or_else(|| panic!("the fixture names `{field}`"))
}

fn number(request: &serde_json::Value, field: &str) -> Option<u32> {
    request[field]
        .as_str()
        .map(|v| v.parse().expect("a numeric axis"))
}

#[test]
fn the_fixture_key_prefix_is_the_play_family() {
    assert_eq!(
        text(&fixture(), "key_prefix"),
        crate::cache::families::play(),
        "replay-row.json's key_prefix is the play family this build reads"
    );
}

#[test]
fn the_fixture_ttl_is_the_play_resolution_ttl() {
    assert_eq!(
        fixture()["ttl_seconds"].as_u64(),
        Some(crate::cache::ttl::PLAY_RESOLUTION_TTL.as_secs())
    );
}

#[test]
fn the_fixture_keys_are_the_keys_a_play_reads() {
    let f = fixture();
    for sample in [&f["sample"], &f["sample"]["versioned"]] {
        let request = &sample["request"];
        let version = request["version"].as_str().map(|v| {
            serde_json::from_value::<crate::commands::play_native_versions::VersionKind>(
                serde_json::Value::String(v.to_string()),
            )
            .expect("a version the wire names")
        });
        let key = cache_key_for_version(
            text(request, "title"),
            text(request, "mode"),
            text(request, "episode"),
            number(request, "year"),
            number(request, "episode_count"),
            request["subtype"].as_str(),
            version,
        );
        assert_eq!(key, text(sample, "key"), "{request}");
        assert!(key.starts_with(text(&f, "key_prefix")));
    }
}

/// The insert the spec runs, against this build's schema, leaves a
/// row this cache reads back and a play would replay: one naming a
/// provider the shipped app walks, with no track listing at the cap.
#[test]
fn a_row_seeded_by_the_fixture_insert_is_read_and_replayable() {
    let f = fixture();
    let pool = open_in_memory().expect("in-memory pool");
    let key = text(&f["sample"], "key");
    let mut row = f["row"].clone();
    row["upstream_url"] = serde_json::json!("http://127.0.0.1:9/master.m3u8");
    row["played"] = serde_json::json!("6");
    row["subtitles"] = serde_json::json!([
        {"lang": "en", "label": "English", "default": true, "url": "http://127.0.0.1:9/en.vtt"}
    ]);
    let now = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("after the epoch")
            .as_secs(),
    )
    .expect("fits");
    pool.get()
        .expect("conn")
        .execute(
            text(&f, "insert"),
            rusqlite::params![
                key,
                row.to_string(),
                now,
                f["ttl_seconds"].as_i64().expect("ttl")
            ],
        )
        .expect("the fixture's insert runs against this build's schema");

    let cached = get(&pool, key)
        .expect("read")
        .expect("the seeded row is read as a resolution");
    assert_eq!(cached.upstream_url, "http://127.0.0.1:9/master.m3u8");
    assert_eq!(cached.media_kind, MediaKind::Hls);
    assert_eq!(cached.played.as_deref(), Some("6"));
    assert_eq!(cached.subtitles.len(), 1);
    assert_eq!(
        crate::commands::play_cache::cached_row_provider(&cached),
        Some(crate::scraper::provider::ProviderId::Hianime)
    );
    assert!(
        crate::provider_set::ProviderSet::shipped()
            .order()
            .contains(&crate::scraper::provider::ProviderId::Hianime),
        "the row names a provider the shipped app walks"
    );
    assert!(!crate::commands::play_cache::cached_row_may_be_cut(
        &cached.subtitles
    ));
}

#[test]
fn the_fixture_media_kinds_are_the_proxy_kinds() {
    for kind in ["hls", "mp4"] {
        serde_json::from_value::<MediaKind>(serde_json::json!(kind))
            .unwrap_or_else(|e| panic!("{kind}: {e}"));
    }
}
