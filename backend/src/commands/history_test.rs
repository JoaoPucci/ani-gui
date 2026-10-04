//! The history commands: listing, removal, clearing, and the
//! numbering a row is shown in.

use super::*;
use crate::app::AppState;
use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
use std::path::PathBuf;
use std::sync::Arc;

fn make_state(history_path: PathBuf) -> AppState {
    AppState {
        anidb_base: None,
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 0),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path,
        anidb_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        hianime_base: None,
        hianime_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        provider_order: vec![crate::scraper::provider::ProviderId::Anidb],
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::new(reqwest::Client::new()),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

#[test]
fn list_empty_when_file_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let s = make_state(tmp.path().join("nope"));
    let v = history_list(&s).unwrap();
    assert!(v.is_empty());
}

#[test]
fn list_translates_provider_numbering_back_to_kitsu() {
    // The history file speaks the provider's numbering (a reader greps the
    // stored ep_no in the provider's episode list), while every
    // GUI surface counts per-entry like Kitsu. The read boundary
    // subtracts the offset stamped at resolve time; rows without
    // a stamp pass through unchanged — that's today's behavior
    // for shows the GUI has never resolved.
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    write_atomic(
        &path,
        &[
            HistoryEntry {
                ep_no: "41".into(),
                id: "the-sequel-88".into(),
                title: "The Sequel".into(),
                watched_at: None,
                kitsu_id: None,
            },
            HistoryEntry {
                ep_no: "5".into(),
                id: "plain-1".into(),
                title: "Plain Show".into(),
                watched_at: None,
                kitsu_id: None,
            },
        ],
    )
    .unwrap();
    crate::commands::anidb_offset::put(&s, "the-sequel-88", 40);

    let listed = history_list(&s).unwrap();
    assert_eq!(listed[0].ep_no, "1", "provider 41 minus offset 40");
    assert_eq!(listed[1].ep_no, "5", "no stamp: served raw");
}

#[test]
fn by_kitsu_translates_provider_numbering_back_to_kitsu() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    write_atomic(
        &path,
        &[HistoryEntry {
            ep_no: "42".into(),
            id: "the-sequel-88".into(),
            title: "The Sequel".into(),
            watched_at: None,
            kitsu_id: None,
        }],
    )
    .unwrap();
    crate::commands::kitsu::allmanga_kitsu_put(&s, "the-sequel-88", "K9").unwrap();
    crate::commands::anidb_offset::put(&s, "the-sequel-88", 40);

    let hit = history_by_kitsu(&s, "K9").unwrap().expect("match");
    assert_eq!(hit.ep_no, "2");
}

#[test]
fn by_kitsu_returns_the_matching_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());

    write_atomic(
        &path,
        &[
            HistoryEntry {
                ep_no: "5".into(),
                id: "amA".into(),
                title: "Show A (10 episodes)".into(),
                watched_at: None,
                kitsu_id: None,
            },
            HistoryEntry {
                ep_no: "12".into(),
                id: "amB".into(),
                title: "Show B (24 episodes)".into(),
                watched_at: None,
                kitsu_id: None,
            },
        ],
    )
    .unwrap();

    // Prime the (provider show_id → kitsu_id) reverse mapping
    // the play path stamps after a successful play.
    crate::commands::kitsu::allmanga_kitsu_put(&s, "amA", "K1").unwrap();
    crate::commands::kitsu::allmanga_kitsu_put(&s, "amB", "K2").unwrap();

    let hit = history_by_kitsu(&s, "K2").unwrap().expect("match");
    assert_eq!(hit.id, "amB");
    assert_eq!(hit.ep_no, "12");
}

#[test]
fn by_kitsu_returns_none_when_no_history_entry_maps_to_id() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());

    write_atomic(
        &path,
        &[HistoryEntry {
            ep_no: "5".into(),
            id: "amA".into(),
            title: "Show A (10 episodes)".into(),
            watched_at: None,
            kitsu_id: None,
        }],
    )
    .unwrap();
    crate::commands::kitsu::allmanga_kitsu_put(&s, "amA", "K1").unwrap();

    // No history entry maps to K-other.
    assert!(history_by_kitsu(&s, "K-other").unwrap().is_none());
}

#[test]
fn by_kitsu_returns_none_when_history_is_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let s = make_state(tmp.path().join("nope"));
    assert!(history_by_kitsu(&s, "K1").unwrap().is_none());
}

