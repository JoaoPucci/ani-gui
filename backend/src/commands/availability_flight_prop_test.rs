//! The flight's pure decisions over every input.

use super::*;
use proptest::prelude::*;

fn args(kitsu_id: Option<String>, mode: &str, bypass: bool, background: bool) -> AvailabilityArgs {
    serde_json::from_value(serde_json::json!({
        "title": "Any Show",
        "mode": mode,
        "kitsu_id": kitsu_id,
        "bypass_cache": bypass,
        "background": background
    }))
    .expect("args")
}

proptest! {
    #[test]
    fn a_lookup_shares_the_cache_row_it_reads(
        id in "[0-9]{1,6}",
        mode in prop_oneof![Just("sub"), Just("dub"), Just("raw")],
        background: bool,
    ) {
        let got = shared_row(&args(Some(id.clone()), mode, false, background));
        prop_assert_eq!(
            got,
            Some(crate::commands::availability::cache_key(&id, mode))
        );
    }

    #[test]
    fn a_refresh_or_an_unkeyed_lookup_shares_nothing(
        id in proptest::option::of("[0-9]{0,6}"),
        bypass: bool,
        background: bool,
    ) {
        let unkeyed = id.as_deref().is_none_or(str::is_empty);
        prop_assume!(bypass || unkeyed);
        prop_assert_eq!(shared_row(&args(id, "sub", bypass, background)), None);
    }

    #[test]
    fn only_an_interactive_caller_refuses_a_walk_and_only_a_background_one(
        walk_interactive: bool,
        caller_interactive: bool,
    ) {
        prop_assert_eq!(
            joins(walk_interactive, caller_interactive),
            walk_interactive || !caller_interactive
        );
    }

    #[test]
    fn only_a_background_walk_under_the_bound_outlives_its_callers(
        interactive: bool,
        unwatched in 0usize..(UNWATCHED_MAX * 3),
    ) {
        prop_assert_eq!(
            outlives_its_callers(interactive, unwatched),
            !interactive && unwatched < UNWATCHED_MAX
        );
    }
}
