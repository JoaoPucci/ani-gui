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

/// A state dir holding the files the retired updater wrote there.
fn state_with(entries: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tmp");
    for name in entries {
        std::fs::write(dir.path().join(name), b"[]\n").expect("write");
    }
    dir
}

#[test]
fn the_orphaned_copy_is_removed_and_named() {
    let dir = cache_with(&["ani-cli"]);
    let report = sweep_legacy_files(dir.path(), state_with(&[]).path());
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
    assert!(sweep_legacy_files(dir.path(), state_with(&[]).path())
        .removed
        .is_empty());
}

#[test]
fn the_sweep_is_idempotent() {
    // Every launch runs this. The second run must be silent, or the
    // diagnostics panel reports a removal that did not happen.
    let dir = cache_with(&["ani-cli"]);
    let state = state_with(&["anicli-update-log.json"]);
    assert_eq!(
        sweep_legacy_files(dir.path(), state.path()).removed.len(),
        2
    );
    assert!(sweep_legacy_files(dir.path(), state.path())
        .removed
        .is_empty());
}

#[test]
fn nothing_else_in_the_cache_is_touched() {
    // The cache root holds live state — an over-eager sweep would take
    // the image cache or the database with it.
    let dir = cache_with(&["ani-cli", "ani-cli.bak", "images", "meta.sqlite3"]);
    let report = sweep_legacy_files(dir.path(), state_with(&[]).path());
    assert_eq!(report.removed.len(), 1, "only the script itself");
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
    assert!(sweep_legacy_files(&absent, &absent).removed.is_empty());
}

#[test]
fn a_directory_named_like_the_script_is_left_alone() {
    // Only the file the updater wrote is ours to remove. Anything else
    // wearing that name belongs to whoever made it.
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::create_dir(dir.path().join("ani-cli")).expect("mkdir");
    assert!(sweep_legacy_files(dir.path(), state_with(&[]).path())
        .removed
        .is_empty());
    assert!(dir.path().join("ani-cli").is_dir());
}

#[test]
fn the_updaters_outcome_log_is_removed_too() {
    // The updater appended every `-U` result to this file, and the
    // diagnostics panel read it back. Both are gone, so on any profile
    // where auto-update ran — the default — it is app-written state
    // with no reader and nothing left to remove it.
    let cache = cache_with(&[]);
    let state = state_with(&["anicli-update-log.json"]);
    let report = sweep_legacy_files(cache.path(), state.path());
    assert_eq!(report.removed.len(), 1);
    assert!(report.removed[0].ends_with("anicli-update-log.json"));
    assert!(!state.path().join("anicli-update-log.json").exists());
}

#[test]
fn the_logs_half_written_temporary_is_removed_too() {
    // The log was written `.new`-then-rename, so a crash mid-write
    // leaves the temporary behind. Sweeping the log and not its
    // scratch file would leave the same orphan under a longer name.
    let cache = cache_with(&[]);
    let state = state_with(&["anicli-update-log.json.new"]);
    let report = sweep_legacy_files(cache.path(), state.path());
    assert_eq!(report.removed.len(), 1);
    assert!(report.removed[0].ends_with("anicli-update-log.json.new"));
}

#[test]
fn the_state_dirs_live_files_are_untouched() {
    // The state dir also holds the watch history and the account
    // tokens. Losing either would be considerably worse than the
    // orphan this sweep exists to clear.
    let cache = cache_with(&[]);
    let state = state_with(&["anicli-update-log.json", "history", "accounts.json"]);
    let report = sweep_legacy_files(cache.path(), state.path());
    assert_eq!(report.removed.len(), 1, "only the retired log");
    for kept in ["history", "accounts.json"] {
        assert!(state.path().join(kept).exists(), "{kept} must survive");
    }
}

#[test]
fn a_sweep_reports_the_script_and_the_log_together() {
    // One upgrade clears both, and the diagnostics block names each.
    let cache = cache_with(&["ani-cli"]);
    let state = state_with(&["anicli-update-log.json"]);
    assert_eq!(
        sweep_legacy_files(cache.path(), state.path()).removed.len(),
        2
    );
}

#[test]
fn an_interrupted_updates_staging_copy_is_removed() {
    // The updater staged an upstream-shaped copy beside the script
    // before going to the network, named with its own pid so two
    // launches could not collide, and removed it when the run ended.
    // A process killed mid-update never got there — so the file it
    // left is the one thing the retired machinery could not clean up
    // after itself.
    let cache = cache_with(&["ani-cli", "ani-cli.update-staging.4242"]);
    let report = sweep_legacy_files(cache.path(), state_with(&[]).path());
    assert_eq!(report.removed.len(), 2);
    assert!(!cache.path().join("ani-cli.update-staging.4242").exists());
}

#[test]
fn every_staging_copy_goes_not_just_the_first() {
    // Several interrupted launches leave several pids behind.
    let cache = cache_with(&[
        "ani-cli.update-staging.1",
        "ani-cli.update-staging.2",
        "ani-cli.update-staging.3",
    ]);
    assert_eq!(
        sweep_legacy_files(cache.path(), state_with(&[]).path())
            .removed
            .len(),
        3
    );
}

#[test]
fn a_staging_name_without_a_pid_suffix_survives() {
    // The updater only ever produced a process id after the prefix.
    // Anything else wearing that prefix was written by someone else,
    // and the argument for deleting it — that the machinery which
    // made it is gone — does not apply.
    let cache = cache_with(&[
        "ani-cli.update-staging.notes",
        "ani-cli.update-staging.",
        "ani-cli.update-staging.12a",
    ]);
    let report = sweep_legacy_files(cache.path(), state_with(&[]).path());
    assert!(
        report.removed.is_empty(),
        "only pid-suffixed names are ours"
    );
    assert_eq!(std::fs::read_dir(cache.path()).expect("read").count(), 3);
}

#[test]
fn a_cache_entry_that_merely_starts_with_the_script_name_survives() {
    // The prefix has to be the staging one specifically. A user's own
    // `ani-cli.bak`, or anything else that happens to begin with the
    // script's name, is not ours to delete.
    let cache = cache_with(&["ani-cli.bak", "ani-cli-notes.txt"]);
    let report = sweep_legacy_files(cache.path(), state_with(&[]).path());
    assert!(report.removed.is_empty());
    for kept in ["ani-cli.bak", "ani-cli-notes.txt"] {
        assert!(cache.path().join(kept).exists(), "{kept} must survive");
    }
}
