//! The boot sweep over what a provider the app no longer walks left in
//! the user's stores.
//!
//! A provider's resolves stamp its show keys on every store a play
//! writes: history rows, the numbering sidecar beside them, watched-at
//! stamps, the reverse mapping to Kitsu, resolution rows, availability
//! verdicts and title matches. Removing the provider from
//! [`crate::provider_set::ProviderSet::shipped`] leaves all of it on
//! disk. The read side already refuses to serve most of it, and
//! nothing reclaims a cache row past its lifetime, so without this it
//! would stay for good. A key is a retired provider's when it names no
//! provider the app walks (a bare id included); nothing here names a
//! provider, so the next removal needs no change.
//!
//! Each launch, before the API serves:
//!
//! 1. **History.** The rule is [`crate::retired_provider_plan`]: a
//!    retired row a walked row supersedes is removed through
//!    [`crate::commands::history::history_delete`], which takes what
//!    is kept about it beside it — its stamp, mapping, resolution
//!    rows, numbering and title matches, as a removal from the page
//!    would. A retired row that is still all its show has is kept, its
//!    episode rewritten in the app's own numbering, the number the
//!    page already shows for it. A row is never given a Kitsu id here:
//!    a guess never becomes a row's id.
//! 2. **Numbering.** Rows for a retired key no remaining row reads
//!    through go. A walked key's rows stay, rowless ones included: a
//!    resolve stamps them before the show's first row.
//! 3. **Cache** ([`cache`]). Rows only a retired provider's key or
//!    verdict could be read by go: stamps and mappings of a retired
//!    key with no history row; resolution rows whose show key is
//!    retired, which are never replayed; availability rows naming no
//!    walked provider, which are never served; title matches filed
//!    under a retired provider's segment, which are never asked for.
//!
//! The sweep is idempotent and keeps no marker: what it looks for is
//! the retired data itself, so a launch that finds none writes
//! nothing, and a history or cache restored from an older copy is
//! swept on the next launch. Only one app runs per profile (the shell's
//! single-instance lock), and the stores it writes take their own
//! locks: the history's guard, the numbering store's file lock, and
//! SQLite's for the cache.

mod cache;

use std::collections::HashSet;

use serde::Serialize;

use crate::app::AppState;
use crate::commands::history_resume::latest_of;
use crate::commands::numbering_offset::Numbering;
use crate::error::Result;
use crate::history::{kitsu_id_in, HistoryEntry};
use crate::retired_provider_plan::{plan, PlanRow};
#[cfg(test)]
use crate::scraper::provider::ProviderId;
use crate::scraper::provider::ShowKey;

/// What one sweep changed. Empty on every launch that finds nothing a
/// retired provider left.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RetiredSweepReport {
    /// Titles of the history rows removed because a walked provider's
    /// row for the same show supersedes them.
    pub superseded: Vec<String>,
    /// Titles of the history rows kept, their episode rewritten in the
    /// app's own numbering.
    pub renumbered: Vec<String>,
    /// Numbering-sidecar rows removed.
    pub numbering_rows: usize,
    /// Cache rows removed.
    pub cache_rows: usize,
}

impl RetiredSweepReport {
    /// Whether the sweep changed nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Whether `id` names a provider the state walks.
pub(crate) fn walks(state: &AppState, id: &str) -> bool {
    ShowKey::parse(id)
        .provider
        .is_some_and(|p| state.providers.order().contains(&p))
}

/// Run the sweep. Never fails the caller: a store that cannot be read
/// or written is logged and left for the next launch.
#[must_use]
pub fn sweep(state: &AppState) -> RetiredSweepReport {
    let mut report = RetiredSweepReport::default();
    let cache_before = cache::row_count(state);
    let numbering_before = retired_numbering(state).len();
    if let Err(e) = sweep_history(state, &mut report) {
        tracing::warn!(error = ?e, "retired-provider sweep: history left as it is");
    }
    let remaining = match crate::history::read_all(&state.history_path) {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!(error = ?e, "retired-provider sweep: history unreadable");
            return report;
        }
    };
    if let Err(e) = sweep_numbering(state, &remaining) {
        tracing::warn!(error = ?e, "retired-provider sweep: numbering left as it is");
    }
    let remaining: HashSet<String> = remaining.into_iter().map(|e| e.id).collect();
    if let Err(e) = cache::sweep(state, &remaining) {
        tracing::warn!(error = ?e, "retired-provider sweep: cache left as it is");
    }
    report.numbering_rows = numbering_before.saturating_sub(retired_numbering(state).len());
    report.cache_rows = cache_before.saturating_sub(cache::row_count(state));
    if !report.is_empty() {
        tracing::info!(
            target: "retired_provider",
            superseded = report.superseded.len(),
            renumbered = report.renumbered.len(),
            numbering_rows = report.numbering_rows,
            cache_rows = report.cache_rows,
            "removed what a provider the app no longer walks left behind"
        );
    }
    report
}

