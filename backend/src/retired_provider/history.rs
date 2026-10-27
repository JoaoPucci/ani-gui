//! The history half of the retired-provider sweep — split from
//! [`super`] for the per-file complexity bar.

use crate::app::AppState;
use crate::commands::history_resume::latest_of;
use crate::error::{AniError, Result};
use crate::history::HistoryEntry;
use crate::retired_provider_plan::{plan, PlanRow};

use super::renumber::{renumber, shown_rows};
use super::{evidence, walks, RetiredSweepReport};

/// Remove the superseded retired rows and renumber the kept ones.
pub(super) fn sweep(state: &AppState, report: &mut RetiredSweepReport) -> Result<()> {
    let rows = crate::history::read_all(&state.history_path)?;
    if rows.iter().all(|r| walks(state, &r.id)) {
        return Ok(());
    }
    let the_plan = crate::history::guard::hold(&state.history_path, |held| {
        let raw = held.rows()?;
        let shown = shown_rows(state)?;
        let rows = raw
            .iter()
            .zip(&shown)
            .map(|(e, s)| plan_row(state, e, &s.ep_no))
            .collect::<Result<Vec<_>>>()?;
        Ok::<_, AniError>(plan(&rows))
    })?;
    for id in &the_plan.superseded {
        if crate::commands::history::history_delete(state, id)? {
            report
                .superseded
                .extend(rows.iter().find(|r| &r.id == id).map(|r| r.title.clone()));
        }
    }
    if !the_plan.renumber.is_empty() {
        report.renumbered = renumber(state, &the_plan)?;
    }
    Ok(())
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
