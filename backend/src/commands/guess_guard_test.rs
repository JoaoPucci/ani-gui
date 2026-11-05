//! A guessed session's play never records its guess over the show a row
//! already records: a link the user made stands.

use crate::commands::guess_guard::keeps_recorded;
use proptest::prelude::*;

#[test]
fn a_guess_over_a_linked_row_records_nothing() {
    // JoJo's 2000 line, linked to the OVA (4013), played again as the
    // guessed 2012 series (7158).
    assert_eq!(keeps_recorded(true, Some("4013"), Some("7158")), None);
}

#[test]
fn a_guess_naming_the_rows_own_show_records_it() {
    // A row written before the routes refused non-ids can hold a padded
    // id: it is the same show.
    assert_eq!(
        keeps_recorded(true, Some("4013"), Some("4013")),
        Some("4013")
    );
    assert_eq!(
        keeps_recorded(true, Some(" 4013 "), Some("4013")),
        Some("4013")
    );
}

#[test]
fn a_guess_over_a_row_that_records_nothing_records_it() {
    assert_eq!(keeps_recorded(true, None, Some("7158")), Some("7158"));
}

#[test]
fn a_play_no_guess_opened_records_what_it_names() {
    // The show's own page names the show: it is the user's choice.
    assert_eq!(
        keeps_recorded(false, Some("4013"), Some("7158")),
        Some("7158")
    );
}

proptest! {
    /// A guessed session records an id only where the row records none
    /// or the same one; any other play records what it names.
    #[test]
    fn a_guess_never_replaces_a_recorded_id(
        guessed in any::<bool>(),
        recorded in prop::option::of("[1-9][0-9]{0,4}"),
        candidate in prop::option::of("[1-9][0-9]{0,4}"),
    ) {
        let kept = keeps_recorded(guessed, recorded.as_deref(), candidate.as_deref());
        if !guessed || recorded.is_none() {
            prop_assert_eq!(kept, candidate.as_deref());
        } else {
            prop_assert!(kept.is_none() || kept == recorded.as_deref());
            prop_assert!(kept.is_none() || kept == candidate.as_deref());
            if candidate == recorded {
                prop_assert_eq!(kept, candidate.as_deref());
            }
        }
    }
}
