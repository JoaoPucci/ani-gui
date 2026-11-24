//! The watch order's reads, where they are pure: which ids the history
//! is read for, and which entry each row counts toward.

use std::collections::BTreeMap;

use super::{ids_of, rows_from_named};
use crate::history::HistoryEntry;
use crate::watch_order::{Chain, Franchise, HistoryRow, WatchEntry};

fn e(id: &str) -> WatchEntry {
    WatchEntry {
        id: id.into(),
        title: id.into(),
        subtype: None,
        start: None,
        episodes: None,
        status: None,
        poster: None,
        cover: None,
    }
}

fn franchise() -> Franchise {
    Franchise {
        anchor: "4".into(),
        name: "F".into(),
        name_from: "1".into(),
        entries: vec![e("1"), e("2")],
        links: Vec::new(),
        versions: vec![e("3")],
        films: vec![e("4")],
        sides: vec![e("5")],
        recaps: vec![e("6")],
        parents: BTreeMap::new(),
        complete: true,
        chain: Some(Chain {
            entries: vec![e("2"), e("4"), e("7")],
            links: Vec::new(),
            seams: vec!["2".into()],
        }),
    }
}

#[test]
fn the_history_is_read_for_every_id_the_franchise_names_its_chains_included_once_each() {
    assert_eq!(ids_of(&franchise()), ["1", "2", "3", "4", "5", "6", "7"]);
}

fn row(recorded: Option<&str>, ep_no: &str) -> HistoryEntry {
    HistoryEntry {
        ep_no: ep_no.into(),
        id: "p:show".into(),
        title: "Show".into(),
        watched_at: None,
        kitsu_id: recorded.map(Into::into),
        own_number: true,
    }
}

#[test]
fn a_row_counts_toward_the_entry_that_named_it_whatever_id_it_recorded() {
    let named: BTreeMap<String, Option<HistoryEntry>> = [
        // A row recorded under an entry Kitsu deleted, named for its replacement.
        ("7".to_string(), Some(row(Some("99"), "3"))),
        ("1".to_string(), Some(row(None, "12"))),
        ("2".to_string(), None),
    ]
    .into_iter()
    .collect();
    let want = vec![
        HistoryRow {
            kitsu_id: Some("1".into()),
            ep_no: "12".into(),
        },
        HistoryRow {
            kitsu_id: Some("7".into()),
            ep_no: "3".into(),
        },
    ];
    assert_eq!(rows_from_named(named), want);
}
