//! The pick itself: [`pick`] reads the list once (`Entries`) and
//! returns the hero, the row and the walk together, so the walk is
//! what made the pick. The rule is the module's (`super`).

use super::verdict::Verdicts;
use super::{
    art, is_airing, Art, HeroShow, Pick, SkippedBecause, TakenBecause, Verdict, Walk, WalkStep,
};

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
pub(super) struct Entries<'a> {
    shows: &'a [HeroShow],
    art: Vec<Art>,
    pub(super) first: Vec<bool>,
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

    pub(super) fn has_art(&self, i: usize) -> bool {
        self.art[i] != Art::None
    }

    /// Whether the airing preference ranks `i` ahead of the shows with
    /// art that are not airing.
    pub(super) fn lifted(&self, i: usize, airing_first: bool) -> bool {
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
