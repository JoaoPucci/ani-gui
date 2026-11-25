//! The tip registry: one declaration per tip, read from `tips.json`.
//!
//! A release that ships a feature worth pointing at adds its tip to
//! `tips.json`. The file is data the backend owns: it is compiled in,
//! read once, and held to the rules [`validate`] states by the tests.
//! `tests/arch/onboarding_places.sh` reads the same file with a JSON
//! parser to hold the renderer to it: every place a tip names is marked
//! once in the renderer's source, and every step has its words in every
//! locale.
//!
//! The backend never sends a tip's words (AGENTS.md §4). Each step's
//! title and body are Paraglide messages named after the tip and the
//! step (`onboarding_<id>_<step>_title`, `…_body`, the id with `-` as
//! `_` and the step counted from 1), resolved in the renderer.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use super::release::Release;

/// The most steps a sequence may have. Completion falls with every step
/// a tour adds, and a feature that needs more gets an entry point with
/// "Show me how" on its own page instead.
pub const MAX_STEPS: usize = 3;

/// Minor releases a tip is offered for, counted from the release that
/// shipped it, or for a flagged feature from the first launch with its
/// flag on: shipped in 0.19.0, a tip is offered in 0.19.x and 0.20.x
/// and gone from 0.21.0.
pub const OFFERED_MINORS: u32 = 2;

/// Which of the three tools a tip uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// One balloon on one place, with the page dimmed around it.
    Single,
    /// Up to [`MAX_STEPS`] balloons on one page, with Next, Back and
    /// Skip.
    Sequence,
    /// One balloon on a feature's entry point, offering the feature's
    /// walkthrough ([`TipDecl::how_to`]) on demand.
    Entry,
}

/// The page a tip's places are on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    /// Home.
    Home,
    /// A show's page.
    Show,
    /// A show's franchise page.
    Franchise,
    /// Settings.
    Settings,
    /// The player.
    Play,
    /// The app's own chrome, on any page.
    Any,
}

/// When a tip may show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Moment {
    /// When its place is reached and still.
    Arrive,
    /// In the player, while a stream loads, before playback starts.
    StreamLoad,
}

/// One declared tip.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TipDecl {
    /// Stable, and the key its seen state is kept under: lower-case
    /// words joined by `-`.
    pub id: String,
    /// Which tool it uses.
    pub kind: Kind,
    /// The release that shipped the feature.
    pub since: Release,
    /// The first release that no longer offers it. Left out, it is
    /// [`OFFERED_MINORS`] after `since`. A flagged tip must declare it:
    /// its window counts from the flag's first launch on, with this as
    /// the hard limit.
    #[serde(default)]
    pub until: Option<Release>,
    /// The page its places are on.
    pub surface: Surface,
    /// When it may show.
    pub moment: Moment,
    /// The default-OFF flag gating its feature, if any: the tip is
    /// offered only while the flag is on.
    #[serde(default)]
    pub flag: Option<String>,
    /// The `data-onboard` ids its steps point at, in step order.
    pub places: Vec<String>,
    /// An entry tip's walkthrough: the id of a sequence tip, run on its
    /// own page when asked for.
    #[serde(default)]
    pub how_to: Option<String>,
    /// Runs only when asked for: a feature's walkthrough, started by
    /// "Show me how", or a tool's demonstration, started from the Debug
    /// page. Never offered on its own, never pending, and never marked
    /// seen.
    #[serde(default)]
    pub on_demand: bool,
}

/// Every tip this build declares, in the order they are offered.
pub static TIPS: LazyLock<Vec<TipDecl>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("tips.json")).expect("tips.json declares the tips")
});

/// Look a declared tip up by its id.
#[must_use]
pub fn find<'a>(tips: &'a [TipDecl], id: &str) -> Option<&'a TipDecl> {
    tips.iter().find(|t| t.id == id)
}

/// Check a registry: ids well-formed and unique, places well-formed
/// and unique within a tip, each tool's shape, windows that open before
/// they close, flags this build declares, and walkthroughs that name a
/// sequence on a page of their own.
///
/// # Errors
/// A description of the first problem found.
pub fn validate(tips: &[TipDecl], flags: &[crate::flags::FlagDecl]) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    for tip in tips {
        check_one(tip, flags)?;
        if !ids.insert(tip.id.as_str()) {
            return Err(format!("{} is declared twice", tip.id));
        }
    }
    tips.iter().try_for_each(|t| check_how_to(t, tips))
}

/// Lower-case ASCII words joined by single `-`.
fn is_kebab(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn check_one(tip: &TipDecl, flags: &[crate::flags::FlagDecl]) -> Result<(), String> {
    let id = &tip.id;
    if !is_kebab(id) {
        return Err(format!("{id:?} is not lower-case words joined by -"));
    }
    check_places(tip)?;
    check_shape(tip)?;
    check_window(tip)?;
    if let Some(flag) = &tip.flag {
        if !flags.iter().any(|d| d.name == flag) {
            return Err(format!(
                "{id} names {flag}, which this build does not declare"
            ));
        }
    }
    Ok(())
}

fn check_places(tip: &TipDecl) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for place in &tip.places {
        if !is_kebab(place) {
            return Err(format!("{}: place {place:?} is not kebab-case", tip.id));
        }
        if !seen.insert(place) {
            return Err(format!("{}: place {place} is named twice", tip.id));
        }
    }
    Ok(())
}

fn check_shape(tip: &TipDecl) -> Result<(), String> {
    let id = &tip.id;
    let steps = tip.places.len();
    let fits = match tip.kind {
        Kind::Single | Kind::Entry => steps == 1,
        Kind::Sequence => (1..=MAX_STEPS).contains(&steps),
    };
    if !fits {
        return Err(format!(
            "{id}: a {:?} tip cannot have {steps} steps",
            tip.kind
        ));
    }
    if tip.how_to.is_some() != (tip.kind == Kind::Entry) {
        return Err(format!(
            "{id}: an entry tip, and only one, names a walkthrough"
        ));
    }
    if tip.moment == Moment::StreamLoad
        && (tip.kind != Kind::Single || tip.surface != Surface::Play)
    {
        return Err(format!(
            "{id}: a stream-load tip is a single tip in the player"
        ));
    }
    Ok(())
}

fn check_window(tip: &TipDecl) -> Result<(), String> {
    let id = &tip.id;
    if tip.flag.is_some() && tip.until.is_none() {
        return Err(format!("{id}: a flagged tip declares its hard end"));
    }
    match tip.until {
        Some(until) if until <= tip.since => Err(format!(
            "{id}: its window closes at {until}, before it opens"
        )),
        _ => Ok(()),
    }
}

fn check_how_to(tip: &TipDecl, tips: &[TipDecl]) -> Result<(), String> {
    let Some(target) = &tip.how_to else {
        return Ok(());
    };
    let Some(guide) = find(tips, target) else {
        return Err(format!(
            "{}: its walkthrough {target} is not declared",
            tip.id
        ));
    };
    if guide.kind != Kind::Sequence || !guide.on_demand {
        return Err(format!(
            "{}: its walkthrough {target} is not an on-demand sequence",
            tip.id
        ));
    }
    if guide.surface == tip.surface {
        return Err(format!(
            "{}: its walkthrough runs where the feature lives, not on the entry's page",
            tip.id
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "decl_test.rs"]
mod tests;
