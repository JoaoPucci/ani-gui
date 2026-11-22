//! What the old-scratch sweep removes from a folder, and what it
//! leaves: everything that is not a stale regular file of the old
//! shape.

use super::{sweep, OldScratchVisits, MIN_AGE};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const HEX: &str = "0123456789abcdef0123456789abcdef";
const HOUR: Duration = Duration::from_secs(60 * 60);

fn old(tail: &str) -> String {
    format!(".ani-gui-4242-{HEX}{tail}")
}

/// Write `name` in `dir`, last written at `at`.
fn file_at(dir: &Path, name: &str, at: SystemTime) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, b"most of an episode").expect("write");
    std::fs::File::options()
        .write(true)
        .open(&path)
        .expect("open")
        .set_modified(at)
        .expect("set mtime");
    path
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn stale_leftovers_of_every_shape_go() {
    let dir = tempfile::tempdir().expect("dir");
    let now = SystemTime::now();
    let stale = now - MIN_AGE - HOUR;
    for tail in [
        ".part.mp4",
        ".part.mp4.part",
        ".part.mp4.ytdl",
        ".part.mp4.part-Frag12",
        ".part.mp4.part-Frag16.part",
        ".part.temp.mp4",
        ".part.vtt",
        ".part.fhls-1080p.mp4.part",
    ] {
        file_at(dir.path(), &old(tail), stale);
    }
    file_at(dir.path(), "Frieren Episode 1.mp4", stale);
    sweep(dir.path(), now);
    assert_eq!(names(dir.path()), vec!["Frieren Episode 1.mp4"]);
}

#[test]
fn a_leftover_written_within_the_day_stays() {
    // An older version beside this one may still be writing it.
    let dir = tempfile::tempdir().expect("dir");
    let now = SystemTime::now();
    let fresh = file_at(dir.path(), &old(".part.mp4.part"), now - HOUR);
    let almost = file_at(
        dir.path(),
        &old(".part.mp4.ytdl"),
        now - MIN_AGE + Duration::from_secs(60),
    );
    sweep(dir.path(), now);
    assert!(fresh.exists());
    assert!(almost.exists(), "a minute short of a day is within it");
}

#[test]
fn exactly_a_day_is_not_past_it() {
    // Asked of the age alone: a volume with coarse timestamps (FAT
    // keeps two seconds) would move a file set to the boundary.
    let now = SystemTime::now();
    assert!(!super::is_stale(Some(now - MIN_AGE), now));
    assert!(super::is_stale(
        Some(now - MIN_AGE - Duration::from_secs(1)),
        now
    ));
    assert!(!super::is_stale(None, now), "no time is no age");
}

#[test]
fn a_leftover_dated_in_the_future_stays() {
    // A clock set back, or a volume another machine wrote: no age can
    // be told, so it is not taken for stale.
    let dir = tempfile::tempdir().expect("dir");
    let now = SystemTime::now();
    let ahead = file_at(dir.path(), &old(".part.mp4"), now + MIN_AGE * 3);
    sweep(dir.path(), now);
    assert!(ahead.exists());
}

#[test]
fn names_that_only_resemble_a_leftover_stay() {
    let dir = tempfile::tempdir().expect("dir");
    let now = SystemTime::now();
    let stale = now - MIN_AGE * 10;
    let kept = [
        // This version's key, as the working folder holds it.
        ".ani-gui-4242-0123456789abcdef-0.part.mp4".to_string(),
        ".ani-gui-4242-0123456789abcdef.hold".to_string(),
        // The user's.
        "Frieren Episode 1.part.mp4".to_string(),
        old(".part.mp4~"),
        old(".mp4"),
        format!(".ani-gui-4242-{}.part.mp4", HEX.to_uppercase()),
    ];
    for name in &kept {
        file_at(dir.path(), name, stale);
    }
    sweep(dir.path(), now);
    let mut want: Vec<String> = kept.to_vec();
    want.sort();
    assert_eq!(names(dir.path()), want);
}

#[test]
fn a_directory_of_that_name_stays_and_is_not_entered() {
    let dir = tempfile::tempdir().expect("dir");
    let now = SystemTime::now();
    let folder = dir.path().join(old(".part.mp4"));
    std::fs::create_dir(&folder).expect("dir");
    let inside = file_at(&folder, &old(".part.mp4"), now - MIN_AGE * 2);
    sweep(dir.path(), now);
    assert!(folder.is_dir());
    assert!(inside.exists());
}

