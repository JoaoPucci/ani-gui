//! The resume lookup — which history row a Kitsu entry resumes from,
//! for one id or several — split from `history` for the per-file
//! complexity bar.

use super::{to_kitsu_numbering, Numbering};
use crate::commands::history_claim::{id_gone, row_identity, RowIdentity};
use crate::error::Result;
use crate::history::{read_all, HistoryEntry};

/// Find the history entry (if any) for the supplied `kitsu_id`: a row
/// that records it as the show played, matched with no mapping at
/// all; a row whose recorded entry Kitsu answered gone (a 404 or 410,
/// [`crate::commands::kitsu_gone`]), matched as one that records none or else by
/// the title match the home page stored for it; or, for a row that
/// records none, one whose show id maps to it
/// — resolving that row's `id`, a provider slug on rows written since
/// the migration, through the `(show id → kitsu_id)` reverse cache a
/// successful play stamps.
/// Of two rows that map to the entry, the one the user watched last
/// is returned — the latest watched-at stamp, a stamped row over an
/// unstamped one, then the further progress, then file order — the
/// rule the Continue Watching strip applies to the same rows, so
/// the two surfaces name one episode. Returns `None` when:
///   - The history file is missing or empty.
///   - Kitsu answered `kitsu_id` itself gone.
///   - No entry records `kitsu_id` while Kitsu still has it; no entry
///     without a standing recorded id has a standing mapping to it;
///     and no entry whose recorded entry is gone, with no standing
///     mapping, has a title match naming it.
///
/// A row that records no id can be in the file without being in the
/// reverse cache: a play that had no Kitsu id to record, a pairing the
/// cross-cour guard refused, or a row older than the mapping itself.
/// Those rows are skipped here and show no Resume affordance until a
/// play records them — by design, since the alternative is a Kitsu
/// search per row.
///
/// # Errors
/// Returns [`crate::error::AniError::Io`] when the history file
/// exists but cannot be read; SQLite errors propagate from the
/// reverse-cache lookup and from the watched-at stamp's, since a
/// read that fails says nothing about the row it was for.
pub fn history_by_kitsu(
    state: &crate::app::AppState,
    kitsu_id: &str,
) -> Result<Option<HistoryEntry>> {
    if kitsu_id.is_empty() {
        return Ok(None);
    }
    // Under the writers' guard, as the list is.
    crate::history::guard::hold(&state.history_path, |_| {
        history_by_kitsu_held(state, kitsu_id)
    })
}

/// The ids [`history_by_kitsu_each`] answers for.
#[derive(Debug, serde::Deserialize)]
pub struct ByKitsuEachArgs {
    /// A value that is not a Kitsu id is dropped.
    #[serde(deserialize_with = "crate::kitsu_id::deserialize_list")]
    pub ids: Vec<String>,
}

/// [`history_by_kitsu`] for each of `ids` at once: each id with the
/// row it resumes from, or `None`, all read from one state of the
/// history under one hold of the writers' guard.
///
/// # Errors
/// As [`history_by_kitsu`].
pub fn history_by_kitsu_each(
    state: &crate::app::AppState,
    ids: &[String],
) -> Result<std::collections::BTreeMap<String, Option<HistoryEntry>>> {
    if ids.is_empty() {
        return Ok(std::collections::BTreeMap::new());
    }
    crate::history::guard::hold(&state.history_path, |_| {
        let rows = identified(state)?;
        let numbering = Numbering::read(&state.history_path);
        ids.iter()
            .map(|id| Ok((id.clone(), resumed_from(state, &rows, &numbering, id)?)))
            .collect()
    })
}

fn history_by_kitsu_held(
    state: &crate::app::AppState,
    kitsu_id: &str,
) -> Result<Option<HistoryEntry>> {
    let rows = identified(state)?;
    let numbering = Numbering::read(&state.history_path);
    resumed_from(state, &rows, &numbering, kitsu_id)
}

/// The history's rows, each with what the cache knows of its Kitsu
/// identity, read once however many ids are then asked about.
fn identified(state: &crate::app::AppState) -> Result<Vec<(HistoryEntry, RowIdentity)>> {
    read_all(&state.history_path)?
        .into_iter()
        .map(|entry| {
            let identity = row_identity(state, &entry)?;
            Ok((entry, identity))
        })
        .collect()
}

/// The row of `rows` that `kitsu_id` resumes from, in its numbering.
fn resumed_from(
    state: &crate::app::AppState,
    rows: &[(HistoryEntry, RowIdentity)],
    numbering: &Numbering,
    kitsu_id: &str,
) -> Result<Option<HistoryEntry>> {
    let gone = id_gone(state, kitsu_id)?;
    // Two providers can leave two rows for one show, one under each
    // provider's id. The one the user watched last is the one to
    // resume from, by the rule the Continue Watching strip applies to
    // the same rows ([`crate::commands::history_resume::resumes_over`]): the
    // latest watched-at stamp wins, a stamped row beats an unstamped
    // one, the further progress decides when the stamps do not — in
    // the entry's numbering, the one the strip counts in, since two
    // providers' rows can count from different offsets — and file
    // order stands only when the rows are equal on every count.
    // A read the cache cannot serve is the caller's error, never a
    // row without a mapping or a stamp: taken as one, the row watched
    // last would lose to its sibling's older stamp, or be skipped for
    // it, and the page would resume the stale episode. The stamp is
    // the later of the row's own moment and the cache's
    // ([`crate::commands::history_resume::latest_of`]): a watch writes its
    // moment beside the row, so a cache that refused the stamp still
    // leaves the row ranked as the watch it was.
    let mut best: Option<(HistoryEntry, Option<i64>)> = None;
    for (entry, identity) in rows {
        // The id the row records is the show the user played; the
        // reverse mapping stands in only for a row without one, or one
        // whose recorded entry Kitsu has since deleted
        // ([`crate::commands::history_claim::names`]).
        if !identity.names(kitsu_id, gone) {
            continue;
        }
        let stamp = crate::commands::history_resume::latest_of(
            entry.watched_at,
            crate::commands::kitsu::watched_at_get(state, &entry.id)?,
        );
        let entry = to_kitsu_numbering(state, numbering, entry.clone(), Some(kitsu_id));
        let newer = match &best {
            None => true,
            Some((current, current_stamp)) => crate::commands::history_resume::resumes_over(
                (stamp, &entry.ep_no),
                (*current_stamp, &current.ep_no),
            ),
        };
        if newer {
            best = Some((entry, stamp));
        }
    }
    Ok(best.map(|(entry, _)| entry))
}