/// The numbering store's slugs that name no walked provider.
fn retired_numbering(state: &AppState) -> Vec<String> {
    Numbering::read(&state.history_path)
        .slugs()
        .filter(|s| !walks(state, s))
        .map(str::to_string)
        .collect()
}

fn sweep_history(state: &AppState, report: &mut RetiredSweepReport) -> Result<()> {
    let rows = crate::history::read_all(&state.history_path)?;
    if rows.iter().all(|r| walks(state, &r.id)) {
        return Ok(());
    }
    let the_plan = crate::history::guard::hold(&state.history_path, |held| {
        let numbering = Numbering::read(&state.history_path);
        let rows = held
            .rows()?
            .iter()
            .map(|e| plan_row(state, &numbering, e))
            .collect::<Result<Vec<_>>>()?;
        Ok::<_, crate::error::AniError>(plan(&rows))
    })?;
    let title_of = |id: &str| rows.iter().find(|r| r.id == id).map(|r| r.title.clone());
    for id in &the_plan.superseded {
        if crate::commands::history::history_delete(state, id)? {
            report.superseded.extend(title_of(id));
        }
    }
    if the_plan.renumber.is_empty() {
        return Ok(());
    }
    report.renumbered = crate::history::guard::hold(&state.history_path, |held| {
        let numbering = Numbering::read(&state.history_path);
        let mut rows = held.rows()?;
        let mut renumbered = Vec::new();
        for row in rows
            .iter_mut()
            .filter(|r| !r.own_number && the_plan.renumber.contains(&r.id))
        {
            row.ep_no = numbering.read_ep_no(&row.id, &row.ep_no);
            row.own_number = true;
            renumbered.push(row.title.clone());
        }
        if !renumbered.is_empty() {
            held.write(&rows)?;
        }
        Ok::<_, crate::error::AniError>(renumbered)
    })?;
    Ok(())
}

/// What the plan needs of `entry`.
fn plan_row(state: &AppState, numbering: &Numbering, entry: &HistoryEntry) -> Result<PlanRow> {
    let stamp = latest_of(
        entry.watched_at,
        crate::commands::kitsu::watched_at_get(state, &entry.id)?,
    );
    let ep = if entry.own_number {
        entry.ep_no.clone()
    } else {
        numbering.read_ep_no(&entry.id, &entry.ep_no)
    };
    Ok(PlanRow {
        id: entry.id.clone(),
        walked: walks(state, &entry.id),
        shown_as: shown_as(state, entry)?,
        stamp,
        ep,
        own_number: entry.own_number,
    })
}

/// The Kitsu entries Continue Watching shows `entry` as, in the order
/// it looks: the id the row recorded, else the row's stored mapping,
/// else the title matches stored for the row's title.
fn shown_as(state: &AppState, entry: &HistoryEntry) -> Result<Vec<String>> {
    if let Some(id) = entry.kitsu_id.as_deref().and_then(kitsu_id_in) {
        return Ok(vec![id.to_string()]);
    }
    if let Some(id) = crate::commands::kitsu::allmanga_kitsu_get(state, &entry.id)? {
        return Ok(vec![id]);
    }
    let provider = ShowKey::parse(&entry.id).provider;
    let mut ids = Vec::new();
    let tailless = crate::commands::history_title_tail::without_episode_tail(&entry.title);
    for searched in [entry.title.as_str(), tailless] {
        let prefix = crate::commands::kitsu::title_match_prefix(provider, searched);
        for (key, body) in crate::cache::meta_cache_list_prefix(&state.cache_pool, &prefix)? {
            let cour = &key[prefix.len()..];
            if !cour.is_empty() && cour.bytes().all(|b| b.is_ascii_digit()) && !ids.contains(&body)
            {
                ids.push(body);
            }
        }
    }
    Ok(ids)
}

/// Drop the numbering of retired keys no remaining row reads through:
/// a row in the app's own numbering reads through none.
fn sweep_numbering(state: &AppState, remaining: &[HistoryEntry]) -> std::io::Result<()> {
    let needed: HashSet<&str> = remaining
        .iter()
        .filter(|r| !r.own_number)
        .map(|r| r.id.as_str())
        .collect();
    let stale = retired_numbering(state)
        .into_iter()
        .any(|s| !needed.contains(s.as_str()));
    if !stale {
        return Ok(());
    }
    crate::commands::numbering_offset::forget_where(state, |slug| {
        !walks(state, slug) && !needed.contains(slug)
    })
}

#[cfg(test)]
#[path = "retired_provider_test.rs"]
mod tests;