// — history_delete ————————————————————————————————————————————
//
// Per-row delete operates on the app's own TSV file.
// Pins: removes the matching id, preserves others byte-identically,
// is idempotent (no-op delete of an unknown id returns false), and
// handles a missing file as "nothing to delete" rather than erroring.

#[test]
fn delete_removes_matching_row_and_preserves_others() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    write_atomic(
        &path,
        &[
            HistoryEntry {
                ep_no: "5".into(),
                id: "amA".into(),
                title: "Show A".into(),
                watched_at: None,
                kitsu_id: None,
            },
            HistoryEntry {
                ep_no: "12".into(),
                id: "amB".into(),
                title: "Show B".into(),
                watched_at: None,
                kitsu_id: None,
            },
            HistoryEntry {
                ep_no: "3".into(),
                id: "amC".into(),
                title: "Show C".into(),
                watched_at: None,
                kitsu_id: None,
            },
        ],
    )
    .unwrap();

    let removed = history_delete(&s, "amB").unwrap();
    assert!(removed, "delete reports true when a row is removed");

    let after = history_list(&s).unwrap();
    assert_eq!(after.len(), 2);
    assert_eq!(after[0].id, "amA");
    assert_eq!(after[1].id, "amC");
}

#[test]
fn delete_unknown_id_is_idempotent_no_op() {
    // Per-card double-clicks and bad client retries must be safe.
    // No-op delete returns false; the file stays byte-identical.
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    write_atomic(
        &path,
        &[HistoryEntry {
            ep_no: "5".into(),
            id: "amA".into(),
            title: "Show A".into(),
            watched_at: None,
            kitsu_id: None,
        }],
    )
    .unwrap();
    let body_before = std::fs::read_to_string(&path).unwrap();

    let removed = history_delete(&s, "does-not-exist").unwrap();
    assert!(!removed);
    let body_after = std::fs::read_to_string(&path).unwrap();
    assert_eq!(body_before, body_after);
}

#[test]
fn delete_against_missing_file_returns_false() {
    let tmp = tempfile::tempdir().unwrap();
    let s = make_state(tmp.path().join("nope"));
    let removed = history_delete(&s, "amA").unwrap();
    assert!(!removed);
}

#[test]
fn delete_with_empty_id_returns_false() {
    // Defensive: a malformed client call with empty id mustn't
    // accidentally wipe rows with id="" (parser already drops
    // those at read, but pin the contract anyway).
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    write_atomic(
        &path,
        &[HistoryEntry {
            ep_no: "5".into(),
            id: "amA".into(),
            title: "Show A".into(),
            watched_at: None,
            kitsu_id: None,
        }],
    )
    .unwrap();

    let removed = history_delete(&s, "").unwrap();
    assert!(!removed);
    assert_eq!(history_list(&s).unwrap().len(), 1);
}

#[test]
fn list_then_clear_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    // Pre-populate with a known fixture.
    write_atomic(
        &path,
        &[HistoryEntry {
            ep_no: "5".into(),
            id: "abc".into(),
            title: "T (10 episodes)".into(),
            watched_at: None,
            kitsu_id: None,
        }],
    )
    .unwrap();

    let listed = history_list(&s).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "abc");

    history_clear(&s).unwrap();
    let after = history_list(&s).unwrap();
    assert!(after.is_empty());
}

/// A row that records the Kitsu id played is found by it, whatever the
/// reverse mapping says or whether there is one.
#[test]
fn by_kitsu_reads_the_kitsu_id_the_row_records() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    write_atomic(
        &path,
        &[
            HistoryEntry {
                ep_no: "3".into(),
                id: "hianime:seitokai-10497".into(),
                title: "There Is Also a Hole in the Student Organization!".into(),
                watched_at: None,
                kitsu_id: Some("49877".into()),
            },
            HistoryEntry {
                ep_no: "5".into(),
                id: "hianime:here-is-greenwood-3081".into(),
                title: "Here is Greenwood".into(),
                watched_at: None,
                kitsu_id: Some("1623".into()),
            },
        ],
    )
    .unwrap();
    // A wrong mapping a guess left behind does not outrank the row.
    crate::commands::kitsu::allmanga_kitsu_put(&s, "hianime:seitokai-10497", "1623").unwrap();

    let hit = history_by_kitsu(&s, "49877")
        .unwrap()
        .expect("found by its own id");
    assert_eq!(hit.id, "hianime:seitokai-10497");
    let hit = history_by_kitsu(&s, "1623").unwrap().expect("greenwood");
    assert_eq!(
        hit.id, "hianime:here-is-greenwood-3081",
        "the mapping does not pull the other row in"
    );
}

