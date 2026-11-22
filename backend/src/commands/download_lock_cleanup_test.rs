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

/// Whether `path` is gone, allowing for a delete that a moment's outside
/// handle refused (Windows Defender or the indexer opening a new file)
/// and that is being retried off the releasing thread.
async fn gone_soon(path: &std::path::Path) -> bool {
    let settled = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    while path.exists() && tokio::time::Instant::now() < settled {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    !path.exists()
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
        gone_soon(&lock_path).await,
        "a released lock must not leave its file behind"
    );
    assert!(
        gone_soon(lock_path.parent().expect("lock dir")).await,
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
    assert!(gone_soon(second_lock.parent().expect("lock dir")).await);
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
    assert!(
        gone_soon(&lock_path).await,
        "the last holder removes the file"
    );
    assert!(gone_soon(lock_path.parent().expect("lock dir")).await);
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

    assert!(gone_soon(&stale).await, "an unheld leftover must be swept");
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
    assert!(
        gone_soon(&lock_path).await,
        "its holder removes it on release"
    );
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

/// An acquisition cancelled at any point leaves nothing behind.
///
/// Cancelling drops the future wherever it is, including while a
/// filesystem call it handed to the blocking pool is still running —
/// and that call carries on regardless. A lock file opened or locked
/// there has to be released by whoever ends up owning it, or every
/// cancel in that window keeps one.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_acquisitions_leave_nothing_behind() {
    let dest = tempfile::tempdir().expect("dest");
    // One episode per round: a later acquisition of the same name would
    // take over and remove an earlier round's leftover, hiding it.
    for round in 0..200u64 {
        let attempt = {
            let target = dest
                .path()
                .join(format!("Cancelled Show Episode {round}.mp4"));
            tokio::spawn(async move {
                let held = acquire_instance_lock(&target, long_deadline()).await;
                drop(held);
            })
        };
        // Spread the cancels across the open, the lock and the check.
        tokio::time::sleep(std::time::Duration::from_micros(round * 7 % 400)).await;
        attempt.abort();
        let _ = attempt.await;
    }
    // Let any blocking call a cancel left running finish.
    let lock_dir = dest.path().join(".ani-gui-locks");
    let left = || -> Vec<String> {
        std::fs::read_dir(&lock_dir)
            .map(|d| {
                d.flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default()
    };
    let settled = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while !left().is_empty() && tokio::time::Instant::now() < settled {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    // What a leftover is still doing, for a failure on a platform this
    // cannot be run on locally: whether its lock is free and whether
    // removing it now succeeds, which tells "nobody tried to remove it"
    // from "a removal was refused".
    let report: Vec<String> = left()
        .iter()
        .map(|name| {
            let path = lock_dir.join(name);
            let lock = std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .map(|f| match fs4::FileExt::try_lock(&f) {
                    Ok(()) => "lock free".to_owned(),
                    Err(e) => format!("lock refused: {e:?}"),
                });
            let removed = std::fs::remove_file(&path);
            format!("{name}: open {lock:?}, removal now {removed:?}")
        })
        .collect();
    assert_eq!(
        report,
        Vec::<String>::new(),
        "cancelled acquisitions kept files"
    );
}

/// The sweep never touches the lock file of a download this process
/// is running in the same folder.
///
/// On a local volume that download's lock already refuses the sweep.
/// Where locks belong to the process rather than the open file — NFS
/// emulates `flock` with POSIX record locks — the sweep would be
/// granted it, remove the file, and release the download's lock when
/// it closed its handle.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_sweep_spares_a_download_this_process_is_running() {
    let dest = tempfile::tempdir().expect("dest");
    let sibling = dest.path().join("Sibling Show Episode 2.mp4");
    let sibling_lock = target_lock_path(&sibling).expect("a lock path");
    // Registered the way a running download registers, its file
    // present but — standing in for a process-wide lock the sweep
    // would be granted — not locked against another handle.
    let _registered = super::download::target_lock(&sibling);
    drop(foreign_handle(&sibling_lock));

    let target = dest.path().join("Sibling Show Episode 1.mp4");
    let held = acquire_instance_lock(&target, long_deadline())
        .await
        .expect("a lock");
    assert!(
        sibling_lock.is_file(),
        "the sweep must not remove a file this process's own download uses"
    );
    drop(held);
}

/// A download cancelled while it waits for another holder leaves
/// nothing behind once that holder is gone.
///
/// On Windows this is the case that needs care. The waiter's open
/// handle is what makes the holder's delete fail, so the holder leaves
/// the file for the waiter, and a waiter cancelled before its next try
/// is then the only one who can remove it. On Unix the holder removes
/// the name under its lock whatever the waiter has open, so this holds
/// by construction there.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_waiter_cancelled_after_its_holder_left_leaves_nothing() {
    let dest = tempfile::tempdir().expect("dest");
    let target = dest.path().join("Waiting Show Episode 1.mp4");
    let lock_path = target_lock_path(&target).expect("a lock path");
    let held = acquire_instance_lock(&target, long_deadline())
        .await
        .expect("the first holder");
    let waiting = {
        let target = target.clone();
        tokio::spawn(async move { acquire_instance_lock(&target, long_deadline()).await })
    };
    // Long enough for the waiter to have opened the file and be
    // polling.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    drop(held);
    // Before the waiter's next try, which would take the lock.
    waiting.abort();
    let _ = waiting.await;

    let settled = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while lock_path.exists() && tokio::time::Instant::now() < settled {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(
        !lock_path.exists(),
        "a cancelled waiter must not leave the lock file behind"
    );
    assert!(gone_soon(lock_path.parent().expect("lock dir")).await);
}

/// A waiter that gives up never removes a file somebody else holds,
/// even one opened the way the standard library opens files by
/// default — sharing delete access, as an earlier version of the app
/// does. On Windows such a handle does not stop a delete, so only the
/// lock can say whether the file is free.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_waiter_that_gives_up_keeps_a_file_another_handle_holds() {
    let dest = tempfile::tempdir().expect("dest");
    let target = dest.path().join("Held Show Episode 1.mp4");
    let lock_path = target_lock_path(&target).expect("a lock path");
    let holder = foreign_handle(&lock_path);
    fs4::FileExt::lock(&holder).expect("holder locks");

    let got = acquire_instance_lock(
        &target,
        tokio::time::Instant::now() + std::time::Duration::from_millis(300),
    )
    .await;
    assert!(
        matches!(got, Err(crate::error::AniError::Timeout)),
        "the lock is held: {got:?}"
    );
    assert!(
        lock_path.is_file(),
        "a waiter giving up must not remove a file another handle holds"
    );
    drop(holder);
}

/// On Windows a release's delete that meets a brief outside handle —
/// a scanner or indexer opening the new file without sharing delete —
/// tries again for a moment instead of leaving the file for good.
#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_release_outlasts_a_brief_outside_handle_on_windows() {
    use std::os::windows::fs::OpenOptionsExt;
    let dest = tempfile::tempdir().expect("dest");
    let target = dest.path().join("Scanned Show Episode 1.mp4");
    let lock_path = target_lock_path(&target).expect("a lock path");
    let held = acquire_instance_lock(&target, long_deadline())
        .await
        .expect("a lock");
    let (opened_tx, opened_rx) = std::sync::mpsc::channel();
    let scanner = {
        let lock_path = lock_path.clone();
        std::thread::spawn(move || {
            let handle = std::fs::OpenOptions::new()
                .read(true)
                .share_mode(0x1 | 0x2)
                .open(&lock_path)
                .expect("a scanner's open");
            opened_tx.send(()).expect("signal");
            std::thread::sleep(std::time::Duration::from_millis(15));
            drop(handle);
        })
    };
    opened_rx.recv().expect("the scanner opened the file");
    drop(held);
    scanner.join().expect("scanner");
    // The retries may run after the release returns.
    let settled = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    while lock_path.exists() && tokio::time::Instant::now() < settled {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        !lock_path.exists(),
        "a release must outlast a brief outside handle"
    );
}

/// On Windows a release with a waiter behind it returns at once.
///
/// The waiter's open handle refuses every delete, and the waiter
/// removes the file itself when its own download ends, so trying again
/// only costs time — and a release runs where the download's task was
/// dropped, on a runtime worker the playback proxy shares. Whatever
/// retrying a delete still does happens off that worker.
#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_release_with_a_waiter_behind_it_returns_at_once_on_windows() {
    let dest = tempfile::tempdir().expect("dest");
    let target = dest.path().join("Queued Show Episode 1.mp4");
    let held = acquire_instance_lock(&target, long_deadline())
        .await
        .expect("the first holder");
    let waiting = {
        let target = target.clone();
        tokio::spawn(async move { acquire_instance_lock(&target, long_deadline()).await })
    };
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let started = std::time::Instant::now();
    drop(held);
    let took = started.elapsed();
    let next = waiting.await.expect("join").expect("the waiter takes over");
    drop(next);
    assert!(
        took < std::time::Duration::from_millis(30),
        "a release must not wait on a waiter's handle: took {took:?}"
    );
}

