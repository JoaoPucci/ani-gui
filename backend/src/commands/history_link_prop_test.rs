//! The link plan's properties over arbitrary histories.

use super::*;
use proptest::prelude::*;

fn entry() -> impl Strategy<Value = HistoryEntry> {
    (
        "[a-c]{1,2}",
        "[0-9]{1,3}",
        proptest::option::of(1_700_000_000_000i64..1_800_000_000_000),
        proptest::option::of("[1-4]"),
    )
        .prop_map(|(id, ep_no, watched_at, kitsu_id)| HistoryEntry {
            ep_no,
            id,
            title: "Show".into(),
            watched_at,
            kitsu_id,
            own_number: false,
        })
}

/// A history: rows with distinct ids, as the writer keeps them.
fn history() -> impl Strategy<Value = Vec<HistoryEntry>> {
    proptest::collection::vec(entry(), 0..6).prop_map(|mut rows| {
        let mut seen = std::collections::HashSet::new();
        rows.retain(|r| seen.insert(r.id.clone()));
        rows
    })
}

fn apply(entries: &mut [HistoryEntry], row: &str, kitsu_id: Option<String>) {
    for e in entries.iter_mut().filter(|e| e.id == row) {
        e.kitsu_id.clone_from(&kitsu_id);
    }
}

proptest! {
    #[test]
    fn a_plan_names_held_rows_once_in_the_order_asked(
        entries in history(),
        asked in proptest::collection::vec("[a-c]{1,2}", 0..8),
    ) {
        let planned = plan_link(&entries, &asked);
        let mut expected: Vec<&String> = Vec::new();
        for a in &asked {
            if entries.iter().any(|e| &e.id == a) && !expected.contains(&a) {
                expected.push(a);
            }
        }
        let names: Vec<&String> = planned.iter().map(|p| &p.row).collect();
        prop_assert_eq!(names, expected);
        for p in &planned {
            let e = entries.iter().find(|e| e.id == p.row).unwrap();
            prop_assert_eq!(&p.previous, &e.kitsu_id);
            prop_assert_eq!(&p.ep_no, &e.ep_no);
            prop_assert_eq!(p.watched_at, e.watched_at);
        }
    }

    #[test]
    fn an_undo_with_nothing_between_gives_the_history_back(
        entries in history(),
        asked in proptest::collection::vec("[a-c]{1,2}", 0..8),
        linked in "[5-9]",
    ) {
        let planned = plan_link(&entries, &asked);
        let mut after = entries.clone();
        for p in &planned {
            apply(&mut after, &p.row, Some(linked.clone()));
        }
        let restored = plan_unlink(&after, &linked, &planned);
        prop_assert_eq!(restored.len(), planned.len());
        for r in restored {
            apply(&mut after, &r.row, r.previous.clone());
        }
        prop_assert_eq!(after, entries);
    }

    #[test]
    fn a_row_written_since_the_link_is_not_undone(
        entries in history(),
        linked in "[5-9]",
        which in 0usize..6,
        moved_episode in any::<bool>(),
    ) {
        prop_assume!(!entries.is_empty());
        let asked: Vec<String> = entries.iter().map(|e| e.id.clone()).collect();
        let planned = plan_link(&entries, &asked);
        let mut after = entries.clone();
        for p in &planned {
            apply(&mut after, &p.row, Some(linked.clone()));
        }
        let i = which % after.len();
        if moved_episode {
            after[i].ep_no.push('0');
        } else {
            after[i].watched_at = Some(after[i].watched_at.unwrap_or(0) + 1);
        }
        let restored = plan_unlink(&after, &linked, &planned);
        prop_assert!(restored.iter().all(|r| r.row != after[i].id));
        prop_assert_eq!(restored.len(), planned.len() - 1);
    }
}
