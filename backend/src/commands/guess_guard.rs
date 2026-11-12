//! A play a Continue card asked for never records its show over a show
//! the row already records.
//!
//! A play a guessed Continue card asked for (`guess_row`) records the
//! guess on the card's row once it lands on the row's own listing. The
//! user can link the row to another show while that session is still
//! playing, from the player itself. The history guard holds back plays
//! begun before the link; a later episode of the same session is begun
//! after it, and would record the old guess over the link. So whatever a
//! guessed session's play would record — the watch's id, or the resolve's
//! guess — it records only where neither the row, nor, for a row that
//! records none (or a show Kitsu answered gone), the card's own line on
//! its listing, records another show ([`line_record`]).
//! A session a card opened with no guess — a line the user named, played
//! from Home — keeps the line's show the same way on the card's own
//! listing (`card_row`): a player mounted again after "Keep watching
//! this" sends its plays as the page's show until it has read the
//! line's pick back. Any other play is the user's choice of show, from
//! its own page, and records what it names.
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

/// Whether a play keeps the show a row `played` records: one a guessed
/// card asked for (`guess_row`), on any row, and one a card no guess
/// opened asked for (`card_row`), on that card's own listing.
#[must_use]
pub fn guards_row(guess_row: Option<&str>, card_row: Option<&str>, played: &str) -> bool {
    guess_row.is_some()
        || card_row.is_some_and(|row| crate::scraper::provider::same_listing(row, played))
}

/// The show a play landing on `played` would write over: the one that
/// row records, or, for a row that records none, the one the card's line
/// `card` records when the play landed on that card's own listing. A
/// show Kitsu answered gone (`gone`) counts at neither step.
#[must_use]
pub fn line_record<'a>(
    rows: &'a [crate::history::HistoryEntry],
    played: &str,
    card: Option<&str>,
    gone: impl Fn(&str) -> bool,
) -> Option<&'a str> {
    // A show Kitsu answered gone is no show to keep: the line is a guess
    // again, and the next step is asked.
    let recorded = |id: &str| {
        rows.iter()
            .find(|r| r.id == id)
            .and_then(|r| r.kitsu_id.as_deref())
            .filter(|k| !gone(k))
    };
    recorded(played).or_else(|| {
        card.filter(|c| *c != played && crate::scraper::provider::same_listing(c, played))
            .and_then(recorded)
    })
}

/// `candidate`, the id the play `args` would record on the row `played`,
/// unless a card asked for the play there ([`guards_row`]) and the row,
/// or else the card's line on its own listing ([`line_record`]), records
/// another ([`keeps_recorded`]). A recorded show Kitsu answered gone does
/// not count. A history that cannot be read keeps such a play's id back:
/// what the row records is not known.
pub(crate) fn guarded<'a>(
    state: &AppState,
    args: &PlayArgs,
    played: &str,
    candidate: Option<&'a str>,
) -> Option<&'a str> {
    if !guards_row(args.guess_row.as_deref(), args.card_row.as_deref(), played)
        || candidate.is_none()
    {
        return candidate;
    }
    let Ok(rows) = crate::history::read_all(&state.history_path) else {
        return None;
    };
    // A recorded show Kitsu answered gone is no link: the line is a guess
    // again, and a play of the guess on its own listing repairs it.
    let card = args.guess_row.as_deref().or(args.card_row.as_deref());
    let recorded = line_record(&rows, played, card, |id| {
        crate::commands::kitsu_gone::is_gone(state, id).unwrap_or(false)
    });
    keeps_recorded(true, recorded, candidate)
}

/// Whether the play `args`, asked by a card no guess opened
/// (`card_row`), keeps the other show the card's line `played` records:
/// it records nothing there, and the renderer writes no tracker for it.
pub(crate) fn line_kept(state: &AppState, args: &PlayArgs, played: &str) -> bool {
    let named = args.kitsu_id.as_deref();
    args.guess_row.is_none()
        && guards_row(None, args.card_row.as_deref(), played)
        && named.is_some()
        && guarded(state, args, played, named) != named
}

/// Whether the guessed play `args` landed on the card's own listing
/// `played` and may stand for its row: the row records no other show
/// than the one the play names as its guess.
pub(crate) fn own_listing(state: &AppState, args: &PlayArgs, played: &str) -> bool {
    let guess = args.guess_id.as_deref();
    args.lands_on_guess_row(played) && guarded(state, args, played, guess) == guess
}
