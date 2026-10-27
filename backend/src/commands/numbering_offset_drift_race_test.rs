//! A stamp repair racing another writer of the offsets store.
//!
//! [`put_drift_right`] decides from reads of the store and the history
//! file taken without the store's locks, and either can change before
//! the repair holds them: another app instance writes the store under
//! the file lock alone and its history file under no lock of the
//! store's, and this process's own puts can land first on the mutex.
//! So the repair decides again, under both locks, against the store
//! row it is about to rewrite and the history row as it stands then.
//! Each case here plays the other instance: it holds the file lock,
//! lets the repair queue behind it, writes the store or the history
//! file as that instance would, and then lets the repair go.

use super::put_drift_right;
use crate::app::AppState;
use crate::scraper::provider::EpisodeRef;

const SLUG: &str = "the-show-77";

fn row(number: u32, tag: Option<&str>) -> EpisodeRef {
    EpisodeRef {
        id: u64::from(number),
        number,
        number2: tag.map(str::to_string),
        former_tag: None,
        title: None,
    }
}

/// Seven episodes with a recap after the third: slot 4 is the recap
/// "3.5", and slots 5 to 8 are episodes 4 to 7.
fn recap_after_three() -> Vec<EpisodeRef> {
    vec![
        row(1, None),
        row(2, None),
        row(3, None),
        row(4, Some("3.5")),
        row(5, Some("4")),
        row(6, Some("5")),
        row(7, Some("6")),
        row(8, Some("7")),
    ]
}

fn state(td: &tempfile::TempDir) -> AppState {
    let mut state = crate::commands::play::tests::state_with_proxy_origin();
    state.config_path = td.path().join("config.toml");
    state.history_path = td.path().join("history");
    state
}

/// Episode 5 watched before the marker (slot 6), then a warm of
/// episode 6 moved the stamp to slot 7: the row reads as 6.
fn seed_drifted_row(state: &AppState) {
    let row = crate::history::parse(&format!("6\t{SLUG}\tThe Show\n"))
        .pop()
        .expect("a row");
    crate::history::upsert_and_write(&state.history_path, row).expect("write");
    crate::commands::numbering_offset::put_display(state, SLUG, 0, 7, "6");
}

fn store_body(state: &AppState) -> String {
    std::fs::read_to_string(state.history_path.with_file_name("ani-gui-offsets"))
        .unwrap_or_default()
}

/// Write `body` as the store, as the other instance's rename does.
fn store_is(history: &std::path::Path, body: &str) {
    std::fs::write(history.with_file_name("ani-gui-offsets"), body)
        .expect("the other instance's write");
}

/// Write the show's history row holding `ep_no`, as a play writes it.
fn row_is(history: &std::path::Path, ep_no: &str) {
    let row = crate::history::parse(&format!("{ep_no}\t{SLUG}\tThe Show\n"))
        .pop()
        .expect("a row");
    crate::history::upsert_and_write(history, row).expect("the row");
}

/// Run the repair from `recap_after_three` while another instance holds
/// the store's file lock; once the repair is queued behind it, that
/// instance's writes, `meanwhile` (given the history file's path), land,
/// and the lock is let go. The store's body after the repair.
fn repair_racing(state: AppState, meanwhile: impl FnOnce(&std::path::Path)) -> String {
    let history = state.history_path.clone();
    let lock_path = history.with_file_name("ani-gui-offsets.lock");
    let foreign = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .expect("lock file");
    fs4::FileExt::lock(&foreign).expect("foreign lock");
    let queue = crate::held::queue(lock_path.display().to_string());
    let (tx, rx) = std::sync::mpsc::channel();
    let repair = std::thread::spawn(move || {
        put_drift_right(&state, SLUG, &recap_after_three(), None);
        tx.send(state).ok();
    });
    // Behind the lock: the unlocked decision is made, the write is not.
    queue.joined_blocking(|| rx.try_recv().is_ok());
    meanwhile(&history);
    fs4::FileExt::unlock(&foreign).expect("unlock");
    let state = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the repair never finished after the lock was released");
    repair.join().expect("repair thread");
    store_body(&state)
}

