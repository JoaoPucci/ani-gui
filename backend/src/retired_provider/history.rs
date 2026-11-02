//! The history half of the retired-provider sweep: what to do with
//! each retired row — split from [`super`] for the per-file complexity
//! bar.

use crate::app::AppState;
use crate::commands::history_resume::latest_of;
use crate::commands::numbering_offset::Numbering;
use crate::error::{AniError, Result};
use crate::history::HistoryEntry;
use crate::retired_provider_plan::{plan, PlanRow};

use super::{evidence, walks};

/// The rows to remove and the rows to renumber, each with what the
/// write needs: a removed row's title, a renumbered row's episode as
/// the read boundary shows it.
pub(super) struct HistoryPlan {
    pub superseded: Vec<(String, String)>,
    pub renumber: Vec<(String, String)>,
}

/// The history's plan, read under the history's guard through
/// `numbering` and the cache; any of those reads that fails is an
/// error.
pub(super) fn plan_history(state: &AppState, numbering: &Numbering) -> Result<HistoryPlan> {
    crate::history::guard::hold(&state.history_path, |held| {
        let rows = held.rows()?;
        if rows.iter().all(|r| walks(state, &r.id)) {
            return Ok(HistoryPlan {
                superseded: Vec::new(),
                renumber: Vec::new(),
            });
        }
        let shown = crate::commands::history::history_list_checked(state, numbering)?;
        let planned = plan(
            &rows
                .iter()
                .zip(&shown)
                .map(|(e, s)| plan_row(state, e, &s.ep_no))
                .collect::<Result<Vec<_>>>()?,
        );
        let titled = |id: &String, of: &[HistoryEntry], pick: fn(&HistoryEntry) -> String| {
            of.iter()
                .find(|r| &r.id == id)
                .map(|r| (id.clone(), pick(r)))
        };
        Ok::<_, AniError>(HistoryPlan {
            superseded: planned
                .superseded
                .iter()
                .filter_map(|id| titled(id, &rows, |r| r.title.clone()))
                .collect(),
            renumber: planned
                .renumber
                .iter()
                .filter_map(|id| titled(id, &shown, |r| r.ep_no.clone()))
                .collect(),
        })
    })
}

/// What the plan needs of `entry`, shown at episode `ep`.
fn plan_row(state: &AppState, entry: &HistoryEntry, ep: &str) -> Result<PlanRow> {
    let walked = walks(state, &entry.id);
    let shown_as = if walked {
        evidence::walked(state, entry)?
    } else {
        evidence::retired(state, entry)?
    };
    Ok(PlanRow {
        id: entry.id.clone(),
        walked,
        shown_as,
        stamp: latest_of(
            entry.watched_at,
            crate::commands::kitsu::watched_at_get(state, &entry.id)?,
        ),
        ep: ep.to_string(),
        own_number: entry.own_number,
    })
}
