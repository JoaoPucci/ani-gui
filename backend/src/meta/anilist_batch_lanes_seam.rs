//! What a test reads of a kind's lanes ([`super::Lanes`]). Test-only;
//! split from [`super`] so the file the app runs stays inside the CRAP
//! gate's per-file bar.

use super::Lanes;

impl<T> Lanes<T> {
    /// How many lookups wait in `lane`'s next batch now, whatever gate
    /// class they count as. For a test to know every lookup it started
    /// has joined the lane before it lets the lane's window pass.
    pub(crate) fn gathered(&self, lane: &str) -> usize {
        let prefix = format!("{lane}\u{0}");
        self.open()
            .iter()
            .filter(|(key, _)| key.starts_with(&prefix))
            .map(|(_, lane)| lane.pending.len())
            .sum()
    }

    /// How many lanes are open under `lane`, one per gate class.
    pub(crate) fn opened(&self, lane: &str) -> usize {
        let prefix = format!("{lane}\u{0}");
        self.open()
            .keys()
            .filter(|key| key.starts_with(&prefix))
            .count()
    }
}
