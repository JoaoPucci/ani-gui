//! A guessed session's play never records its guess over the show a row
//! already records: a link the user made stands.

use crate::commands::guess_guard::{guards_row, keeps_recorded, line_record};
use crate::history::HistoryEntry;
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

#[test]
fn a_card_play_guards_the_cards_own_listing_and_no_other() {
    let card = Some("hianime:black-clover-949");
    // A guessed card's play guards every row it lands on.
    assert!(guards_row(card, None, "hianime:gintama-1237"));
    // A card no guess opened guards its own line, and a play of its
    // listing when the card is a retired provider's bare row (a play
    // always lands on a walked provider's key).
    assert!(guards_row(None, card, "hianime:black-clover-949"));
    assert!(guards_row(
        None,
        Some("black-clover-77"),
        "hianime:black-clover-949"
    ));
    // Another listing's row is that show's history, and a play from a
    // show's own page is the user's choice.
    assert!(!guards_row(None, card, "hianime:gintama-1237"));
    assert!(!guards_row(None, None, "hianime:black-clover-949"));
}

fn row(id: &str, kitsu: Option<&str>) -> HistoryEntry {
    HistoryEntry {
        ep_no: "1".into(),
        id: id.into(),
        title: "Black Clover".into(),
        watched_at: None,
        kitsu_id: kitsu.map(str::to_string),
        own_number: false,
    }
}

#[test]
fn a_new_row_of_the_cards_listing_is_guarded_by_the_show_its_line_records() {
    // The card is a retired provider's bare row the user named 4013; the
    // play lands on the walked listing, whose row holds no id (or none
    // yet): the line's show is what the play must not write over.
    let rows = [row("black-clover-77", Some("4013"))];
    assert_eq!(
        line_record(&rows, "hianime:black-clover-949", Some("black-clover-77")),
        Some("4013")
    );
    let walked = [
        row("black-clover-77", Some("4013")),
        row("hianime:black-clover-949", None),
    ];
    assert_eq!(
        line_record(&walked, "hianime:black-clover-949", Some("black-clover-77")),
        Some("4013")
    );
}

#[test]
fn the_played_rows_own_show_comes_first_and_another_listing_is_not_the_lines() {
    let rows = [
        row("black-clover-77", Some("4013")),
        row("hianime:black-clover-949", Some("12")),
        row("hianime:gintama-1237", None),
    ];
    assert_eq!(
        line_record(&rows, "hianime:black-clover-949", Some("black-clover-77")),
        Some("12")
    );
    assert_eq!(
        line_record(&rows, "hianime:gintama-1237", Some("black-clover-77")),
        None
    );
    assert_eq!(
        line_record(&rows, "hianime:black-clover-949", None),
        Some("12")
    );
}

fn key() -> impl Strategy<Value = String> {
    proptest::sample::select(
        &[
            "hianime:black-clover-949",
            "hianime:black-clover-950",
            "black-clover-77",
            "hianime:gintama-1237",
            "vDTSJHSpYnrkZnAvG",
        ][..],
    )
    .prop_map(str::to_string)
}

proptest! {
    /// The show a play would write over is the played row's own, else
    /// the card's line's when the play is on that card's own listing,
    /// and never another listing's.
    #[test]
    fn a_play_is_guarded_by_its_row_else_its_cards_line(
        played_id in prop::option::of("[1-9][0-9]{0,3}"),
        card_id in prop::option::of("[1-9][0-9]{0,3}"),
        card in prop::option::of(key()),
        played in key(),
    ) {
        let mut rows = vec![row(&played, played_id.as_deref())];
        if let Some(c) = card.as_deref().filter(|c| *c != played) {
            rows.push(row(c, card_id.as_deref()));
        }
        let got = line_record(&rows, &played, card.as_deref());
        let own_line = card.as_deref().is_some_and(|c| {
            c != played && crate::scraper::provider::same_listing(c, &played)
        });
        let expected = played_id
            .as_deref()
            .or(if own_line { card_id.as_deref() } else { None });
        prop_assert_eq!(got, expected);
    }
}

proptest! {
    /// A guessed card's play guards every row; a card no guess opened
    /// guards its own listing; no card guards nothing.
    #[test]
    fn a_play_guards_a_row_exactly_when_a_card_asked_for_it_there(
        guess in prop::option::of(key()),
        card in prop::option::of(key()),
        played in key(),
    ) {
        let expected = guess.is_some()
            || card
                .as_deref()
                .is_some_and(|c| crate::scraper::provider::same_listing(c, &played));
        prop_assert_eq!(guards_row(guess.as_deref(), card.as_deref(), &played), expected);
        prop_assert!(!guards_row(None, None, &played));
    }
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