/// A directory whose permissions refuse the delete, until a thread
/// lifts them after `after` — the stand-in, where nothing else can
/// refuse a delete for a moment, for a Windows handle that closes.
/// `None` when the refusal does not hold (run as root).
#[cfg(unix)]
fn refused_for(after: std::time::Duration) -> Option<(tempfile::TempDir, std::path::PathBuf)> {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().expect("root");
    let dir = root.path().join(".ani-gui-locks");
    std::fs::create_dir(&dir).expect("lock dir");
    let file = dir.join("Refused Show Episode 1.mp4");
    std::fs::write(&file, b"").expect("lock file");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).expect("refuse");
    if std::fs::write(dir.join("probe"), b"").is_ok() {
        return None;
    }
    let lift = dir.clone();
    std::thread::spawn(move || {
        std::thread::sleep(after);
        let _ = std::fs::set_permissions(&lift, std::fs::Permissions::from_mode(0o755));
    });
    Some((root, file))
}

#[cfg(unix)]
fn refusal_passes(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::PermissionDenied
}

/// A refusal that passes is outlasted off the caller: the delete
/// returns at once, and the file and its emptied directory go once the
/// refusal lifts.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_passing_refusal_is_retried_off_the_caller() {
    let Some((_root, file)) = refused_for(std::time::Duration::from_millis(5)) else {
        return;
    };
    let started = std::time::Instant::now();
    super::download::remove_lock_file_with(&file, refusal_passes);
    assert!(
        started.elapsed() < std::time::Duration::from_millis(50),
        "the retries must not run on the caller: {:?}",
        started.elapsed()
    );
    let settled = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    while file.parent().expect("dir").exists() && tokio::time::Instant::now() < settled {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(!file.exists(), "the retry removes the file");
    assert!(
        !file.parent().expect("dir").exists(),
        "and then the directory"
    );
}

