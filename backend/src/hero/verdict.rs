//! Why the walk took or passed over each entry, read against the pick
//! it describes.

use super::pick::Entries;
use super::{SkippedBecause, TakenBecause, Verdict};

/// What the walk's verdicts are read against.
pub(super) struct Verdicts<'a> {
    pub(super) hero: &'a [usize],
    /// The entries with art the pick off would have taken.
    pub(super) off_art: &'a [usize],
    /// Whether shows with art filled every slot.
    pub(super) art_full: bool,
    pub(super) airing_first: bool,
}

impl Verdicts<'_> {
    pub(super) fn verdict(&self, e: &Entries<'_>, i: usize) -> Verdict {
        if !e.first[i] {
            return skipped(SkippedBecause::Repeat);
        }
        if let Some(slot) = self.hero.iter().position(|&h| h == i) {
            return Verdict::Taken {
                slot,
                because: self.taken_because(e, i, slot),
            };
        }
        skipped(self.skipped_because(e, i))
    }

    /// A show with art is taken because it airs only when the pick off
    /// would not have it in this slot: the shows with art lead the
    /// pick off in `off_art`'s order, so its slot there is its index.
    fn taken_because(&self, e: &Entries<'_>, i: usize, slot: usize) -> TakenBecause {
        if !e.has_art(i) {
            TakenBecause::Fill
        } else if e.lifted(i, self.airing_first)
            && self.off_art.iter().position(|&o| o == i) != Some(slot)
        {
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
