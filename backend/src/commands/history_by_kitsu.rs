//! The history row a Kitsu id resumes from — split from [`super`] for
//! the per-file complexity bar.

use crate::commands::numbering_offset::Numbering;
use crate::error::Result;
use crate::history::{read_all, HistoryEntry};

use super::to_kitsu_numbering;

/// [`super::history_by_kitsu`] among the rows `except` does not name: the
/// pick's own progress, less the card's listing, for a switch
/// ([`crate::commands::switch_episode`]).
///
/// # Errors
/// As [`super::history_by_kitsu`].
pub fn history_by_kitsu_except(
    state: &crate::app::AppState,
    kitsu_id: &str,
    except: impl Fn(&str) -> bool,
) -> Result<Option<HistoryEntry>> {
    if kitsu_id.is_empty() {
        return Ok(None);
    }
    // Under the writers' guard, as the list is.
    crate::history::guard::hold(&state.history_path, |_| {
        history_by_kitsu_held_except(state, kitsu_id, &except)
    })
}

fn history_by_kitsu_held_except(
    state: &crate::app::AppState,
    kitsu_id: &str,
    except: impl Fn(&str) -> bool,
) -> Result<Option<HistoryEntry>> {
    let entries = read_all(&state.history_path)?
        .into_iter()
        .filter(|e| !except(&e.id));
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
    let numbering = Numbering::read(&state.history_path);
    for entry in entries {
        // The id the row records is the show the user played; the
        // reverse mapping stands in only for a row without one, or one
        // whose recorded entry Kitsu has since deleted
        // ([`crate::commands::history_claim::names`]).
        if !crate::commands::history_claim::row_names(state, &entry, kitsu_id)? {
            continue;
        }
        let stamp = crate::commands::history_resume::latest_of(
            entry.watched_at,
            crate::commands::kitsu::watched_at_get(state, &entry.id)?,
        );
        let entry = to_kitsu_numbering(state, &numbering, entry, Some(kitsu_id));
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
