//! Property coverage for the user's row as MAL's anime read returns it.
//!
//! Strategies are boxed, as in the other MAL property modules, so the
//! Windows test build's debug info stays under the linker's PDB limit.

use super::*;
use crate::account::status::ListStatus;
use proptest::prelude::*;

const STATUSES: [&str; 5] = [
    "plan_to_watch",
    "watching",
    "completed",
    "on_hold",
    "dropped",
];

fn optional<T: std::fmt::Debug + Clone + 'static>(
    s: impl Strategy<Value = T> + 'static,
) -> BoxedStrategy<Option<T>> {
    proptest::option::of(s).boxed()
}

proptest! {
    /// The row reads back as MAL wrote it: its status and watched count
    /// as before, and its rewatch count where the read carried one,
    /// unknown where it did not.
    #[test]
    fn a_row_reads_back_with_its_rewatch_count(
        status in proptest::sample::select(STATUSES.to_vec()).boxed(),
        rewatching in optional(any::<bool>()),
        watched in optional(0u32..5000),
        times in optional(any::<u32>()),
    ) {
        let mut row = serde_json::json!({ "status": status });
        if let Some(r) = rewatching { row["is_rewatching"] = r.into(); }
        if let Some(w) = watched { row["num_episodes_watched"] = w.into(); }
        if let Some(t) = times { row["num_times_rewatched"] = t.into(); }
        let body = serde_json::json!({ "id": 21, "my_list_status": row }).to_string();
        let got = parse_my_list_status_entry(body.as_bytes())
            .expect("parse ok")
            .expect("a row");
        prop_assert_eq!(
            Some(got.status),
            ListStatus::from_mal(status, rewatching.unwrap_or(false))
        );
        prop_assert_eq!(got.progress_episodes, watched.unwrap_or(0));
        prop_assert_eq!(got.times_rewatched, times);
    }
}