// — what a row leaves in the cache ————————————————————————————————
//
// A row records the Kitsu id of the show played, and the cache holds
// the same answer three more ways: the show's watch stamp, its
// reverse mapping, and the title-match rows Continue Watching stored
// for its title. Removing the row removes those with it; clearing the
// history removes all of them, whichever version wrote them.

fn row(id: &str, title: &str) -> HistoryEntry {
    HistoryEntry {
        ep_no: "1".into(),
        id: id.into(),
        title: title.into(),
        watched_at: None,
        kitsu_id: Some("49877".into()),
    }
}

fn cached(s: &AppState, key: &str) -> Option<String> {
    crate::cache::meta_cache_get(&s.cache_pool, key).unwrap()
}

fn put(s: &AppState, key: &str, body: &str) {
    crate::cache::meta_cache_put(&s.cache_pool, key, body, 86_400).unwrap();
}

#[test]
fn delete_removes_what_the_row_left_in_the_cache() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    let gone = "hianime:there-is-also-a-hole-10497";
    let kept = "hianime:one-piece-100";
    write_atomic(
        &path,
        &[
            row(gone, "There Is Also a Hole (12 episodes)"),
            row(kept, "One Piece"),
        ],
    )
    .unwrap();
    for id in [gone, kept] {
        crate::commands::kitsu::watched_at_put(&s, id, 1_790_000_000_000).unwrap();
        crate::commands::kitsu::allmanga_kitsu_put(&s, id, "49877").unwrap();
    }
    put(
        &s,
        "title-match:v3:hianime:there is also a hole:c1",
        "49877",
    );
    put(
        &s,
        "title-match:v3:hianime:there is also a hole:c2",
        "49878",
    );
    put(&s, "title-match:v3:hianime:one piece:c1", "12");

    assert!(history_delete(&s, gone).unwrap());

    assert_eq!(
        crate::commands::kitsu::watched_at_get(&s, gone).unwrap(),
        None
    );
    assert_eq!(
        crate::commands::kitsu::allmanga_kitsu_get(&s, gone).unwrap(),
        None
    );
    assert_eq!(
        cached(&s, "title-match:v3:hianime:there is also a hole:c1"),
        None
    );
    assert_eq!(
        cached(&s, "title-match:v3:hianime:there is also a hole:c2"),
        None
    );
    assert!(crate::commands::kitsu::watched_at_get(&s, kept)
        .unwrap()
        .is_some());
    assert!(crate::commands::kitsu::allmanga_kitsu_get(&s, kept)
        .unwrap()
        .is_some());
    assert!(cached(&s, "title-match:v3:hianime:one piece:c1").is_some());
}

#[test]
fn clear_removes_what_every_row_left_in_the_cache() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    write_atomic(&path, &[row("abc", "T")]).unwrap();
    let history_keys = [
        "watched-at:v1:abc",
        "allmanga2kitsu:v3:abc",
        "allmanga2kitsu:v2:abc",
        "title-match:v3:anidb:t:c1",
        "title-match:v2:t:c1",
    ];
    for key in history_keys {
        put(&s, key, "1");
    }
    put(&s, "kitsu:v5:anime:49877", "{}");

    history_clear(&s).unwrap();

    for key in history_keys {
        assert_eq!(cached(&s, key), None, "{key} outlived the history");
    }
    assert!(
        cached(&s, "kitsu:v5:anime:49877").is_some(),
        "the catalogue cache is not history"
    );
}

/// A title that starts another's keeps the other's title-match rows:
/// "Re" is not "Re:Creators".
#[test]
fn delete_keeps_title_match_rows_of_a_title_that_extends_the_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    write_atomic(&path, &[row("re-1", "Re")]).unwrap();
    put(&s, "title-match:v3:anidb.app:re:c1", "1");
    put(&s, "title-match:v3:anidb.app:re:creators:c1", "2");

    assert!(history_delete(&s, "re-1").unwrap());

    assert_eq!(cached(&s, "title-match:v3:anidb.app:re:c1"), None);
    assert!(cached(&s, "title-match:v3:anidb.app:re:creators:c1").is_some());
}

/// A cache that cannot forget a row's entries fails the delete before
/// the row is removed, so the error the caller sees is true and a
/// retry finds the row still there.
#[test]
fn delete_that_cannot_forget_the_cache_leaves_the_row() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    write_atomic(&path, &[row("abc", "T")]).unwrap();
    s.cache_pool
        .get()
        .unwrap()
        .execute("DROP TABLE meta_cache", [])
        .unwrap();

    assert!(history_delete(&s, "abc").is_err());
    assert_eq!(history_list(&s).unwrap().len(), 1, "the row stays");
    assert!(history_clear(&s).is_err());
    assert_eq!(history_list(&s).unwrap().len(), 1, "the history stays");
}

