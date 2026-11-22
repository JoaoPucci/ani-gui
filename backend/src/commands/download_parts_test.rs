//! The working folder and its holds, on every platform the suite runs
//! on: nothing here stages a shell stub, so the Windows leg runs the
//! same cases against its own locking and its own hidden attribute.

use super::{owner_of, parts_dir, Entry, Hold, PARTS_DIR};

fn names(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// A dead download's files as an earlier process left them: a hold
/// nobody has open, and parts under its key.
fn leave_dead_owner(dest: &std::path::Path, key: &str) -> Vec<std::path::PathBuf> {
    let dir = parts_dir(dest);
    std::fs::create_dir_all(&dir).expect("working folder");
    std::fs::write(dir.join(format!("{key}.hold")), b"").expect("hold");
    let parts = vec![
        dir.join(format!("{key}-0123.part.mp4.part")),
        dir.join(format!("{key}-0123.part.mp4.part-Frag12")),
        dir.join(format!("{key}-0123.part.mp4.ytdl")),
    ];
    for p in &parts {
        std::fs::write(p, b"most of an episode").expect("part");
    }
    parts
}

const DEAD: &str = ".ani-gui-7-0123456789abcdef";

#[test]
fn a_hold_names_its_files_inside_the_working_folder_under_its_own_key() {
    let dest = tempfile::tempdir().expect("dest");
    let hold = Hold::take(dest.path(), false).expect("hold");
    assert_eq!(hold.dir(), parts_dir(dest.path()));
    assert!(hold.dir().is_dir(), "the working folder exists");
    let part = hold.part(".part.mp4");
    assert_eq!(part.parent(), Some(hold.dir()));
    let name = part.file_name().and_then(|n| n.to_str()).expect("name");
    let (key, kind) = owner_of(name).expect("a name the scheme gave out");
    assert_eq!(kind, Entry::Part);
    let hold_name = format!("{key}.hold");
    assert_eq!(names(hold.dir()), vec![hold_name], "the hold is its key");
    assert_ne!(part, hold.part(".part.mp4"), "every part name is fresh");
}

#[test]
fn a_scratch_name_adds_no_more_to_the_path_than_the_old_one_did() {
    // Windows paths have a ceiling, and every name a tool derives in
    // the working folder carries the scratch's on top of the
    // destination's path — yt-dlp adds `.part-Frag<n>.part` to it. The
    // scratch used to sit beside the target as
    // `.ani-gui-<pid>-<32 hex>.part.mp4`, about 55 characters; the
    // working folder and the hold's key must not make it longer.
    let dest = tempfile::tempdir().expect("dest");
    let hold = Hold::take(dest.path(), false).expect("hold");
    let scratch = hold.part(".part.mp4");
    let added = scratch
        .strip_prefix(dest.path())
        .expect("inside the destination")
        .as_os_str()
        .len();
    // `/.ani-gui-parts/` and the key and a counter: room for a
    // ten-digit pid.
    assert!(
        added <= 64,
        "{added} characters on top of the destination: {}",
        scratch.display()
    );
}

#[test]
fn a_dropped_hold_takes_its_files_its_hold_and_the_empty_folder() {
    let dest = tempfile::tempdir().expect("dest");
    let hold = Hold::take(dest.path(), false).expect("hold");
    let part = hold.part(".part.mp4");
    std::fs::write(&part, b"x").expect("scratch");
    std::fs::write(format!("{}.part-Frag3.part", part.display()), b"x").expect("fragment");
    drop(hold);
    assert!(
        !parts_dir(dest.path()).exists(),
        "nothing is left in the destination: {:?}",
        names(&parts_dir(dest.path()))
    );
    assert!(names(dest.path()).is_empty());
}

#[test]
fn a_dropped_hold_leaves_another_downloads_files_and_the_folder_they_are_in() {
    let dest = tempfile::tempdir().expect("dest");
    let other = Hold::take(dest.path(), false).expect("other");
    let theirs = other.part(".part.mp4.part");
    std::fs::write(&theirs, b"their episode").expect("theirs");
    let mine = Hold::take(dest.path(), false).expect("mine");
    std::fs::write(mine.part(".part.mp4"), b"x").expect("mine");
    drop(mine);
    assert!(
        theirs.exists(),
        "another download's part is not this hold's"
    );
    assert!(
        parts_dir(dest.path()).is_dir(),
        "their files keep the folder"
    );
    drop(other);
    assert!(!parts_dir(dest.path()).exists());
}

#[test]
fn the_sweep_removes_a_dead_downloads_files_and_hold() {
    let dest = tempfile::tempdir().expect("dest");
    let parts = leave_dead_owner(dest.path(), DEAD);
    let hold = Hold::take(dest.path(), true).expect("hold");
    for p in &parts {
        assert!(!p.exists(), "{} is a dead download's", p.display());
    }
    assert!(
        !parts_dir(dest.path()).join(format!("{DEAD}.hold")).exists(),
        "the dead hold goes too"
    );
    drop(hold);
    assert!(!parts_dir(dest.path()).exists());
}

#[test]
fn the_sweep_removes_parts_whose_hold_is_gone() {
    // A hold exists before its first part and goes after its last, so
    // parts without one have no writer: a crash between removals.
    let dest = tempfile::tempdir().expect("dest");
    let parts = leave_dead_owner(dest.path(), DEAD);
    std::fs::remove_file(parts_dir(dest.path()).join(format!("{DEAD}.hold"))).expect("unhold");
    drop(Hold::take(dest.path(), true).expect("hold"));
    for p in &parts {
        assert!(!p.exists(), "{} has no writer", p.display());
    }
}

#[test]
fn the_sweep_keeps_a_running_downloads_files() {
    // Another download — this process's or another instance's — holds
    // its key locked for as long as it writes.
    let dest = tempfile::tempdir().expect("dest");
    let running = Hold::take(dest.path(), false).expect("running");
    let theirs = running.part(".part.mp4.part-Frag16.part");
    std::fs::write(&theirs, b"in flight").expect("theirs");
    let sweeper = Hold::take(dest.path(), true).expect("sweeper");
    assert!(theirs.exists(), "a running download's fragment is kept");
    drop(sweeper);
    assert!(theirs.exists(), "and kept again when the sweeper ends");
    drop(running);
    assert!(!parts_dir(dest.path()).exists());
}

#[test]
fn the_sweep_keeps_a_hold_another_handle_has_locked() {
    // The same, with the lock held by a handle of its own rather than
    // by a hold: what a second app instance looks like from here.
    let dest = tempfile::tempdir().expect("dest");
    let parts = leave_dead_owner(dest.path(), DEAD);
    let foreign = std::fs::OpenOptions::new()
        .write(true)
        .open(parts_dir(dest.path()).join(format!("{DEAD}.hold")))
        .expect("their hold");
    fs4::FileExt::lock(&foreign).expect("their lock");
    drop(Hold::take(dest.path(), true).expect("hold"));
    for p in &parts {
        assert!(p.exists(), "{} is still being written", p.display());
    }
    drop(foreign);
}

#[test]
fn the_sweep_leaves_names_it_did_not_give_out() {
    let dest = tempfile::tempdir().expect("dest");
    let dir = parts_dir(dest.path());
    std::fs::create_dir_all(&dir).expect("dir");
    let foreign = dir.join("notes.txt");
    let near = dir.join(".ani-gui-7-nothex.part.mp4");
    std::fs::write(&foreign, b"a user's").expect("foreign");
    std::fs::write(&near, b"not a key").expect("near");
    drop(Hold::take(dest.path(), true).expect("hold"));
    assert!(foreign.exists() && near.exists());
    assert!(
        dir.is_dir(),
        "a folder with someone else's file in it stays"
    );
}

#[test]
fn the_working_folder_is_a_child_of_the_destination() {
    // Publishing is a rename or a link, both of which need one volume.
    let dest = std::path::Path::new("/media/videos");
    assert_eq!(parts_dir(dest), dest.join(PARTS_DIR));
    assert!(PARTS_DIR.starts_with('.'), "dot-hidden off Windows");
}

#[cfg(windows)]
fn is_hidden(path: &std::path::Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    std::fs::metadata(path).is_ok_and(|m| m.file_attributes() & 0x2 != 0)
}

#[cfg(windows)]
#[test]
fn on_windows_the_working_folder_and_its_hold_are_hidden() {
    // A dot hides nothing in Explorer: the folder carries the attribute.
    let dest = tempfile::tempdir().expect("dest");
    let hold = Hold::take(dest.path(), false).expect("hold");
    assert!(is_hidden(hold.dir()), "the working folder is hidden");
    let held = names(hold.dir());
    assert_eq!(held.len(), 1);
    assert!(is_hidden(&hold.dir().join(&held[0])), "the hold is hidden");
    assert!(hold.dir().is_dir(), "hiding kept it a directory");
}

#[cfg(windows)]
#[test]
fn on_windows_a_folder_left_visible_is_hidden_by_the_next_hold() {
    // A folder an interrupted run created before it could hide it, or
    // one the user unhid, is hidden again by the next download.
    let dest = tempfile::tempdir().expect("dest");
    std::fs::create_dir_all(parts_dir(dest.path())).expect("visible folder");
    assert!(!is_hidden(&parts_dir(dest.path())));
    let hold = Hold::take(dest.path(), true).expect("hold");
    assert!(is_hidden(hold.dir()));
}

#[cfg(windows)]
#[test]
fn on_windows_a_running_downloads_hold_cannot_be_removed_by_a_sweep() {
    // Opened without delete sharing: the name outlives every sweep
    // for as long as its owner has it open, so a sweep that found it
    // before the lock cannot take it away.
    let dest = tempfile::tempdir().expect("dest");
    let running = Hold::take(dest.path(), false).expect("running");
    let held = names(running.dir());
    let path = running.dir().join(&held[0]);
    assert!(std::fs::remove_file(&path).is_err(), "the hold is pinned");
    drop(running);
    assert!(!parts_dir(dest.path()).exists(), "and goes with its owner");
}

#[test]
fn a_listing_that_missed_a_running_downloads_hold_does_not_cost_it_its_parts() {
    // A directory listing is not a snapshot: a name created while it
    // runs may be returned or not, so a hold made mid-listing can be
    // missing beside a part that is not. Parts are no one's only when
    // their hold is really gone, not when a listing did not show it.
    let dest = tempfile::tempdir().expect("dest");
    let running = Hold::take(dest.path(), false).expect("running");
    let theirs = running.part(".part.mp4.part");
    std::fs::write(&theirs, b"in flight").expect("theirs");
    let sweeper = Hold::take(dest.path(), false).expect("sweeper");
    let listed = vec![theirs.file_name().expect("name").to_os_string()];
    sweeper.sweep_listed(listed);
    assert!(
        theirs.exists(),
        "a running download's part survives a listing that missed its hold"
    );
    drop(sweeper);
    drop(running);
}
