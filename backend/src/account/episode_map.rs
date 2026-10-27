//! Where a Kitsu episode lands on a tracker: which entry, which
//! number, and whether it completes that entry.
//!
//! The app numbers a show by Kitsu's list. A tracker can file the same
//! show differently — one entry split across several of its own, several
//! Kitsu entries merged into one of its own, a prologue or recaps Kitsu
//! does not count — and the mark-watched write-back has to send each
//! episode to the right entry under the right number, or send the
//! episode as it is when nothing conclusive says otherwise. This module
//! holds the shape of that answer, [`EpisodeMap`], and how a Kitsu
//! episode is read through it ([`EpisodeMap::target`]). The rules that
//! build one from evidence live in [`super::episode_map_rules`]; the
//! evidence is gathered and the map cached by
//! `crate::commands::tracker_map`. `docs/title-resolution.md` ("Episode
//! numbers") is the contract these rules follow.

use serde::{Deserialize, Serialize};

/// A Kitsu entry as the mapping reads it: its announced count and its
/// run, as `YYYY-MM-DD` strings (Kitsu's wire form; a shorter prefix when
/// a source knows less).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct KitsuShow {
    /// Kitsu's anime id.
    pub id: String,
    /// `episodeCount`, `None` while unknown.
    pub episodes: Option<u32>,
    /// `startDate`.
    pub start: Option<String>,
    /// `endDate`.
    pub end: Option<String>,
}

/// A tracker's entry as the mapping reads it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TrackerShow {
    /// The tracker's own id for the entry.
    pub id: u32,
    /// The MyAnimeList id the entry records, where the tracker is not
    /// MyAnimeList itself and records one.
    #[serde(default)]
    pub id_mal: Option<u32>,
    /// The tracker's episode total, `None` while unknown.
    pub episodes: Option<u32>,
    /// Whether the tracker says the entry finished airing.
    pub finished: bool,
    /// First air date, `YYYY-MM-DD` or a shorter prefix.
    pub start: Option<String>,
    /// Last air date, `YYYY-MM-DD` or a shorter prefix.
    pub end: Option<String>,
}

/// Everything the rules read about one Kitsu entry on one tracker.
/// Each field is what one request answers, so a recorded fixture is one
/// of these with the answers as the tracker and Kitsu gave them.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Evidence {
    /// The Kitsu entry whose episodes are being mapped.
    pub kitsu: KitsuShow,
    /// The tracker entry Kitsu's mappings (or a prequel's, for a part
    /// with no mapping of its own) resolve the show to.
    pub entry: TrackerShow,
    /// The tracker's chain of sequels from `entry`, as far as it was
    /// followed: each hop is the only series sequel of the one before.
    #[serde(default)]
    pub chain: Vec<TrackerShow>,
    /// The ids among `chain` that some Kitsu entry maps to itself.
    #[serde(default)]
    pub claimed: Vec<u32>,
    /// The numbers the tracker's own episode list carries (`"0"`,
    /// `"6.5"`, `"7"`), `None` when the tracker lists none.
    #[serde(default)]
    pub listed: Option<Vec<String>>,
    /// Every Kitsu entry mapped to `entry`, `kitsu` among them.
    #[serde(default)]
    pub sharing: Vec<KitsuShow>,
    /// The air dates Kitsu's own episode list gives the show's episodes,
    /// by number — read only for the bounds of an exact, unclaimed chain
    /// that Kitsu's start and end dates leave outside its run.
    #[serde(default)]
    pub kitsu_airdates: std::collections::BTreeMap<u32, String>,
    /// Kitsu's chain of parts filed under `entry`: the entry that maps
    /// to it first, then each single sequel with no entry of its own on
    /// this tracker, as far as it was followed; `kitsu` is one of them.
    #[serde(default)]
    pub parts: Vec<KitsuShow>,
}

/// One run of Kitsu episodes `kitsu_from..=kitsu_to` filed under one
/// tracker entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    /// The tracker entry these episodes are sent to.
    pub entry: u32,
    /// Its episode total.
    pub entry_total: u32,
    /// Whether the tracker says it finished airing.
    pub entry_finished: bool,
    /// The first Kitsu episode in the segment.
    pub kitsu_from: u32,
    /// The last Kitsu episode in the segment.
    pub kitsu_to: u32,
    /// Episodes of the entry before `kitsu_from`'s: earlier Kitsu parts
    /// filed under the same entry.
    pub lead: u32,
    /// For each episode the entry counts that Kitsu does not list, the
    /// Kitsu episode it follows — `0` for one before the first.
    #[serde(default)]
    pub extras_after: Vec<u32>,
}

