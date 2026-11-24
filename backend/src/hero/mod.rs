//! Home's hero: which trending shows it rotates through, which the
//! trending row below it lists, and the walk that explains the choice.
//!
//! The renderer fetches the trending list and drops the shows the
//! availability filter knows are gone, then hands what is left here
//! (`POST /api/home/hero`). The pick is made once, by [`pick`], which
//! returns the walk beside the result: the Debug page shows the last
//! one (`GET /api/home/hero/walk`), so what it shows is what made the
//! pick, never a second computation of it.
//!
//! The rule: a show listed more than once counts once, at its first
//! entry. The hero takes the first [`HERO_ROTATION_COUNT`] shows with
//! art of their own ([`art`]: AniList's banner, or any slot of Kitsu's
//! cover), in trending order. When fewer have art, the first shows
//! without it fill the rest, in trending order, after the ones with
//! art; they are drawn as their poster, blurred. The row lists every
//! other show in trending order, so the shows the hero passed over lead
//! it: no show leaves Home and none appears twice.
//!
//! With [`HERO_AIRING_FIRST`] on, the shows with art that are airing
//! now (Kitsu's status `current`) come before the other shows with art,
//! each group in trending order. The fill is untouched. With it off the
//! pick is exactly the rule above.

pub mod last;

use serde::{Deserialize, Serialize};

use crate::flags::{FlagDecl, FlagKind, Resolved};

/// How many shows the hero rotates through.
pub const HERO_ROTATION_COUNT: usize = 3;

/// The most shows one request may carry. The trending list is a few
/// dozen; anything near this is not a trending list.
pub const MAX_SHOWS: usize = 500;

/// The flag that puts airing shows first among the hero's shows with
/// art.
pub const HERO_AIRING_FIRST: FlagDecl = FlagDecl {
    name: "hero_airing_first",
    kind: FlagKind::Rollout,
    safe_because: "on, it only reorders which trending shows with art the home banner \
                   rotates through, airing ones first; every show stays on Home either way, \
                   and off is the build's own order",
};

/// One trending show, as much of it as the pick reads.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeroShow {
    /// Kitsu id.
    pub id: String,
    /// The title the walk shows.
    #[serde(default)]
    pub title: String,
    /// Kitsu's status: `current` while it airs.
    #[serde(default)]
    pub status: Option<String>,
    /// AniList's banner, when the backend has it.
    #[serde(default)]
    pub anilist_banner: Option<String>,
    /// Kitsu's cover, in the slots the hero can draw.
    #[serde(default)]
    pub cover_image: Option<CoverSlots>,
}

/// The slots of Kitsu's cover that count as art.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverSlots {
    /// The large slot.
    #[serde(default)]
    pub large: Option<String>,
    /// The original upload.
    #[serde(default)]
    pub original: Option<String>,
    /// The small slot.
    #[serde(default)]
    pub small: Option<String>,
}

/// What art of its own a show has for the hero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Art {
    /// AniList's banner.
    Banner,
    /// A slot of Kitsu's cover, and no banner.
    Cover,
    /// Neither: the hero would draw its poster, blurred.
    None,
}

/// Why the walk took a show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TakenBecause {
    /// It has art, and came early enough in trending order.
    Art,
    /// It has art and is airing, and the airing preference put it
    /// ahead of the shows with art that are not.
    AiringArt,
    /// It has no art, and too few shows did: it fills a slot.
    Fill,
}

/// Why the walk passed a show over. A show passed over is in the row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkippedBecause {
    /// An earlier entry is the same show; only the first counts.
    Repeat,
    /// It has art and would have been taken in trending order, but it
    /// is not airing and airing shows took its place.
    NotAiring,
    /// The hero had its shows before this one's turn.
    HeroFull,
    /// It has no art, and enough shows did.
    NoArt,
}

/// What the walk did with one entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Verdict {
    /// Taken into the hero, at `slot` (0 first).
    Taken {
        /// Its place in the rotation.
        slot: usize,
        /// Why.
        because: TakenBecause,
    },
    /// Left to the row.
    Skipped {
        /// Why.
        because: SkippedBecause,
    },
}

/// One entry of the trending list, as the walk saw it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WalkStep {
    /// Its place in the list it came in, 1 first.
    pub rank: usize,
    /// Kitsu id.
    pub id: String,
    /// Its title.
    pub title: String,
    /// Its art.
    pub art: Art,
    /// Kitsu's status, as sent.
    pub status: Option<String>,
    /// Whether it is airing now.
    pub airing: bool,
    /// Whether the airing preference decided this entry: taken because
    /// it airs, or passed over because it does not.
    pub preference_applied: bool,
    /// What the walk did with it.
    pub verdict: Verdict,
}