/// What older versions left for a row, its numbering offsets and the
/// resolution rows that played it go with it too; another show's stay.
#[test]
fn delete_removes_every_per_show_store_of_every_version() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    let gone = "hianime:seitokai-10497";
    let kept = "one-piece-69";
    write_atomic(&path, &[row(gone, "Seitokai"), row(kept, "One Piece")]).unwrap();
    for (key, body) in [
        ("allmanga2kitsu:v2:hianime:seitokai-10497", "1"),
        ("title-match:v2:seitokai:c1", "1"),
        ("title-match:v1:seitokai:c1", "1"),
        (
            "play:v14:Seitokai:sub:best:1:::",
            r#"{"show_id":"hianime:seitokai-10497"}"#,
        ),
        (
            "play:v13:Seitokai:sub:best:2:::",
            r#"{"show_id":"hianime:seitokai-10497"}"#,
        ),
        (
            "play:v14:One Piece:sub:best:1:::",
            r#"{"show_id":"one-piece-69"}"#,
        ),
        ("allmanga2kitsu:v2:one-piece-69", "12"),
    ] {
        put(&s, key, body);
    }
    crate::commands::anidb_offset::put(&s, gone, 3);
    crate::commands::anidb_offset::put(&s, kept, 4);

    assert!(history_delete(&s, gone).unwrap());

    for key in [
        "allmanga2kitsu:v2:hianime:seitokai-10497",
        "title-match:v2:seitokai:c1",
        "title-match:v1:seitokai:c1",
        "play:v14:Seitokai:sub:best:1:::",
        "play:v13:Seitokai:sub:best:2:::",
    ] {
        assert_eq!(cached(&s, key), None, "{key} outlived the row");
    }
    assert!(cached(&s, "play:v14:One Piece:sub:best:1:::").is_some());
    assert!(cached(&s, "allmanga2kitsu:v2:one-piece-69").is_some());
    let offsets = std::fs::read_to_string(tmp.path().join("ani-gui-offsets")).unwrap();
    assert!(!offsets.contains(gone), "the row's offset outlived it");
    assert_eq!(crate::commands::anidb_offset::get(&s, kept), 4);
}

/// Clearing the history removes every row's offsets and every
/// resolution row along with the cache entries.
#[test]
fn clear_removes_the_offsets_and_the_resolution_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    write_atomic(&path, &[row("one-piece-69", "One Piece")]).unwrap();
    put(
        &s,
        "play:v14:One Piece:sub:best:1:::",
        r#"{"show_id":"one-piece-69"}"#,
    );
    put(&s, "play:v12:Naruto:sub:best:1:::", "{}");
    crate::commands::anidb_offset::put(&s, "one-piece-69", 4);

    history_clear(&s).unwrap();

    assert_eq!(cached(&s, "play:v14:One Piece:sub:best:1:::"), None);
    assert_eq!(cached(&s, "play:v12:Naruto:sub:best:1:::"), None);
    assert_eq!(crate::commands::anidb_offset::get(&s, "one-piece-69"), 0);
}

// — what a failed removal leaves ——————————————————————————————————
//
// A row and its numbering offset are a pair: the offset makes the row
// readable and has no use without it. A removal that fails part-way
// must not leave a row without its offset; an offset left without its
// row is swept by the next removal.

/// Block the atomic write whose temp file is `name`, beside `dir`'s
/// history, by putting a directory where the temp file would go.
fn block_write(dir: &std::path::Path, name: &str) {
    std::fs::create_dir(dir.join(name)).unwrap();
}

#[test]
fn a_delete_whose_history_write_fails_keeps_the_row_and_its_offset() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    write_atomic(&path, &[row("one-piece-69", "One Piece")]).unwrap();
    crate::commands::anidb_offset::put(&s, "one-piece-69", 4);
    block_write(tmp.path(), "history.new");

    assert!(history_delete(&s, "one-piece-69").is_err());

    assert_eq!(history_list(&s).unwrap().len(), 1, "the row stays");
    assert_eq!(
        crate::commands::anidb_offset::get(&s, "one-piece-69"),
        4,
        "with its offset"
    );
}

