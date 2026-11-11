//! The Kitsu-searchable words a history row's show key carries
//! ([`super::key_search_term`]): what the picker searches beside the
//! row's saved title, read here so the renderer never reads a slug.

use super::key_search_term;
use proptest::prelude::*;

#[test]
fn a_walked_providers_key_is_read_without_its_label_and_number() {
    assert_eq!(
        key_search_term("hianime:jojos-bizarre-adventure-2000-5182").as_deref(),
        Some("jojos bizarre adventure 2000")
    );
}

#[test]
fn a_retired_providers_bare_key_is_read_the_same_way() {
    assert_eq!(
        key_search_term("naruto-shippuden-3687").as_deref(),
        Some("naruto shippuden")
    );
}

#[test]
fn a_key_that_is_not_slug_shaped_has_none() {
    // An allanime-era id: mixed case, no hyphen.
    assert_eq!(key_search_term("vDTSJHSpYnrkZnAvG"), None);
    // A number alone names no show.
    assert_eq!(key_search_term("hianime:5182"), None);
    // Neither does a slug in another case than a provider spells one.
    assert_eq!(key_search_term("hianime:Black-Clover-949"), None);
    assert_eq!(key_search_term(""), None);
}

#[test]
fn empty_words_between_hyphens_are_dropped() {
    assert_eq!(key_search_term("hianime:a--b-12").as_deref(), Some("a b"));
}

proptest! {
    /// The words of any `words-id` key come back space-separated, in
    /// order, under any provider label or none.
    #[test]
    fn the_words_come_back_in_order(
        words in prop::collection::vec("[a-z0-9]{1,6}", 1..5),
        id in 0u32..100_000,
        labelled in any::<bool>(),
    ) {
        let slug = format!("{}-{id}", words.join("-"));
        let key = if labelled { format!("hianime:{slug}") } else { slug };
        prop_assert_eq!(key_search_term(&key), Some(words.join(" ")));
    }

    /// A key with an uppercase letter or a character no slug carries
    /// has no words to search.
    #[test]
    fn a_character_no_slug_carries_names_no_words(
        before in "[a-z]{1,5}", odd in "[A-Z_ .!]", after in "[a-z]{0,5}", id in 0u32..1000,
    ) {
        let key = format!("hianime:{before}{odd}{after}-{id}");
        prop_assert_eq!(key_search_term(&key), None);
    }
}
