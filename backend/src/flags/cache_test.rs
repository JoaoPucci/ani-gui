use std::time::Duration;

use proptest::prelude::*;

use super::{is_fresh, read, write, CacheEntry, FRESH_FOR};

const DAY: u64 = 24 * 60 * 60;

fn entry(fetched_at: u64) -> CacheEntry {
    let doc = serde_json::json!({ "test_a": false });
    CacheEntry::admitted(fetched_at, doc.as_object().expect("object").clone())
}

#[test]
fn the_window_is_seven_days() {
    assert_eq!(FRESH_FOR, Duration::from_secs(7 * DAY));
}

#[test]
fn an_entry_is_fresh_up_to_the_window_and_not_after() {
    let now = 1_800_000_000;
    assert!(is_fresh(now, now, FRESH_FOR));
    assert!(is_fresh(now - 7 * DAY, now, FRESH_FOR));
    assert!(!is_fresh(now - 7 * DAY - 1, now, FRESH_FOR));
}

#[test]
fn an_entry_dated_in_the_future_is_not_fresh() {
    let now = 1_800_000_000;
    assert!(!is_fresh(now + 1, now, FRESH_FOR));
}

proptest! {
    #[test]
    fn freshness_is_age_within_the_window(
        now in (1u64 << 32)..(1u64 << 62),
        age in 0u64..(28 * DAY),
        future in any::<bool>(),
        window in 0u64..(14 * DAY),
    ) {
        let fetched_at = if future { now + age + 1 } else { now - age };
        let expect = !future && age <= window;
        prop_assert_eq!(is_fresh(fetched_at, now, Duration::from_secs(window)), expect);
    }

    #[test]
    fn freshness_never_panics_at_the_extremes(fetched_at in any::<u64>(), now in any::<u64>(), window in any::<u64>()) {
        let expect = fetched_at <= now && now - fetched_at <= window;
        prop_assert_eq!(is_fresh(fetched_at, now, Duration::from_secs(window)), expect);
    }
}

#[tokio::test]
async fn a_written_entry_reads_back() {
    let td = tempfile::tempdir().expect("tempdir");
    let path = td.path().join("nested").join("remote-flags.json");
    write(&path, &entry(42)).await;
    assert_eq!(read(&path).await, Some(entry(42)));
}

#[tokio::test]
async fn a_rewrite_replaces_the_entry() {
    let td = tempfile::tempdir().expect("tempdir");
    let path = td.path().join("remote-flags.json");
    write(&path, &entry(1)).await;
    write(&path, &entry(2)).await;
    assert_eq!(read(&path).await, Some(entry(2)));
}

#[tokio::test]
async fn a_missing_or_corrupt_file_reads_as_nothing() {
    let td = tempfile::tempdir().expect("tempdir");
    let path = td.path().join("remote-flags.json");
    assert_eq!(read(&path).await, None);
    for body in [
        "",
        "{",
        "[]",
        r#"{"fetched_at":"x","flags":{}}"#,
        r#"{"flags":{}}"#,
    ] {
        std::fs::write(&path, body).expect("write");
        assert_eq!(read(&path).await, None, "{body:?}");
    }
}

