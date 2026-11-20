//! The lock's lifetime on disk: what a download leaves behind once it
//! ends, and that tidying up keeps the exclusion it exists for.
//!
//! A lock file used to outlive its download for good, one per episode
//! ever attempted, in a `.ani-gui-locks` directory that Windows shows
//! in the download folder. Removing it is only safe if a holder that
//! deletes the file cannot leave a waiter holding a lock on a name
//! nobody else will ever open again — so most of what follows contends
//! the lock while it is being removed.
//!
//! Each call to `acquire_instance_lock` opens its own handle, so two
//! calls in one process contend exactly as two app instances do:
//! `flock` and `LockFileEx` both belong to the open file, not to the
//! process.

use super::download::{acquire_instance_lock, target_lock_path};

fn long_deadline() -> tokio::time::Instant {
    tokio::time::Instant::now() + std::time::Duration::from_secs(30)
}

/// A handle opened the way an outside party would, for staging a
/// holder or a stale file.
fn foreign_handle(path: &std::path::Path) -> std::fs::File {
    std::fs::create_dir_all(path.parent().expect("lock dir")).expect("stage the lock dir");
    std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .expect("lock file")
}

/// Ending a download takes its lock file with it, and the lock
/// directory once nothing else is in it.
///
/// Dropping the lock is how every ending reaches it — success, a
/// failure's early return, a cancel dropping the future, and a quit's
/// runtime teardown dropping the task — so this is all four.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn releasing_the_lock_removes_its_file_and_the_empty_directory() {
    let dest = tempfile::tempdir().expect("dest");
    let target = dest.path().join("Tidy Show Episode 1.mp4");
    let lock_path = target_lock_path(&target).expect("a lock path");

    let held = acquire_instance_lock(&target, long_deadline())
        .await
        .expect("an uncontended lock");
    assert!(lock_path.is_file(), "the lock file exists while it is held");
    drop(held);

    assert!(
        !lock_path.exists(),
        "a released lock must not leave its file behind"
    );
    assert!(
        !lock_path.parent().expect("lock dir").exists(),
        "an emptied lock directory must not stay in the download folder"
    );
}

/// The directory goes only when it is empty: another download's lock
/// in the same folder keeps it, and keeps its own file.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_directory_stays_while_another_lock_is_in_it() {
    let dest = tempfile::tempdir().expect("dest");
    let first = dest.path().join("Show Episode 1.mp4");
    let second = dest.path().join("Show Episode 2.mp4");

    let a = acquire_instance_lock(&first, long_deadline())
        .await
        .expect("first lock");
    let b = acquire_instance_lock(&second, long_deadline())
        .await
        .expect("second lock");
    drop(a);

    let second_lock = target_lock_path(&second).expect("a lock path");
    assert!(
        second_lock.is_file(),
        "a held lock's file survives a neighbour's cleanup"
    );
    drop(b);
    assert!(!second_lock.parent().expect("lock dir").exists());
}

/// A waiter handed the lock after its holder removed the file holds
/// the lock at the path, not on a file that no longer has a name.
///
/// This is the race that removal opens and the reason the lock is
/// checked after it is taken: the waiter opened the old file before
/// it was unlinked, and `flock` happily grants a lock on an orphan.
/// A third party opening the path would then create a fresh file and
/// lock it too, and two holders would run at once. Here the holder is
/// staged by hand so the unlink lands exactly while the waiter waits.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_waiter_whose_file_was_removed_ends_up_holding_the_path() {
    let dest = tempfile::tempdir().expect("dest");
    let target = dest.path().join("Raced Show Episode 1.mp4");
    let lock_path = target_lock_path(&target).expect("a lock path");
    let holder = foreign_handle(&lock_path);
    fs4::FileExt::lock(&holder).expect("holder locks");

    let waiting = {
        let target = target.clone();
        tokio::spawn(async move { acquire_instance_lock(&target, long_deadline()).await })
    };
    // Long enough for the waiter to have opened the file and started
    // polling for its lock.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    std::fs::remove_file(&lock_path).expect("holder removes under its lock");
    drop(holder);

    let held = waiting
        .await
        .expect("join")
        .expect("the waiter gets a lock");
    assert!(
        lock_path.is_file(),
        "the waiter's lock must be on a file that still has the lock's name"
    );
    let third = std::fs::OpenOptions::new()
        .write(true)
        .open(&lock_path)
        .expect("open the path");
    assert!(
        matches!(
            fs4::FileExt::try_lock(&third),
            Err(fs4::TryLockError::WouldBlock)
        ),
        "a newcomer at the path must find it held"
    );
    drop(third);
    drop(held);
}

