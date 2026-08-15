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

/// The lock file is the target's own name plus a suffix.
///
/// This is what makes the lock correct rather than approximately
/// correct, and it replaces two cases that asserted the mechanism
/// instead — that case-equivalent names and sigma variants hash to
/// one path. They were checking a case table this code should never
/// have been keeping.
///
/// Every question about whether two names are one file is the
/// filesystem's to answer, and it answers about `Show.mp4.lock`
/// exactly as it answers about `Show.mp4`: same case rules, same
/// normalization rules, same trailing-dot rules, same anything a
/// future volume invents. Hashing a folded key threw that away and
/// substituted a table — which is why case, then final sigma, then
/// canonical normalization each had to be found separately.
///
/// There is no bound and no fallback. A digest past some byte
/// threshold was the shape before this, and it put the table back: the
/// threshold is measured on the spelling, so two spellings of one file
/// can straddle it and then differ again once hashed. Carrying the
/// name into a directory of its own needs no threshold, because a name
/// that fits as a target fits as a sibling of one.
#[test]
fn the_lock_is_named_for_the_target_verbatim() {
    let dest = std::path::Path::new("/tmp/ani-gui-prop");
    for stem in [
        "Show Episode 1",
        "Pokémon Episode 1",
        "Show σ",
        "Show ς",
        "SHOW",
    ] {
        let target = dest.join(format!("{stem}.mp4"));
        let lock = target_lock_path(&target).expect("a lock path");
        assert_eq!(
            lock.file_name().and_then(|n| n.to_str()),
            Some(format!("{stem}.mp4").as_str()),
            "the target's name has to appear verbatim for the filesystem to \
             answer equivalence about it"
        );
    }
}

/// There is no length at which the name stops being carried.
///
/// A digest fallback past a byte threshold reintroduced exactly what
/// the delegation removes: the threshold is measured on the spelling,
/// so two spellings one file can differ in length and take different
/// branches — and once hashed, they differ again. Carrying the name
/// as a directory has no such limit, because the name already fits
/// where the target itself fits.
#[test]
fn an_overlong_name_is_still_carried_verbatim() {
    let dest = std::path::Path::new("/tmp/ani-gui-prop");
    let stem = "n".repeat(300);
    let target = dest.join(format!("{stem}.mp4"));
    let lock = target_lock_path(&target).expect("a lock path");
    assert_eq!(
        lock.file_name().and_then(|n| n.to_str()),
        Some(format!("{stem}.mp4").as_str()),
        "no threshold may switch the naming, or the two branches disagree"
    );
}

/// The lock adds one fixed directory component and nothing else.
///
/// A destination deep enough that the target only just fits under
/// PATH_MAX leaves very little room, and a lock path that overshoots
/// refuses every download to an otherwise writable target — the
/// fail-closed path turning into a denial. Nothing can make the lock
/// shorter than the target while still carrying its name, so the
/// bound is that it adds exactly `/.ani-gui-locks`: sixteen bytes,
/// once, no per-name growth.
#[test]
fn the_lock_path_adds_only_its_directory() {
    let dest = std::path::Path::new("/tmp/ani-gui-prop");
    let target = dest.join("Show Episode 1.mp4");
    let lock = target_lock_path(&target).expect("a lock path");
    assert_eq!(
        lock.as_os_str().len(),
        target.as_os_str().len() + "/.ani-gui-locks".len(),
        "the lock must not grow with the name: {}",
        lock.display()
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
    #[test]
    fn distinct_targets_take_distinct_locks(
        a in "[a-zA-Z0-9 ._-]{1,40}",
        b in "[a-zA-Z0-9 ._-]{1,40}",
    ) {
        proptest::prop_assume!(a != b);
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
