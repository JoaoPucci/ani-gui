//! The tip registry: one declaration per tip, read from `tips.json`.
//!
//! A release that ships a feature worth pointing at adds its tip to
//! `tips.json`. The file is data the backend owns: it is compiled in,
//! read once, and held to the rules [`validate`] states (in
//! `rules.rs` and `rules_shape.rs`) by the tests.
//! `tests/arch/onboarding_places.sh` reads the same file with a JSON
//! parser to hold the renderer to it: every place a tip names is marked
//! once in the renderer's source, and every step has its words in every
//! locale.
//!
//! The backend never sends a tip's words (AGENTS.md §4). Each step's
//! title and body are Paraglide messages named after the tip and the
//! step (`onboarding_<id>_<step>_title`, `…_body`, the id with `-` as
//! `_` and the step counted from 1), resolved in the renderer.

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use super::release::Release;

/// The most steps a sequence may have. Completion falls with every step
/// a tour adds, and a feature that needs more gets an entry point with
/// "Show me how" on its own page instead.
pub const MAX_STEPS: usize = 3;

/// Minor releases a tip is offered for, counted from the release that
/// shipped it, or for a flagged feature from the first read of the tips
/// with its flag on: shipped in 0.19.0, a tip is offered in 0.19.x and
/// 0.20.x and gone from 0.21.0. The count stays within one major
/// number: a major release (0.20.x to 1.0.0, say) closes every window
/// the major before it opened.
pub const OFFERED_MINORS: u32 = 2;

/// Which of the three tools a tip uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// One step, on one place.
    Single,
    /// Up to [`MAX_STEPS`] steps, all on one page.
    Sequence,
    /// One step on a feature's entry point, naming the feature's
    /// walkthrough ([`TipDecl::how_to`]), run on demand.
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
    /// its window counts from the flag's first read on, with this as
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
    /// Runs only when asked for (a feature's walkthrough, or a tool's
    /// demonstration): never offered on its own, never pending, and
    /// never marked seen.
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

pub use super::rules::validate;

#[cfg(test)]
#[path = "decl_test.rs"]
mod tests;
