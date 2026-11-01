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

    #[test]
    fn a_replacement_walk_is_one_its_caller_and_every_mover_joins_and_no_more(
        caller_interactive: bool,
        displaced in proptest::option::of((any::<bool>(), 0usize..4)),
    ) {
        let started = starts_interactive(caller_interactive, displaced);
        prop_assert!(joins(started, caller_interactive), "the caller joins it");
        if let Some((mover_interactive, waiters)) = displaced {
            if waiters > 0 {
                prop_assert!(joins(started, mover_interactive), "every mover joins it");
            }
        }
        let interactive_mover = displaced.is_some_and(|(i, w)| i && w > 0);
        if !caller_interactive && !interactive_mover {
            prop_assert!(!started, "nobody waits on it interactively, so it is paced");
        }
    }

    #[test]
    fn an_interactive_walk_counts_as_interactive_at_anilist_whoever_started_it(
        interactive: bool,
        caller in prop_oneof![
            Just(crate::meta::anilist_gate::Class::Interactive),
            Just(crate::meta::anilist_gate::Class::Background)
        ],
    ) {
        let expected = if interactive {
            crate::meta::anilist_gate::Class::Interactive
        } else {
            caller
        };
        prop_assert_eq!(walk_class(interactive, caller), expected);
    }
}
