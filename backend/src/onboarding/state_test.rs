//! The record on disk, and the launch's first-ever detection.

use super::*;

fn r(text: &str) -> Release {
    Release::parse(text).expect("a release")
}

#[test]
fn a_saved_record_reads_back() {
    let td = tempfile::tempdir().expect("tempdir");
    let path = path_in(td.path());
    let mut rec = Record::new_install(true, r("0.19.0"));
    rec.seen.insert("watch-order".into());
    rec.flag_on_since.insert("a_feature".into(), r("0.22.1"));
    save(&path, &rec).expect("saved");
    assert_eq!(load(&path), Loaded::Record(rec));
    assert!(
        !path.with_extension("json.tmp").exists(),
        "the temporary file was renamed over the record"
    );
}

#[test]
fn the_record_sits_in_the_state_directory() {
    let dir = Path::new("state");
    assert_eq!(path_in(dir), dir.join("onboarding.json"));
}

#[test]
fn saving_creates_the_state_directory() {
    let td = tempfile::tempdir().expect("tempdir");
    let path = path_in(&td.path().join("not-yet"));
    save(&path, &Record::default()).expect("saved");
    assert!(path.is_file());
}

#[test]
fn no_file_is_absent_and_reads_as_an_updates_empty_record() {
    let td = tempfile::tempdir().expect("tempdir");
    let loaded = load(&path_in(td.path()));
    assert_eq!(loaded, Loaded::Absent);
    assert_eq!(loaded.or_empty(), Record::default());
}

#[test]
fn a_corrupt_file_reads_as_an_updates_empty_record() {
    let td = tempfile::tempdir().expect("tempdir");
    let path = path_in(td.path());
    for bytes in [
        &b"{not json"[..],
        b"null",
        br#"{"seen": "watch-order"}"#,
        br#"{"installed_in": "soon"}"#,
    ] {
        std::fs::write(&path, bytes).expect("written");
        let loaded = load(&path);
        assert_eq!(
            loaded,
            Loaded::Unreadable,
            "{}",
            String::from_utf8_lossy(bytes)
        );
        let empty = loaded.or_empty();
        assert_eq!(empty.installed_in, None);
        assert!(
            !empty.baseline_pending,
            "a corrupt record is not a new install"
        );
    }
}

#[test]
fn a_field_left_out_reads_as_its_default() {
    let td = tempfile::tempdir().expect("tempdir");
    let path = path_in(td.path());
    std::fs::write(&path, br#"{"seen": ["watch-order"]}"#).expect("written");
    let Loaded::Record(rec) = load(&path) else {
        panic!("a record");
    };
    assert!(rec.seen.contains("watch-order"));
    assert_eq!(rec.installed_in, None);
    assert!(rec.flag_on_since.is_empty());
}

#[test]
fn the_profile_existed_when_any_of_its_files_is_there() {
    let td = tempfile::tempdir().expect("tempdir");
    let (a, b) = (td.path().join("history"), td.path().join("config.toml"));
    assert!(!profile_existed(&[&a, &b]));
    std::fs::write(&b, "").expect("written");
    assert!(profile_existed(&[&a, &b]));
}

#[test]
fn a_first_ever_launch_records_the_release_it_started_on() {
    let td = tempfile::tempdir().expect("tempdir");
    boot(td.path(), false, r("0.19.0"));
    assert_eq!(
        load(&path_in(td.path())),
        Loaded::Record(Record {
            installed_in: Some(r("0.19.0")),
            baseline_pending: true,
            ..Record::default()
        })
    );
}

#[test]
fn an_updates_first_launch_records_no_install() {
    let td = tempfile::tempdir().expect("tempdir");
    boot(td.path(), true, r("0.19.0"));
    assert_eq!(load(&path_in(td.path())), Loaded::Record(Record::default()));
}

#[test]
fn a_later_launch_leaves_the_record_as_it_is() {
    let td = tempfile::tempdir().expect("tempdir");
    let path = path_in(td.path());
    let mut rec = Record::default();
    rec.seen.insert("watch-order".into());
    save(&path, &rec).expect("saved");
    boot(td.path(), false, r("0.20.0"));
    assert_eq!(load(&path), Loaded::Record(rec));
}

#[test]
fn a_corrupt_record_is_left_for_the_next_write_not_replaced_as_a_new_install() {
    let td = tempfile::tempdir().expect("tempdir");
    let path = path_in(td.path());
    std::fs::write(&path, b"{not json").expect("written");
    boot(td.path(), false, r("0.19.0"));
    assert_eq!(load(&path), Loaded::Unreadable);
}

#[test]
fn a_record_that_cannot_be_written_leaves_the_launch_going() {
    let td = tempfile::tempdir().expect("tempdir");
    // The state directory is a file, so nothing can be written under it.
    let blocked = td.path().join("state");
    std::fs::write(&blocked, b"").expect("written");
    boot(&blocked, false, r("0.19.0"));
    assert!(save(&path_in(&blocked), &Record::default()).is_err());
}
