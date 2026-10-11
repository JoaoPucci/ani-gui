//! Removals through the guard: recording a show removed or the
//! history cleared, and the reads a write in flight asks of them —
//! removed, changed or overtaken since it began. Split from
//! [`super`] so each file stays inside the CRAP gate's per-file bar.

use super::{Asked, Epoch, Held};

impl Held<'_> {
    /// Record that the show `id` was removed from the history, known
    /// by each of `kitsu_ids` that no remaining row claims.
    pub fn removed_show(&mut self, id: &str, kitsu_ids: &[String]) {
        self.removals.moment += 1;
        let now = self.removals.moment;
        self.removals.shows.insert(id.to_owned(), now);
        self.removals.watched.remove(id);
        self.removals.pages.remove(id);
        for kitsu_id in kitsu_ids {
            self.removals.kitsu.insert(kitsu_id.clone(), now);
        }
    }

    /// Record that the history was cleared.
    pub fn removed_all(&mut self) {
        self.removals.moment += 1;
        self.removals.cleared = self.removals.moment;
        // The clear stands for everything removed before it.
        self.removals.shows.clear();
        self.removals.watched.clear();
        self.removals.kitsu.clear();
        self.removals.pages.clear();
    }

    /// Whether the show `id` was removed, or the history cleared,
    /// since `begun`.
    #[must_use]
    pub fn show_removed_since(&self, begun: Epoch, id: &str) -> bool {
        let removed = self.removals.shows.get(id).copied().unwrap_or(0);
        removed.max(self.removals.cleared) > begun.0
    }

    /// Whether the show `id`'s row changed since `begun`: the show was
    /// removed, or the history cleared, or another write gave the row
    /// its watch moment or Kitsu id. A watch takes `begun` just after
    /// writing its row, so its own write is not a change.
    #[must_use]
    pub fn show_changed_since(&self, begun: Epoch, id: &str) -> bool {
        self.show_removed_since(begun, id)
            || self.removals.watched.get(id).copied().unwrap_or(0) > begun.0
    }

    /// Whether what the play `asked` for would write under the show
    /// key `id` was removed since the play began: the key's own row,
    /// or — the play having resolved under another key than the row
    /// had — the show known by the page it was asked from.
    #[must_use]
    pub fn removed_since(&self, asked: Asked<'_>, id: &str) -> bool {
        self.show_removed_since(asked.begun, id)
            || asked
                .page
                .is_some_and(|page| self.kitsu_removed_since(asked.begun, page))
    }

    /// Whether what the play `asked` for would write under the show
    /// key `id` was overtaken since the play began: removed
    /// ([`Self::removed_since`]), or given a watch moment or Kitsu id
    /// by another watch ([`Self::show_changed_since`]). A request begun
    /// before a watch was recorded does not replace that watch's row.
    #[must_use]
    pub fn overtaken_since(&self, asked: Asked<'_>, id: &str) -> bool {
        self.removed_since(asked, id) || self.show_changed_since(asked.begun, id)
    }
}
