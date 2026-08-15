//! Property and cross-platform coverage for the download lock.
//!
//! Its own file rather than an append to `download_test.rs`, matching
//! the other property modules: appended blocks collide with every
//! later addition to the same file.
//!
//! Nothing here is `#[cfg(unix)]`. The cases in `download_test.rs`
//! that contend the lock have to stage a shell stub, which is why
//! they are unix-only — and that left the Windows leg compiling this
//! path without ever running it, on the one platform whose file
//! locking is a different system call. These exercise the lock
//! itself, so they need no stub and run wherever the suite runs.

use super::download::{acquire_instance_lock, target_lock_path};
use crate::error::AniError;

/// The lock actually excludes a second handle, and lets go afterwards.
///
/// This is the whole premise: `flock` on unix and `LockFileEx` on
/// Windows are different mechanisms reached through one `fs4` call,
/// and only one of them had ever been run. A platform where the
/// second acquisition succeeded immediately would leave two app
/// instances writing one file while every unix test stayed green.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_held_lock_excludes_a_second_holder_and_releases() {
    let dest = tempfile::tempdir().expect("dest");
    let target = dest.path().join("Locked Show Episode 1.mp4");
    let lock_path = target_lock_path(&target).expect("a lock path for the target");
    std::fs::create_dir_all(lock_path.parent().expect("lock dir")).expect("stage the lock dir");
    let foreign = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .expect("lock file");
    fs4::FileExt::lock(&foreign).expect("foreign lock");

    let blocked = acquire_instance_lock(
        &target,
        tokio::time::Instant::now() + std::time::Duration::from_millis(300),
    )
    .await;
    assert!(
        matches!(blocked, Err(AniError::Timeout)),
        "a lock another handle holds must not be acquirable: {blocked:?}"
    );

    fs4::FileExt::unlock(&foreign).expect("unlock");
    let freed = acquire_instance_lock(
        &target,
        tokio::time::Instant::now() + std::time::Duration::from_secs(5),
    )
    .await;
    assert!(
        freed.is_ok(),
        "the lock must be acquirable once its holder releases: {freed:?}"
    );
}

/// Names that differ only in case take one lock.
///
/// Windows and the default macOS filesystem resolve
/// `Show Episode 1.mp4` and `show episode 1.mp4` to the same file, so
/// two downloads whose resolved titles differ only in case contend
/// for one target while hashing to two different locks — and the
/// repackage path then removes and replaces the other's output.
///
/// Folded unconditionally rather than per-platform. On a
/// case-sensitive filesystem those really are two files, and folding
/// makes them queue behind each other when they need not: a download
/// waits, which the user may notice and nothing loses. Not folding on
/// a case-insensitive one loses a finished file, which nobody sees
/// until it is gone. The costs are not comparable, and a `cfg` on
/// target_os would be guessing anyway — Linux mounts NTFS, and macOS
/// can be formatted either way.
#[test]
fn case_equivalent_targets_share_one_lock() {
    let dest = std::path::Path::new("/tmp/ani-gui-prop");
    assert_eq!(
        target_lock_path(&dest.join("Show Episode 1.mp4")),
        target_lock_path(&dest.join("show episode 1.MP4")),
        "on a case-insensitive filesystem these are one file and must be one lock"
    );
}

/// Case-equivalence the filesystem sees but lowercasing does not.
///
/// NTFS compares by uppercasing through the volume's case table, so
/// Greek `σ` and final-sigma `ς` are one character to it — and one
/// file. Rust's `to_lowercase` keeps them apart, because lowercasing
/// `ς` is `ς`; only the uppercase direction collapses the pair.
///
/// So the fold goes up and then back down. Every pair either
/// direction alone would collapse, this collapses too, plus the ones
/// only uppercasing reaches. It over-folds in places NTFS does not —
/// `ß` becomes `ss` where the case table leaves it — which is the
/// direction that costs a wait rather than a file.
#[test]
fn sigma_variants_that_are_one_file_take_one_lock() {
    let dest = std::path::Path::new("/tmp/ani-gui-prop");
    assert_eq!(
        target_lock_path(&dest.join("Show σ Episode 1.mp4")),
        target_lock_path(&dest.join("Show ς Episode 1.mp4")),
        "NTFS uppercases both to Σ, so these are one file and must be one lock"
    );
}

proptest::proptest! {
    /// The lock never leaves the directory of the file it guards.
    ///
    /// The property the location has to satisfy, generated rather
    /// than tabulated because the ways a path escapes its parent are
    /// the ones an example does not think to write: a name that is
    /// itself dotted, one that looks like a traversal, a directory
    /// several levels down.
    #[test]
    fn the_lock_stays_in_the_targets_directory(
        dirs in proptest::collection::vec("[a-z0-9 ]{1,8}", 1..4),
        stem in "[a-zA-Z0-9 ._-]{1,40}",
    ) {
        let dest = std::path::PathBuf::from("/tmp").join(dirs.join("/"));
        let target = dest.join(format!("{stem}.mp4"));
        let path = target_lock_path(&target).expect("a lock path");
        proptest::prop_assert!(
            path.starts_with(&dest),
            "escaped its directory: {} not under {}",
            path.display(),
            dest.display()
        );
    }

    /// Same target, same lock — every time, in every process.
    ///
    /// Two instances only meet on this file if they both compute the
    /// same name from the same path, so a naming step that carried
    /// any per-run state would be silently useless.
    #[test]
    fn one_target_always_names_one_lock(stem in "[a-zA-Z0-9 ._-]{1,40}") {
        let target = std::path::Path::new("/tmp/ani-gui-prop").join(format!("{stem}.mp4"));
        proptest::prop_assert_eq!(
            target_lock_path(&target),
            target_lock_path(&target)
        );
    }

    /// Two targets in one directory never share a lock.
    ///
    /// Sharing would serialize unrelated episodes — a range download
    /// is the common case — and, worse, make the dock look like it
    /// had stalled for a reason nobody could see.
    ///
    /// Distinct up to case, because case-equivalent names are one
    /// file wherever the filesystem says so and share a lock on
    /// purpose; that is the case above.
    #[test]
    fn distinct_targets_take_distinct_locks(
        a in "[a-zA-Z0-9 ._-]{1,40}",
        b in "[a-zA-Z0-9 ._-]{1,40}",
    ) {
        proptest::prop_assume!(a.to_lowercase() != b.to_lowercase());
        let dest = std::path::Path::new("/tmp/ani-gui-prop");
        proptest::prop_assert_ne!(
            target_lock_path(&dest.join(format!("{a}.mp4"))),
            target_lock_path(&dest.join(format!("{b}.mp4")))
        );
    }

    /// The lock is never the target itself, whatever the name looks
    /// like. Opening the target for writing to lock it would create
    /// or truncate the very file the download is about to produce.
    #[test]
    fn the_lock_is_never_the_target(stem in "[a-zA-Z0-9 ._-]{1,40}") {
        let target = std::path::Path::new("/tmp/ani-gui-prop").join(format!("{stem}.mp4"));
        proptest::prop_assert_ne!(
            target_lock_path(&target).expect("a lock path"),
            target.clone()
        );
    }
}
