//! What the history's guard knows by Kitsu id: the removals of the
//! shows known by it, and Kitsu serving it or answering it gone —
//! split from `guard` for the per-file complexity bar.

use super::{Epoch, Held};

impl Held<'_> {
    /// Whether a show known by `kitsu_id` was removed with no remaining
    /// row claiming the id, or the history cleared, since `begun`.
    #[must_use]
    pub fn kitsu_removed_since(&self, begun: Epoch, kitsu_id: &str) -> bool {
        let removed = self.removals.kitsu.get(kitsu_id).copied().unwrap_or(0);
        removed.max(self.removals.cleared) > begun.0
    }

    /// Record that Kitsu served `kitsu_id`, now, and return the moment.
    pub fn kitsu_served(&mut self, kitsu_id: &str) -> Epoch {
        self.removals.moment += 1;
        self.removals
            .served
            .insert(kitsu_id.to_owned(), self.removals.moment);
        Epoch(self.removals.moment)
    }

    /// Record that Kitsu answered `kitsu_id` is gone, now.
    pub fn kitsu_gone(&mut self, kitsu_id: &str) {
        self.removals.moment += 1;
        self.removals
            .gone
            .insert(kitsu_id.to_owned(), self.removals.moment);
    }

    /// Whether Kitsu answered `kitsu_id` is gone after `served`.
    #[must_use]
    pub fn kitsu_gone_since(&self, served: Epoch, kitsu_id: &str) -> bool {
        self.removals.gone.get(kitsu_id).copied().unwrap_or(0) > served.0
    }

    /// Whether Kitsu served `kitsu_id` after `begun`.
    #[must_use]
    pub fn kitsu_served_since(&self, begun: Epoch, kitsu_id: &str) -> bool {
        self.removals.served.get(kitsu_id).copied().unwrap_or(0) > begun.0
    }
}
