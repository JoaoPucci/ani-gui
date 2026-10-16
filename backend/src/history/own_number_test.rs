//! The marker column: a row whose episode is the app's own number.

use super::*;

#[test]
fn a_row_marked_as_the_apps_own_number_reads_as_one() {
    let rows = parse("7.5\thianime:show-1\tThe Show\tep:own\t1700000000000\tkitsu:49877\n");
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.ep_no, "7.5");
    assert!(row.own_number);
    assert_eq!(row.title, "The Show");
    assert_eq!(row.watched_at, Some(1_700_000_000_000));
    assert_eq!(row.kitsu_id.as_deref(), Some("49877"));
}

#[test]
fn the_marker_reads_without_a_moment_or_a_kitsu_id() {
    let rows = parse("8\thianime:show-1\tThe Show\tep:own\n");
    assert_eq!(rows[0].title, "The Show");
    assert!(rows[0].own_number);
    assert_eq!(rows[0].watched_at, None);
    assert_eq!(rows[0].kitsu_id, None);
}

#[test]
fn a_row_without_the_marker_is_a_provider_slot() {
    let rows = parse("9\thianime:show-1\tThe Show\t1700000000000\tkitsu:49877\n");
    assert!(!rows[0].own_number);
    assert_eq!(rows[0].kitsu_id.as_deref(), Some("49877"));
}

#[test]
fn the_marker_is_written_after_the_title_and_round_trips() {
    let row = HistoryEntry {
        ep_no: "8".into(),
        id: "hianime:show-1".into(),
        title: "The Show".into(),
        watched_at: Some(1_700_000_000_000),
        kitsu_id: Some("49877".into()),
        own_number: true,
    };
    let body = serialize(std::slice::from_ref(&row));
    assert_eq!(
        body,
        "8\thianime:show-1\tThe Show\tep:own\t1700000000000\tkitsu:49877\n"
    );
    assert_eq!(parse(&body), vec![row]);
}

#[test]
fn an_upsert_takes_the_new_rows_numbering() {
    let mut rows = parse("9\thianime:show-1\tThe Show\n");
    upsert(
        &mut rows,
        HistoryEntry {
            ep_no: "8".into(),
            id: "hianime:show-1".into(),
            title: "The Show".into(),
            watched_at: None,
            kitsu_id: None,
            own_number: true,
        },
    );
    assert_eq!(rows[0].ep_no, "8");
    assert!(rows[0].own_number);
}

#[test]
fn a_build_from_before_the_marker_keeps_a_marked_rows_kitsu_id_and_moment() {
    // A build from before the marker (v0.16.0's parser) splits the Kitsu
    // id and then the moment off the end of what follows the id, and
    // reads the rest as the title. With the marker right after the
    // title, both still split off; only the title carries the marker.
    let body = serialize(&[HistoryEntry {
        ep_no: "8".into(),
        id: "hianime:show-1".into(),
        title: "The Show".into(),
        watched_at: Some(1_700_000_000_000),
        kitsu_id: Some("49877".into()),
        own_number: true,
    }]);
    let line = body.trim_end();
    let rest = line.splitn(3, '\t').nth(2).expect("what follows the id");
    let (rest, kitsu_id) = split_kitsu_id(rest);
    let (title, watched_at) = split_moment(rest);
    assert_eq!(kitsu_id.as_deref(), Some("49877"));
    assert_eq!(watched_at, Some(1_700_000_000_000));
    assert_eq!(title, "The Show\tep:own");
}
