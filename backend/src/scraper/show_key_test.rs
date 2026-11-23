//! Whether a play landed on the listing a history row names
//! ([`super::same_listing`]).

use super::same_listing;
use proptest::prelude::*;

#[test]
fn a_walked_providers_row_is_its_listing_only_under_its_own_key() {
    assert!(same_listing(
        "hianime:bleach-thousand-year-blood-war-the-separation-1009",
        "hianime:bleach-thousand-year-blood-war-the-separation-1009",
    ));
    // The other version of one season (standard beside uncensored) is
    // another listing, whatever its words.
    assert!(!same_listing(
        "hianime:jojos-bizarre-adventure-diamond-is-unbreakable-1388",
        "hianime:jojo-s-bizarre-adventure-part-4-diamond-is-unbreakable-9758",
    ));
    assert!(!same_listing(
        "hianime:jojos-bizarre-adventure-2000-5182",
        "hianime:jojos-bizarre-adventure-2012-708",
    ));
    // The same words under another id are another listing too.
    assert!(!same_listing(
        "hianime:show-name-12",
        "hianime:show-name-13"
    ));
    // And the same slug from another provider is another provider's.
    assert!(!same_listing("test-a:show-name-12", "test-b:show-name-12"));
}

#[test]
fn a_bare_row_is_the_listing_whose_slug_carries_its_words() {
    // anidb.app's ids and hianime's spell a show with the same words
    // before their own numeric ids.
    assert!(same_listing(
        "bleach-thousand-year-blood-war-the-separation-677",
        "hianime:bleach-thousand-year-blood-war-the-separation-1009",
    ));
    assert!(same_listing(
        "naruto-shippuden-3687",
        "hianime:naruto-shippuden-600"
    ));
    assert!(!same_listing(
        "jojos-bizarre-adventure-2000-5182",
        "hianime:jojos-bizarre-adventure-2012-708",
    ));
    // One word more or less is another show's slug.
    assert!(!same_listing(
        "gintama-1826",
        "hianime:gintama-love-incense-arc-1160"
    ));
}

#[test]
fn a_bare_row_reads_its_words_without_case() {
    assert!(same_listing(
        "Naruto-Shippuden-3687",
        "hianime:naruto-shippuden-600"
    ));
}

#[test]
fn a_row_without_words_is_no_listing_a_play_lands_on() {
    // An allanime-era id carries no words, and a slug that is only an
    // id carries none either.
    assert!(!same_listing(
        "ReooPAxPMsHM4KPMY",
        "hianime:reoopaxpmshm4kpmy-1"
    ));
    assert!(!same_listing("123", "hianime:123"));
    assert!(!same_listing("", "hianime:show-1"));
}

#[test]
fn a_play_that_landed_on_no_walked_provider_names_no_listing() {
    // A play lands on a walked provider's key; a bare one is not it,
    // whatever its words.
    assert!(!same_listing("show-name-12", "show-name-13"));
    assert!(!same_listing("show-name-12", "show-name-12"));
}

fn words() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec("[a-z][a-z0-9]{0,6}", 1..5)
}

/// Two word lists that are the same as often as not, so a property
/// about equal words is exercised and not only one about different
/// ones.
fn word_pair() -> impl Strategy<Value = (Vec<String>, Vec<String>)> {
    (words(), words(), any::<bool>()).prop_map(|(a, b, same)| {
        let b = if same { a.clone() } else { b };
        (a, b)
    })
}

fn label() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec!["hianime", "test-a", "test-b"])
}

fn cased(s: &str, upper: bool) -> String {
    if upper {
        s.to_ascii_uppercase()
    } else {
        s.to_string()
    }
}

proptest! {
    /// A walked provider's row is the play's listing exactly when the
    /// two keys are one — the same words under another id or another
    /// provider are another listing.
    #[test]
    fn a_labelled_row_is_its_listing_exactly_under_its_key(
        l1 in label(), id1 in 1u32..100_000,
        l2 in label(), (w1, w2) in word_pair(), same_id in any::<bool>(), id2 in 1u32..100_000,
    ) {
        let id2 = if same_id { id1 } else { id2 };
        let row = format!("{l1}:{}-{id1}", w1.join("-"));
        let played = format!("{l2}:{}-{id2}", w2.join("-"));
        prop_assert!(same_listing(&row, &row));
        prop_assert_eq!(same_listing(&row, &played), row == played);
    }

    /// A bare row is the play's listing exactly when the played slug
    /// carries the row's words, whatever either numeric id and
    /// whatever either side's letter case.
    #[test]
    fn a_bare_row_is_its_listing_exactly_when_the_words_agree(
        l in label(), (w1, w2) in word_pair(), id1 in 1u32..100_000,
        id2 in 1u32..100_000, upper_row in any::<bool>(), upper_played in any::<bool>(),
    ) {
        let row = cased(&format!("{}-{id1}", w1.join("-")), upper_row);
        let played = format!("{l}:{}", cased(&format!("{}-{id2}", w2.join("-")), upper_played));
        prop_assert_eq!(same_listing(&row, &played), w1 == w2);
    }

    /// A played key that names no walked provider is never the row's
    /// listing.
    #[test]
    fn a_bare_play_is_no_listing(w in words(), id1 in 1u32..100_000, id2 in 1u32..100_000) {
        let row = format!("{}-{id1}", w.join("-"));
        let played = format!("{}-{id2}", w.join("-"));
        prop_assert!(!same_listing(&row, &played));
    }
}
