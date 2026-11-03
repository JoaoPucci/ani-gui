//! A guessed session never records its guess over a show the row
//! already records.
//!
//! A play a guessed Continue card asked for (`guess_row`) records the
//! guess on the card's row once it lands on the row's own listing. The
//! user can link the row to another show while that session is still
//! playing, from the player itself. The history guard holds back plays
//! begun before the link; a later episode of the same session is begun
//! after it, and would record the old guess over the link. So whatever a
//! guessed session's play would record — the watch's id, or the resolve's
//! guess — it records only on a row that records no id, or the same one.
//! Any other play is the user's choice of show, from its own page, and
//! records what it names.
//!
//! Such a play does not stand for the row either: it reports that it did
//! not land on the card's own listing (`own_listing`), so the renderer
//! neither records it as the row's show nor mirrors it to the trackers.

use crate::app::AppState;
use crate::commands::play::PlayArgs;
use crate::history::same_kitsu_id;

/// The id a play may record on a row that records `recorded`, of the
/// `candidate` it names: none when a guessed session's (`guessed`) would
/// replace another id the row records.
#[must_use]
pub fn keeps_recorded<'a>(
    guessed: bool,
    recorded: Option<&str>,
    candidate: Option<&'a str>,
) -> Option<&'a str> {
    match (guessed, recorded, candidate) {
        (true, Some(r), Some(c)) if !same_kitsu_id(r, c) => None,
        _ => candidate,
    }
}

/// `candidate`, the id the play `args` would record on the row `played`,
/// unless the play is a guessed session's and the row records another
/// ([`keeps_recorded`]). A history that cannot be read keeps a guessed
/// session's id back: what the row records is not known.
pub(crate) fn guarded<'a>(
    state: &AppState,
    args: &PlayArgs,
    played: &str,
    candidate: Option<&'a str>,
) -> Option<&'a str> {
    if args.guess_row.is_none() || candidate.is_none() {
        return candidate;
    }
    let Ok(rows) = crate::history::read_all(&state.history_path) else {
        return None;
    };
    let recorded = rows
        .iter()
        .find(|r| r.id == played)
        .and_then(|r| r.kitsu_id.as_deref());
    keeps_recorded(true, recorded, candidate)
}

/// Whether the guessed play `args` landed on the card's own listing
/// `played` and may stand for its row: the row records no other show
/// than the one the play names as its guess.
pub(crate) fn own_listing(state: &AppState, args: &PlayArgs, played: &str) -> bool {
    let guess = args.guess_id.as_deref();
    args.lands_on_guess_row(played) && guarded(state, args, played, guess) == guess
}
