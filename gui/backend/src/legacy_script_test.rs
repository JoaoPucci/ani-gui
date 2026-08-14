//! Tests for `crate::legacy_script`. Extracted via `#[path]` so the
//! inline `mod tests { ... }` block doesn't count toward the file's
//! CCN — per `project_crap_inline_test_gotcha`.

use super::*;

fn cache_with(entries: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tmp");
    for name in entries {
        std::fs::write(dir.path().join(name), b"#!/bin/sh\n").expect("write");
    }
    dir
}

#[test]
fn the_orphaned_copy_is_removed_and_named() {
    let dir = cache_with(&["ani-cli"]);
    let report = sweep_legacy_script(dir.path());
    assert_eq!(report.removed.len(), 1);
    assert!(report.removed[0].ends_with("ani-cli"));
    assert!(
        !dir.path().join("ani-cli").exists(),
        "the copy must actually be gone, not merely reported"
    );
}

#[test]
fn a_cache_without_the_copy_reports_nothing() {
    let dir = cache_with(&[]);
    assert!(sweep_legacy_script(dir.path()).removed.is_empty());
}

#[test]
fn the_sweep_is_idempotent() {
    // Every launch runs this. The second run must be silent, or the
    // diagnostics panel reports a removal that did not happen.
    let dir = cache_with(&["ani-cli"]);
    assert_eq!(sweep_legacy_script(dir.path()).removed.len(), 1);
    assert!(sweep_legacy_script(dir.path()).removed.is_empty());
}

#[test]
fn nothing_else_in_the_cache_is_touched() {
    // The cache root holds live state — an over-eager sweep would take
    // the image cache or the database with it.
    let dir = cache_with(&["ani-cli", "ani-cli.bak", "images", "meta.sqlite3"]);
    sweep_legacy_script(dir.path());
    for kept in ["ani-cli.bak", "images", "meta.sqlite3"] {
        assert!(dir.path().join(kept).exists(), "{kept} must survive");
    }
}

#[test]
fn a_missing_cache_directory_is_not_an_error() {
    // First launch on a fresh profile: nothing has created the cache
    // root yet, and a sweep that failed there would fail the boot.
    let dir = tempfile::tempdir().expect("tmp");
    let absent = dir.path().join("not-created-yet");
    assert!(sweep_legacy_script(&absent).removed.is_empty());
}

#[test]
fn a_directory_named_like_the_script_is_left_alone() {
    // Only the file the updater wrote is ours to remove. Anything else
    // wearing that name belongs to whoever made it.
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::create_dir(dir.path().join("ani-cli")).expect("mkdir");
    assert!(sweep_legacy_script(dir.path()).removed.is_empty());
    assert!(dir.path().join("ani-cli").is_dir());
}