/// A refusal that outlasts the retries leaves the file in place.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refusal_that_outlasts_the_retries_leaves_the_file() {
    let Some((_root, file)) = refused_for(std::time::Duration::from_millis(600)) else {
        return;
    };
    super::download::remove_lock_file_with(&file, refusal_passes);
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(file.is_file(), "a refusal that does not pass is left alone");
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
}

/// Without a runtime the retries run in place, and the delete has
/// landed by the time the call returns.
#[cfg(unix)]
#[test]
fn without_a_runtime_the_retries_run_in_place() {
    let Some((_root, file)) = refused_for(std::time::Duration::from_millis(5)) else {
        return;
    };
    super::download::remove_lock_file_with(&file, refusal_passes);
    assert!(!file.exists(), "the in-place retries remove the file");
}

/// On Unix a refused delete is final: nothing but the lock's own
/// protocol holds or removes the name there, so the platform asks for
/// no retry, and the file stays for the next run's sweep.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn on_unix_a_refused_delete_is_not_retried() {
    let Some((_root, file)) = refused_for(std::time::Duration::from_millis(150)) else {
        return;
    };
    super::download::remove_lock_file(&file);
    // Past the refusal's end: a retry would have removed it by now.
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    assert!(file.is_file(), "a refused delete on Unix is not retried");
}

/// A waiter outlasts any number of handovers.
///
/// Each time a holder removes the file and the next one creates a
/// fresh file at the name, a waiter that was waiting on the old file
/// finds, once it gets that file's lock, that it no longer has the name,
/// and opens again. That is others making progress, not the volume
/// refusing, so it must not count toward the cap on refusals that only
/// look like a removal: a waiter queued behind a busy episode was
/// refused outright once that count ran out (seen on macOS CI, where
/// four contenders handed one lock around a hundred times).
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_waiter_outlasts_any_number_of_handovers() {
    let dest = tempfile::tempdir().expect("dest");
    let target = dest.path().join("Busy Show Episode 9.mp4");
    let lock_path = target_lock_path(&target).expect("a lock path");
    let mut holder = foreign_handle(&lock_path);
    fs4::FileExt::lock(&holder).expect("first holder");

    let waiting = {
        let target = target.clone();
        tokio::spawn(async move { acquire_instance_lock(&target, long_deadline()).await })
    };
    // More handovers than the cap allows refusals: each replaces the
    // file at the name with a fresh, held one, then lets the old go.
    for _ in 0..45 {
        tokio::time::sleep(std::time::Duration::from_millis(110)).await;
        std::fs::remove_file(&lock_path).expect("the holder removes under its lock");
        let next = foreign_handle(&lock_path);
        fs4::FileExt::lock(&next).expect("the next holder");
        drop(std::mem::replace(&mut holder, next));
    }
    drop(holder);
    let got = waiting.await.expect("join");
    assert!(
        got.is_ok(),
        "a waiter must not be refused for others' progress: {got:?}"
    );
}
