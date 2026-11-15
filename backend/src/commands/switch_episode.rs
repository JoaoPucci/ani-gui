//! The episode "Switch to it now" opens on the show the viewer picked
//! for a Continue card.
//!
//! A switch plays the pick's own stream. It used to open the card's
//! episode, from the point playing; when the viewer had watched the
//! pick further on its own, that watch took the pick's row back, and
//! Continue Watching offered an episode the viewer was past. Progress
//! never moves back now: where the pick's own row stands, by the app's
//! Continue rule (the next episode, the last one again when it was left
//! part-way or when it is the last one available — Kitsu's count capped
//! at the episodes aired, as the renderer sends it — and an extra going
//! on to the whole episode above it: `frontend/src/lib/play/next-episode.ts`),
//! is opened when it is past the card's episode. The streaming source's
//! own count, which Home's Continue also caps at, is not read: it takes
//! a request to the provider. A pick whose next episode is aired but
//! not on the source yet fails the switch rather than open an earlier
//! episode. Less, the same or no
//! progress of its own keeps the card's episode, as before.
//!
//! The pick's own row is the one its Kitsu id finds in the history, by
//! the rule the detail page resumes from
//! ([`crate::commands::history::history_by_kitsu`]), less the rows of
//! the card's own listing: the card's line was just named for the pick,
//! and its episode is the one playing, not the pick's progress. Which
//! of the pick's episodes were left part-way is what only the renderer
//! keeps (the positions stay on this computer), and arrives as facts.

use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::error::Result;

/// A request for the episode a switch to `pick` opens.
#[derive(Debug, Clone, Deserialize)]
pub struct SwitchArgs {
    /// The Kitsu id of the show picked.
    pub pick: String,
    /// The card's history row.
    pub card: String,
    /// The episode playing on the card's session.
    pub episode: f64,
    /// The pick's last episode available: Kitsu's count, capped at the
    /// episodes aired for a show still airing, when known.
    #[serde(default)]
    pub episode_count: Option<u32>,
    /// The pick's episodes with a kept position: left part-way.
    #[serde(default)]
    pub part_way: Vec<f64>,
}

/// Where a switch opens the pick.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SwitchEpisode {
    /// The pick's own episode, further than the card's; `None` keeps
    /// the card's episode and point.
    pub episode: Option<f64>,
}

/// The episode a row's `ep_no` names, as the history reads it (the
/// renderer's `parseHistoryEpisode`): its leading number, a fraction
/// included when a dot and digits follow (`7.5`, the first of a range
/// `1-12`), `None` for no positive episode.
#[must_use]
pub fn row_episode(ep_no: &str) -> Option<f64> {
    let text = ep_no.trim_start();
    let whole = text.chars().take_while(char::is_ascii_digit).count();
    if whole == 0 {
        return None;
    }
    let rest = &text[whole..];
    let frac = rest
        .strip_prefix('.')
        .map_or(0, |r| r.chars().take_while(char::is_ascii_digit).count());
    let end = if frac > 0 { whole + 1 + frac } else { whole };
    text[..end].parse::<f64>().ok().filter(|n| *n > 0.0)
}

/// The episode Continue opens for a show last watched at `own`.
fn resume(own: f64, cap: Option<u32>, part_way: &[f64]) -> f64 {
    if part_way.contains(&own) {
        return own;
    }
    let next = if own.fract() == 0.0 {
        own + 1.0
    } else {
        own.ceil()
    };
    match cap {
        Some(cap) if next > f64::from(cap) => own,
        _ => next,
    }
}

/// The pick's own episode a switch from the card's `card_episode`
/// opens, when its own row stands at `own` and Continue would take it
/// past the card's; `None` keeps the card's episode.
#[must_use]
pub fn switch_episode(
    card_episode: f64,
    own: Option<f64>,
    cap: Option<u32>,
    part_way: &[f64],
) -> Option<f64> {
    let opened = resume(own?, cap, part_way);
    (opened > card_episode).then_some(opened)
}

/// Where a switch to `args.pick` from the card `args.card` opens it.
///
/// # Errors
/// History and cache read errors: a switch that cannot tell where the
/// pick stands does not guess.
pub fn plan(state: &AppState, args: &SwitchArgs) -> Result<SwitchEpisode> {
    let kitsu_id = crate::kitsu_id::require(&args.pick)?;
    let own = crate::commands::history::history_by_kitsu_except(state, kitsu_id, |row| {
        crate::commands::history_link::card_listing(&args.card, row)
    })?;
    let own = own.and_then(|r| row_episode(&r.ep_no));
    Ok(SwitchEpisode {
        episode: switch_episode(args.episode, own, args.episode_count, &args.part_way),
    })
}

#[cfg(test)]
#[path = "switch_episode_test.rs"]
mod tests;