/// The walk behind one pick: every entry considered, in trending
/// order, up to the one that settled the hero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Walk {
    /// Whether airing shows came first.
    pub airing_first: bool,
    /// How many shows the hero takes.
    pub count: usize,
    /// How many entries the list had. Those past the last step were
    /// not needed to settle the hero, and are in the row.
    pub entries: usize,
    /// The entries considered.
    pub steps: Vec<WalkStep>,
}

/// The hero's shows and the row's, by id, and the walk that chose them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pick {
    /// The hero's rotation, in order.
    pub hero: Vec<String>,
    /// The row's shows, in trending order.
    pub row: Vec<String>,
    /// How they were chosen.
    pub walk: Walk,
}

/// What art of its own `show` has: AniList's banner first, then any
/// slot of Kitsu's cover. An empty string is no art.
#[must_use]
pub fn art(show: &HeroShow) -> Art {
    let real = |v: &Option<String>| v.as_deref().is_some_and(|u| !u.is_empty());
    if real(&show.anilist_banner) {
        return Art::Banner;
    }
    match &show.cover_image {
        Some(c) if real(&c.large) || real(&c.original) || real(&c.small) => Art::Cover,
        _ => Art::None,
    }
}

/// Whether `show` is airing now: Kitsu's status `current`.
#[must_use]
pub fn is_airing(show: &HeroShow) -> bool {
    show.status.as_deref() == Some("current")
}

/// The pick under the launch's flags.
#[must_use]
pub fn pick_for(flags: &Resolved, shows: &[HeroShow]) -> Pick {
    pick(
        shows,
        HERO_ROTATION_COUNT,
        flags.enabled(&HERO_AIRING_FIRST),
    )
}

/// Split `shows`, in trending order, into the hero's `count` and the
/// row, airing shows with art first when `airing_first`; see the
/// module's rule.
#[must_use]
pub fn pick(shows: &[HeroShow], count: usize, airing_first: bool) -> Pick {
    let entries = Entries::read(shows);
    let hero = entries.hero(count, airing_first);
    let row = entries
        .firsts()
        .filter(|&i| !hero.contains(&i))
        .map(|i| shows[i].id.clone())
        .collect();
    let walk = Walk {
        airing_first,
        count,
        entries: shows.len(),
        steps: entries.walk(&hero, count, airing_first),
    };
    Pick {
        hero: hero.iter().map(|&i| shows[i].id.clone()).collect(),
        row,
        walk,
    }
}

/// The list as the pick reads it: each entry's art and airing, and
/// whether it is its show's first entry.
struct Entries<'a> {
    shows: &'a [HeroShow],
    art: Vec<Art>,
    first: Vec<bool>,
}

impl<'a> Entries<'a> {
    fn read(shows: &'a [HeroShow]) -> Self {
        let mut seen = std::collections::HashSet::new();
        Self {
            shows,
            art: shows.iter().map(art).collect(),
            first: shows.iter().map(|s| seen.insert(s.id.as_str())).collect(),
        }
    }