#[test]
fn the_working_folder_is_not_entered() {
    // The sweep reads the folder the user chose, not this version's
    // working folder, whose files its own holds account for.
    let dir = tempfile::tempdir().expect("dir");
    let now = SystemTime::now();
    let parts = dir.path().join(".ani-gui-parts");
    std::fs::create_dir(&parts).expect("parts");
    let inside = file_at(&parts, &old(".part.mp4"), now - MIN_AGE * 2);
    sweep(dir.path(), now);
    assert!(inside.exists());
}

#[cfg(unix)]
#[test]
fn a_link_of_that_name_stays_and_is_not_followed() {
    let dir = tempfile::tempdir().expect("dir");
    let elsewhere = tempfile::tempdir().expect("elsewhere");
    let now = SystemTime::now();
    let target = file_at(elsewhere.path(), "Episode 1.mp4", now - MIN_AGE * 2);
    let link = dir.path().join(old(".part.mp4"));
    std::os::unix::fs::symlink(&target, &link).expect("link");
    sweep(dir.path(), now);
    assert!(std::fs::symlink_metadata(&link).is_ok(), "the link stays");
    assert!(target.exists(), "what it points at stays");
}

#[test]
fn a_folder_that_cannot_be_read_is_left() {
    let dir = tempfile::tempdir().expect("dir");
    sweep(&dir.path().join("gone"), SystemTime::now());
}

#[cfg(windows)]
#[test]
fn a_leftover_another_process_holds_open_stays_and_the_rest_go() {
    // yt-dlp, like most programs, opens its files without delete
    // sharing, so while an older version's tool still has one open,
    // Windows refuses its removal. That file stays; the others go.
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    let dir = tempfile::tempdir().expect("dir");
    let now = SystemTime::now();
    let stale = now - MIN_AGE * 2;
    let held = file_at(dir.path(), &old(".part.mp4.part"), stale);
    let free = file_at(dir.path(), &old(".part.mp4.ytdl"), stale);
    let handle = std::fs::File::options()
        .read(true)
        .write(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(&held)
        .expect("hold it open");
    sweep(dir.path(), now);
    assert!(held.exists(), "a file held open is left");
    assert!(!free.exists(), "the rest still go");
    drop(handle);
}

#[test]
fn each_folder_is_swept_once_per_download() {
    let mut visits = OldScratchVisits::default();
    let chosen = Path::new("/chosen");
    let show = Path::new("/chosen/Frieren");
    assert!(visits.first_visit(chosen));
    assert!(visits.first_visit(show));
    assert!(!visits.first_visit(chosen));
    assert!(!visits.first_visit(show));
    assert!(
        OldScratchVisits::default().first_visit(chosen),
        "a new download sweeps again"
    );
}

#[tokio::test]
async fn a_second_visit_by_the_same_download_does_not_sweep() {
    // A range's episodes after the first go into a folder it already
    // swept: no second listing of it. A leftover that appears after
    // the first visit is still there once a second visit's sweep would
    // have finished.
    let dir = tempfile::tempdir().expect("dir");
    let mut visits = OldScratchVisits::default();
    let first = file_at(
        dir.path(),
        &old(".part.mp4"),
        SystemTime::now() - MIN_AGE * 2,
    );
    visits.visit(dir.path());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while first.exists() {
        assert!(tokio::time::Instant::now() < deadline, "first visit sweeps");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let later = file_at(
        dir.path(),
        &old(".part.vtt"),
        SystemTime::now() - MIN_AGE * 2,
    );
    visits.visit(dir.path());
    // Longer than the first visit's sweep of this folder took.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(later.exists(), "the second visit did not sweep");
}

#[tokio::test]
async fn a_visit_sweeps_the_folder() {
    let dir = tempfile::tempdir().expect("dir");
    let leftover = file_at(
        dir.path(),
        &old(".part.mp4.part"),
        SystemTime::now() - MIN_AGE * 2,
    );
    OldScratchVisits::default().visit(dir.path());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while leftover.exists() {
        assert!(tokio::time::Instant::now() < deadline, "not swept");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
