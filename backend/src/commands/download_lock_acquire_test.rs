//! Opening the lock file while other downloads remove its directory.

use super::{open_creating_with, os};
use crate::commands::download_lock_deadline_test::Deadline;

/// What creating a directory reports when it found the directory there
/// and a releasing download removed it before the check that it is one:
/// the standard library's `create_dir_all` answers "already exists"
/// for a directory that, by the time it returns, does not.
fn found_then_removed(_dir: &std::path::Path) -> std::io::Result<()> {
    Err(std::io::ErrorKind::AlreadyExists.into())
}

/// A lock directory removed while it was being created is a loss to a
/// removal, which the waiter answers by trying again, not a refusal.
///
/// Every release removes the directory once it is empty, so a download
/// that starts as another one ends can find the directory present, then
/// gone before the creation's own check. Refused there, it gave up the
/// lock with an I/O error — on macOS CI within a fraction of a second,
/// four contenders taking one lock in turn.
#[test]
fn a_lock_directory_removed_while_it_was_created_is_a_loss() {
    let _deadline = Deadline::arm();
    let dest = tempfile::tempdir().expect("dest");
    let path = dest
        .path()
        .join(".ani-gui-locks")
        .join("Raced Show Episode 1.mp4");
    match open_creating_with(&path, found_then_removed) {
        Err(e) => assert!(
            os::lost_to_removal(&e),
            "a directory removed mid-creation must read as a loss: {e:?}"
        ),
        Ok(_) => panic!("there is no directory to open the file in"),
    }
}

/// The same race, with another download having made the directory again
/// by the check: the open goes ahead in it.
#[test]
fn a_lock_directory_made_again_while_it_was_created_is_opened_in() {
    let _deadline = Deadline::arm();
    let dest = tempfile::tempdir().expect("dest");
    let path = dest
        .path()
        .join(".ani-gui-locks")
        .join("Raced Show Episode 2.mp4");
    let remade = |dir: &std::path::Path| {
        std::fs::create_dir(dir).expect("another download makes it");
        found_then_removed(dir)
    };
    assert!(
        open_creating_with(&path, remade).is_ok(),
        "a directory that is there must be opened in"
    );
}

/// Something that is not a directory at the directory's name stays a
/// refusal, at once: no removal is in progress there.
#[test]
fn a_file_at_the_directory_name_is_still_refused() {
    let _deadline = Deadline::arm();
    let dest = tempfile::tempdir().expect("dest");
    let dir = dest.path().join(".ani-gui-locks");
    std::fs::write(&dir, b"").expect("stage an obstruction");
    let path = dir.join("Blocked Show Episode 1.mp4");
    match open_creating_with(&path, |d| std::fs::create_dir_all(d)) {
        Err(e) => assert!(
            !os::lost_to_removal(&e),
            "an obstruction is not a removal in progress: {e:?}"
        ),
        Ok(_) => panic!("a file is in the directory's way"),
    }
}
