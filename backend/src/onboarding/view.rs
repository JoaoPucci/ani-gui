//! What the routes send: the tips offered to this install, and the
//! list of every declared tip with its state. Ids, kinds,
//! pages, places and release numbers only: the words are the
//! renderer's (AGENTS.md §4).

use serde::Serialize;

use super::decl::{find, Kind, Moment, Surface, TipDecl};
use super::eligibility::{offered, pending, window};
use super::release::Release;
use super::state::Record;

/// An entry tip's walkthrough, as the renderer runs it: the sequence's
/// id (its words are named after it), its page and its places.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HowToView {
    /// The sequence tip's id.
    pub id: String,
    /// The page it runs on.
    pub surface: Surface,
    /// Its places, in step order.
    pub places: Vec<String>,
}

/// A tip offered to this install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TipView {
    /// The tip's id.
    pub id: String,
    /// Its tool.
    pub kind: Kind,
    /// The page its places are on.
    pub surface: Surface,
    /// When it may show.
    pub moment: Moment,
    /// The release that shipped its feature.
    pub since: Release,
    /// Its places, in step order.
    pub places: Vec<String>,
    /// An entry tip's walkthrough.
    pub how_to: Option<HowToView>,
    /// Offered and not yet seen.
    pub pending: bool,
}

/// `GET /api/onboarding`: the release the windows were counted at, and
/// every tip offered at it, seen or not, in declared order, each saying
/// whether it is still pending.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OnboardingView {
    /// The release the windows were counted at.
    pub version: Release,
    /// The tips offered.
    pub tips: Vec<TipView>,
}

/// A declared tip, as the debug route lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DebugTip {
    /// The tip's id.
    pub id: String,
    /// Its tool.
    pub kind: Kind,
    /// The page its places are on.
    pub surface: Surface,
    /// When it may show.
    pub moment: Moment,
    /// The release that shipped its feature.
    pub since: Release,
    /// The end it declares, if any.
    pub until: Option<Release>,
    /// This install's window: where it opens and the first release past
    /// it. `None` for an on-demand tip, and for a flagged one with its
    /// flag off or first on at or after its declared end.
    pub opens: Option<Release>,
    /// See [`Self::opens`].
    pub ends: Option<Release>,
    /// The flag gating its feature, if any.
    pub flag: Option<String>,
    /// Whether that flag is on for this launch.
    pub flag_on: bool,
    /// The release of the first read of the tips with it on, if any.
    pub flag_on_since: Option<Release>,
    /// Run only on demand.
    pub on_demand: bool,
    /// Offered at the running release.
    pub offered: bool,
    /// In the seen list.
    pub seen: bool,
    /// Offered and not seen.
    pub pending: bool,
    /// Its places, in step order.
    pub places: Vec<String>,
    /// An entry tip's walkthrough.
    pub how_to: Option<HowToView>,
}

/// `GET /api/onboarding/debug`: every declared tip, the release counted
/// at, and how this install's record began.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DebugView {
    /// The release the windows were counted at.
    pub version: Release,
    /// The release a first-ever install started on; `None` for an
    /// update.
    pub installed_in: Option<Release>,
    /// The declared tips.
    pub tips: Vec<DebugTip>,
}

/// The walkthrough an entry tip names, from the registry.
#[must_use]
pub fn how_to(tip: &TipDecl, tips: &[TipDecl]) -> Option<HowToView> {
    let guide = find(tips, tip.how_to.as_deref()?)?;
    Some(HowToView {
        id: guide.id.clone(),
        surface: guide.surface,
        places: guide.places.clone(),
    })
}

/// The tips offered to this install at `current`.
#[must_use]
pub fn offered_view(
    tips: &[TipDecl],
    record: &Record,
    current: Release,
    flag_on: impl Fn(&str) -> bool,
) -> OnboardingView {
    let tips_out = tips
        .iter()
        .filter(|t| offered(t, record, current, &flag_on))
        .map(|t| TipView {
            id: t.id.clone(),
            kind: t.kind,
            surface: t.surface,
            moment: t.moment,
            since: t.since,
            places: t.places.clone(),
            how_to: how_to(t, tips),
            pending: pending(t, record, current, &flag_on),
        })
        .collect();
    OnboardingView {
        version: current,
        tips: tips_out,
    }
}

/// Every declared tip with this install's state.
#[must_use]
pub fn debug_view(
    tips: &[TipDecl],
    record: &Record,
    current: Release,
    flag_on: impl Fn(&str) -> bool,
) -> DebugView {
    let tips_out = tips
        .iter()
        .map(|t| debug_tip(t, tips, record, current, &flag_on))
        .collect();
    DebugView {
        version: current,
        installed_in: record.installed_in,
        tips: tips_out,
    }
}

fn debug_tip(
    t: &TipDecl,
    tips: &[TipDecl],
    record: &Record,
    current: Release,
    flag_on: &impl Fn(&str) -> bool,
) -> DebugTip {
    let win = window(t, record, current, flag_on);
    DebugTip {
        id: t.id.clone(),
        kind: t.kind,
        surface: t.surface,
        moment: t.moment,
        since: t.since,
        until: t.until,
        opens: win.map(|w| w.0),
        ends: win.map(|w| w.1),
        flag: t.flag.clone(),
        flag_on: t.flag.as_deref().is_some_and(flag_on),
        flag_on_since: t.flag.as_deref().and_then(|f| record.flag_on_since(f)),
        on_demand: t.on_demand,
        offered: offered(t, record, current, flag_on),
        seen: record.seen.contains(&t.id),
        pending: pending(t, record, current, flag_on),
        places: t.places.clone(),
        how_to: how_to(t, tips),
    }
}

/// Why a tip cannot be marked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkRefused {
    /// No tip has that id.
    Unknown,
    /// The tip runs only on demand, and keeps no seen state.
    OnDemand,
}

/// The record with tip `id` marked seen, or unseen.
///
/// # Errors
/// [`MarkRefused`] for an id no tip has, and for an on-demand tip.
pub fn mark(
    tips: &[TipDecl],
    record: &Record,
    id: &str,
    seen: bool,
) -> Result<Record, MarkRefused> {
    let tip = find(tips, id).ok_or(MarkRefused::Unknown)?;
    if tip.on_demand {
        return Err(MarkRefused::OnDemand);
    }
    let mut next = record.clone();
    if seen {
        next.seen.insert(tip.id.clone());
    } else {
        next.seen.remove(&tip.id);
    }
    Ok(next)
}

#[cfg(test)]
#[path = "view_test.rs"]
mod tests;
