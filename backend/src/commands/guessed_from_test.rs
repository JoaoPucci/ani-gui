//! What a recorded play says of the row it wrote: which card it came
//! from, when a guessed card's play landed on another listing.

use crate::commands::guessed_from::{note, Note};
use proptest::prelude::*;

const CARD: &str = "hianime:jojos-bizarre-adventure-2000-5182";
const PLAYED: &str = "hianime:jojos-bizarre-adventure-2012-708";

#[test]
fn the_jojo_cards_play_on_the_2012_listing_came_from_the_card() {
    assert_eq!(note(Some(CARD), PLAYED), Note::From(CARD));
}

#[test]
fn a_play_on_the_cards_own_listing_came_from_no_other_card() {
    assert_eq!(note(Some(PLAYED), PLAYED), Note::Clear);
    // A retired provider's bare row with the listing's words is its own.
    assert_eq!(
        note(Some("jojos-bizarre-adventure-2012-77"), PLAYED),
        Note::Clear
    );
}

#[test]
fn a_play_no_guess_asked_for_came_from_no_card() {
    assert_eq!(note(None, PLAYED), Note::Clear);
    assert_eq!(note(Some(""), PLAYED), Note::Clear);
}

fn key() -> impl Strategy<Value = String> {
    (
        prop::option::of(prop::sample::select(vec!["hianime", "test-a"])),
        prop::collection::vec("[a-z][a-z0-9]{0,5}", 1..4),
        1u32..50,
    )
        .prop_map(|(provider, words, n)| {
            let slug = format!("{}-{n}", words.join("-"));
            match provider {
                Some(p) => format!("{p}:{slug}"),
                None => slug,
            }
        })
}

proptest! {
    /// A row never came from itself, nor from a card of its own
    /// listing; it came from a card exactly when a guessed card of
    /// another listing asked for the play.
    #[test]
    fn a_row_came_from_a_card_only_of_another_listing(
        card in prop::option::of(key()),
        played in key(),
    ) {
        match note(card.as_deref(), &played) {
            Note::From(from) => {
                prop_assert_eq!(Some(from), card.as_deref());
                prop_assert_ne!(from, played.as_str());
                prop_assert!(!crate::scraper::provider::same_listing(from, &played));
            }
            Note::Clear => prop_assert!(
                card.as_deref().is_none_or(|c| c.is_empty()
                    || c == played
                    || crate::scraper::provider::same_listing(c, &played))
            ),
        }
    }

    /// The card's own row is never noted as coming from anywhere.
    #[test]
    fn the_cards_own_row_comes_from_nowhere(card in key()) {
        prop_assert_eq!(note(Some(&card), &card), Note::Clear);
    }
}