#[test]
fn a_store_on_another_offset_by_the_time_the_repair_writes_is_left_as_it_is() {
    // A build from before rows recorded the app's own number, running
    // beside this one on the same files, stamped the offset on every
    // play and warm; its warm of a re-listing numbered the show on 12. The
    // listing at hand numbers it on 0, so it no longer proves anything
    // about the row, and the repair decided on the store as it was must
    // not land on the store as it is.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_drifted_row(&state);
    let theirs = format!("{SLUG}\t12\t7\t6\n");
    assert_eq!(repair_racing(state, |h| store_is(h, &theirs)), theirs);
}

#[test]
fn a_stamp_moved_to_a_slot_the_listing_tags_otherwise_is_left_as_it_is() {
    // That build's warm, of a show it had listed otherwise, stamped
    // slot 8 with a tag the listing at hand does not give it: the
    // listings disagree, and nothing is known.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_drifted_row(&state);
    let theirs = format!("{SLUG}\t0\t8\t6\n");
    assert_eq!(repair_racing(state, |h| store_is(h, &theirs)), theirs);
}

#[test]
fn a_stamp_the_other_instance_already_put_right_keeps_what_it_kept() {
    // The other instance's repair landed first, from a listing that
    // recorded the tag an earlier version gave the row. The stamp names
    // the row already, so there is nothing left to decide, and the tag
    // it kept beside the stamp stays.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_drifted_row(&state);
    let theirs = format!("{SLUG}\t0\t6\t5\t6\t4.5\n");
    assert_eq!(repair_racing(state, |h| store_is(h, &theirs)), theirs);
}

#[test]
fn a_stamp_moved_to_a_slot_the_listing_agrees_with_is_still_put_right() {
    // That build's warm of episode 7 moved the stamp again, to a slot
    // the listing tags as the stamp does, and left the row as it was: the
    // listing still proves the row's slot, and the repair still lands.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_drifted_row(&state);
    let theirs = format!("{SLUG}\t0\t8\t7\n");
    assert_eq!(
        repair_racing(state, |h| store_is(h, &theirs)),
        format!("{SLUG}\t0\t6\t5\n")
    );
}

#[test]
fn a_show_forgotten_while_the_repair_waited_gets_no_row_back() {
    // The show was removed from history, its numbering with it; the
    // store keeps every other show's row. A repair must not write a row
    // for a show the store no longer has.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_drifted_row(&state);
    let theirs = "another-show-12\t0\n";
    let forget = |h: &std::path::Path| {
        std::fs::write(h, "").expect("the other instance's removal");
        store_is(h, theirs);
    };
    assert_eq!(repair_racing(state, forget), theirs);
}

#[test]
fn a_play_caught_between_its_stamp_and_its_row_is_put_right_by_the_next_listing() {
    // Builds from before rows recorded the app's own number stamp a
    // play and then write its row, and the history file is written
    // outside the store's locks. A repair that runs between the two
    // sees the new stamp beside the old row: what a warm moving the
    // stamp looks like, and nothing it reads tells them apart. It puts
    // the stamp on the old row's slot. Once the row lands, the next
    // listing at hand puts the stamp on the new row.
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    seed_drifted_row(&state);
    let history = state.history_path.clone();
    let stamped = |h: &std::path::Path| store_is(h, &format!("{SLUG}\t0\t8\t7\n"));
    assert_eq!(repair_racing(state, stamped), format!("{SLUG}\t0\t6\t5\n"));
    row_is(&history, "8");
    let state = self::state(&td);
    put_drift_right(&state, SLUG, &recap_after_three(), None);
    assert_eq!(store_body(&state), format!("{SLUG}\t0\t8\t7\n"));
}