    /// Indices of first entries, in trending order.
    fn firsts(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.shows.len()).filter(|&i| self.first[i])
    }

    fn has_art(&self, i: usize) -> bool {
        self.art[i] != Art::None
    }

    /// Whether the airing preference ranks `i` ahead of the shows with
    /// art that are not airing.
    fn lifted(&self, i: usize, airing_first: bool) -> bool {
        airing_first && is_airing(&self.shows[i])
    }

    /// The first entries with art, in the order the hero takes them:
    /// trending order, airing ones first when `airing_first`.
    fn art_order(&self, airing_first: bool) -> Vec<usize> {
        let (lifted, rest): (Vec<usize>, Vec<usize>) = self
            .firsts()
            .filter(|&i| self.has_art(i))
            .partition(|&i| self.lifted(i, airing_first));
        lifted.into_iter().chain(rest).collect()
    }

    /// The hero's entries, in rotation order.
    fn hero(&self, count: usize, airing_first: bool) -> Vec<usize> {
        let mut hero: Vec<usize> = self
            .art_order(airing_first)
            .into_iter()
            .take(count)
            .collect();
        let fill = count.saturating_sub(hero.len());
        hero.extend(self.firsts().filter(|&i| !self.has_art(i)).take(fill));
        hero
    }

    /// How many entries the walk reads: up to the one that settled
    /// the hero, the `count`th it takes first ([`Self::art_order`]
    /// ranks airing shows ahead only when they air, so on that is the
    /// `count`th airing show with art). Past it no entry can change
    /// the pick. The whole list when no entry settles it.
    fn walk_len(&self, count: usize, airing_first: bool) -> usize {
        let Some(nth) = count.checked_sub(1) else {
            return 0;
        };
        self.firsts()
            .filter(|&i| self.has_art(i) && (!airing_first || self.lifted(i, true)))
            .nth(nth)
            .map_or(self.shows.len(), |i| i + 1)
    }

    fn walk(&self, hero: &[usize], count: usize, airing_first: bool) -> Vec<WalkStep> {
        let off_art: Vec<usize> = self.art_order(false).into_iter().take(count).collect();
        let art_taken = hero.iter().filter(|&&i| self.has_art(i)).count();
        let ctx = Verdicts {
            hero,
            off_art: &off_art,
            art_full: art_taken == count,
            airing_first,
        };
        (0..self.walk_len(count, airing_first))
            .map(|i| self.step(i, &ctx))
            .collect()
    }

    fn step(&self, i: usize, ctx: &Verdicts<'_>) -> WalkStep {
        let show = &self.shows[i];
        let verdict = ctx.verdict(self, i);
        WalkStep {
            rank: i + 1,
            id: show.id.clone(),
            title: show.title.clone(),
            art: self.art[i],
            status: show.status.clone(),
            airing: is_airing(show),
            preference_applied: matches!(
                verdict,
                Verdict::Taken {
                    because: TakenBecause::AiringArt,
                    ..
                } | Verdict::Skipped {
                    because: SkippedBecause::NotAiring
                }
            ),
            verdict,
        }
    }
}

/// What the walk's verdicts are read against.
struct Verdicts<'a> {
    hero: &'a [usize],
    /// The entries with art the pick off would have taken.
    off_art: &'a [usize],
    /// Whether shows with art filled every slot.
    art_full: bool,
    airing_first: bool,
}

impl Verdicts<'_> {
    fn verdict(&self, e: &Entries<'_>, i: usize) -> Verdict {
        if !e.first[i] {
            return skipped(SkippedBecause::Repeat);
        }
        if let Some(slot) = self.hero.iter().position(|&h| h == i) {
            return Verdict::Taken {
                slot,
                because: self.taken_because(e, i),
            };
        }
        skipped(self.skipped_because(e, i))
    }

    fn taken_because(&self, e: &Entries<'_>, i: usize) -> TakenBecause {
        if !e.has_art(i) {
            TakenBecause::Fill
        } else if e.lifted(i, self.airing_first) {
            TakenBecause::AiringArt
        } else {
            TakenBecause::Art
        }
    }

    fn skipped_because(&self, e: &Entries<'_>, i: usize) -> SkippedBecause {
        if !e.has_art(i) {
            return if self.art_full {
                SkippedBecause::NoArt
            } else {
                SkippedBecause::HeroFull
            };
        }
        let passed_over = self.airing_first && !e.lifted(i, true) && self.off_art.contains(&i);
        if passed_over {
            SkippedBecause::NotAiring
        } else {
            SkippedBecause::HeroFull
        }
    }
}

fn skipped(because: SkippedBecause) -> Verdict {
    Verdict::Skipped { because }
}

/// The flag-matrix check with the airing preference off: the hero is
/// the trending order's shows with art.
#[cfg(test)]
pub(crate) fn check_off(r: &Resolved) {
    let p = pick_for(r, &matrix_list());
    assert_eq!(p.hero, ["1", "2", "3"]);
    assert!(!p.walk.airing_first);
}

/// The same, on: the airing shows with art lead it.
#[cfg(test)]
pub(crate) fn check_on(r: &Resolved) {
    let p = pick_for(r, &matrix_list());
    assert_eq!(p.hero, ["4", "1", "2"]);
    assert!(p.walk.airing_first);
}

/// Three finished shows with covers, then an airing one.
#[cfg(test)]
fn matrix_list() -> Vec<HeroShow> {
    let show = |id: &str, status: &str| HeroShow {
        id: id.into(),
        status: Some(status.into()),
        anilist_banner: Some(format!("https://banner.example/{id}.jpg")),
        ..HeroShow::default()
    };
    vec![
        show("1", "finished"),
        show("2", "finished"),
        show("3", "finished"),
        show("4", "current"),
    ]
}

#[cfg(test)]
#[path = "pick_test.rs"]
mod pick_tests;

#[cfg(test)]
#[path = "pick_prop_test.rs"]
mod pick_prop_tests;