/// Contenders that keep taking, releasing and removing one lock are
/// never inside it together, and the last one out leaves nothing.
///
/// The randomized companion to the staged race above: removal on
/// release without the check after locking lets two holders overlap
/// within a few rounds.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn contenders_removing_the_lock_never_hold_it_together() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let dest = tempfile::tempdir().expect("dest");
    let target = dest.path().join("Busy Show Episode 1.mp4");
    let inside = std::sync::Arc::new(AtomicUsize::new(0));
    let overlaps = std::sync::Arc::new(AtomicUsize::new(0));

    let mut tasks = Vec::new();
    for _ in 0..4 {
        let target = target.clone();
        let inside = inside.clone();
        let overlaps = overlaps.clone();
        tasks.push(tokio::spawn(async move {
            for _ in 0..25 {
                let held = acquire_instance_lock(&target, long_deadline())
                    .await
                    .expect("a lock within the deadline");
                if inside.fetch_add(1, Ordering::SeqCst) != 0 {
                    overlaps.fetch_add(1, Ordering::SeqCst);
                }
                tokio::time::sleep(std::time::Duration::from_millis(2)).await;
                inside.fetch_sub(1, Ordering::SeqCst);
                drop(held);
            }
        }));
    }
    for task in tasks {
        task.await.expect("contender");
    }

    assert_eq!(overlaps.load(Ordering::SeqCst), 0, "two holders overlapped");
    let lock_path = target_lock_path(&target).expect("a lock path");
    assert!(!lock_path.exists(), "the last holder removes the file");
    assert!(!lock_path.parent().expect("lock dir").exists());
}

/// Lock files earlier versions left behind are removed when a download
/// next uses the folder — but never one somebody still holds.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stale_lock_files_are_swept_and_held_ones_are_kept() {
    let dest = tempfile::tempdir().expect("dest");
    let stale = target_lock_path(&dest.path().join("Old Show Episode 3.mp4")).expect("stale");
    let live = target_lock_path(&dest.path().join("Old Show Episode 4.mp4")).expect("live");
    drop(foreign_handle(&stale));
    let holder = foreign_handle(&live);
    fs4::FileExt::lock(&holder).expect("another instance holds this one");

    let target = dest.path().join("New Show Episode 1.mp4");
    let held = acquire_instance_lock(&target, long_deadline())
        .await
        .expect("a lock");
    drop(held);

    assert!(!stale.exists(), "an unheld leftover must be swept");
    assert!(live.is_file(), "a lock another instance holds must survive");
    drop(holder);
}

/// A download that ends before any transfer — here, no tool to run it
/// with — leaves no lock behind either. This is how a folder came to
/// hold locks for episodes that were never downloaded: the lock is
/// taken before the checks that refuse a download, and every refused,
/// failed or cancelled attempt kept its file.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_download_that_never_starts_leaves_no_lock() {
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let got = super::download::spawn_download_tool(
        &crate::scraper::provider::StreamSource {
            master_url: "https://cdn.example/x/master.m3u8".into(),
            referer: None,
            subtitles: Vec::new(),
        },
        dest.path(),
        "Never Show Episode 1",
        None,
        &bin.path().display().to_string(),
        std::time::Duration::from_secs(5),
        &mut |_l: &str| {},
    )
    .await;
    assert!(matches!(got, Err(crate::error::AniError::FfmpegMissing)));
    assert!(
        !dest.path().join(".ani-gui-locks").exists(),
        "a refused download must not leave the lock directory behind"
    );
}

