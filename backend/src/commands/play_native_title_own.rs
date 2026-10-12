//! Which listing beside a broad one is the entry's own: the entry
//! heading a broad listing, a title reading as the entry, a broad
//! listing's first half — split from `play_native_title_marker` for
//! the per-file complexity bar.

use super::*;

impl EntryTitles<'_> {
    /// Whether the broad listing `wide` has the entry for its head: it
    /// carries the name one of the entry's titles carries ([`name`])
    /// and reads as that title does ([`reading`]) — the one reader the
    /// spanning cut compares titles with. "Show 2" and "Show Second
    /// Cour" are headed by "Show 2nd Season"; "Other Show Season 2",
    /// which reads alike, is another show.
    ///
    /// [`name`]: super::play_native_title_reading::name
    /// [`reading`]: super::play_native_title_reading::reading
    pub(crate) fn heads(&self, wide: &str) -> bool {
        let (theirs, called) = (reading(wide), name(wide));
        self.all()
            .any(|t| reading(t) == theirs && name(t) == called)
    }

    /// Whether `title` reads as one of the entry's titles does and
    /// starts with that title's name ([`starts_with_name_of`]): "Show
    /// 2nd Season" and "Show Part 2" for "Show 2", but not "Other Show
    /// 2nd Season".
    ///
    /// [`starts_with_name_of`]: super::play_native_title_reading::starts_with_name_of
    pub(crate) fn reads_as_entry(&self, title: &str) -> bool {
        let theirs = reading(title);
        self.all()
            .any(|t| reading(t) == theirs && starts_with_name_of(title, t))
    }

    /// Whether `listing` is the first part of the listing `wide` that
    /// would span it ([`opens_on_a_first_division`]) and not the
    /// entry's own: beside a `wide` the entry is not the head of, that
    /// is `wide`'s first half, never the entry.
    ///
    /// [`opens_on_a_first_division`]: super::play_native_title_reading::opens_on_a_first_division
    pub(crate) fn first_part_of(&self, listing: &str, wide: &str) -> bool {
        opens_on_a_first_division(listing, wide) && !self.names_own_part(listing, wide)
    }

    /// Whether `listing` is this entry's own beside the listing `wide`
    /// that would span it: it is one of the entry's titles, or it
    /// reads as the entry ([`Self::reads_as_entry`]) and either carries
    /// `wide`'s stem alone or opens what follows it with a division
    /// ([`names_past_stem`]). So "Show 2", "Show 2nd Season" and "Show
    /// 2nd Season Part 1" are "Show 2"'s own beside "Show" while "Show
    /// 2nd Season Part 2" is not; "Lucky 2 2nd Season" is not "Lucky
    /// 2"'s; and a spinoff — "Show Side Story", "Show Side Story 2" —
    /// is another show of the franchise, numbered or not.
    ///
    /// [`names_past_stem`]: super::play_native_title_reading::names_past_stem
    pub(crate) fn names_own_part(&self, listing: &str, wide: &str) -> bool {
        let exact = normalized(listing);
        self.all().any(|t| normalized(t) == exact)
            || (self.reads_as_entry(listing)
                && (stem(listing) == stem(wide) || names_past_stem(listing, wide)))
    }

    /// Whether every title of the entry reads as naming a division
    /// other than the first ([`reading`]) while `candidate` is one of
    /// their names followed by first divisions alone or by nothing
    /// ([`is_name_and_first_divisions`]), and is not one of the entry's
    /// titles: the entry before this one ("Show" or "Show Season 1" for
    /// "Show 2"), never picked. A candidate adding a word or a mark to
    /// the name ("Gintama'" for "Gintama 2", "Eighty Six" for "86") is
    /// not the entry before it, nor a title carrying a number an entry
    /// title carries after the same words ("Show20" for an entry also
    /// known as "Show 20"). An entry with any title naming no later
    /// division never refuses on this.
    ///
    /// [`reading`]: super::play_native_title_reading::reading
    /// [`is_name_and_first_divisions`]: super::play_native_title_reading::is_name_and_first_divisions
    pub(crate) fn before_entry(&self, candidate: &str) -> bool {
        let exact = normalized(candidate);
        reading(candidate).is_empty()
            && self.all().all(|t| !reading(t).is_empty())
            && self
                .all()
                .any(|t| is_name_and_first_divisions(candidate, t))
            && !self
                .all()
                .any(|t| normalized(t) == exact || carries_number_of(candidate, t))
    }
}