#[test]
fn a_clear_whose_history_write_fails_keeps_the_rows_and_their_offsets() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    write_atomic(&path, &[row("one-piece-69", "One Piece")]).unwrap();
    crate::commands::anidb_offset::put(&s, "one-piece-69", 4);
    block_write(tmp.path(), "history.new");

    assert!(history_clear(&s).is_err());

    assert_eq!(history_list(&s).unwrap().len(), 1, "the row stays");
    assert_eq!(
        crate::commands::anidb_offset::get(&s, "one-piece-69"),
        4,
        "with its offset"
    );
}

#[test]
fn a_delete_whose_offsets_write_fails_still_removes_the_row() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    write_atomic(&path, &[row("one-piece-69", "One Piece")]).unwrap();
    crate::commands::anidb_offset::put(&s, "one-piece-69", 4);
    block_write(tmp.path(), "ani-gui-offsets.new");

    // The row goes; the offset it leaves has no row to misread.
    assert!(history_delete(&s, "one-piece-69").unwrap());
    assert!(history_list(&s).unwrap().is_empty());
}

/// A page's pre-resolve stamps a show's offset before any row exists,
/// and a cache-hit play later writes the row through it. Removal takes
/// the removed rows' offsets and no others.
#[test]
fn removal_keeps_the_offsets_of_shows_without_a_row() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    write_atomic(
        &path,
        &[row("one-piece-69", "One Piece"), row("naruto-20", "Naruto")],
    )
    .unwrap();
    crate::commands::anidb_offset::put(&s, "one-piece-69", 4);
    crate::commands::anidb_offset::put(&s, "naruto-20", 2);
    crate::commands::anidb_offset::put(&s, "bleach-30", 1);

    assert!(history_delete(&s, "one-piece-69").unwrap());
    assert_eq!(crate::commands::anidb_offset::get(&s, "one-piece-69"), 0);
    assert_eq!(crate::commands::anidb_offset::get(&s, "naruto-20"), 2);
    assert_eq!(crate::commands::anidb_offset::get(&s, "bleach-30"), 1);

    history_clear(&s).unwrap();
    assert_eq!(crate::commands::anidb_offset::get(&s, "naruto-20"), 0);
    assert_eq!(
        crate::commands::anidb_offset::get(&s, "bleach-30"),
        1,
        "never a row"
    );
}

/// Skip times are cached per episode played, under the Kitsu id the
/// player asked with: the row's recorded id, or for an older row the
/// id its mapping or title match named. Removing the row removes them;
/// rows keyed the old way, by MAL id alone, are read by nothing and go
/// with any removal.
#[test]
fn delete_removes_the_shows_skip_times() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    let older = HistoryEntry {
        kitsu_id: None,
        ..row("hianime:seitokai-10497", "Seitokai")
    };
    write_atomic(&path, &[row("one-piece-69", "One Piece"), older]).unwrap();
    crate::commands::kitsu::allmanga_kitsu_put(&s, "hianime:seitokai-10497", "1555").unwrap();
    put(&s, "title-match:v3:hianime:seitokai:c1", "1777");
    for key in [
        "aniskip:v2:49877:59970:1",
        "aniskip:v2:1555:111:1",
        "aniskip:v2:1777:222:2",
        "aniskip:v2:4987:333:1",
        "aniskip:v2:12:21:1",
        "aniskip:v1:21:1",
    ] {
        put(&s, key, "[]");
    }

    assert!(history_delete(&s, "one-piece-69").unwrap());
    assert_eq!(
        cached(&s, "aniskip:v2:49877:59970:1"),
        None,
        "the recorded id's"
    );
    assert_eq!(cached(&s, "aniskip:v1:21:1"), None, "the old key's");
    assert!(
        cached(&s, "aniskip:v2:4987:333:1").is_some(),
        "another id's"
    );

    assert!(history_delete(&s, "hianime:seitokai-10497").unwrap());
    assert_eq!(cached(&s, "aniskip:v2:1555:111:1"), None, "the mapped id's");
    assert_eq!(
        cached(&s, "aniskip:v2:1777:222:2"),
        None,
        "the title match's"
    );
    assert!(
        cached(&s, "aniskip:v2:12:21:1").is_some(),
        "a show never in history"
    );
}

#[test]
fn clear_removes_every_skip_time() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    write_atomic(&path, &[row("one-piece-69", "One Piece")]).unwrap();
    put(&s, "aniskip:v2:12:21:1", "[]");
    put(&s, "aniskip:v1:21:1", "[]");

    history_clear(&s).unwrap();

    assert_eq!(cached(&s, "aniskip:v2:12:21:1"), None);
    assert_eq!(cached(&s, "aniskip:v1:21:1"), None);
}
