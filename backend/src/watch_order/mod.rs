//! The watch order: the decisions the show page, the franchise page,
//! the player's Up next card and Continue Watching's hand-off make from
//! the franchise walk (`commands::franchise_walk`) and the watch
//! history. Ordering, numbering, grouping, progress and the finale's
//! next title are decided here; the renderer only words and draws them
//! (`docs/architecture.md`, "Order and position numbers").
//!
//! Every function in these modules is pure over the walk's
//! [`Franchise`] and the history rows it is handed.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use crate::commands::franchise_walk::{Chain, Franchise};
pub use crate::meta::kitsu::FranchiseEntry as WatchEntry;

pub mod by_start;
pub mod chain_end;
pub mod chain_view;
mod components;
pub mod entry;
pub mod franchise_link;
pub mod franchise_page;
pub mod graph;
pub mod handoff;
pub mod js_text;
mod no_clear_place;
pub mod page_model;
pub mod place;
pub mod progress;
pub mod rail;
pub mod run;
pub mod sections;
pub mod segments;
pub mod sequence;
pub mod strip_view;
pub mod up_next;
pub mod wire;

/// A prequel → sequel pair between two run members.
pub type RunLink = (String, String);

/// The order setting's two values (`config::watch_order`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WatchOrder {
    /// Along the prequel/sequel links.
    Linked,
    /// By start date.
    Released,
}

impl WatchOrder {
    /// The order a stored setting names: anything but `released` reads
    /// as linked (`config::watch_order::normalize`).
    #[must_use]
    pub fn of(value: &str) -> Self {
        if value == crate::config::watch_order::RELEASED {
            Self::Released
        } else {
            Self::Linked
        }
    }
}

/// A run as the ordering works on it: the payload's sections plus the
/// entry the run is numbered from ([`name_id`](run::name_id)).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchRun {
    /// The run's first entry.
    #[serde(alias = "nameId")]
    pub name_id: String,
    /// The run's members.
    pub entries: Vec<WatchEntry>,
    /// Links between two different members, each pair once.
    pub links: Vec<RunLink>,
    /// Other versions of the run's entries.
    pub versions: Vec<WatchEntry>,
    /// Films around the run.
    pub films: Vec<WatchEntry>,
    /// Side stories around the run.
    pub sides: Vec<WatchEntry>,
    /// Recaps of the run's entries.
    pub recaps: Vec<WatchEntry>,
    /// A side story → its parent stories among the members.
    pub parents: BTreeMap<String, Vec<String>>,
}

impl WatchRun {
    /// Every title the franchise page lists: the run, then its other
    /// versions, films, side stories and recaps.
    pub fn all(&self) -> impl Iterator<Item = &WatchEntry> {
        self.entries
            .iter()
            .chain(&self.versions)
            .chain(&self.films)
            .chain(&self.sides)
            .chain(&self.recaps)
    }
}

/// An entry watched to its end, or the furthest episode played.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EntryProgress {
    /// A row reached its last known episode.
    Watched,
    /// The furthest episode a row names (a recap's `7.5` included).
    Episode(f64),
}

impl Serialize for EntryProgress {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Watched => s.serialize_str("watched"),
            Self::Episode(n) => s.serialize_f64(*n),
        }
    }
}

impl<'de> Deserialize<'de> for EntryProgress {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Word(String),
            Number(f64),
        }
        match Raw::deserialize(d)? {
            Raw::Word(w) if w == "watched" => Ok(Self::Watched),
            Raw::Word(w) => Err(serde::de::Error::custom(format!("not a progress: {w}"))),
            Raw::Number(n) => Ok(Self::Episode(n)),
        }
    }
}

/// Progress per Kitsu id; an entry with no row is absent.
pub type Progress = BTreeMap<String, EntryProgress>;

/// The fields of a history row the progress reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryRow {
    /// The Kitsu id the row counts toward, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kitsu_id: Option<String>,
    /// The episode the row names, as the history stores it.
    pub ep_no: String,
}

#[cfg(test)]
#[path = "fixtures_test.rs"]
pub(crate) mod fixtures;

#[cfg(test)]
#[path = "order_test.rs"]
mod order_tests;

#[cfg(test)]
#[path = "strip_test.rs"]
mod strip_tests;

#[cfg(test)]
#[path = "strategies_test.rs"]
pub(crate) mod strategies;

#[cfg(test)]
#[path = "page_test.rs"]
mod page_tests;

#[cfg(test)]
#[path = "chain_test.rs"]
mod chain_tests;

#[cfg(test)]
#[path = "up_next_test.rs"]
mod up_next_tests;

#[cfg(test)]
#[path = "handoff_test.rs"]
mod handoff_tests;

#[cfg(test)]
#[path = "progress_test.rs"]
mod progress_tests;

#[cfg(test)]
#[path = "franchise_page_test.rs"]
mod franchise_page_tests;

#[cfg(test)]
#[path = "parity_test.rs"]
mod parity_tests;

#[cfg(test)]
#[path = "scenarios_test.rs"]
pub(crate) mod scenarios;

#[cfg(test)]
#[path = "scenario_strip_test.rs"]
mod scenario_strip_tests;

#[cfg(test)]
#[path = "scenario_franchise_page_test.rs"]
mod scenario_franchise_page_tests;

#[cfg(test)]
#[path = "scenario_up_next_test.rs"]
mod scenario_up_next_tests;

#[cfg(test)]
#[path = "scenario_handoff_test.rs"]
mod scenario_handoff_tests;

#[cfg(test)]
#[path = "scenario_arrival_test.rs"]
mod scenario_arrival_tests;

#[cfg(test)]
#[path = "text_test.rs"]
mod text_tests;

#[cfg(test)]
#[path = "order_prop_test.rs"]
mod order_prop_tests;
