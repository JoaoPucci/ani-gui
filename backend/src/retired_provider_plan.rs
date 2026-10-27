//! What becomes of the history rows a provider the app no longer walks
//! left behind — decided from the rows alone, with no I/O, so the
//! rule can be checked over every shape of history.
//!
//! A row is a retired provider's when its show key names no provider
//! the app walks: a bare id (anidb.app's slugs, the allanime-era ids
//! before them), or the label of a provider since removed from
//! [`crate::provider_set::ProviderSet::shipped`]. Nothing names
//! providers one by one here, so removing the next provider needs no
//! change to this rule.
//!
//! Such a row is one of two things:
//!
//! - **Superseded**, when for every Kitsu entry it is shown as, a row
//!   of a walked provider shown as the same entry resumes over it, by
//!   the rule both Continue Watching and the detail page apply
//!   ([`crate::commands::history_resume::resumes_over`]). Both surfaces
//!   already show that row in its place, so the retired row is removed
//!   and nothing either surface shows changes. A retired row that is
//!   still the one a show resumes from stays until a watch of the show
//!   overtakes it.
//! - **Kept** otherwise: its show has nothing else to resume from. Its
//!   episode is rewritten in the app's own numbering (the number the
//!   read boundary already shows for it), so the numbering sidecar
//!   holds nothing for a provider that is gone.
//!
//! A row is never given a Kitsu id here: the entry a row is shown as
//! may be a guess, and a guess never becomes a row's id.

use crate::commands::history_resume::resumes_over;
use crate::history::same_kitsu_id;

/// What the plan needs to know of one history row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanRow {
    /// The row's show key.
    pub id: String,
    /// Whether the key names a provider the app walks.
    pub walked: bool,
    /// The Kitsu entries the row is shown as: the id it recorded, else
    /// its stored mapping, else its title matches — what Continue
    /// Watching matches it by. Empty when nothing is known.
    pub shown_as: Vec<String>,
    /// The moment it was watched: the later of the row's own and the
    /// cache's stamp.
    pub stamp: Option<i64>,
    /// Its episode in the entry's numbering, as the read boundary
    /// shows it.
    pub ep: String,
    /// Whether the file already holds that number as the app's own.
    pub own_number: bool,
}

/// What to do with a history's retired rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// Rows a walked provider's row supersedes, to remove.
    pub superseded: Vec<String>,
    /// Kept rows whose episode is to be written in the app's own
    /// numbering.
    pub renumber: Vec<String>,
}

impl Plan {
    /// Whether the plan changes nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.superseded.is_empty() && self.renumber.is_empty()
    }
}

/// The plan for `rows`, in file order.
#[must_use]
pub fn plan(rows: &[PlanRow]) -> Plan {
    let _ = rows;
    Plan::default()
}

#[cfg(test)]
#[path = "retired_provider_plan_test.rs"]
mod tests;

#[cfg(test)]
#[path = "retired_provider_plan_prop_test.rs"]
mod prop_tests;