/// On Windows a leading dot hides nothing, so the directory carries
/// the hidden attribute instead.
#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_lock_directory_is_hidden_on_windows() {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    let dest = tempfile::tempdir().expect("dest");
    let target = dest.path().join("Hidden Show Episode 1.mp4");
    let held = acquire_instance_lock(&target, long_deadline())
        .await
        .expect("a lock");
    let dir = target_lock_path(&target)
        .expect("a lock path")
        .parent()
        .expect("lock dir")
        .to_path_buf();
    let attrs = std::fs::metadata(&dir).expect("lock dir").file_attributes();
    assert_ne!(
        attrs & FILE_ATTRIBUTE_HIDDEN,
        0,
        "the lock directory must be hidden: attributes {attrs:#x}"
    );
    drop(held);
}

/// On Windows nobody can remove a lock file while it is held.
///
/// That is what makes removal safe there: the handle is opened without
/// sharing delete access, so a file can be deleted only once no handle
/// is open on it at all, and no waiter can be left holding a file that
/// lost its name. Unix gets the same guarantee by checking after it
/// locks; Windows gets it from the share mode.
#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_held_lock_file_cannot_be_deleted_on_windows() {
    let dest = tempfile::tempdir().expect("dest");
    let target = dest.path().join("Pinned Show Episode 1.mp4");
    let lock_path = target_lock_path(&target).expect("a lock path");
    let held = acquire_instance_lock(&target, long_deadline())
        .await
        .expect("a lock");
    assert!(
        std::fs::remove_file(&lock_path).is_err(),
        "a held lock file must not be deletable by anyone else"
    );
    assert!(lock_path.is_file());
    drop(held);
    assert!(!lock_path.exists(), "its holder removes it on release");
}

/// A lock directory that cannot be created is a refusal, not a wait:
/// here a file already has the directory's name.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unusable_lock_directory_is_refused_at_once() {
    let dest = tempfile::tempdir().expect("dest");
    std::fs::write(dest.path().join(".ani-gui-locks"), b"").expect("stage an obstruction");
    let target = dest.path().join("Blocked Show Episode 1.mp4");
    let got = acquire_instance_lock(&target, long_deadline()).await;
    assert!(
        matches!(got, Err(crate::error::AniError::Io)),
        "an obstructed lock directory must refuse: {got:?}"
    );
}

/// A release leaves alone a file somebody else put at the lock's name
/// after removing this one from outside the protocol — it is theirs
/// now, and removing it would free a lock they hold.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_release_keeps_a_file_that_replaced_its_own() {
    let dest = tempfile::tempdir().expect("dest");
    let target = dest.path().join("Replaced Show Episode 1.mp4");
    let lock_path = target_lock_path(&target).expect("a lock path");
    let held = acquire_instance_lock(&target, long_deadline())
        .await
        .expect("a lock");
    std::fs::remove_file(&lock_path).expect("removed from outside");
    let replacement = foreign_handle(&lock_path);
    drop(held);
    assert!(
        lock_path.is_file(),
        "a release must not remove a file that is not the one it held"
    );
    drop(replacement);
}

/// An open that keeps failing the way a concurrent removal does gives
/// up after a bounded number of tries instead of polling to the
/// deadline. A dangling link at the lock's name, into a directory that
/// does not exist, makes every open report "not found".
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_open_that_keeps_losing_gives_up_before_the_deadline() {
    let dest = tempfile::tempdir().expect("dest");
    let target = dest.path().join("Dangling Show Episode 1.mp4");
    let lock_path = target_lock_path(&target).expect("a lock path");
    std::fs::create_dir_all(lock_path.parent().expect("lock dir")).expect("lock dir");
    std::os::unix::fs::symlink(dest.path().join("missing/dir/file"), &lock_path)
        .expect("stage a dangling link");
    let started = std::time::Instant::now();
    let got = acquire_instance_lock(
        &target,
        tokio::time::Instant::now() + std::time::Duration::from_secs(60),
    )
    .await;
    assert!(
        matches!(got, Err(crate::error::AniError::Io)),
        "a lock that cannot be opened must be given up: {got:?}"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(30),
        "giving up must not wait out the deadline: {:?}",
        started.elapsed()
    );
}