#[tokio::test]
async fn an_oversized_file_reads_as_nothing() {
    let td = tempfile::tempdir().expect("tempdir");
    let path = td.path().join("remote-flags.json");
    let pad = " ".repeat(4 * super::MAX_BYTES);
    std::fs::write(&path, format!(r#"{{"fetched_at":1,"flags":{{}}}}{pad}"#)).expect("write");
    assert_eq!(read(&path).await, None);
}

#[tokio::test]
async fn a_write_that_cannot_land_is_ignored() {
    let td = tempfile::tempdir().expect("tempdir");
    // A regular file where the parent directory should be.
    let blocker = td.path().join("blocker");
    std::fs::write(&blocker, "").expect("write");
    write(&blocker.join("remote-flags.json"), &entry(1)).await;
    assert_eq!(read(&blocker.join("remote-flags.json")).await, None);
}

#[tokio::test]
async fn a_directory_at_the_cache_path_reads_as_nothing() {
    let td = tempfile::tempdir().expect("tempdir");
    let path = td.path().join("remote-flags.json");
    std::fs::create_dir(&path).expect("mkdir");
    assert_eq!(read(&path).await, None);
}

/// A link is not followed: a link to a device or a pipe would
/// otherwise get past the size check, which reads its length as 0.
#[cfg(unix)]
#[tokio::test]
async fn a_symlink_at_the_cache_path_reads_as_nothing() {
    let td = tempfile::tempdir().expect("tempdir");
    let real = td.path().join("real.json");
    write(&real, &entry(5)).await;
    assert_eq!(read(&real).await, Some(entry(5)));
    let link = td.path().join("remote-flags.json");
    std::os::unix::fs::symlink(&real, &link).expect("symlink");
    assert_eq!(read(&link).await, None);
}

/// Builds without the signature check (the remote flags' first
/// release) cached the file as fetched, with nothing checked, and with
/// no `verified` field. Read by this build, such an entry keeps its
/// offs and loses its ons: they never verified.
#[tokio::test]
async fn a_cache_written_before_the_signature_check_reads_as_unsigned() {
    let td = tempfile::tempdir().expect("tempdir");
    let path = td.path().join("remote-flags.json");
    let older = serde_json::json!({
        "fetched_at": 42,
        "flags": { "test_a": true, "test_b": false }
    });
    std::fs::write(&path, older.to_string()).expect("write");
    let got = read(&path).await.expect("an entry");
    assert_eq!(got.fetched_at, 42);
    assert!(!got.verified);
    assert_eq!(got.flags.get("test_a"), None);
    assert_eq!(got.flags.get("test_b"), Some(&serde_json::json!(false)));
}

/// An entry marked as not known to have verified is admitted again, as
/// unsigned, so it cannot turn a flag on either.
#[tokio::test]
async fn an_entry_not_known_to_have_verified_cannot_turn_a_flag_on() {
    let td = tempfile::tempdir().expect("tempdir");
    let path = td.path().join("remote-flags.json");
    let unmarked = serde_json::json!({
        "fetched_at": 42,
        "verified": false,
        "flags": { "test_a": true, "test_b": false }
    });
    std::fs::write(&path, unmarked.to_string()).expect("write");
    let got = read(&path).await.expect("an entry");
    assert_eq!(got.flags.get("test_a"), None);
    assert_eq!(got.flags.get("test_b"), Some(&serde_json::json!(false)));
}

/// What this build caches it admitted first, so a `true` in it came
/// from a verified file: the document says so by holding it, and the
/// entry is written marked and reads back whole.
#[tokio::test]
async fn an_entry_written_from_a_verified_document_keeps_its_turn_on() {
    let td = tempfile::tempdir().expect("tempdir");
    let path = td.path().join("remote-flags.json");
    let doc = serde_json::json!({ "test_a": true, "test_b": false });
    write(
        &path,
        &CacheEntry::admitted(42, doc.as_object().expect("object").clone()),
    )
    .await;
    let got = read(&path).await.expect("an entry");
    assert!(got.verified);
    assert_eq!(got.flags.get("test_a"), Some(&serde_json::json!(true)));
}

/// The mark is the document's own evidence, taken as it is written: an
/// entry handed in marked verified, whose document holds nothing only a
/// verified file keeps, is written unmarked.
#[tokio::test]
async fn the_mark_is_taken_from_the_document_as_it_is_written() {
    let td = tempfile::tempdir().expect("tempdir");
    let path = td.path().join("remote-flags.json");
    let mut claimed = entry(42);
    claimed.verified = true;
    write(&path, &claimed).await;
    let on_disk: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("read")).expect("json");
    assert_eq!(on_disk["verified"], serde_json::json!(false));
}
