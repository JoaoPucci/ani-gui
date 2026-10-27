//! Properties of the retired-row plan over generated histories.

use super::*;
use crate::commands::history_resume::resumes_over;
use proptest::prelude::*;

fn arb_row(index: usize) -> impl Strategy<Value = PlanRow> {
    (
        any::<bool>(),
        prop::collection::vec(1u8..5, 0..3),
        prop::option::of(0i64..4),
        (1u8..6, any::<bool>()),
        any::<bool>(),
    )
        .prop_map(
            move |(walked, ids, stamp, (ep, half), own_number)| PlanRow {
                id: format!("{}row-{index}", if walked { "test-a:" } else { "" }),
                walked,
                shown_as: ids.iter().map(ToString::to_string).collect(),
                stamp,
                ep: if half {
                    format!("{ep}.5")
                } else {
                    ep.to_string()
                },
                own_number,
            },
        )
}

fn arb_rows() -> impl Strategy<Value = Vec<PlanRow>> {
    (0usize..7).prop_flat_map(|n| (0..n).map(arb_row).collect::<Vec<_>>())
}

/// The rows once the plan is applied: superseded rows removed, the
/// rest renumbered — their episode already reads in the entry's
/// numbering, so only the marker moves.
fn apply(rows: &[PlanRow], plan: &Plan) -> Vec<PlanRow> {
    rows.iter()
        .filter(|r| !plan.superseded.contains(&r.id))
        .cloned()
        .map(|mut r| {
            if plan.renumber.contains(&r.id) {
                r.own_number = true;
            }
            r
        })
        .collect()
}

/// The row each Kitsu entry resumes from, as the detail page's lookup
/// picks it: the first row, in file order, no later row resumes over.
fn resumed(rows: &[PlanRow], kitsu: &str) -> Option<(String, Option<i64>, String)> {
    let mut best: Option<&PlanRow> = None;
    for row in rows
        .iter()
        .filter(|r| r.shown_as.iter().any(|k| k == kitsu))
    {
        let newer = best.is_none_or(|b| resumes_over((row.stamp, &row.ep), (b.stamp, &b.ep)));
        if newer {
            best = Some(row);
        }
    }
    best.map(|r| (r.id.clone(), r.stamp, r.ep.clone()))
}

proptest! {
    #[test]
    fn a_second_plan_over_the_applied_one_changes_nothing(rows in arb_rows()) {
        let first = plan(&rows);
        let after = apply(&rows, &first);
        prop_assert!(plan(&after).is_empty(), "{:?}", plan(&after));
    }

    #[test]
    fn only_retired_rows_with_a_walked_row_for_each_of_their_shows_go(rows in arb_rows()) {
        let p = plan(&rows);
        for id in &p.superseded {
            let row = rows.iter().find(|r| &r.id == id).expect("a planned row exists");
            prop_assert!(!row.walked);
            prop_assert!(!row.shown_as.is_empty());
            for kitsu in &row.shown_as {
                prop_assert!(
                    rows.iter().any(|w| w.walked && w.shown_as.contains(kitsu)),
                    "{id} went with no walked row shown as {kitsu}"
                );
            }
        }
        for id in &p.renumber {
            let row = rows.iter().find(|r| &r.id == id).expect("a planned row exists");
            prop_assert!(!row.walked && !row.own_number);
        }
    }

    #[test]
    fn every_kitsu_entry_resumes_from_the_same_episode_after(rows in arb_rows()) {
        let after = apply(&rows, &plan(&rows));
        for kitsu in 1u8..5 {
            let kitsu = kitsu.to_string();
            prop_assert_eq!(resumed(&rows, &kitsu), resumed(&after, &kitsu));
        }
    }

    #[test]
    fn a_retired_row_that_is_not_superseded_is_kept(rows in arb_rows()) {
        let p = plan(&rows);
        for row in rows.iter().filter(|r| !r.walked && !r.own_number) {
            prop_assert!(p.superseded.contains(&row.id) != p.renumber.contains(&row.id));
        }
    }
}