/// Why a map sends every episode as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Identity {
    /// Kitsu's count and the tracker's total agree.
    CountsAgree,
    /// One of the two counts is unknown.
    CountUnknown,
    /// The counts differ and no rule explains the difference.
    Unexplained,
    /// Two rules each explain the difference, differently.
    Conflict,
}

/// How a map explains the show's filing on the tracker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "shape", content = "why")]
pub enum Shape {
    /// Nothing conclusive: every episode is sent as it is.
    Identity(Identity),
    /// One Kitsu entry spans a chain of the tracker's entries.
    Chain,
    /// Several Kitsu entries share the tracker's entry, each mapped to it.
    SharedEntry,
    /// Several Kitsu parts are filed under the tracker's entry, the
    /// later ones with no entry of their own there.
    SplitParts,
    /// The tracker's entry counts episodes Kitsu does not list.
    Extras,
    /// A special the tracker has no entry for is among the extras a
    /// related entry's own map reads.
    SpecialExtras,
}

/// A show's filing on one tracker: its shape and, unless identity, the
/// segments that cover Kitsu's episodes `1..=count`, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpisodeMap {
    /// What explains the filing.
    #[serde(flatten)]
    pub shape: Shape,
    /// The segments, empty for an identity map.
    #[serde(default)]
    pub segments: Vec<Segment>,
}

/// A segment before the one an episode lands in: its entry must be
/// complete once the user is past it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EarlierSegment {
    /// The entry.
    pub entry: u32,
    /// The total it is brought to.
    pub total: u32,
    /// Whether the tracker says it finished, so it is completed too.
    pub finished: bool,
}

/// What one Kitsu episode becomes on the tracker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// No segment covers the episode: it is sent as it is, to the entry
    /// Kitsu maps the show to.
    Identity,
    /// The episode is filed under a segment's entry.
    Mapped(MappedWrite),
}

/// The write a mapped episode becomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedWrite {
    /// The entry written to.
    pub entry: u32,
    /// The progress sent: the episode's number in that entry.
    pub progress: u32,
    /// Whether the episode is the last the entry counts, on an entry the
    /// tracker says finished — the write completes it.
    pub complete: bool,
    /// The segments before this one, whose entries the write brings to
    /// their totals first.
    pub earlier: Vec<EarlierSegment>,
}

impl EpisodeMap {
    /// A map that sends every episode as it is, for `why`.
    #[must_use]
    pub fn identity(why: Identity) -> Self {
        Self {
            shape: Shape::Identity(why),
            segments: Vec::new(),
        }
    }

    /// Whether the map rests on evidence that does not move — the
    /// counts agreeing, or a rule explaining their difference — rather
    /// than on evidence that was missing or inconclusive.
    #[must_use]
    pub fn conclusive(&self) -> bool {
        !matches!(
            self.shape,
            Shape::Identity(Identity::CountUnknown | Identity::Unexplained | Identity::Conflict)
        )
    }

    /// Where Kitsu's episode `k` lands.
    #[must_use]
    pub fn target(&self, k: u32) -> Target {
        let Some((i, seg)) = self
            .segments
            .iter()
            .enumerate()
            .find(|(_, s)| s.kitsu_from <= k && k <= s.kitsu_to)
        else {
            return Target::Identity;
        };
        let extras_before =
            u32::try_from(seg.extras_after.iter().filter(|e| **e < k).count()).unwrap_or(u32::MAX);
        let position = seg
            .lead
            .saturating_add(k - seg.kitsu_from + 1)
            .saturating_add(extras_before);
        let extras_total = u32::try_from(seg.extras_after.len()).unwrap_or(u32::MAX);
        let ends_entry = k == seg.kitsu_to
            && seg
                .lead
                .saturating_add(seg.kitsu_to - seg.kitsu_from + 1)
                .saturating_add(extras_total)
                == seg.entry_total;
        let complete = ends_entry && seg.entry_finished;
        let progress = if ends_entry {
            seg.entry_total
        } else {
            position
        };
        let earlier = self.segments[..i]
            .iter()
            .map(|s| EarlierSegment {
                entry: s.entry,
                total: s.entry_total,
                finished: s.entry_finished,
            })
            .collect();
        Target::Mapped(MappedWrite {
            entry: seg.entry,
            progress,
            complete,
            earlier,
        })
    }
}
