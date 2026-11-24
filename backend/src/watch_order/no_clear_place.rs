//! The franchise page's titles with no clear place: line titles with no
//! number and no note, or a note leading into such a title. They leave
//! their lines and gather in one stretch at the end of the main story.

use std::collections::{BTreeMap, BTreeSet};

use super::rail::{path_note, PathNote};
use super::sections::{FranchiseRow, FranchiseSection, RowContext};

/// The titles of `rows` with no clear place: no number, and either no
/// note or a note leading into a title with no clear place itself.
fn no_clear_place(ctx: RowContext<'_>, rows: &[&FranchiseRow]) -> BTreeSet<String> {
    let mut leads: BTreeMap<&str, String> = BTreeMap::new();
    for r in rows {
        if let Some(PathNote::LeadsInto { entry }) = path_note(ctx.run, &r.entry.id, ctx.order) {
            leads.insert(&r.entry.id, entry.id);
        }
    }
    let unnumbered: Vec<&str> = rows
        .iter()
        .filter(|r| r.position.is_none())
        .map(|r| r.entry.id.as_str())
        .collect();
    let mut loose: BTreeSet<String> = unnumbered
        .iter()
        .filter(|id| !leads.contains_key(**id))
        .map(|id| (*id).to_string())
        .collect();
    let mut grew = true;
    while grew {
        grew = false;
        for id in &unnumbered {
            if loose.contains(*id) || !leads.get(id).is_some_and(|to| loose.contains(to)) {
                continue;
            }
            loose.insert((*id).to_string());
            grew = true;
        }
    }
    loose
}

/// `sections` with the titles that have no clear place gathered at the
/// end, and the lines left next to each other joined into one.
pub(super) fn gather_unplaced(
    ctx: RowContext<'_>,
    sections: Vec<FranchiseSection>,
) -> Vec<FranchiseSection> {
    let line_rows: Vec<&FranchiseRow> = sections
        .iter()
        .flat_map(|s| match s {
            FranchiseSection::Line { rows, .. } => rows.iter().collect(),
            _ => Vec::new(),
        })
        .collect();
    let loose = no_clear_place(ctx, &line_rows);
    let rest: Vec<FranchiseRow> = line_rows
        .into_iter()
        .filter(|r| loose.contains(&r.entry.id))
        .cloned()
        .collect();
    let mut out: Vec<FranchiseSection> = Vec::new();
    for s in sections {
        let s = match s {
            FranchiseSection::Line { rows, unplaced } => {
                let rows: Vec<FranchiseRow> = rows
                    .into_iter()
                    .filter(|r| !loose.contains(&r.entry.id))
                    .collect();
                if rows.is_empty() {
                    continue;
                }
                FranchiseSection::Line { rows, unplaced }
            }
            other => other,
        };
        // Numbered stretches left next to each other read as one list.
        if let (
            FranchiseSection::Line { rows, .. },
            Some(FranchiseSection::Line { rows: last, .. }),
        ) = (&s, out.last_mut())
        {
            last.extend(rows.iter().cloned());
            continue;
        }
        out.push(s);
    }
    if !rest.is_empty() {
        out.push(FranchiseSection::Line {
            rows: rest,
            unplaced: true,
        });
    }
    out
}
