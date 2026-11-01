//! What the scenarios read of the flight table.

use super::AvailabilityFlights;

impl AvailabilityFlights {
    /// Walks in flight.
    pub(crate) fn live(&self) -> usize {
        self.table().flights.len()
    }

    /// Callers waiting on the walk out for `key`, if one is.
    pub(crate) fn waiting(&self, key: &str) -> Option<usize> {
        self.table().flights.get(key).map(|f| f.waiters)
    }

    /// Every live walk nobody waits on is counted, and nothing else.
    pub(crate) fn assert_consistent(&self) {
        let table = self.table();
        let unwatched = table.flights.values().filter(|f| f.waiters == 0).count();
        assert_eq!(table.unwatched, unwatched, "the unwatched count");
    }

    /// Whether the walk out for `key` is interactive, if one is.
    pub(crate) fn interactive(&self, key: &str) -> Option<bool> {
        self.table().flights.get(key).map(|f| f.interactive)
    }

    /// Which walk is out for `key`, if one is: walks are numbered in
    /// the order they start.
    pub(crate) fn walk_id(&self, key: &str) -> Option<u64> {
        self.table().flights.get(key).map(|f| f.id)
    }
}
